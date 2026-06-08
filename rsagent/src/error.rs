use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentSyncError {
    Timeout,
    Transport,
    HttpError(u16),
    DecodeError,
    Rejection(String),
    Unknown(String),
}

impl fmt::Display for AgentSyncError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AgentSyncError::Timeout => write!(f, "Timeout"),
            AgentSyncError::Transport => write!(f, "Transport error"),
            AgentSyncError::HttpError(status) => write!(f, "HTTP error {}", status),
            AgentSyncError::DecodeError => write!(f, "Decode error"),
            AgentSyncError::Rejection(reason) => write!(f, "Rejection: {}", reason),
            AgentSyncError::Unknown(msg) => write!(f, "Unknown error: {}", msg),
        }
    }
}

pub fn categorize_sync_error(error_str: &str) -> AgentSyncError {
    let lower = error_str.to_lowercase();
    if lower.contains("timeout") {
        AgentSyncError::Timeout
    } else if lower.contains("connection refused")
        || lower.contains("dns error")
        || lower.contains("connect error")
    {
        AgentSyncError::Transport
    } else if lower.contains("failed to decode") || lower.contains("invalid json") {
        AgentSyncError::DecodeError
    } else if let Some(status_idx) = lower.find("status ") {
        let status_part = &lower[status_idx + 7..];
        if status_part.len() >= 3 {
            if let Ok(status) = status_part[..3].parse::<u16>() {
                return AgentSyncError::HttpError(status);
            }
        }
        AgentSyncError::Transport
    } else {
        AgentSyncError::Unknown(error_str.to_string())
    }
}
