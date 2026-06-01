use chrono::Utc;
use chrono::{DateTime, TimeDelta};
use query_engine::{HeartbeatStore, QueryEngine};

use crate::{
    AgentRegistration, AgentRunMode, AgentSyncRequest, AgentSyncResponse, BindingState, CreateNode,
    HeartbeatConfig, InstallNodeRequest, InstallNodeResult, JobManageConfig, Node,
    NodeAgentBinding, NodeManageError, NodeRepository, NodeStatus, NodeStatusSnapshot,
    OnlineStatus, PaginatedResult, PaginationParams, Result, RsAgentInstaller, SyncBindingState,
    TaskFilterDefaults, UpdateNode,
};

const DEFAULT_CONFIG_VERSION: &str = "2026-05-29T10:00:00Z";
const DEFAULT_HEARTBEAT_CONFIG_VERSION: &str = "1";
const DEFAULT_HEARTBEAT_DATA_LINK_ID: &str = "dl_heartbeat_001";
const DEFAULT_HEARTBEAT_VM_BASE_URL: &str = "http://victoriametrics:8428";
const DEFAULT_HEARTBEAT_INTERVAL_SECS: u64 = 60;
const DEFAULT_JOB_MANAGE_CONFIG_VERSION: &str = "1";
const DEFAULT_JOB_MANAGE_BASE_URL: &str = "http://job-manage:3000/api/job-manage/v1/tasks";
const DEFAULT_SYNC_INTERVAL_SECS: u64 = 30;
const DEFAULT_TASK_SYNC_INTERVAL_SECS: u64 = 10;

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

    pub async fn aggregate_status_snapshot_from_query<D, H>(
        &self,
        id: &str,
        query_engine: &QueryEngine<D, H>,
        heartbeat_data_link_id: &str,
        now: DateTime<Utc>,
        status_window: TimeDelta,
    ) -> Result<NodeStatusSnapshot>
    where
        D: datalink_engine::DataLinkRepository,
        H: HeartbeatStore,
    {
        let node = self
            .repository
            .get(id)
            .await?
            .ok_or_else(|| NodeManageError::NotFound(id.to_string()))?;

        let (online_status, status_reason) = match query_engine
            .latest_heartbeat_by_data_link_id(heartbeat_data_link_id, node.id.clone())
        {
            Ok(Some(sample)) if now - sample.observed_at <= status_window => {
                (OnlineStatus::Online, None)
            }
            Ok(Some(sample)) => (
                OnlineStatus::Offline,
                Some(format!(
                    "heartbeat expired at {}",
                    sample.observed_at.to_rfc3339()
                )),
            ),
            Ok(None) => (
                OnlineStatus::Offline,
                Some("heartbeat missing for node".to_string()),
            ),
            Err(err) => (
                OnlineStatus::Unknown,
                Some(format!("heartbeat query failed: {err}")),
            ),
        };

        Ok(NodeStatusSnapshot::new(
            node.id,
            online_status,
            status_reason,
        ))
    }

    pub async fn install_node(&self, request: InstallNodeRequest) -> Result<InstallNodeResult> {
        self.installer.install(request).await
    }

    pub async fn sync_agent(&self, request: AgentSyncRequest) -> Result<AgentSyncResponse> {
        let existing_binding = self
            .repository
            .agent_binding_by_agent_id(&request.agent_id)
            .await?;
        let bound_node_id = request.node_id.clone().or_else(|| {
            existing_binding
                .as_ref()
                .map(|binding| binding.node_id.clone())
        });

        let Some(bound_node_id) = bound_node_id else {
            return Ok(self.rejected_sync_response(
                request.agent_id,
                String::new(),
                SyncBindingState::Unbound,
                "node_id is required for initial sync".to_string(),
            ));
        };

        let now = Utc::now();

        if let Some(existing_binding) = existing_binding
            && existing_binding.node_id != bound_node_id
        {
            return Ok(self.rejected_sync_response(
                request.agent_id,
                existing_binding.node_id.clone(),
                SyncBindingState::Conflict,
                format!(
                    "agent {} is already bound to node {}",
                    existing_binding.agent_id, existing_binding.node_id
                ),
            ));
        }

        if let Some(conflict_binding) = self
            .repository
            .bound_agent_binding_by_node_id(&bound_node_id)
            .await?
            .filter(|binding| binding.agent_id != request.agent_id)
        {
            return Ok(self.rejected_sync_response(
                request.agent_id,
                bound_node_id,
                SyncBindingState::Conflict,
                format!(
                    "node {} is already bound to agent {}",
                    conflict_binding.node_id, conflict_binding.agent_id
                ),
            ));
        }

        let binding = match self
            .repository
            .agent_binding_by_agent_id(&request.agent_id)
            .await?
        {
            Some(mut binding) => {
                binding.node_id = bound_node_id.clone();
                binding.binding_state = BindingState::Bound;
                binding.last_handshake_at = now;
                binding.unbind_reason = None;
                self.repository.upsert_agent_binding(binding).await?
            }
            None => {
                let mut binding =
                    NodeAgentBinding::new(bound_node_id.clone(), request.agent_id.clone());
                binding.first_registered_at = now;
                binding.last_handshake_at = now;
                self.repository.upsert_agent_binding(binding).await?
            }
        };

        Ok(self.accepted_sync_response(binding))
    }

    pub async fn agent_binding(&self, agent_id: &str) -> Option<NodeAgentBinding> {
        self.repository
            .agent_binding_by_agent_id(agent_id)
            .await
            .ok()
            .flatten()
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

    fn accepted_sync_response(&self, binding: NodeAgentBinding) -> AgentSyncResponse {
        AgentSyncResponse {
            accepted: true,
            agent_id: binding.agent_id,
            bound_node_id: binding.node_id,
            binding_state: SyncBindingState::Bound,
            agent_run_mode: AgentRunMode::Active,
            config_version: DEFAULT_CONFIG_VERSION.to_string(),
            heartbeat_config: HeartbeatConfig {
                version: DEFAULT_HEARTBEAT_CONFIG_VERSION.to_string(),
                data_link_id: DEFAULT_HEARTBEAT_DATA_LINK_ID.to_string(),
                vm_base_url: DEFAULT_HEARTBEAT_VM_BASE_URL.to_string(),
                interval_secs: DEFAULT_HEARTBEAT_INTERVAL_SECS,
            },
            job_manage_config: JobManageConfig {
                version: DEFAULT_JOB_MANAGE_CONFIG_VERSION.to_string(),
                base_url: DEFAULT_JOB_MANAGE_BASE_URL.to_string(),
                task_filter_defaults: TaskFilterDefaults {
                    states: vec![
                        "queued".to_string(),
                        "acknowledged".to_string(),
                        "running".to_string(),
                    ],
                },
            },
            sync_interval_secs: DEFAULT_SYNC_INTERVAL_SECS,
            task_sync_interval_secs: DEFAULT_TASK_SYNC_INTERVAL_SECS,
            rejection_reason: None,
        }
    }

    fn rejected_sync_response(
        &self,
        agent_id: String,
        bound_node_id: String,
        binding_state: SyncBindingState,
        rejection_reason: String,
    ) -> AgentSyncResponse {
        AgentSyncResponse {
            accepted: false,
            agent_id,
            bound_node_id,
            binding_state,
            agent_run_mode: AgentRunMode::Idle,
            config_version: DEFAULT_CONFIG_VERSION.to_string(),
            heartbeat_config: HeartbeatConfig {
                version: DEFAULT_HEARTBEAT_CONFIG_VERSION.to_string(),
                data_link_id: DEFAULT_HEARTBEAT_DATA_LINK_ID.to_string(),
                vm_base_url: DEFAULT_HEARTBEAT_VM_BASE_URL.to_string(),
                interval_secs: DEFAULT_HEARTBEAT_INTERVAL_SECS,
            },
            job_manage_config: JobManageConfig {
                version: DEFAULT_JOB_MANAGE_CONFIG_VERSION.to_string(),
                base_url: DEFAULT_JOB_MANAGE_BASE_URL.to_string(),
                task_filter_defaults: TaskFilterDefaults {
                    states: vec![
                        "queued".to_string(),
                        "acknowledged".to_string(),
                        "running".to_string(),
                    ],
                },
            },
            sync_interval_secs: DEFAULT_SYNC_INTERVAL_SECS,
            task_sync_interval_secs: DEFAULT_TASK_SYNC_INTERVAL_SECS,
            rejection_reason: Some(rejection_reason),
        }
    }
}
