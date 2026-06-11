use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum NodeStatus {
    Online,
    Offline,
    Maintenance,
}

impl NodeStatus {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "online" => Some(Self::Online),
            "offline" => Some(Self::Offline),
            "maintenance" => Some(Self::Maintenance),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Node {
    pub id: String,
    pub name: String,
    pub endpoint: String,
    pub status: NodeStatus,
    pub labels: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_heartbeat_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub environment: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ssh_port: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ssh_username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ssh_password: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ssh_private_key: Option<String>,
}

impl Node {
    pub fn new(name: String, endpoint: String, labels: Vec<String>) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            name,
            endpoint,
            status: NodeStatus::Offline,
            labels,
            created_at: now,
            updated_at: now,
            last_heartbeat_at: None,
            environment: None,
            ssh_port: None,
            ssh_username: None,
            ssh_password: None,
            ssh_private_key: None,
        }
    }

    pub fn from_create(input: CreateNode) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            name: input.name,
            endpoint: input.endpoint,
            status: NodeStatus::Offline,
            labels: input.labels,
            created_at: now,
            updated_at: now,
            last_heartbeat_at: None,
            environment: input.environment,
            ssh_port: input.ssh_port,
            ssh_username: input.ssh_username,
            ssh_password: input.ssh_password,
            ssh_private_key: input.ssh_private_key,
        }
    }

    /// Returns the effective SSH port (explicit value or default 22).
    pub fn effective_ssh_port(&self) -> u16 {
        self.ssh_port.unwrap_or(22)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CreateNode {
    pub name: String,
    pub endpoint: String,
    #[serde(default)]
    pub labels: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub environment: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ssh_port: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ssh_username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ssh_password: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ssh_private_key: Option<String>,
}

impl CreateNode {
    /// Create a node with only the required fields (name, endpoint).
    pub fn simple(name: impl Into<String>, endpoint: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            endpoint: endpoint.into(),
            labels: vec![],
            environment: None,
            ssh_port: None,
            ssh_username: None,
            ssh_password: None,
            ssh_private_key: None,
        }
    }

    pub fn into_node(self) -> Node {
        Node::from_create(self)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct UpdateNode {
    pub name: Option<String>,
    pub endpoint: Option<String>,
    pub status: Option<NodeStatus>,
    pub labels: Option<Vec<String>>,
    pub environment: Option<String>,
    pub ssh_port: Option<u16>,
    pub ssh_username: Option<String>,
    pub ssh_password: Option<String>,
    pub ssh_private_key: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct PaginationParams {
    pub page: u32,
    pub page_size: u32,
}

impl PaginationParams {
    pub fn new(page: u32, page_size: u32) -> Self {
        Self { page, page_size }
    }

    pub fn offset(self) -> usize {
        self.page.saturating_sub(1) as usize * self.page_size as usize
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PaginatedResult<T> {
    pub items: Vec<T>,
    pub total: u64,
    pub page: u32,
    pub page_size: u32,
    pub total_pages: u32,
}

impl<T> PaginatedResult<T> {
    pub fn new(items: Vec<T>, total: u64, pagination: PaginationParams) -> Self {
        let total_pages = if pagination.page_size == 0 {
            0
        } else {
            total.div_ceil(pagination.page_size as u64) as u32
        };

        Self {
            items,
            total,
            page: pagination.page,
            page_size: pagination.page_size,
            total_pages,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InstallTaskState {
    Pending,
    Running,
    WaitingRegister,
    Succeeded,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InstallTaskStep {
    PrepareInstall,
    ResolveArtifacts,
    WriteRuntimeConfig,
    UploadPackage,
    RunInstallScript,
    StartAgent,
    WaitRegister,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeInstallTask {
    pub install_task_id: String,
    pub node_id: String,
    pub task_state: InstallTaskState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_step: Option<InstallTaskStep>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
    pub started_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<DateTime<Utc>>,
    pub retryable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_host: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_ssh_port: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_rsagent_package_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_install_root: Option<String>,
    #[serde(default)]
    pub request_labels: Vec<String>,
    #[serde(default)]
    pub request_plugin_names: Vec<String>,
}

impl NodeInstallTask {
    pub fn new(node_id: String) -> Self {
        Self {
            install_task_id: Uuid::new_v4().to_string(),
            node_id,
            task_state: InstallTaskState::Pending,
            current_step: None,
            error_code: None,
            error_message: None,
            started_at: Utc::now(),
            finished_at: None,
            retryable: false,
            request_host: None,
            request_ssh_port: None,
            request_username: None,
            request_rsagent_package_url: None,
            request_install_root: None,
            request_labels: vec![],
            request_plugin_names: vec![],
        }
    }

    pub fn with_install_request(mut self, request: &crate::bootstrap::ResolvedInstallRequest) -> Self {
        self.request_host = Some(request.host.clone());
        self.request_ssh_port = Some(request.ssh_port);
        self.request_username = Some(request.username.clone());
        self.request_rsagent_package_url = Some(request.rsagent_package_url.clone());
        self.request_install_root = Some(request.install_root.clone());
        self.request_labels = request.labels.clone();
        self.request_plugin_names = request
            .plugins
            .iter()
            .map(|plugin| plugin.name.clone())
            .collect();
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum BindingState {
    Bound,
    Stale,
    Unbound,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeAgentBinding {
    pub node_id: String,
    pub agent_id: String,
    pub binding_state: BindingState,
    pub first_registered_at: DateTime<Utc>,
    pub last_handshake_at: DateTime<Utc>,
    pub unbind_reason: Option<String>,
}

impl NodeAgentBinding {
    pub fn new(node_id: String, agent_id: String) -> Self {
        let now = Utc::now();
        Self {
            node_id,
            agent_id,
            binding_state: BindingState::Bound,
            first_registered_at: now,
            last_handshake_at: now,
            unbind_reason: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum OnlineStatus {
    Online,
    Offline,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeStatusSnapshot {
    pub node_id: String,
    pub online_status: OnlineStatus,
    pub status_reason: Option<String>,
    pub aggregated_at: DateTime<Utc>,
}

impl NodeStatusSnapshot {
    pub fn new(
        node_id: String,
        online_status: OnlineStatus,
        status_reason: Option<String>,
    ) -> Self {
        Self {
            node_id,
            online_status,
            status_reason,
            aggregated_at: Utc::now(),
        }
    }
}
