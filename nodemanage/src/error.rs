use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeManageError {
    NotFound(String),
    InvalidInput(String),
    Storage(String),
}

impl fmt::Display for NodeManageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound(message) => write!(f, "node not found: {message}"),
            Self::InvalidInput(message) => write!(f, "invalid node input: {message}"),
            Self::Storage(message) => write!(f, "node storage error: {message}"),
        }
    }
}

impl std::error::Error for NodeManageError {}

pub type Result<T> = std::result::Result<T, NodeManageError>;

impl From<serde_json::Error> for NodeManageError {
    fn from(err: serde_json::Error) -> Self {
        Self::Storage(err.to_string())
    }
}
