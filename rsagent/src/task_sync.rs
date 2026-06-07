use std::{
    fs,
    future::Future,
    io::Write,
    path::{Path, PathBuf},
    pin::Pin,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, anyhow};
use chrono::{DateTime, SecondsFormat, Utc};
use job_manage::{
    TaskApplyIdentity, TaskApplyPatch, TaskListQuery, TaskObservedState, TaskResource, TaskType,
};

use crate::{
    clients::job_manage::{JobManageSyncClient, JobManageTransport, ReqwestJobManageTransport},
    executor::{ExecutionError, ExecutionResult, LocalTaskExecutor},
    registration::AgentRuntimeState,
    runtime_coordinator::{
        TransientRetryPolicy, default_transient_retry_policy, retry_blocking_on_transient,
    },
};

const TASK_REPLAY_FILE_NAME: &str = "task-replay.toml";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskSyncSkipReason {
    MissingConfig,
    LoopsDisabled,
    NoTasks,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskSyncTick {
    Applied {
        task_id: String,
        final_state: TaskObservedState,
    },
    Skipped {
        reason: TaskSyncSkipReason,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskDecisionStage {
    Poll,
    Replay,
    Execute,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskDecisionOutcome {
    Skipped,
    Completed,
    Recovered,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskDecisionDiagnostic {
    pub task_id: Option<String>,
    pub stage: TaskDecisionStage,
    pub outcome: TaskDecisionOutcome,
    pub detail: String,
}

pub struct TaskSyncLoop<T, E> {
    transport: T,
    executor: E,
    retry_policy: TransientRetryPolicy,
    cursor: TaskListCursor,
    pending_tasks: Vec<TaskResource>,
    pending_updated_after: Option<String>,
    replay_identity: Option<TaskApplyIdentity>,
    last_tick_diagnostic: Option<TaskDecisionDiagnostic>,
}

#[derive(Debug, Clone, Default)]
struct TaskListCursor {
    updated_after: Option<String>,
    blocked_task_ids: Vec<String>,
}

impl<T, E> TaskSyncLoop<T, E>
where
    T: JobManageTransport,
    E: TaskExecutor,
{
    pub fn new(transport: T, executor: E) -> Self {
        Self::with_retry_policy(transport, executor, default_transient_retry_policy())
    }

    pub fn with_retry_policy(
        transport: T,
        executor: E,
        retry_policy: TransientRetryPolicy,
    ) -> Self {
        Self {
            transport,
            executor,
            retry_policy,
            cursor: TaskListCursor::default(),
            pending_tasks: Vec::new(),
            pending_updated_after: None,
            replay_identity: None,
            last_tick_diagnostic: None,
        }
    }

    pub async fn tick(
        &mut self,
        state: &AgentRuntimeState,
        agent_id: &str,
        now: DateTime<Utc>,
    ) -> Result<TaskSyncTick> {
        if !state.loops_enabled() {
            let tick = TaskSyncTick::Skipped {
                reason: TaskSyncSkipReason::LoopsDisabled,
            };
            self.last_tick_diagnostic = Some(skip_diagnostic(
                TaskSyncSkipReason::LoopsDisabled,
                "task sync skipped because loops are disabled",
            ));
            return Ok(tick);
        }

        if state.effective_config().is_none() {
            let tick = TaskSyncTick::Skipped {
                reason: TaskSyncSkipReason::MissingConfig,
            };
            self.last_tick_diagnostic = Some(skip_diagnostic(
                TaskSyncSkipReason::MissingConfig,
                "task sync skipped because runtime config is unavailable",
            ));
            return Ok(tick);
        }

        if let Some(identity) = self.replay_identity.clone() {
            let tick = replay_persisted_terminal_patch(
                &mut self.transport,
                state,
                &identity,
                &self.retry_policy,
            )
            .await;
            match tick {
                Ok((tick, diagnostic)) => {
                    self.replay_identity = None;
                    self.last_tick_diagnostic = Some(diagnostic);
                    if self.pending_tasks.is_empty()
                        && let Some(updated_after) = self.pending_updated_after.take()
                    {
                        self.cursor.updated_after = Some(updated_after);
                        self.cursor.blocked_task_ids.clear();
                    }
                    return Ok(tick);
                }
                Err(error) => {
                    self.replay_identity = Some(identity);
                    self.last_tick_diagnostic = Some(TaskDecisionDiagnostic {
                        task_id: self
                            .replay_identity
                            .as_ref()
                            .map(|identity| identity.task_id.clone()),
                        stage: TaskDecisionStage::Replay,
                        outcome: TaskDecisionOutcome::Failed,
                        detail: format!("failed to replay persisted terminal patch: {error}"),
                    });
                    return Err(error);
                }
            }
        }

        if !self.pending_tasks.is_empty() {
            let task = select_next_task(&self.pending_tasks, &TaskListCursor::default())
                .ok_or_else(|| anyhow!("pending task selection failed"))?;
            let remaining_pending_tasks = self
                .pending_tasks
                .iter()
                .filter(|candidate| candidate.task_id != task.task_id)
                .cloned()
                .collect::<Vec<_>>();
            let tick = process_selected_task(
                &mut self.transport,
                &self.executor,
                state,
                task.clone(),
                now,
                &self.retry_policy,
            )
            .await;
            match tick {
                Ok((tick, diagnostic)) => {
                    self.pending_tasks = remaining_pending_tasks;
                    self.last_tick_diagnostic = Some(diagnostic);
                    if self.pending_tasks.is_empty()
                        && let Some(updated_after) = self.pending_updated_after.take()
                    {
                        self.cursor.updated_after = Some(updated_after);
                        self.cursor.blocked_task_ids.clear();
                    }
                    return Ok(tick);
                }
                Err(error) => {
                    self.last_tick_diagnostic = Some(TaskDecisionDiagnostic {
                        task_id: Some(task.task_id.clone()),
                        stage: TaskDecisionStage::Execute,
                        outcome: TaskDecisionOutcome::Failed,
                        detail: format!("task execution/apply failed: {error}"),
                    });
                    if load_replay_patch(state, &task_identity(&task))?.is_some() {
                        self.pending_tasks = remaining_pending_tasks;
                        self.replay_identity = Some(task_identity(&task));
                    }
                    return Err(error);
                }
            }
        }

        let tasks = list_visible_tasks(
            &mut self.transport,
            state,
            agent_id,
            self.cursor.updated_after.clone(),
            &self.retry_policy,
        )?;
        let Some(task) = select_next_task(&tasks, &TaskListCursor::default()) else {
            self.cursor.updated_after = Some(timestamp(now));
            self.cursor.blocked_task_ids.clear();
            let tick = TaskSyncTick::Skipped {
                reason: TaskSyncSkipReason::NoTasks,
            };
            self.last_tick_diagnostic = Some(skip_diagnostic(
                TaskSyncSkipReason::NoTasks,
                "task sync found no visible tasks",
            ));
            return Ok(tick);
        };

        let selected_updated_at = task.updated_at.clone();
        let batch_max_updated_at = max_updated_at(&tasks);
        let remaining_tasks = tasks
            .into_iter()
            .filter(|candidate| candidate.task_id != task.task_id)
            .collect::<Vec<_>>();

        if remaining_tasks
            .iter()
            .any(|candidate| !reachable_after_cursor(candidate, selected_updated_at.as_deref()))
        {
            self.pending_tasks = remaining_tasks;
            self.pending_updated_after = batch_max_updated_at;
        } else {
            self.cursor.updated_after = selected_updated_at.or(self.cursor.updated_after.clone());
            self.cursor.blocked_task_ids.clear();
        }

        let tick = process_selected_task(
            &mut self.transport,
            &self.executor,
            state,
            task.clone(),
            now,
            &self.retry_policy,
        )
        .await;
        match tick {
            Ok((tick, diagnostic)) => {
                self.last_tick_diagnostic = Some(diagnostic);
                Ok(tick)
            }
            Err(error) => {
                let identity = task_identity(&task);
                self.last_tick_diagnostic = Some(TaskDecisionDiagnostic {
                    task_id: Some(identity.task_id.clone()),
                    stage: TaskDecisionStage::Execute,
                    outcome: TaskDecisionOutcome::Failed,
                    detail: format!("task execution/apply failed: {error}"),
                });
                if load_replay_patch(state, &identity)?.is_some() {
                    self.replay_identity = Some(identity);
                }
                Err(error)
            }
        }
    }

    pub async fn tick_async(
        mut self,
        state: AgentRuntimeState,
        agent_id: String,
        now: DateTime<Utc>,
    ) -> (Self, Result<TaskSyncTick>)
    where
        T: Send + 'static,
        E: Send + 'static,
    {
        tokio::task::spawn_blocking(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("build blocking task-sync runtime");
            let result = runtime.block_on(self.tick(&state, &agent_id, now));
            (self, result)
        })
        .await
        .expect("task-sync blocking task panicked")
    }

    pub fn into_transport(self) -> T {
        self.transport
    }

    pub fn last_tick_diagnostic(&self) -> Option<&TaskDecisionDiagnostic> {
        self.last_tick_diagnostic.as_ref()
    }
}

impl TaskSyncLoop<ReqwestJobManageTransport, LocalTaskExecutor> {
    pub fn production() -> Self {
        Self::new(ReqwestJobManageTransport::default(), LocalTaskExecutor)
    }
}

pub trait TaskExecutor {
    type ExecuteFuture<'a>: Future<Output = Result<ExecutionResult>> + Send + 'a
    where
        Self: 'a;

    fn execute<'a>(&'a self, task: &'a TaskResource) -> Self::ExecuteFuture<'a>;
}

impl TaskExecutor for LocalTaskExecutor {
    type ExecuteFuture<'a>
        = Pin<Box<dyn Future<Output = Result<ExecutionResult>> + Send + 'a>>
    where
        Self: 'a;

    fn execute<'a>(&'a self, task: &'a TaskResource) -> Self::ExecuteFuture<'a> {
        Box::pin(async move { LocalTaskExecutor::execute(self, task).await })
    }
}

impl<T> TaskExecutor for &T
where
    T: TaskExecutor + Sync,
{
    type ExecuteFuture<'a>
        = T::ExecuteFuture<'a>
    where
        Self: 'a;

    fn execute<'a>(&'a self, task: &'a TaskResource) -> Self::ExecuteFuture<'a> {
        (*self).execute(task)
    }
}

struct BorrowedTransport<'a, T>(&'a mut T);

impl<T> JobManageTransport for BorrowedTransport<'_, T>
where
    T: JobManageTransport,
{
    fn list_tasks(&mut self, endpoint: &str, query: &TaskListQuery) -> Result<Vec<TaskResource>> {
        self.0.list_tasks(endpoint, query)
    }

    fn apply_task(
        &mut self,
        endpoint: &str,
        identity: &TaskApplyIdentity,
        patch: &TaskApplyPatch,
    ) -> Result<crate::clients::job_manage::TaskApplyAck> {
        self.0.apply_task(endpoint, identity, patch)
    }
}

fn reachable_after_cursor(task: &TaskResource, selected_updated_at: Option<&str>) -> bool {
    match (task.updated_at.as_deref(), selected_updated_at) {
        (Some(task_updated_at), Some(selected_updated_at)) => task_updated_at > selected_updated_at,
        (Some(_), None) => true,
        (None, _) => false,
    }
}

fn list_visible_tasks<T>(
    transport: &mut T,
    state: &AgentRuntimeState,
    agent_id: &str,
    updated_after: Option<String>,
    retry_policy: &TransientRetryPolicy,
) -> Result<Vec<TaskResource>>
where
    T: JobManageTransport,
{
    let config = state
        .effective_config()
        .ok_or_else(|| anyhow!("effective config unavailable for task sync"))?;
    let node_id = state
        .local_node_id()
        .ok_or_else(|| anyhow!("node_id unavailable for task sync"))?;
    let client = JobManageSyncClient::new(config.job_manage_config.base_url.clone());
    let agent_id = agent_id.to_string();
    let node_id = node_id.to_string();
    let states = parse_default_states(&config.job_manage_config.task_filter_defaults.states)?;

    retry_blocking_on_transient(retry_policy, || {
        client.list_tasks(
            transport,
            &TaskListQuery {
                agent_id: agent_id.clone(),
                node_id: node_id.clone(),
                states: states.clone(),
                updated_after: updated_after.clone(),
            },
        )
    })
}

async fn process_selected_task<T, E>(
    transport: &mut T,
    executor: &E,
    state: &AgentRuntimeState,
    task: TaskResource,
    now: DateTime<Utc>,
    retry_policy: &TransientRetryPolicy,
) -> Result<(TaskSyncTick, TaskDecisionDiagnostic)>
where
    T: JobManageTransport,
    E: TaskExecutor,
{
    let identity = TaskApplyIdentity {
        task_id: task.task_id.clone(),
        agent_id: task.agent_id.clone(),
        node_id: task.node_id.clone(),
    };
    let now = timestamp(now);

    if matches!(
        task.observed_state,
        TaskObservedState::Queued | TaskObservedState::Dispatched
    ) {
        let config = state
            .effective_config()
            .ok_or_else(|| anyhow!("effective config unavailable for task sync"))?;
        let client = JobManageSyncClient::new(config.job_manage_config.base_url.clone());
        apply_task_with_retry(
            transport,
            &client,
            &identity,
            &TaskApplyPatch {
                observed_state: Some(TaskObservedState::Acknowledged),
                claimed_at: Some(now.clone()),
                updated_at: Some(now.clone()),
                ..Default::default()
            },
            retry_policy,
        )?;
    }

    if task.observed_state == TaskObservedState::Running
        && let Some(replay) = load_replay_patch(state, &identity)?
    {
        let config = state
            .effective_config()
            .ok_or_else(|| anyhow!("effective config unavailable for task sync"))?;
        let client = JobManageSyncClient::new(config.job_manage_config.base_url.clone());
        let applied = apply_task_with_retry(transport, &client, &identity, &replay, retry_policy)?;
        remove_replay_patch(state, &identity)?;
        return Ok((
            TaskSyncTick::Applied {
                task_id: applied.task_id.clone(),
                final_state: applied.observed_state,
            },
            TaskDecisionDiagnostic {
                task_id: Some(applied.task_id),
                stage: TaskDecisionStage::Replay,
                outcome: TaskDecisionOutcome::Recovered,
                detail: "replayed persisted terminal patch for running task".to_string(),
            },
        ));
    }

    let final_patch = match validate_task_payload(&task) {
        Ok(()) => {
            if task.observed_state != TaskObservedState::Running {
                let config = state
                    .effective_config()
                    .ok_or_else(|| anyhow!("effective config unavailable for task sync"))?;
                let client = JobManageSyncClient::new(config.job_manage_config.base_url.clone());
                apply_task_with_retry(
                    transport,
                    &client,
                    &identity,
                    &TaskApplyPatch {
                        observed_state: Some(TaskObservedState::Running),
                        started_at: Some(now.clone()),
                        updated_at: Some(now.clone()),
                        ..Default::default()
                    },
                    retry_policy,
                )?;
            }

            match executor.execute(&task).await {
                Ok(result) => result_patch(result, &now),
                Err(error) => execution_failure_patch(error, &now),
            }
        }
        Err(error) => failure_patch(error.to_string(), &now),
    };

    persist_replay_patch(state, &identity, &final_patch)?;
    let config = state
        .effective_config()
        .ok_or_else(|| anyhow!("effective config unavailable for task sync"))?;
    let client = JobManageSyncClient::new(config.job_manage_config.base_url.clone());
    let applied = apply_task_with_retry(transport, &client, &identity, &final_patch, retry_policy)?;
    remove_replay_patch(state, &identity)?;
    Ok((
        TaskSyncTick::Applied {
            task_id: applied.task_id.clone(),
            final_state: applied.observed_state,
        },
        TaskDecisionDiagnostic {
            task_id: Some(applied.task_id),
            stage: TaskDecisionStage::Execute,
            outcome: TaskDecisionOutcome::Completed,
            detail: "completed task execution and applied terminal patch".to_string(),
        },
    ))
}

async fn replay_persisted_terminal_patch<T>(
    transport: &mut T,
    state: &AgentRuntimeState,
    identity: &TaskApplyIdentity,
    retry_policy: &TransientRetryPolicy,
) -> Result<(TaskSyncTick, TaskDecisionDiagnostic)>
where
    T: JobManageTransport,
{
    let replay = load_replay_patch(state, identity)?.ok_or_else(|| {
        anyhow!(
            "persisted replay patch missing for task {}",
            identity.task_id
        )
    })?;
    let config = state
        .effective_config()
        .ok_or_else(|| anyhow!("effective config unavailable for task sync"))?;
    let client = JobManageSyncClient::new(config.job_manage_config.base_url.clone());
    let applied = apply_task_with_retry(transport, &client, identity, &replay, retry_policy)?;
    remove_replay_patch(state, identity)?;
    Ok((
        TaskSyncTick::Applied {
            task_id: applied.task_id.clone(),
            final_state: applied.observed_state,
        },
        TaskDecisionDiagnostic {
            task_id: Some(applied.task_id),
            stage: TaskDecisionStage::Replay,
            outcome: TaskDecisionOutcome::Recovered,
            detail: "replayed persisted terminal patch before polling new work".to_string(),
        },
    ))
}

fn task_identity(task: &TaskResource) -> TaskApplyIdentity {
    TaskApplyIdentity {
        task_id: task.task_id.clone(),
        agent_id: task.agent_id.clone(),
        node_id: task.node_id.clone(),
    }
}

pub async fn sync_once<T, E>(
    transport: &mut T,
    executor: &E,
    state: &AgentRuntimeState,
    agent_id: &str,
    updated_after: Option<String>,
    now: DateTime<Utc>,
) -> Result<TaskSyncTick>
where
    T: JobManageTransport,
    E: TaskExecutor + Sync,
{
    let mut loop_runner = TaskSyncLoop::with_retry_policy(
        BorrowedTransport(transport),
        executor,
        default_transient_retry_policy(),
    );
    loop_runner.cursor.updated_after = updated_after;
    loop_runner.tick(state, agent_id, now).await
}

fn select_next_task(tasks: &[TaskResource], cursor: &TaskListCursor) -> Option<TaskResource> {
    let mut first_dispatched = None;
    let mut first_new = None;

    for task in tasks {
        if cursor.blocked_task_ids.iter().any(|id| id == &task.task_id) {
            continue;
        }

        match task.observed_state {
            TaskObservedState::Running | TaskObservedState::Acknowledged => {
                return Some(task.clone());
            }
            TaskObservedState::Dispatched => {
                if first_dispatched.is_none() {
                    first_dispatched = Some(task.clone());
                }
            }
            _ => {
                if first_new.is_none() {
                    first_new = Some(task.clone());
                }
            }
        }
    }

    first_dispatched.or(first_new)
}

fn max_updated_at(tasks: &[TaskResource]) -> Option<String> {
    tasks
        .iter()
        .filter_map(|task| task.updated_at.clone())
        .max()
}

fn parse_default_states(states: &[String]) -> Result<Vec<TaskObservedState>> {
    states
        .iter()
        .map(|state| parse_state(state))
        .collect::<Result<Vec<_>>>()
}

fn parse_state(value: &str) -> Result<TaskObservedState> {
    serde_json::from_str::<TaskObservedState>(&format!("\"{value}\""))
        .with_context(|| format!("invalid task state default: {value}"))
}

fn validate_task_payload(task: &TaskResource) -> std::result::Result<(), ExecutionError> {
    match task.task_type {
        TaskType::Script => {
            if populated_field(task.command_line.as_deref()) {
                return Err(ExecutionError::invalid_payload(
                    task.task_id.clone(),
                    "script task cannot include command_line",
                ));
            }
            if !populated_field(task.script_content.as_deref()) {
                return Err(ExecutionError::invalid_payload(
                    task.task_id.clone(),
                    "script task missing script_content",
                ));
            }
        }
        TaskType::Command => {
            if populated_field(task.script_content.as_deref()) {
                return Err(ExecutionError::invalid_payload(
                    task.task_id.clone(),
                    "command task cannot include script_content",
                ));
            }
            if !populated_field(task.command_line.as_deref()) {
                return Err(ExecutionError::invalid_payload(
                    task.task_id.clone(),
                    "command task missing command_line",
                ));
            }
        }
    }

    Ok(())
}

fn populated_field(value: Option<&str>) -> bool {
    value.is_some_and(|value| !value.is_empty())
}

fn failure_patch(error_message: String, now: &str) -> TaskApplyPatch {
    TaskApplyPatch {
        observed_state: Some(TaskObservedState::Failed),
        finished_at: Some(now.to_string()),
        error_message: Some(error_message),
        updated_at: Some(now.to_string()),
        ..Default::default()
    }
}

fn execution_failure_patch(error: anyhow::Error, now: &str) -> TaskApplyPatch {
    if let Some(error) = error.downcast_ref::<ExecutionError>() {
        return failure_patch(error.message().to_string(), now);
    }

    failure_patch(error.to_string(), now)
}

fn result_patch(result: ExecutionResult, now: &str) -> TaskApplyPatch {
    TaskApplyPatch {
        observed_state: Some(result.normalized_state()),
        finished_at: Some(now.to_string()),
        stdout: Some(result.stdout),
        stderr: Some(result.stderr),
        exit_code: result.exit_code,
        updated_at: Some(now.to_string()),
        ..Default::default()
    }
}

fn apply_task_with_retry<T>(
    transport: &mut T,
    client: &JobManageSyncClient,
    identity: &TaskApplyIdentity,
    patch: &TaskApplyPatch,
    retry_policy: &TransientRetryPolicy,
) -> Result<crate::clients::job_manage::TaskApplyAck>
where
    T: JobManageTransport,
{
    retry_blocking_on_transient(retry_policy, || {
        client.apply_task(transport, identity, patch)
    })
}

fn skip_diagnostic(reason: TaskSyncSkipReason, detail: &str) -> TaskDecisionDiagnostic {
    TaskDecisionDiagnostic {
        task_id: None,
        stage: TaskDecisionStage::Poll,
        outcome: TaskDecisionOutcome::Skipped,
        detail: format!("{detail}; reason={reason:?}"),
    }
}

fn timestamp(now: DateTime<Utc>) -> String {
    now.to_rfc3339_opts(SecondsFormat::Secs, true)
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct PersistedReplayPatch {
    identity: TaskApplyIdentity,
    patch: TaskApplyPatch,
}

fn persist_replay_patch(
    state: &AgentRuntimeState,
    identity: &TaskApplyIdentity,
    patch: &TaskApplyPatch,
) -> Result<()> {
    let payload = toml::to_string(&PersistedReplayPatch {
        identity: identity.clone(),
        patch: patch.clone(),
    })
    .context("failed to encode persisted task replay patch")?;
    let path = replay_patch_path(state);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create replay dir {}", parent.display()))?;
    }
    atomic_write(&path, &payload)
        .with_context(|| format!("failed to persist task replay patch to {}", path.display()))
}

fn load_replay_patch(
    state: &AgentRuntimeState,
    identity: &TaskApplyIdentity,
) -> Result<Option<TaskApplyPatch>> {
    let path = replay_patch_path(state);
    if !path.exists() {
        return Ok(None);
    }

    let raw = match fs::read_to_string(&path) {
        Ok(raw) => raw,
        Err(_) => {
            discard_corrupt_replay_patch(&path)?;
            return Ok(None);
        }
    };
    let persisted: PersistedReplayPatch = match toml::from_str(&raw) {
        Ok(persisted) => persisted,
        Err(error) => {
            discard_corrupt_replay_patch(&path)?;
            tracing::warn!(
                path = %path.display(),
                error = %error,
                "discarded corrupt task replay patch"
            );
            return Ok(None);
        }
    };
    if persisted.identity == *identity {
        Ok(Some(persisted.patch))
    } else {
        Ok(None)
    }
}

fn remove_replay_patch(state: &AgentRuntimeState, identity: &TaskApplyIdentity) -> Result<()> {
    let path = replay_patch_path(state);
    if !path.exists() {
        return Ok(());
    }

    let raw = fs::read_to_string(&path)
        .with_context(|| format!("failed to read task replay patch from {}", path.display()))?;
    let persisted: PersistedReplayPatch = match toml::from_str(&raw) {
        Ok(persisted) => persisted,
        Err(error) => {
            discard_corrupt_replay_patch(&path)?;
            tracing::warn!(
                path = %path.display(),
                error = %error,
                "discarded corrupt task replay patch during cleanup"
            );
            return Ok(());
        }
    };

    if persisted.identity != *identity {
        return Ok(());
    }

    fs::remove_file(&path)
        .with_context(|| format!("failed to remove task replay patch at {}", path.display()))
}

fn discard_corrupt_replay_patch(path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }

    fs::remove_file(path).with_context(|| {
        format!(
            "failed to remove corrupt task replay patch at {}",
            path.display()
        )
    })
}

fn atomic_write(path: &Path, payload: &str) -> Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let temp_path = parent.join(format!(
        "{}.tmp-{}-{}",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("task-replay.toml"),
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));

    let mut file = fs::File::create(&temp_path)
        .with_context(|| format!("failed to create temp replay file {}", temp_path.display()))?;
    file.write_all(payload.as_bytes())
        .with_context(|| format!("failed to write temp replay file {}", temp_path.display()))?;
    file.sync_all()
        .with_context(|| format!("failed to flush temp replay file {}", temp_path.display()))?;
    drop(file);

    fs::rename(&temp_path, path).with_context(|| {
        format!(
            "failed to atomically replace replay patch {} with {}",
            path.display(),
            temp_path.display()
        )
    })?;

    let dir = fs::File::open(parent)
        .with_context(|| format!("failed to open replay dir {} for sync", parent.display()))?;
    dir.sync_all()
        .with_context(|| format!("failed to sync replay dir {}", parent.display()))?;

    Ok(())
}

fn replay_patch_path(state: &AgentRuntimeState) -> PathBuf {
    PathBuf::from(state.data_dir()).join(TASK_REPLAY_FILE_NAME)
}
