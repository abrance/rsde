use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueryEngineError {
    DataLink(String),
    HeartbeatStore(String),
}

impl fmt::Display for QueryEngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DataLink(message) => write!(f, "{message}"),
            Self::HeartbeatStore(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for QueryEngineError {}

pub type Result<T> = std::result::Result<T, QueryEngineError>;

impl From<datalink_engine::DataLinkError> for QueryEngineError {
    fn from(err: datalink_engine::DataLinkError) -> Self {
        Self::DataLink(err.to_string())
    }
}
