use std::fmt;

use crate::models::TaskObservedState;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobManageError {
    NodeNotFound(String),
    InvalidTaskObservedStateTransition {
        from: TaskObservedState,
        to: TaskObservedState,
    },
}

impl fmt::Display for JobManageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NodeNotFound(node_id) => write!(f, "node not found: {node_id}"),
            Self::InvalidTaskObservedStateTransition { from, to } => {
                write!(
                    f,
                    "invalid task observed state transition: {from:?} -> {to:?}"
                )
            }
        }
    }
}

impl std::error::Error for JobManageError {}

pub type Result<T> = std::result::Result<T, JobManageError>;
