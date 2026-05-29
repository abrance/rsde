use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobManageError {
    NodeNotFound(String),
}

impl fmt::Display for JobManageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NodeNotFound(node_id) => write!(f, "node not found: {node_id}"),
        }
    }
}

impl std::error::Error for JobManageError {}

pub type Result<T> = std::result::Result<T, JobManageError>;
