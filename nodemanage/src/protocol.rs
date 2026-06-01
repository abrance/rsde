use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{Node, NodeStatus, Result};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentRegistration {
    pub agent_id: String,
    pub hostname: String,
    pub endpoint: String,
    #[serde(default)]
    pub labels: Vec<String>,
}

impl AgentRegistration {
    pub fn into_node(self) -> Node {
        let now = Utc::now();
        Node {
            id: self.agent_id,
            name: self.hostname,
            endpoint: self.endpoint,
            status: NodeStatus::Online,
            labels: self.labels,
            created_at: now,
            updated_at: now,
            last_heartbeat_at: Some(now),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentSyncRequest {
    pub agent_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_id: Option<String>,
    pub agent_version: String,
    pub hostname: String,
    pub os_family: String,
    pub os_distribution: String,
    pub arch: String,
    #[serde(default)]
    pub capabilities: Vec<String>,
    pub started_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SyncBindingState {
    Bound,
    Conflict,
    Unbound,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AgentRunMode {
    Active,
    Idle,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HeartbeatConfig {
    pub version: String,
    pub data_link_id: String,
    pub vm_base_url: String,
    pub interval_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaskFilterDefaults {
    pub states: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct JobManageConfig {
    pub version: String,
    pub base_url: String,
    pub task_filter_defaults: TaskFilterDefaults,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentSyncResponse {
    pub accepted: bool,
    pub agent_id: String,
    pub bound_node_id: String,
    pub binding_state: SyncBindingState,
    pub agent_run_mode: AgentRunMode,
    pub config_version: String,
    pub heartbeat_config: HeartbeatConfig,
    pub job_manage_config: JobManageConfig,
    pub sync_interval_secs: u64,
    pub task_sync_interval_secs: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rejection_reason: Option<String>,
}

#[async_trait]
pub trait AgentRegistry: Clone + Send + Sync + 'static {
    async fn register(&self, registration: AgentRegistration) -> Result<Node>;
    async fn sync(&self, request: AgentSyncRequest) -> Result<AgentSyncResponse>;
}
