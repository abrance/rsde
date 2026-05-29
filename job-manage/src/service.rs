use nodemanage::{NodeManageError, NodeManager, NodeRepository, RsAgentInstaller};

use crate::{JobManageError, NodePrecheck, Result};

#[derive(Debug, Clone)]
pub struct PrecheckService<R, I>
where
    R: NodeRepository,
    I: RsAgentInstaller,
{
    node_manager: NodeManager<R, I>,
}

impl<R, I> PrecheckService<R, I>
where
    R: NodeRepository,
    I: RsAgentInstaller,
{
    pub fn new(node_manager: NodeManager<R, I>) -> Self {
        Self { node_manager }
    }

    pub async fn precheck(&self, node_id: &str) -> Result<NodePrecheck> {
        let node = self
            .node_manager
            .get(node_id)
            .await
            .map_err(map_node_error)?
            .ok_or_else(|| JobManageError::NodeNotFound(node_id.to_string()))?;

        let (allowed, reason) = match node.status {
            nodemanage::NodeStatus::Online => (true, "node is online".to_string()),
            nodemanage::NodeStatus::Offline => (false, "node is offline".to_string()),
            nodemanage::NodeStatus::Maintenance => (false, "node is in maintenance".to_string()),
        };

        Ok(NodePrecheck {
            node_id: node.id,
            allowed,
            status: node.status,
            reason,
        })
    }
}

fn map_node_error(err: NodeManageError) -> JobManageError {
    match err {
        NodeManageError::NotFound(node_id) => JobManageError::NodeNotFound(node_id),
        NodeManageError::InvalidInput(message) | NodeManageError::Storage(message) => {
            JobManageError::NodeNotFound(message)
        }
    }
}
