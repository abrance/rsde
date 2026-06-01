use chrono::{DateTime, Utc};
use nodemanage::{
    AgentRunMode, AgentSyncResponse, HeartbeatConfig, JobManageConfig, SyncBindingState,
};

use crate::config::AgentRuntimeConfig;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentIdentity {
    pub agent_version: String,
    pub hostname: String,
    pub os_family: String,
    pub os_distribution: String,
    pub arch: String,
    pub capabilities: Vec<String>,
    pub started_at: DateTime<Utc>,
}

impl AgentIdentity {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        agent_version: String,
        hostname: String,
        os_family: String,
        os_distribution: String,
        arch: String,
        capabilities: Vec<String>,
        started_at: DateTime<Utc>,
    ) -> Self {
        Self {
            agent_version,
            hostname,
            os_family,
            os_distribution,
            arch,
            capabilities,
            started_at,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectiveRuntimeConfig {
    pub config_version: String,
    pub heartbeat_config: HeartbeatConfig,
    pub job_manage_config: JobManageConfig,
    pub sync_interval_secs: u64,
    pub task_sync_interval_secs: u64,
}

impl From<&AgentSyncResponse> for EffectiveRuntimeConfig {
    fn from(value: &AgentSyncResponse) -> Self {
        Self {
            config_version: value.config_version.clone(),
            heartbeat_config: value.heartbeat_config.clone(),
            job_manage_config: value.job_manage_config.clone(),
            sync_interval_secs: value.sync_interval_secs,
            task_sync_interval_secs: value.task_sync_interval_secs,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentRuntimeState {
    base_config: AgentRuntimeConfig,
    local_node_id: Option<String>,
    config_version: Option<String>,
    binding_state: Option<SyncBindingState>,
    loops_enabled: bool,
    process_alive: bool,
    degraded: bool,
    effective_config: Option<EffectiveRuntimeConfig>,
    last_sync_error: Option<String>,
}

impl AgentRuntimeState {
    pub fn new(config: AgentRuntimeConfig) -> Self {
        Self {
            local_node_id: config.node_id.clone(),
            base_config: config,
            config_version: None,
            binding_state: None,
            loops_enabled: false,
            process_alive: true,
            degraded: false,
            effective_config: None,
            last_sync_error: None,
        }
    }

    pub fn apply_sync_response(&mut self, response: AgentSyncResponse) {
        self.binding_state = Some(response.binding_state.clone());
        self.last_sync_error = None;
        self.degraded = false;

        if response.accepted {
            self.local_node_id = Some(response.bound_node_id.clone());
            self.base_config.node_id = self.local_node_id.clone();
            self.config_version = Some(response.config_version.clone());
            self.effective_config = Some((&response).into());
            self.loops_enabled = matches!(response.agent_run_mode, AgentRunMode::Active);
            return;
        }

        self.loops_enabled = false;
    }

    pub fn record_temporary_sync_failure(&mut self, error: String) {
        self.last_sync_error = Some(error);
        let has_last_good_config = self.effective_config.is_some();
        if !has_last_good_config {
            self.loops_enabled = false;
        }
        self.degraded = has_last_good_config;
    }

    pub fn local_node_id(&self) -> Option<&str> {
        self.local_node_id.as_deref()
    }

    pub fn config_version(&self) -> Option<&str> {
        self.config_version.as_deref()
    }

    pub fn binding_state(&self) -> Option<&SyncBindingState> {
        self.binding_state.as_ref()
    }

    pub fn loops_enabled(&self) -> bool {
        self.loops_enabled
    }

    pub fn process_alive(&self) -> bool {
        self.process_alive
    }

    pub fn is_degraded(&self) -> bool {
        self.degraded
    }

    pub fn effective_config(&self) -> Option<&EffectiveRuntimeConfig> {
        self.effective_config.as_ref()
    }

    pub fn sync_client(&self) -> crate::clients::nodemanage::NodeManageSyncClient {
        crate::clients::nodemanage::NodeManageSyncClient::new(
            self.base_config.nodemanage_sync_url.clone(),
        )
    }
}
