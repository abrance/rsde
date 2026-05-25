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
