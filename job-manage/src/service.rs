use std::sync::{Arc, RwLock};

use nodemanage::{NodeManageError, NodeManager, NodeRepository, RsAgentInstaller};

use crate::{JobManageError, NodePrecheck, Result, TaskObservedState, TaskResource};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskListQuery {
    pub agent_id: String,
    pub node_id: String,
    pub states: Vec<TaskObservedState>,
    pub updated_after: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskApplyIdentity {
    pub task_id: String,
    pub agent_id: String,
    pub node_id: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TaskApplyPatch {
    pub observed_state: Option<TaskObservedState>,
    pub claimed_at: Option<String>,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub stdout: Option<String>,
    pub stderr: Option<String>,
    pub exit_code: Option<i32>,
    pub error_message: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskServerOwnedField {
    TaskId,
    JobId,
    NodeId,
    AgentId,
    TaskType,
    ScriptContent,
    CommandLine,
    Interpreter,
    Args,
    Env,
    WorkingDir,
    TimeoutSecs,
    DesiredState,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TaskApplyRequest {
    pub patch: TaskApplyPatch,
    pub rejected_fields: Vec<TaskServerOwnedField>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskServiceError {
    TaskNotFound(String),
    TaskOwnershipMismatch {
        task_id: String,
        agent_id: String,
        node_id: String,
    },
    RejectedServerOwnedFields(Vec<TaskServerOwnedField>),
    TaskModel(JobManageError),
}

impl std::fmt::Display for TaskServiceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TaskNotFound(task_id) => write!(f, "task not found: {task_id}"),
            Self::TaskOwnershipMismatch {
                task_id,
                agent_id,
                node_id,
            } => write!(
                f,
                "task ownership mismatch: task_id={task_id}, agent_id={agent_id}, node_id={node_id}"
            ),
            Self::RejectedServerOwnedFields(fields) => {
                write!(f, "rejected server-owned fields: {fields:?}")
            }
            Self::TaskModel(err) => err.fmt(f),
        }
    }
}

impl std::error::Error for TaskServiceError {}

pub type TaskServiceResult<T> = std::result::Result<T, TaskServiceError>;

#[derive(Debug, Clone, Default)]
pub struct TaskSyncService {
    tasks: Arc<RwLock<Vec<TaskResource>>>,
}

impl TaskSyncService {
    pub fn new(tasks: Vec<TaskResource>) -> Self {
        Self {
            tasks: Arc::new(RwLock::new(tasks)),
        }
    }

    pub async fn list_tasks(&self, query: &TaskListQuery) -> TaskServiceResult<Vec<TaskResource>> {
        let tasks = self.tasks.read().expect("task store read lock poisoned");
        let filtered = tasks
            .iter()
            .filter(|task| task.agent_id == query.agent_id && task.node_id == query.node_id)
            .filter(|task| query.states.is_empty() || query.states.contains(&task.observed_state))
            .filter(|task| match query.updated_after.as_deref() {
                Some(updated_after) => task
                    .updated_at
                    .as_deref()
                    .map(|updated_at| updated_at > updated_after)
                    .unwrap_or(false),
                None => true,
            })
            .cloned()
            .collect();

        Ok(filtered)
    }

    pub async fn apply_task(
        &self,
        identity: &TaskApplyIdentity,
        request: TaskApplyRequest,
    ) -> TaskServiceResult<TaskResource> {
        if !request.rejected_fields.is_empty() {
            return Err(TaskServiceError::RejectedServerOwnedFields(
                request.rejected_fields,
            ));
        }

        let mut tasks = self.tasks.write().expect("task store write lock poisoned");
        let task = tasks
            .iter_mut()
            .find(|task| task.task_id == identity.task_id)
            .ok_or_else(|| TaskServiceError::TaskNotFound(identity.task_id.clone()))?;

        if task.agent_id != identity.agent_id || task.node_id != identity.node_id {
            return Err(TaskServiceError::TaskOwnershipMismatch {
                task_id: identity.task_id.clone(),
                agent_id: identity.agent_id.clone(),
                node_id: identity.node_id.clone(),
            });
        }

        apply_task_patch(task, &request.patch).map_err(TaskServiceError::TaskModel)?;

        Ok(task.clone())
    }
}

#[derive(Debug, Clone)]
pub struct PrecheckService<R, I>
where
    R: NodeRepository,
    I: RsAgentInstaller,
{
    node_manager: NodeManager<R, I>,
}

impl<R, I> PrecheckService<R, I>
where
    R: NodeRepository,
    I: RsAgentInstaller,
{
    pub fn new(node_manager: NodeManager<R, I>) -> Self {
        Self { node_manager }
    }

    pub async fn precheck(&self, node_id: &str) -> Result<NodePrecheck> {
        let node = self
            .node_manager
            .get(node_id)
            .await
            .map_err(map_node_error)?
            .ok_or_else(|| JobManageError::NodeNotFound(node_id.to_string()))?;

        let (allowed, reason) = match node.status {
            nodemanage::NodeStatus::Online => (true, "node is online".to_string()),
            nodemanage::NodeStatus::Offline => (false, "node is offline".to_string()),
            nodemanage::NodeStatus::Maintenance => (false, "node is in maintenance".to_string()),
        };

        Ok(NodePrecheck {
            node_id: node.id,
            allowed,
            status: node.status,
            reason,
        })
    }
}

fn map_node_error(err: NodeManageError) -> JobManageError {
    match err {
        NodeManageError::NotFound(node_id) => JobManageError::NodeNotFound(node_id),
        NodeManageError::InvalidInput(message) | NodeManageError::Storage(message) => {
            JobManageError::NodeNotFound(message)
        }
    }
}

fn apply_task_patch(task: &mut TaskResource, patch: &TaskApplyPatch) -> Result<()> {
    if let Some(observed_state) = patch.observed_state
        && observed_state != task.observed_state
    {
        task.transition_observed_state(observed_state)?;
    }

    if let Some(claimed_at) = &patch.claimed_at {
        task.claimed_at = Some(claimed_at.clone());
    }
    if let Some(started_at) = &patch.started_at {
        task.started_at = Some(started_at.clone());
    }
    if let Some(finished_at) = &patch.finished_at {
        task.finished_at = Some(finished_at.clone());
    }
    if let Some(stdout) = &patch.stdout {
        task.stdout = Some(stdout.clone());
    }
    if let Some(stderr) = &patch.stderr {
        task.stderr = Some(stderr.clone());
    }
    if let Some(exit_code) = patch.exit_code {
        task.exit_code = Some(exit_code);
    }
    if let Some(error_message) = &patch.error_message {
        task.error_message = Some(error_message.clone());
    }
    if let Some(updated_at) = &patch.updated_at {
        task.updated_at = Some(updated_at.clone());
    }

    Ok(())
}
