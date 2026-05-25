use async_trait::async_trait;
use chrono::Utc;
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

#[async_trait]
pub trait AgentRegistry: Clone + Send + Sync + 'static {
    async fn register(&self, registration: AgentRegistration) -> Result<Node>;
}
