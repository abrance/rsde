use nodemanage::NodeStatus;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodePrecheck {
    pub node_id: String,
    pub allowed: bool,
    pub status: NodeStatus,
    pub reason: String,
}
