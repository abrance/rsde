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
            environment: None,
            ssh_port: None,
            ssh_username: None,
            ssh_password: None,
            ssh_private_key: None,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeSummary {
    pub node_id: String,
    pub node_name: String,
    pub environment: String,
    pub labels: Vec<String>,
    pub lifecycle_state: String,
    pub install_phase: String,
    pub binding_state: String,
    pub online_status: String,
    pub last_heartbeat_at: Option<DateTime<Utc>>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeBaseInfo {
    pub node_id: String,
    pub node_name: String,
    pub endpoint: String,
    pub environment: String,
    pub labels: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeBindingView {
    pub node_id: String,
    pub agent_id: String,
    pub binding_state: String,
    pub first_registered_at: DateTime<Utc>,
    pub last_handshake_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeStatusView {
    pub lifecycle_state: String,
    pub install_phase: String,
    pub binding_state: String,
    pub online_status: String,
    pub last_heartbeat_at: Option<DateTime<Utc>>,
    pub status_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HeartbeatRef {
    pub data_link_id: String,
    pub link_purpose: String,
    pub owner_service: String,
    pub result_table_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InstallRequestSummary {
    pub host: Option<String>,
    pub ssh_port: Option<u16>,
    pub username: Option<String>,
    pub rsagent_package_url: Option<String>,
    pub install_root: Option<String>,
    pub labels: Vec<String>,
    pub plugin_names: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeInstallTaskView {
    pub install_task_id: String,
    pub node_id: String,
    pub task_state: String,
    pub current_step: Option<String>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub retryable: bool,
    pub request_summary: Option<InstallRequestSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeDetail {
    pub node: NodeBaseInfo,
    pub binding: Option<NodeBindingView>,
    pub status: NodeStatusView,
    pub latest_install_task: Option<NodeInstallTaskView>,
    pub heartbeat_ref: Option<HeartbeatRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeStatusBatchItem {
    pub node_id: String,
    pub install_phase: String,
    pub binding_state: String,
    pub online_status: String,
    pub last_heartbeat_at: Option<DateTime<Utc>>,
    pub updated_at: DateTime<Utc>,
    pub status_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeInstallTaskReceipt {
    pub install_task_id: String,
    pub node_id: String,
    pub accepted: bool,
    pub task_state: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RebindNodeRequest {
    pub target_agent_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RebindNodeResponse {
    pub accepted: bool,
    pub node_id: String,
    pub target_agent_id: String,
    pub binding_state: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_agent_id: Option<String>,
}
