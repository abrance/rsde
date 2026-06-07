use std::{fmt, future::Future, pin::Pin};

use anyhow::{Context, Result, anyhow};
use chrono::{DateTime, SecondsFormat, Utc};
use job_manage::{
    TaskApplyIdentity, TaskApplyPatch, TaskListQuery, TaskObservedState, TaskResource, TaskType,
    models::TaskFinalResultCategory,
};

use crate::{
    clients::job_manage::{JobManageSyncClient, JobManageTransport, ReqwestJobManageTransport},
    executor::{ExecutionResult, LocalTaskExecutor},
    registration::AgentRuntimeState,
    runtime_coordinator::{RetryBackoffPolicy, next_temporary_upstream_retry_policy},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TaskSyncFailureClass {
    TemporaryUpstream,
}

#[derive(Debug)]
struct TaskSyncFailure {
    class: TaskSyncFailureClass,
    message: String,
}

impl TaskSyncFailure {
    fn temporary_upstream(error: &anyhow::Error) -> Self {
        Self {
            class: TaskSyncFailureClass::TemporaryUpstream,
            message: error.to_string(),
        }
    }

    fn is_temporary_upstream(&self) -> bool {
        self.class == TaskSyncFailureClass::TemporaryUpstream
    }
}

impl fmt::Display for TaskSyncFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for TaskSyncFailure {}

#[derive(Debug)]
pub struct TemporaryUpstreamApplyError {
    message: String,
}

impl TemporaryUpstreamApplyError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for TemporaryUpstreamApplyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for TemporaryUpstreamApplyError {}

fn temporary_upstream_error(error: anyhow::Error) -> anyhow::Error {
    anyhow::Error::new(TaskSyncFailure::temporary_upstream(&error))
}

fn classify_apply_error(error: anyhow::Error) -> anyhow::Error {
    if error
        .downcast_ref::<TemporaryUpstreamApplyError>()
        .is_some()
    {
        temporary_upstream_error(error)
    } else {
        error
    }
}

fn is_temporary_upstream_error(error: &anyhow::Error) -> bool {
    error
        .downcast_ref::<TaskSyncFailure>()
        .is_some_and(TaskSyncFailure::is_temporary_upstream)
}

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskLifecycleStage {
    PollDecision,
    PublishedTerminalResult,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskOutcomeKind {
    SkippedMissingConfig,
    SkippedLoopsDisabled,
    NoTasksAvailable,
    Succeeded,
    FailedInvalidPayload,
    FailedExecutorStart,
    FailedNonZeroExit,
    Timeout,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskDecisionDiagnostic {
    pub task_id: Option<String>,
    pub observed_state: Option<TaskObservedState>,
    pub lifecycle_stage: TaskLifecycleStage,
    pub outcome: TaskOutcomeKind,
    pub detail: String,
}

pub struct TaskSyncLoop<T, E> {
    transport: T,
    executor: E,
    updated_after: Option<String>,
    last_retry_policy: Option<RetryBackoffPolicy>,
    last_diagnostic: Option<TaskDecisionDiagnostic>,
}

impl<T, E> TaskSyncLoop<T, E>
where
    T: JobManageTransport,
    E: TaskExecutor,
{
    pub fn new(transport: T, executor: E) -> Self {
        Self {
            transport,
            executor,
            updated_after: None,
            last_retry_policy: None,
            last_diagnostic: None,
        }
    }

    pub async fn tick(
        &mut self,
        state: &AgentRuntimeState,
        agent_id: &str,
        now: DateTime<Utc>,
    ) -> Result<TaskSyncTick> {
        let tick = sync_once_with_diagnostic(
            &mut self.transport,
            &self.executor,
            state,
            agent_id,
            self.updated_after.clone(),
            now,
        )
        .await;
        let (tick, diagnostic) = match tick {
            Ok(result) => {
                self.last_retry_policy = None;
                result
            }
            Err(error) => {
                if is_temporary_upstream_error(&error) {
                    self.last_retry_policy = Some(next_temporary_upstream_retry_policy(
                        self.last_retry_policy.as_ref(),
                    ));
                } else {
                    self.last_retry_policy = None;
                }
                self.last_diagnostic = None;
                return Err(error);
            }
        };
        self.last_diagnostic = diagnostic;
        if matches!(
            tick,
            TaskSyncTick::Skipped {
                reason: TaskSyncSkipReason::NoTasks,
            }
        ) {
            self.updated_after = Some(timestamp(now));
        }
        Ok(tick)
    }

    pub fn into_transport(self) -> T {
        self.transport
    }

    pub fn last_retry_policy(&self) -> Option<&RetryBackoffPolicy> {
        self.last_retry_policy.as_ref()
    }

    pub fn last_diagnostic(&self) -> Option<&TaskDecisionDiagnostic> {
        self.last_diagnostic.as_ref()
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
    E: TaskExecutor,
{
    let (tick, _) =
        sync_once_with_diagnostic(transport, executor, state, agent_id, updated_after, now).await?;
    Ok(tick)
}

async fn sync_once_with_diagnostic<T, E>(
    transport: &mut T,
    executor: &E,
    state: &AgentRuntimeState,
    agent_id: &str,
    updated_after: Option<String>,
    now: DateTime<Utc>,
) -> Result<(TaskSyncTick, Option<TaskDecisionDiagnostic>)>
where
    T: JobManageTransport,
    E: TaskExecutor,
{
    let config = match state.effective_config() {
        Some(config) => config,
        None => {
            if !state.loops_enabled() {
                return Ok((
                    TaskSyncTick::Skipped {
                        reason: TaskSyncSkipReason::LoopsDisabled,
                    },
                    Some(TaskDecisionDiagnostic {
                        task_id: None,
                        observed_state: None,
                        lifecycle_stage: TaskLifecycleStage::PollDecision,
                        outcome: TaskOutcomeKind::SkippedLoopsDisabled,
                        detail: "task sync skipped because subordinate loops are disabled"
                            .to_string(),
                    }),
                ));
            }
            return Ok((
                TaskSyncTick::Skipped {
                    reason: TaskSyncSkipReason::MissingConfig,
                },
                Some(TaskDecisionDiagnostic {
                    task_id: None,
                    observed_state: None,
                    lifecycle_stage: TaskLifecycleStage::PollDecision,
                    outcome: TaskOutcomeKind::SkippedMissingConfig,
                    detail: "task sync skipped because no effective config is available"
                        .to_string(),
                }),
            ));
        }
    };

    let node_id = state
        .local_node_id()
        .ok_or_else(|| anyhow!("node_id unavailable for task sync"))?;
    if !state.loops_enabled() {
        return Ok((
            TaskSyncTick::Skipped {
                reason: TaskSyncSkipReason::LoopsDisabled,
            },
            Some(TaskDecisionDiagnostic {
                task_id: None,
                observed_state: None,
                lifecycle_stage: TaskLifecycleStage::PollDecision,
                outcome: TaskOutcomeKind::SkippedLoopsDisabled,
                detail: "task sync skipped because subordinate loops are disabled".to_string(),
            }),
        ));
    }
    let client = JobManageSyncClient::new(config.job_manage_config.base_url.clone());
    let query = TaskListQuery {
        agent_id: agent_id.to_string(),
        node_id: node_id.to_string(),
        states: parse_default_states(&config.job_manage_config.task_filter_defaults.states)?,
        updated_after,
    };
    let mut tasks = client
        .list_tasks(transport, &query)
        .map_err(temporary_upstream_error)?;

    let Some(task) = tasks.drain(..).next() else {
        return Ok((
            TaskSyncTick::Skipped {
                reason: TaskSyncSkipReason::NoTasks,
            },
            Some(TaskDecisionDiagnostic {
                task_id: None,
                observed_state: None,
                lifecycle_stage: TaskLifecycleStage::PollDecision,
                outcome: TaskOutcomeKind::NoTasksAvailable,
                detail: "task poll returned no visible tasks".to_string(),
            }),
        ));
    };

    let identity = TaskApplyIdentity {
        task_id: task.task_id.clone(),
        agent_id: task.agent_id.clone(),
        node_id: task.node_id.clone(),
    };
    let now = timestamp(now);

    let (final_patch, diagnostic) = match task.observed_state {
        TaskObservedState::Queued => {
            claim_task(transport, &client, &identity, &now)?;
            build_terminal_patch_after_claim(transport, &client, executor, &task, &identity, &now)
                .await?
        }
        TaskObservedState::Acknowledged => {
            build_terminal_patch_after_claim(transport, &client, executor, &task, &identity, &now)
                .await?
        }
        TaskObservedState::Running => (
            restart_reconcile_failure_patch(&now),
            TaskDecisionDiagnostic {
                task_id: Some(task.task_id.clone()),
                observed_state: Some(TaskObservedState::Running),
                lifecycle_stage: TaskLifecycleStage::PublishedTerminalResult,
                outcome: TaskOutcomeKind::FailedExecutorStart,
                detail: "task was already running before reconciliation; skipping re-execution"
                    .to_string(),
            },
        ),
        state => anyhow::bail!("unsupported task state for acquisition: {state:?}"),
    };

    let applied = client
        .apply_task(transport, &identity, &final_patch)
        .map_err(classify_apply_error)?;
    Ok((
        TaskSyncTick::Applied {
            task_id: applied.task_id,
            final_state: applied.observed_state,
        },
        Some(diagnostic),
    ))
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

fn claim_task<T>(
    transport: &mut T,
    client: &JobManageSyncClient,
    identity: &TaskApplyIdentity,
    now: &str,
) -> Result<()>
where
    T: JobManageTransport,
{
    client
        .apply_task(
            transport,
            identity,
            &TaskApplyPatch {
                observed_state: Some(TaskObservedState::Acknowledged),
                claimed_at: Some(now.to_string()),
                updated_at: Some(now.to_string()),
                ..Default::default()
            },
        )
        .map_err(classify_apply_error)?;

    Ok(())
}

async fn build_terminal_patch_after_claim<T, E>(
    transport: &mut T,
    client: &JobManageSyncClient,
    executor: &E,
    task: &TaskResource,
    identity: &TaskApplyIdentity,
    now: &str,
) -> Result<(TaskApplyPatch, TaskDecisionDiagnostic)>
where
    T: JobManageTransport,
    E: TaskExecutor,
{
    build_terminal_patch_and_diagnostic(transport, client, executor, task, identity, now).await
}

async fn build_terminal_patch_and_diagnostic<T, E>(
    transport: &mut T,
    client: &JobManageSyncClient,
    executor: &E,
    task: &TaskResource,
    identity: &TaskApplyIdentity,
    now: &str,
) -> Result<(TaskApplyPatch, TaskDecisionDiagnostic)>
where
    T: JobManageTransport,
    E: TaskExecutor,
{
    match validate_task_payload(task) {
        Ok(()) => {
            client
                .apply_task(
                    transport,
                    identity,
                    &TaskApplyPatch {
                        observed_state: Some(TaskObservedState::Running),
                        started_at: Some(now.to_string()),
                        updated_at: Some(now.to_string()),
                        ..Default::default()
                    },
                )
                .map_err(classify_apply_error)?;

            match executor.execute(task).await {
                Ok(result) => {
                    let ExecutionResult {
                        stdout,
                        stderr,
                        exit_code,
                        state,
                        category,
                    } = result;

                    let error_message = category
                        .filter(|category| {
                            matches!(
                                category,
                                TaskFinalResultCategory::FailedInvalidPayload
                                    | TaskFinalResultCategory::FailedExecutorStart
                            )
                        })
                        .map(|category| category.annotate_error_message(stderr.clone()));
                    let detail = error_message.clone().unwrap_or_else(|| match state {
                        TaskObservedState::Succeeded => "task completed successfully".to_string(),
                        TaskObservedState::Timeout => "task timed out during execution".to_string(),
                        _ => stderr.clone(),
                    });

                    Ok((
                        TaskApplyPatch {
                            observed_state: Some(state),
                            finished_at: Some(now.to_string()),
                            stdout: Some(stdout),
                            stderr: Some(stderr),
                            exit_code,
                            error_message,
                            updated_at: Some(now.to_string()),
                            ..Default::default()
                        },
                        TaskDecisionDiagnostic {
                            task_id: Some(task.task_id.clone()),
                            observed_state: Some(task.observed_state),
                            lifecycle_stage: TaskLifecycleStage::PublishedTerminalResult,
                            outcome: terminal_outcome_kind(state, category),
                            detail,
                        },
                    ))
                }
                Err(error) => {
                    let detail = error.to_string();
                    Ok((
                        executor_failure_patch(detail.clone(), now),
                        TaskDecisionDiagnostic {
                            task_id: Some(task.task_id.clone()),
                            observed_state: Some(task.observed_state),
                            lifecycle_stage: TaskLifecycleStage::PublishedTerminalResult,
                            outcome: TaskOutcomeKind::FailedExecutorStart,
                            detail,
                        },
                    ))
                }
            }
        }
        Err(error) => {
            let detail = error.to_string();
            Ok((
                failure_patch(
                    TaskFinalResultCategory::FailedInvalidPayload,
                    detail.clone(),
                    now,
                ),
                TaskDecisionDiagnostic {
                    task_id: Some(task.task_id.clone()),
                    observed_state: Some(task.observed_state),
                    lifecycle_stage: TaskLifecycleStage::PublishedTerminalResult,
                    outcome: TaskOutcomeKind::FailedInvalidPayload,
                    detail,
                },
            ))
        }
    }
}

fn terminal_outcome_kind(
    state: TaskObservedState,
    category: Option<TaskFinalResultCategory>,
) -> TaskOutcomeKind {
    match category {
        Some(TaskFinalResultCategory::FailedInvalidPayload) => {
            TaskOutcomeKind::FailedInvalidPayload
        }
        Some(TaskFinalResultCategory::FailedExecutorStart) => TaskOutcomeKind::FailedExecutorStart,
        Some(TaskFinalResultCategory::FailedNonZeroExit) => TaskOutcomeKind::FailedNonZeroExit,
        Some(TaskFinalResultCategory::Timeout) => TaskOutcomeKind::Timeout,
        Some(TaskFinalResultCategory::Succeeded) => TaskOutcomeKind::Succeeded,
        None if state == TaskObservedState::Succeeded => TaskOutcomeKind::Succeeded,
        None if state == TaskObservedState::Timeout => TaskOutcomeKind::Timeout,
        _ => TaskOutcomeKind::Failed,
    }
}

fn validate_task_payload(task: &TaskResource) -> Result<()> {
    match task.task_type {
        TaskType::Script => {
            if populated_field(task.command_line.as_deref()) {
                anyhow::bail!("script task cannot include command_line");
            }
            if !populated_field(task.script_content.as_deref()) {
                anyhow::bail!("script task missing script_content");
            }
        }
        TaskType::Command => {
            if populated_field(task.script_content.as_deref()) {
                anyhow::bail!("command task cannot include script_content");
            }
            if !populated_field(task.command_line.as_deref()) {
                anyhow::bail!("command task missing command_line");
            }
        }
    }

    Ok(())
}

fn populated_field(value: Option<&str>) -> bool {
    value.is_some_and(|value| !value.is_empty())
}

fn failure_patch(
    category: TaskFinalResultCategory,
    error_message: String,
    now: &str,
) -> TaskApplyPatch {
    TaskApplyPatch {
        observed_state: Some(TaskObservedState::Failed),
        finished_at: Some(now.to_string()),
        error_message: Some(category.annotate_error_message(error_message)),
        updated_at: Some(now.to_string()),
        ..Default::default()
    }
}

fn executor_failure_patch(error_message: String, now: &str) -> TaskApplyPatch {
    failure_patch(
        TaskFinalResultCategory::FailedExecutorStart,
        error_message,
        now,
    )
}

fn restart_reconcile_failure_patch(now: &str) -> TaskApplyPatch {
    executor_failure_patch(
        "task was already running before reconciliation; skipping re-execution".to_string(),
        now,
    )
}

fn timestamp(now: DateTime<Utc>) -> String {
    now.to_rfc3339_opts(SecondsFormat::Secs, true)
}
