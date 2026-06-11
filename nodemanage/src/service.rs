use chrono::Utc;
use chrono::{DateTime, TimeDelta};
use query_engine::{HeartbeatStore, QueryEngine};

use crate::{
    AgentRegistration, AgentRunMode, AgentSyncRequest, AgentSyncResponse, BindingState, CreateNode,
    HeartbeatConfig, HeartbeatRef, InstallConfigDefaults, InstallNodeRequest, InstallNodeResult,
    InstallRequestSummary, InstallTaskState, InstallTaskStep, JobManageConfig, Node,
    NodeAgentBinding, NodeBindingView, NodeDetail, NodeInstallTask, NodeInstallTaskReceipt,
    NodeInstallTaskView, NodeManageError, NodeRepository, NodeStatus, NodeStatusBatchItem,
    NodeStatusSnapshot, NodeStatusView, NodeSummary, OnlineStatus, PaginatedResult,
    PaginationParams, RebindNodeRequest, RebindNodeResponse, ResolvedInstallRequest, Result,
    RsAgentInstaller, SyncBindingState, TaskFilterDefaults, UpdateNode,
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
    config_defaults: InstallConfigDefaults,
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
            config_defaults: InstallConfigDefaults {
                rsagent_package_url: None,
                install_root: "/opt/rsagent".to_string(),
                register_callback_url: "http://127.0.0.1:3000/api/nm/v1/agents/sync".to_string(),
                plugins: vec![],
            },
        }
    }

    pub fn with_config_defaults(mut self, config_defaults: InstallConfigDefaults) -> Self {
        self.config_defaults = config_defaults;
        self
    }

    pub fn config_defaults(&self) -> &InstallConfigDefaults {
        &self.config_defaults
    }

    pub async fn create(&self, input: CreateNode) -> Result<Node> {
        let name = input.name.trim();
        let endpoint = input.endpoint.trim();

        let mut invalid_fields = Vec::new();
        if name.is_empty() {
            invalid_fields.push("name");
        }
        if endpoint.is_empty() {
            invalid_fields.push("endpoint");
        }

        if !invalid_fields.is_empty() {
            return Err(NodeManageError::InvalidInput(format!(
                "{} is required",
                invalid_fields.join(", ")
            )));
        }

        let existing_nodes = self
            .repository
            .list(PaginationParams::new(1, u32::MAX))
            .await?;
        if existing_nodes
            .items
            .iter()
            .any(|node| node.endpoint == endpoint)
        {
            return Err(NodeManageError::Conflict(format!(
                "endpoint already exists: {endpoint}"
            )));
        }

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
        if let Some(environment) = input.environment {
            node.environment = Some(environment);
        }
        if let Some(ssh_port) = input.ssh_port {
            node.ssh_port = Some(ssh_port);
        }
        if let Some(ssh_username) = input.ssh_username {
            node.ssh_username = Some(ssh_username);
        }
        if let Some(ssh_password) = input.ssh_password {
            node.ssh_password = Some(ssh_password);
        }
        if let Some(ssh_private_key) = input.ssh_private_key {
            node.ssh_private_key = Some(ssh_private_key);
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
                environment: None,
                ssh_port: None,
                ssh_username: None,
                ssh_password: None,
                ssh_private_key: None,
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

    pub async fn install_node(&self, request: ResolvedInstallRequest) -> Result<InstallNodeResult> {
        self.installer.install(request).await
    }

    pub async fn submit_install_task(
        &self,
        node_id: &str,
        request: InstallNodeRequest,
    ) -> Result<NodeInstallTaskReceipt> {
        let node = self
            .repository
            .get(node_id)
            .await?
            .ok_or_else(|| NodeManageError::NotFound(node_id.to_string()))?;

        // Resolve: request > node > config defaults
        let resolved = request.resolve(&node, &self.config_defaults)?;

        // Create task record
        let task = self
            .repository
            .create_install_task(
                NodeInstallTask::new(node_id.to_string()).with_install_request(&resolved),
            )
            .await?;

        let receipt = NodeInstallTaskReceipt {
            install_task_id: task.install_task_id.clone(),
            node_id: task.node_id.clone(),
            accepted: true,
            task_state: "pending".to_string(),
        };

        // Spawn background installation
        let installer = self.installer.clone();
        let repository = self.repository.clone();
        let task_id = task.install_task_id.clone();
        let node_id_owned = node_id.to_string();

        tokio::spawn(async move {
            Self::execute_install(installer, repository, task_id, node_id_owned, resolved).await;
        });

        Ok(receipt)
    }

    async fn execute_install(
        installer: I,
        repository: R,
        task_id: String,
        node_id: String,
        resolved: ResolvedInstallRequest,
    ) {
        // Transition to running
        let mut task = match repository.get_install_task(&task_id).await {
            Ok(Some(t)) => t,
            Ok(None) | Err(_) => return,
        };
        task.task_state = InstallTaskState::Running;
        task.current_step = Some(InstallTaskStep::PrepareInstall);
        let _ = repository.update_install_task(task).await;

        // Execute installation
        let result = installer.install(resolved).await;

        let mut task = match repository.get_install_task(&task_id).await {
            Ok(Some(t)) => t,
            Ok(None) | Err(_) => return,
        };

        match result {
            Ok(install_result) => {
                if install_result.status == crate::InstallStatus::Registered {
                    task.task_state = InstallTaskState::Succeeded;
                    task.current_step = Some(InstallTaskStep::WaitRegister);
                    task.retryable = false;
                } else {
                    task.task_state = InstallTaskState::Failed;
                    task.current_step = Some(InstallTaskStep::WaitRegister);
                    task.error_message = install_result.message;
                    task.retryable = true;
                }
            }
            Err(err) => {
                task.task_state = InstallTaskState::Failed;
                task.error_message = Some(err.to_string());
                task.retryable = true;
            }
        }

        task.finished_at = Some(Utc::now());
        let _ = repository.update_install_task(task).await;

        // Update node status on success
        if let Ok(Some(task)) = repository.get_install_task(&task_id).await {
            if task.task_state == InstallTaskState::Succeeded {
                if let Ok(Some(mut node)) = repository.get(&node_id).await {
                    node.updated_at = Utc::now();
                    let _ = repository.update(node).await;
                }
            }
        }
    }

    pub async fn rebind_node(
        &self,
        node_id: &str,
        request: &RebindNodeRequest,
    ) -> Result<RebindNodeResponse> {
        self.repository
            .get(node_id)
            .await?
            .ok_or_else(|| NodeManageError::NotFound(node_id.to_string()))?;

        let target_agent_id = request.target_agent_id.trim();
        if target_agent_id.is_empty() {
            return Err(NodeManageError::InvalidRebindRequest(
                "target_agent_id is required".to_string(),
            ));
        }

        let now = Utc::now();
        let current_binding = self
            .repository
            .bound_agent_binding_by_node_id(node_id)
            .await?;
        let mut target_binding = self
            .repository
            .agent_binding_by_agent_id(target_agent_id)
            .await?
            .ok_or_else(|| NodeManageError::TargetAgentNotFound(target_agent_id.to_string()))?;

        if target_binding.binding_state == BindingState::Bound && target_binding.node_id != node_id
        {
            return Err(NodeManageError::RebindTargetAlreadyBound(format!(
                "agent {} is already bound to node {}",
                target_binding.agent_id, target_binding.node_id
            )));
        }

        let previous_agent_id = current_binding
            .as_ref()
            .filter(|binding| binding.agent_id != target_binding.agent_id)
            .map(|binding| binding.agent_id.clone());

        if let Some(mut current_binding) = current_binding
            && current_binding.agent_id != target_binding.agent_id
        {
            current_binding.binding_state = BindingState::Stale;
            current_binding.last_handshake_at = now;
            current_binding.unbind_reason = request.reason.clone();
            self.repository
                .upsert_agent_binding(current_binding)
                .await?;
        }

        target_binding.node_id = node_id.to_string();
        target_binding.binding_state = BindingState::Bound;
        target_binding.last_handshake_at = now;
        target_binding.unbind_reason = None;
        let target_binding = self.repository.upsert_agent_binding(target_binding).await?;

        Ok(RebindNodeResponse {
            accepted: true,
            node_id: node_id.to_string(),
            target_agent_id: target_binding.agent_id.clone(),
            binding_state: Self::binding_state_label(Some(&target_binding)).to_string(),
            previous_agent_id,
        })
    }

    pub async fn install_task(&self, install_task_id: &str) -> Result<Option<NodeInstallTaskView>> {
        self.repository
            .get_install_task(install_task_id)
            .await
            .map(|task| task.as_ref().map(|task| self.build_install_task_view(task)))
    }

    pub async fn latest_install_task(&self, node_id: &str) -> Result<Option<NodeInstallTaskView>> {
        self.repository
            .latest_install_task_by_node_id(node_id)
            .await
            .map(|task| task.as_ref().map(|task| self.build_install_task_view(task)))
    }

    pub async fn list_node_summaries(
        &self,
        pagination: PaginationParams,
    ) -> Result<PaginatedResult<NodeSummary>> {
        let nodes = self.repository.list(pagination).await?;
        let mut items = Vec::with_capacity(nodes.items.len());
        for node in &nodes.items {
            items.push(self.build_node_summary(node).await?);
        }

        Ok(PaginatedResult {
            items,
            total: nodes.total,
            page: nodes.page,
            page_size: nodes.page_size,
            total_pages: nodes.total_pages,
        })
    }

    pub async fn node_detail(
        &self,
        node_id: &str,
        heartbeat_data_link_id: Option<String>,
        result_table_name: Option<String>,
    ) -> Result<NodeDetail> {
        let node = self
            .repository
            .get(node_id)
            .await?
            .ok_or_else(|| NodeManageError::NotFound(node_id.to_string()))?;
        let binding = self
            .repository
            .bound_agent_binding_by_node_id(node_id)
            .await?;
        let latest_install_task = self
            .repository
            .latest_install_task_by_node_id(node_id)
            .await?;

        Ok(NodeDetail {
            node: self.build_node_base_info(&node),
            binding: binding.as_ref().map(Self::build_binding_view),
            status: self.build_status_view(&node, latest_install_task.as_ref(), binding.as_ref()),
            latest_install_task: latest_install_task
                .as_ref()
                .map(|task| self.build_install_task_view(task)),
            heartbeat_ref: heartbeat_data_link_id.map(|data_link_id| HeartbeatRef {
                data_link_id,
                link_purpose: "node_heartbeat".to_string(),
                owner_service: "nodemanage".to_string(),
                result_table_name,
            }),
        })
    }

    pub async fn batch_status(&self, node_ids: Vec<String>) -> Result<Vec<NodeStatusBatchItem>> {
        let mut items = Vec::new();
        for node_id in node_ids {
            if let Some(node) = self.repository.get(&node_id).await? {
                let binding = self
                    .repository
                    .bound_agent_binding_by_node_id(&node.id)
                    .await?;
                let latest_install_task = self
                    .repository
                    .latest_install_task_by_node_id(&node.id)
                    .await?;
                let status_view =
                    self.build_status_view(&node, latest_install_task.as_ref(), binding.as_ref());
                items.push(NodeStatusBatchItem {
                    node_id: node.id,
                    install_phase: status_view.install_phase,
                    binding_state: status_view.binding_state,
                    online_status: status_view.online_status,
                    last_heartbeat_at: status_view.last_heartbeat_at,
                    updated_at: node.updated_at,
                    status_reason: status_view.status_reason,
                });
            }
        }
        Ok(items)
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
                        "dispatched".to_string(),
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
                        "dispatched".to_string(),
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

    async fn build_node_summary(&self, node: &Node) -> Result<NodeSummary> {
        let binding = self
            .repository
            .bound_agent_binding_by_node_id(&node.id)
            .await?;
        let latest_install_task = self
            .repository
            .latest_install_task_by_node_id(&node.id)
            .await?;
        let status = self.build_status_view(node, latest_install_task.as_ref(), binding.as_ref());
        Ok(NodeSummary {
            node_id: node.id.clone(),
            node_name: node.name.clone(),
            environment: Self::derive_environment(node),
            labels: node.labels.clone(),
            lifecycle_state: status.lifecycle_state,
            install_phase: status.install_phase,
            binding_state: status.binding_state,
            online_status: status.online_status,
            last_heartbeat_at: status.last_heartbeat_at,
            updated_at: node.updated_at,
        })
    }

    fn build_node_base_info(&self, node: &Node) -> crate::protocol::NodeBaseInfo {
        crate::protocol::NodeBaseInfo {
            node_id: node.id.clone(),
            node_name: node.name.clone(),
            endpoint: node.endpoint.clone(),
            environment: Self::derive_environment(node),
            labels: node.labels.clone(),
            created_at: node.created_at,
            updated_at: node.updated_at,
        }
    }

    fn build_binding_view(binding: &NodeAgentBinding) -> NodeBindingView {
        NodeBindingView {
            node_id: binding.node_id.clone(),
            agent_id: binding.agent_id.clone(),
            binding_state: Self::binding_state_label(Some(binding)).to_string(),
            first_registered_at: binding.first_registered_at,
            last_handshake_at: binding.last_handshake_at,
        }
    }

    fn build_status_view(
        &self,
        node: &Node,
        latest_install_task: Option<&NodeInstallTask>,
        binding: Option<&NodeAgentBinding>,
    ) -> NodeStatusView {
        NodeStatusView {
            lifecycle_state: Self::derive_lifecycle_state(latest_install_task, binding),
            install_phase: Self::install_phase_label(latest_install_task).to_string(),
            binding_state: Self::binding_state_label(binding).to_string(),
            online_status: Self::online_status_label(node.status.clone()).to_string(),
            last_heartbeat_at: node.last_heartbeat_at,
            status_reason: None,
        }
    }

    fn build_install_task_view(&self, task: &NodeInstallTask) -> NodeInstallTaskView {
        NodeInstallTaskView {
            install_task_id: task.install_task_id.clone(),
            node_id: task.node_id.clone(),
            task_state: Self::install_task_state_label(&task.task_state).to_string(),
            current_step: task
                .current_step
                .as_ref()
                .map(|step| Self::install_task_step_label(step).to_string()),
            error_code: task.error_code.clone(),
            error_message: task.error_message.clone(),
            started_at: task.started_at,
            finished_at: task.finished_at,
            retryable: task.retryable,
            request_summary: self.build_install_request_summary(task),
        }
    }

    fn build_install_request_summary(
        &self,
        task: &NodeInstallTask,
    ) -> Option<InstallRequestSummary> {
        if task.request_host.is_none()
            && task.request_ssh_port.is_none()
            && task.request_username.is_none()
            && task.request_rsagent_package_url.is_none()
            && task.request_install_root.is_none()
            && task.request_labels.is_empty()
            && task.request_plugin_names.is_empty()
        {
            return None;
        }

        Some(InstallRequestSummary {
            host: task.request_host.clone(),
            ssh_port: task.request_ssh_port,
            username: task.request_username.clone(),
            rsagent_package_url: task.request_rsagent_package_url.clone(),
            install_root: task.request_install_root.clone(),
            labels: task.request_labels.clone(),
            plugin_names: task.request_plugin_names.clone(),
        })
    }

    fn derive_environment(node: &Node) -> String {
        if let Some(env) = &node.environment {
            return env.clone();
        }
        node.labels
            .iter()
            .find_map(|label| {
                label
                    .strip_prefix("environment:")
                    .or_else(|| label.strip_prefix("env:"))
            })
            .unwrap_or("unknown")
            .to_string()
    }

    fn derive_lifecycle_state(
        latest_install_task: Option<&NodeInstallTask>,
        binding: Option<&NodeAgentBinding>,
    ) -> String {
        match (latest_install_task, binding) {
            (Some(task), Some(binding))
                if task.task_state == crate::InstallTaskState::Succeeded
                    && binding.binding_state == BindingState::Bound =>
            {
                "managed".to_string()
            }
            (Some(_), _) => "onboarding".to_string(),
            _ => "draft".to_string(),
        }
    }

    fn install_phase_label(latest_install_task: Option<&NodeInstallTask>) -> &'static str {
        match latest_install_task.map(|task| &task.task_state) {
            Some(crate::InstallTaskState::Pending) => "pending",
            Some(crate::InstallTaskState::Running) => "running",
            Some(crate::InstallTaskState::WaitingRegister) => "waiting_register",
            Some(crate::InstallTaskState::Succeeded) => "succeeded",
            Some(crate::InstallTaskState::Failed) => "failed",
            Some(crate::InstallTaskState::Cancelled) => "cancelled",
            None => "not_started",
        }
    }

    fn install_task_state_label(task_state: &crate::InstallTaskState) -> &'static str {
        match task_state {
            crate::InstallTaskState::Pending => "pending",
            crate::InstallTaskState::Running => "running",
            crate::InstallTaskState::WaitingRegister => "waiting_register",
            crate::InstallTaskState::Succeeded => "succeeded",
            crate::InstallTaskState::Failed => "failed",
            crate::InstallTaskState::Cancelled => "cancelled",
        }
    }

    fn install_task_step_label(step: &crate::InstallTaskStep) -> &'static str {
        match step {
            crate::InstallTaskStep::PrepareInstall => "prepare_install",
            crate::InstallTaskStep::ResolveArtifacts => "resolve_artifacts",
            crate::InstallTaskStep::WriteRuntimeConfig => "write_runtime_config",
            crate::InstallTaskStep::UploadPackage => "upload_package",
            crate::InstallTaskStep::RunInstallScript => "run_install_script",
            crate::InstallTaskStep::StartAgent => "start_agent",
            crate::InstallTaskStep::WaitRegister => "wait_register",
        }
    }

    fn binding_state_label(binding: Option<&NodeAgentBinding>) -> &'static str {
        match binding.map(|binding| &binding.binding_state) {
            Some(BindingState::Bound) => "bound",
            Some(BindingState::Stale) => "stale",
            Some(BindingState::Unbound) => "unbound",
            None => "unbound",
        }
    }

    fn online_status_label(status: NodeStatus) -> &'static str {
        match status {
            NodeStatus::Online => "online",
            NodeStatus::Offline => "offline",
            NodeStatus::Maintenance => "unknown",
        }
    }
}
