use std::collections::BTreeMap;

use crate::error::{JobManageError, Result};
use nodemanage::NodeStatus;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskType {
    Script,
    Command,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskDesiredState {
    Queued,
    CancelRequested,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskObservedState {
    Queued,
    Acknowledged,
    Running,
    Succeeded,
    Failed,
    Timeout,
}

impl TaskObservedState {
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Succeeded | Self::Failed | Self::Timeout)
    }

    pub fn can_transition_to(self, next: Self) -> bool {
        matches!(
            (self, next),
            (Self::Queued, Self::Acknowledged)
                | (Self::Acknowledged, Self::Running)
                | (Self::Acknowledged, Self::Succeeded)
                | (Self::Acknowledged, Self::Failed)
                | (Self::Acknowledged, Self::Timeout)
                | (Self::Running, Self::Succeeded)
                | (Self::Running, Self::Failed)
                | (Self::Running, Self::Timeout)
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskResource {
    pub task_id: String,
    pub job_id: String,
    pub node_id: String,
    pub agent_id: String,
    pub task_type: TaskType,
    pub script_content: Option<String>,
    pub command_line: Option<String>,
    pub interpreter: Option<String>,
    pub args: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub working_dir: Option<String>,
    pub timeout_secs: Option<u64>,
    pub desired_state: TaskDesiredState,
    pub observed_state: TaskObservedState,
    pub stdout: Option<String>,
    pub stderr: Option<String>,
    pub exit_code: Option<i32>,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub error_message: Option<String>,
    pub claimed_at: Option<String>,
    pub updated_at: Option<String>,
}

impl TaskResource {
    pub fn transition_observed_state(&mut self, next: TaskObservedState) -> Result<()> {
        if !self.observed_state.can_transition_to(next) {
            return Err(JobManageError::InvalidTaskObservedStateTransition {
                from: self.observed_state,
                to: next,
            });
        }

        self.observed_state = next;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodePrecheck {
    pub node_id: String,
    pub allowed: bool,
    pub status: NodeStatus,
    pub reason: String,
}
