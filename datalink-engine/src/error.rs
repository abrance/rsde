use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DataLinkError {
    InvalidArgument(String),
    EnumNotSupported(String),
    ResultTableNameConflict(String),
    EtlPipelineInvalid(String),
    EtlModeNotSupported(String),
    NotFound(String),
    StatusTransitionInvalid { from: String, to: String },
    StatusMessageRequired,
    StatusMessageInvalid(String),
    IdempotencyConflict(String),
    BackendNotSupported(String),
    Repository(String),
}

impl fmt::Display for DataLinkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidArgument(message)
            | Self::EnumNotSupported(message)
            | Self::ResultTableNameConflict(message)
            | Self::EtlPipelineInvalid(message)
            | Self::EtlModeNotSupported(message)
            | Self::NotFound(message)
            | Self::StatusMessageInvalid(message)
            | Self::IdempotencyConflict(message)
            | Self::BackendNotSupported(message)
            | Self::Repository(message) => write!(f, "{message}"),
            Self::StatusTransitionInvalid { from, to } => {
                write!(f, "invalid status transition from {from} to {to}")
            }
            Self::StatusMessageRequired => write!(f, "status_message is required"),
        }
    }
}

impl std::error::Error for DataLinkError {}

pub type Result<T> = std::result::Result<T, DataLinkError>;
