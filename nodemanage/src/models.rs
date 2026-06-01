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
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CreateNode {
    pub name: String,
    pub endpoint: String,
    #[serde(default)]
    pub labels: Vec<String>,
}

impl CreateNode {
    pub fn into_node(self) -> Node {
        Node::new(self.name, self.endpoint, self.labels)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct UpdateNode {
    pub name: Option<String>,
    pub endpoint: Option<String>,
    pub status: Option<NodeStatus>,
    pub labels: Option<Vec<String>>,
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
