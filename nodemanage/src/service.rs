use chrono::Utc;
use chrono::{DateTime, TimeDelta};
use query_engine::{HeartbeatStore, QueryEngine};

use crate::{
    AgentRegistration, CreateNode, InstallNodeRequest, InstallNodeResult, Node, NodeManageError,
    NodeRepository, NodeStatus, PaginatedResult, PaginationParams, Result, RsAgentInstaller,
    UpdateNode,
};

#[derive(Debug, Clone)]
pub struct NodeManager<R, I>
where
    R: NodeRepository,
    I: RsAgentInstaller,
{
    repository: R,
    installer: I,
}

impl<R, I> NodeManager<R, I>
where
    R: NodeRepository,
    I: RsAgentInstaller,
{
    pub fn new(repository: R, installer: I) -> Self {
        Self {
            repository,
            installer,
        }
    }

    pub async fn create(&self, input: CreateNode) -> Result<Node> {
        self.repository.create(input.into_node()).await
    }

    pub async fn get(&self, id: &str) -> Result<Option<Node>> {
        self.repository.get(id).await
    }

    pub async fn list(&self, pagination: PaginationParams) -> Result<PaginatedResult<Node>> {
        self.repository.list(pagination).await
    }

    pub async fn update(&self, id: &str, input: UpdateNode) -> Result<Node> {
        let mut node = self
            .repository
            .get(id)
            .await?
            .ok_or_else(|| NodeManageError::NotFound(id.to_string()))?;

        if let Some(name) = input.name {
            node.name = name;
        }
        if let Some(endpoint) = input.endpoint {
            node.endpoint = endpoint;
        }
        if let Some(status) = input.status {
            node.status = status;
        }
        if let Some(labels) = input.labels {
            node.labels = labels;
        }
        node.updated_at = Utc::now();

        self.repository.update(node).await
    }

    pub async fn delete(&self, id: &str) -> Result<bool> {
        self.repository.delete(id).await
    }

    pub async fn heartbeat(&self, id: &str) -> Result<Node> {
        let now = Utc::now();
        let mut node = self
            .repository
            .get(id)
            .await?
            .ok_or_else(|| NodeManageError::NotFound(id.to_string()))?;
        node.status = NodeStatus::Online;
        node.updated_at = now;
        node.last_heartbeat_at = Some(now);
        self.repository.update(node).await
    }

    pub async fn update_status(&self, id: &str, status: NodeStatus) -> Result<Node> {
        self.update(
            id,
            UpdateNode {
                name: None,
                endpoint: None,
                status: Some(status),
                labels: None,
            },
        )
        .await
    }

    pub async fn refresh_status_from_query<D, H>(
        &self,
        id: &str,
        query_engine: &QueryEngine<D, H>,
        heartbeat_data_link_id: &str,
        now: DateTime<Utc>,
        status_window: TimeDelta,
    ) -> Result<Node>
    where
        D: datalink_engine::DataLinkRepository,
        H: HeartbeatStore,
    {
        let mut node = self
            .repository
            .get(id)
            .await?
            .ok_or_else(|| NodeManageError::NotFound(id.to_string()))?;

        let sample = query_engine
            .latest_heartbeat_by_data_link_id(heartbeat_data_link_id, node.id.clone())
            .map_err(|err| NodeManageError::Storage(err.to_string()))?;

        match sample {
            Some(sample) if now - sample.observed_at <= status_window => {
                node.status = NodeStatus::Online;
                node.last_heartbeat_at = Some(sample.observed_at);
            }
            Some(sample) => {
                node.status = NodeStatus::Offline;
                node.last_heartbeat_at = Some(sample.observed_at);
            }
            None => {
                node.status = NodeStatus::Offline;
                node.last_heartbeat_at = None;
            }
        }

        node.updated_at = now;
        self.repository.update(node).await
    }

    pub async fn install_node(&self, request: InstallNodeRequest) -> Result<InstallNodeResult> {
        self.installer.install(request).await
    }

    pub async fn register_agent(&self, registration: AgentRegistration) -> Result<Node> {
        let mut node = registration.into_node();

        if let Some(mut existing) = self.repository.get(&node.id).await? {
            existing.name = node.name;
            existing.endpoint = node.endpoint;
            existing.status = node.status;
            existing.labels = node.labels;
            existing.updated_at = Utc::now();
            existing.last_heartbeat_at = node.last_heartbeat_at.take();
            self.repository.update(existing).await
        } else {
            self.repository.create(node).await
        }
    }
}
