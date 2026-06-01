use std::{future::Future, pin::Pin};

use anyhow::{Context, Result, anyhow};
use chrono::{DateTime, SecondsFormat, Utc};
use job_manage::{
    TaskApplyIdentity, TaskApplyPatch, TaskListQuery, TaskObservedState, TaskResource, TaskType,
};

use crate::{
    clients::job_manage::{JobManageSyncClient, JobManageTransport, ReqwestJobManageTransport},
    executor::{ExecutionResult, LocalTaskExecutor},
    registration::AgentRuntimeState,
};

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
            TaskSyncTick::Applied { .. }
                | TaskSyncTick::Skipped {
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
        states: parse_default_states(&config.job_manage_config.task_filter_defaults.states)?,
        updated_after,
    };
    let mut tasks = client.list_tasks(transport, &query)?;

    let Some(task) = tasks.drain(..).next() else {
        return Ok(TaskSyncTick::Skipped {
            reason: TaskSyncSkipReason::NoTasks,
        });
    };

    let identity = TaskApplyIdentity {
        task_id: task.task_id.clone(),
        agent_id: task.agent_id.clone(),
        node_id: task.node_id.clone(),
    };
    let now = timestamp(now);

    client.apply_task(
        transport,
        &identity,
        &TaskApplyPatch {
            observed_state: Some(TaskObservedState::Acknowledged),
            claimed_at: Some(now.clone()),
            updated_at: Some(now.clone()),
            ..Default::default()
        },
    )?;

    let final_patch = match validate_task_payload(&task) {
        Ok(()) => {
            client.apply_task(
                transport,
                &identity,
                &TaskApplyPatch {
                    observed_state: Some(TaskObservedState::Running),
                    started_at: Some(now.clone()),
                    updated_at: Some(now.clone()),
                    ..Default::default()
                },
            )?;

            match executor.execute(&task).await {
                Ok(result) => TaskApplyPatch {
                    observed_state: Some(result.state),
                    finished_at: Some(now.clone()),
                    stdout: Some(result.stdout),
                    stderr: Some(result.stderr),
                    exit_code: result.exit_code,
                    updated_at: Some(now.clone()),
                    ..Default::default()
                },
                Err(error) => failure_patch(error.to_string(), &now),
            }
        }
        Err(error) => failure_patch(error.to_string(), &now),
    };

    let applied = client.apply_task(transport, &identity, &final_patch)?;
    Ok(TaskSyncTick::Applied {
        task_id: applied.task_id,
        final_state: applied.observed_state,
    })
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

fn failure_patch(error_message: String, now: &str) -> TaskApplyPatch {
    TaskApplyPatch {
        observed_state: Some(TaskObservedState::Failed),
        finished_at: Some(now.to_string()),
        error_message: Some(error_message),
        updated_at: Some(now.to_string()),
        ..Default::default()
    }
}

fn timestamp(now: DateTime<Utc>) -> String {
    now.to_rfc3339_opts(SecondsFormat::Secs, true)
}
