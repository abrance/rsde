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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskFinalResultCategory {
    Succeeded,
    FailedNonZeroExit,
    FailedInvalidPayload,
    FailedExecutorStart,
    Timeout,
}

impl TaskFinalResultCategory {
    pub fn annotate_error_message(self, error_message: impl Into<String>) -> String {
        let error_message = error_message.into();
        match self {
            Self::FailedInvalidPayload => format!("failed_invalid_payload: {error_message}"),
            Self::FailedExecutorStart => format!("failed_executor_start: {error_message}"),
            Self::Succeeded | Self::FailedNonZeroExit | Self::Timeout => error_message,
        }
    }

    pub fn from_task_result(
        observed_state: TaskObservedState,
        exit_code: Option<i32>,
        error_message: Option<&str>,
    ) -> Option<Self> {
        match observed_state {
            TaskObservedState::Succeeded => Some(Self::Succeeded),
            TaskObservedState::Timeout => Some(Self::Timeout),
            TaskObservedState::Failed if exit_code.is_some_and(|code| code != 0) => {
                Some(Self::FailedNonZeroExit)
            }
            TaskObservedState::Failed
                if has_error_category_prefix(error_message, "failed_invalid_payload:") =>
            {
                Some(Self::FailedInvalidPayload)
            }
            TaskObservedState::Failed
                if has_error_category_prefix(error_message, "failed_executor_start:") =>
            {
                Some(Self::FailedExecutorStart)
            }
            TaskObservedState::Failed
            | TaskObservedState::Queued
            | TaskObservedState::Acknowledged
            | TaskObservedState::Running => None,
        }
    }
}

impl TaskObservedState {
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Succeeded | Self::Failed | Self::Timeout)
    }

    pub fn can_transition_to(self, next: Self) -> bool {
        if self == next {
            return true;
        }

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

    pub fn final_result_category(&self) -> Option<TaskFinalResultCategory> {
        TaskFinalResultCategory::from_task_result(
            self.observed_state,
            self.exit_code,
            self.error_message.as_deref(),
        )
    }
}

fn has_error_category_prefix(error_message: Option<&str>, prefix: &str) -> bool {
    error_message.is_some_and(|error_message| error_message.starts_with(prefix))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodePrecheck {
    pub node_id: String,
    pub allowed: bool,
    pub status: NodeStatus,
    pub reason: String,
}
