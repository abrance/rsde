use std::{future::Future, pin::Pin};

use anyhow::{Context, Result, anyhow};
use chrono::{DateTime, SecondsFormat, Utc};
use job_manage::{
    TaskApplyIdentity, TaskApplyPatch, TaskListQuery, TaskObservedState, TaskResource, TaskType,
};
use tracing::{info, warn};

use crate::{
    clients::job_manage::{JobManageSyncClient, JobManageTransport, ReqwestJobManageTransport},
    executor::{ExecutionResult, LocalTaskExecutor},
    registration::AgentRuntimeState,
    retry::{transport_max_attempts, transport_retry_delay},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskSyncSkipReason {
    MissingConfig,
    LoopsDisabled,
    NoTasks,
    ExistingActiveTask {
        task_id: String,
        observed_state: TaskObservedState,
    },
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

pub struct TaskSyncLoop<T, E> {
    transport: T,
    executor: E,
    updated_after: Option<String>,
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
        }
    }

    pub async fn tick(
        &mut self,
        state: &AgentRuntimeState,
        agent_id: &str,
        now: DateTime<Utc>,
    ) -> Result<TaskSyncTick> {
        let tick = sync_once(
            &mut self.transport,
            &self.executor,
            state,
            agent_id,
            self.updated_after.clone(),
            now,
        )
        .await?;
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
    if !state.loops_enabled() {
        return Ok(TaskSyncTick::Skipped {
            reason: TaskSyncSkipReason::LoopsDisabled,
        });
    }

    let config = match state.effective_config() {
        Some(config) => config,
        None => {
            return Ok(TaskSyncTick::Skipped {
                reason: TaskSyncSkipReason::MissingConfig,
            });
        }
    };

    let node_id = state
        .local_node_id()
        .ok_or_else(|| anyhow!("node_id unavailable for task sync"))?;
    let client = JobManageSyncClient::new(config.job_manage_config.base_url.clone());
    let query = TaskListQuery {
        agent_id: agent_id.to_string(),
        node_id: node_id.to_string(),
        states: task_query_states(&config.job_manage_config.task_filter_defaults.states)?,
        updated_after: updated_after.clone(),
    };

    let mut attempt = 0;
    let mut tasks = loop {
        match client.list_tasks(transport, &query).await {
            Ok(tasks) => {
                if attempt > 0 {
                    info!(
                        agent_id,
                        node_id,
                        retries = attempt,
                        max_attempts = transport_max_attempts(),
                        updated_after = ?query.updated_after,
                        "task sync list recovered after retry"
                    );
                }
                break tasks;
            }
            Err(error) => match transport_retry_delay(attempt) {
                Some(delay) => {
                    attempt += 1;
                    warn!(
                        agent_id,
                        node_id,
                        failed_attempt = attempt,
                        next_attempt = attempt + 1,
                        max_attempts = transport_max_attempts(),
                        delay_ms = delay.as_millis() as u64,
                        error = %error,
                        "task sync list failed; retrying"
                    );
                    tokio::time::sleep(delay).await;
                }
                None => {
                    warn!(
                        agent_id,
                        node_id,
                        failed_attempt = attempt + 1,
                        max_attempts = transport_max_attempts(),
                        error = %error,
                        "task sync list failed; retry budget exhausted"
                    );
                    return Err(error);
                }
            },
        }
    };

    if attempt > 0 {
        info!(
            agent_id,
            node_id,
            updated_after = ?query.updated_after,
            recovered_after_retries = attempt,
            "task sync list completed after retries"
        );
    }

    let Some(task) = tasks.drain(..).next() else {
        return Ok(TaskSyncTick::Skipped {
            reason: TaskSyncSkipReason::NoTasks,
        });
    };

    if matches!(
        task.observed_state,
        TaskObservedState::Acknowledged | TaskObservedState::Running
    ) {
        if updated_after.is_none() {
            warn!(
                task_id = %task.task_id,
                observed_state = ?task.observed_state,
                claimed_at = ?task.claimed_at,
                started_at = ?task.started_at,
                updated_at = ?task.updated_at,
                "active task detected on initial task-sync poll after startup; execution remains stalled until backend state changes"
            );
        }
        return Ok(TaskSyncTick::Skipped {
            reason: TaskSyncSkipReason::ExistingActiveTask {
                task_id: task.task_id,
                observed_state: task.observed_state,
            },
        });
    }

    let identity = TaskApplyIdentity {
        task_id: task.task_id.clone(),
        agent_id: task.agent_id.clone(),
        node_id: task.node_id.clone(),
    };
    let claimed_at = timestamp(now);

    apply_task_with_retry(
        &client,
        transport,
        &identity,
        &TaskApplyPatch {
            observed_state: Some(TaskObservedState::Acknowledged),
            claimed_at: Some(claimed_at.clone()),
            updated_at: Some(claimed_at.clone()),
            ..Default::default()
        },
    )
    .await?;

    let final_patch = match validate_task_payload(&task) {
        Ok(()) => {
            let started_at = timestamp(Utc::now());
            apply_task_with_retry(
                &client,
                transport,
                &identity,
                &TaskApplyPatch {
                    observed_state: Some(TaskObservedState::Running),
                    started_at: Some(started_at.clone()),
                    updated_at: Some(started_at.clone()),
                    ..Default::default()
                },
            )
            .await?;

            match executor.execute(&task).await {
                Ok(result) => execution_result_patch(result, timestamp(Utc::now())),
                Err(error) => execution_failure_patch(error.to_string(), timestamp(Utc::now())),
            }
        }
        Err(error) => validation_failure_patch(error.to_string(), claimed_at),
    };

    let applied = apply_task_with_retry(&client, transport, &identity, &final_patch).await?;
    Ok(TaskSyncTick::Applied {
        task_id: applied.task_id,
        final_state: applied.observed_state,
    })
}

async fn apply_task_with_retry<T>(
    client: &JobManageSyncClient,
    transport: &mut T,
    identity: &TaskApplyIdentity,
    patch: &TaskApplyPatch,
) -> Result<crate::clients::job_manage::TaskApplyAck>
where
    T: JobManageTransport,
{
    let mut attempt = 0;

    loop {
        match client.apply_task(transport, identity, patch).await {
            Ok(ack) => {
                if attempt > 0 {
                    info!(
                        task_id = %identity.task_id,
                        requested_state = ?patch.observed_state,
                        applied_state = ?ack.observed_state,
                        retries = attempt,
                        max_attempts = transport_max_attempts(),
                        "task apply recovered via duplicate-safe retry path"
                    );
                }
                return Ok(ack);
            }
            Err(error) => match transport_retry_delay(attempt) {
                Some(delay) => {
                    attempt += 1;
                    warn!(
                        task_id = %identity.task_id,
                        requested_state = ?patch.observed_state,
                        failed_attempt = attempt,
                        next_attempt = attempt + 1,
                        max_attempts = transport_max_attempts(),
                        delay_ms = delay.as_millis() as u64,
                        error = %error,
                        "task apply failed; retrying duplicate-safe write"
                    );
                    tokio::time::sleep(delay).await;
                }
                None => {
                    warn!(
                        task_id = %identity.task_id,
                        requested_state = ?patch.observed_state,
                        failed_attempt = attempt + 1,
                        max_attempts = transport_max_attempts(),
                        error = %error,
                        "task apply failed; retry budget exhausted"
                    );
                    return Err(error);
                }
            },
        }
    }
}

fn parse_default_states(states: &[String]) -> Result<Vec<TaskObservedState>> {
    states
        .iter()
        .map(|state| parse_state(state))
        .collect::<Result<Vec<_>>>()
}

fn task_query_states(states: &[String]) -> Result<Vec<TaskObservedState>> {
    let mut states = parse_default_states(states)?;

    for active_state in [TaskObservedState::Acknowledged, TaskObservedState::Running] {
        if !states.contains(&active_state) {
            states.push(active_state);
        }
    }

    Ok(states)
}

fn parse_state(value: &str) -> Result<TaskObservedState> {
    serde_json::from_str::<TaskObservedState>(&format!("\"{value}\""))
        .with_context(|| format!("invalid task state default: {value}"))
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

fn validation_failure_patch(error_message: String, updated_at: String) -> TaskApplyPatch {
    TaskApplyPatch {
        observed_state: Some(TaskObservedState::Failed),
        error_message: Some(error_message),
        updated_at: Some(updated_at),
        ..Default::default()
    }
}

fn execution_failure_patch(error_message: String, finished_at: String) -> TaskApplyPatch {
    TaskApplyPatch {
        observed_state: Some(TaskObservedState::Failed),
        finished_at: Some(finished_at.clone()),
        error_message: Some(error_message),
        updated_at: Some(finished_at),
        ..Default::default()
    }
}

fn execution_result_patch(result: ExecutionResult, finished_at: String) -> TaskApplyPatch {
    TaskApplyPatch {
        observed_state: Some(result.state),
        finished_at: Some(finished_at.clone()),
        stdout: Some(result.stdout),
        stderr: Some(result.stderr),
        exit_code: result.exit_code,
        updated_at: Some(finished_at),
        ..Default::default()
    }
}

fn timestamp(now: DateTime<Utc>) -> String {
    now.to_rfc3339_opts(SecondsFormat::Secs, true)
}
