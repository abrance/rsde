use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeManageErrorCode {
    NodeNotFound,
    InstallTaskNotFound,
    BindingNotFound,
    BindingConflict,
    TargetAgentNotFound,
    RebindTargetAlreadyBound,
    InstallConfigInvalid,
    InstallExecutionFailed,
    AgentRegistrationTimeout,
    StatusQueryFailed,
    HeartbeatDatalinkNotReady,
    InvalidArgument,
    InvalidRebindRequest,
    InternalError,
}

impl NodeManageErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NodeNotFound => "NODE_NOT_FOUND",
            Self::InstallTaskNotFound => "INSTALL_TASK_NOT_FOUND",
            Self::BindingNotFound => "BINDING_NOT_FOUND",
            Self::BindingConflict => "BINDING_CONFLICT",
            Self::TargetAgentNotFound => "TARGET_AGENT_NOT_FOUND",
            Self::RebindTargetAlreadyBound => "REBIND_TARGET_ALREADY_BOUND",
            Self::InstallConfigInvalid => "INSTALL_CONFIG_INVALID",
            Self::InstallExecutionFailed => "INSTALL_EXECUTION_FAILED",
            Self::AgentRegistrationTimeout => "AGENT_REGISTRATION_TIMEOUT",
            Self::StatusQueryFailed => "STATUS_QUERY_FAILED",
            Self::HeartbeatDatalinkNotReady => "HEARTBEAT_DATALINK_NOT_READY",
            Self::InvalidArgument => "INVALID_ARGUMENT",
            Self::InvalidRebindRequest => "INVALID_REBIND_REQUEST",
            Self::InternalError => "INTERNAL_ERROR",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeManageApiError {
    pub code: NodeManageErrorCode,
    pub message: String,
    pub retryable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeManageError {
    NotFound(String),
    InstallTaskNotFound(String),
    BindingNotFound(String),
    TargetAgentNotFound(String),
    RebindTargetAlreadyBound(String),
    InvalidRebindRequest(String),
    InvalidInput(String),
    Conflict(String),
    Storage(String),
}

impl fmt::Display for NodeManageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound(message) => write!(f, "node not found: {message}"),
            Self::InstallTaskNotFound(message) => write!(f, "install task not found: {message}"),
            Self::BindingNotFound(message) => write!(f, "binding not found: {message}"),
            Self::TargetAgentNotFound(message) => write!(f, "target agent not found: {message}"),
            Self::RebindTargetAlreadyBound(message) => {
                write!(f, "rebind target already bound: {message}")
            }
            Self::InvalidRebindRequest(message) => write!(f, "invalid rebind request: {message}"),
            Self::InvalidInput(message) => write!(f, "invalid node input: {message}"),
            Self::Conflict(message) => write!(f, "node conflict: {message}"),
            Self::Storage(message) => write!(f, "node storage error: {message}"),
        }
    }
}

impl std::error::Error for NodeManageError {}

pub type Result<T> = std::result::Result<T, NodeManageError>;

impl NodeManageError {
    pub fn to_api_error(&self) -> NodeManageApiError {
        match self {
            Self::NotFound(_message) => NodeManageApiError {
                code: NodeManageErrorCode::NodeNotFound,
                message: self.to_string(),
                retryable: false,
            },
            Self::InstallTaskNotFound(_message) => NodeManageApiError {
                code: NodeManageErrorCode::InstallTaskNotFound,
                message: self.to_string(),
                retryable: false,
            },
            Self::BindingNotFound(_message) => NodeManageApiError {
                code: NodeManageErrorCode::BindingNotFound,
                message: self.to_string(),
                retryable: false,
            },
            Self::TargetAgentNotFound(_message) => NodeManageApiError {
                code: NodeManageErrorCode::TargetAgentNotFound,
                message: self.to_string(),
                retryable: false,
            },
            Self::RebindTargetAlreadyBound(_message) => NodeManageApiError {
                code: NodeManageErrorCode::RebindTargetAlreadyBound,
                message: self.to_string(),
                retryable: false,
            },
            Self::InvalidRebindRequest(_message) => NodeManageApiError {
                code: NodeManageErrorCode::InvalidRebindRequest,
                message: self.to_string(),
                retryable: false,
            },
            Self::InvalidInput(message) => {
                let lowered = message.to_ascii_lowercase();
                let code = if lowered.contains("heartbeat datalink") {
                    NodeManageErrorCode::HeartbeatDatalinkNotReady
                } else if lowered.contains("query engine") || lowered.contains("query ") {
                    NodeManageErrorCode::StatusQueryFailed
                } else {
                    NodeManageErrorCode::InvalidArgument
                };
                NodeManageApiError {
                    code,
                    message: self.to_string(),
                    retryable: false,
                }
            }
            Self::Storage(_) => NodeManageApiError {
                code: NodeManageErrorCode::InternalError,
                message: self.to_string(),
                retryable: false,
            },
        }
    }
}

impl From<serde_json::Error> for NodeManageError {
    fn from(err: serde_json::Error) -> Self {
        Self::Storage(err.to_string())
    }
}
