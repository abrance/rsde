use chrono::{DateTime, Utc};
use nodemanage::{
    AgentRunMode, AgentSyncRequest, AgentSyncResponse, HeartbeatConfig, JobManageConfig,
    SyncBindingState,
};
use serde::{Deserialize, Serialize};

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SubordinateLoopMode {
    Active,
    Limited,
    Withheld,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DurableRuntimeSnapshot {
    pub local_node_id: Option<String>,
    pub latest_seen_config_version: Option<String>,
    pub accepted_config_version: Option<String>,
    pub binding_state: Option<SyncBindingState>,
    pub ownership_confirmed: bool,
    pub subordinate_loop_mode: SubordinateLoopMode,
    pub loops_enabled: bool,
    pub effective_config: Option<EffectiveRuntimeConfig>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentRuntimeState {
    base_config: AgentRuntimeConfig,
    local_node_id: Option<String>,
    latest_seen_config_version: Option<String>,
    accepted_config_version: Option<String>,
    binding_state: Option<SyncBindingState>,
    ownership_confirmed: bool,
    subordinate_loop_mode: SubordinateLoopMode,
    loops_enabled: bool,
    process_alive: bool,
    degraded: bool,
    staged_effective_config: Option<EffectiveRuntimeConfig>,
    effective_config: Option<EffectiveRuntimeConfig>,
    last_sync_error: Option<String>,
}

impl AgentRuntimeState {
    pub fn new(config: AgentRuntimeConfig) -> Self {
        Self {
            local_node_id: config.node_id.clone(),
            base_config: config,
            latest_seen_config_version: None,
            accepted_config_version: None,
            binding_state: None,
            ownership_confirmed: false,
            subordinate_loop_mode: SubordinateLoopMode::Withheld,
            loops_enabled: false,
            process_alive: true,
            degraded: false,
            staged_effective_config: None,
            effective_config: None,
            last_sync_error: None,
        }
    }

    pub fn apply_sync_response(&mut self, response: AgentSyncResponse) {
        if response.accepted {
            match Self::validated_runtime_config(&response) {
                Ok(candidate) => {
                    self.stage_validated_sync_response(response, candidate);
                    self.promote_staged_config_to_effective();
                }
                Err(error) => self.record_invalid_accepted_sync_response(response, error),
            }
            return;
        }

        self.record_rejected_sync_response(response);
    }

    pub fn stage_accepted_sync_response(&mut self, response: AgentSyncResponse) {
        let candidate = Self::validated_runtime_config(&response)
            .expect("accepted sync response should be locally valid");
        self.stage_validated_sync_response(response, candidate);
    }

    pub fn promote_staged_config_to_effective(&mut self) {
        let Some(config) = self.staged_effective_config.take() else {
            return;
        };

        self.effective_config = Some(config);
        self.last_sync_error = None;
        self.degraded = false;
        self.loops_enabled = self.effective_config.is_some()
            && matches!(self.subordinate_loop_mode, SubordinateLoopMode::Active);
    }

    pub fn record_temporary_sync_failure(&mut self, error: String) {
        self.last_sync_error = Some(error);
        self.staged_effective_config = None;
        let has_last_good_config = self.effective_config.is_some();
        if !has_last_good_config {
            self.subordinate_loop_mode = SubordinateLoopMode::Withheld;
            self.loops_enabled = false;
        }
        self.degraded = has_last_good_config;
    }

    pub fn record_rejected_or_invalid_config(
        &mut self,
        latest_seen_config_version: Option<String>,
        binding_state: Option<SyncBindingState>,
        error: String,
    ) {
        self.latest_seen_config_version = latest_seen_config_version;
        self.binding_state = binding_state;
        self.staged_effective_config = None;
        self.last_sync_error = Some(error);
        self.degraded = self.effective_config.is_some();
        self.ownership_confirmed = false;
        self.subordinate_loop_mode = SubordinateLoopMode::Withheld;
        self.loops_enabled = false;
    }

    pub fn local_node_id(&self) -> Option<&str> {
        self.local_node_id.as_deref()
    }

    pub fn config_version(&self) -> Option<&str> {
        self.accepted_config_version.as_deref()
    }

    pub fn latest_seen_config_version(&self) -> Option<&str> {
        self.latest_seen_config_version.as_deref()
    }

    pub fn binding_state(&self) -> Option<&SyncBindingState> {
        self.binding_state.as_ref()
    }

    pub fn accepted_config_version(&self) -> Option<&str> {
        self.config_version()
    }

    pub fn subordinate_loop_mode(&self) -> SubordinateLoopMode {
        self.subordinate_loop_mode
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

    pub fn data_dir(&self) -> &str {
        &self.base_config.data_dir
    }

    pub fn last_sync_error(&self) -> Option<&str> {
        self.last_sync_error.as_deref()
    }

    pub fn durable_snapshot(&self) -> Option<DurableRuntimeSnapshot> {
        if self.effective_config.is_none()
            && self.local_node_id.is_none()
            && self.accepted_config_version.is_none()
            && self.binding_state.is_none()
        {
            return None;
        }

        Some(DurableRuntimeSnapshot {
            local_node_id: self.local_node_id.clone(),
            latest_seen_config_version: self.latest_seen_config_version.clone(),
            accepted_config_version: self.accepted_config_version.clone(),
            binding_state: self.binding_state.clone(),
            ownership_confirmed: self.ownership_confirmed,
            subordinate_loop_mode: self.subordinate_loop_mode,
            loops_enabled: self.loops_enabled,
            effective_config: self.effective_config.clone(),
        })
    }

    pub fn restore_durable_snapshot(
        mut config: AgentRuntimeConfig,
        snapshot: DurableRuntimeSnapshot,
    ) -> Self {
        config.node_id = snapshot.local_node_id.clone();

        let loops_enabled = snapshot.loops_enabled && snapshot.effective_config.is_some();

        Self {
            base_config: config,
            local_node_id: snapshot.local_node_id,
            latest_seen_config_version: snapshot.latest_seen_config_version,
            accepted_config_version: snapshot.accepted_config_version,
            binding_state: snapshot.binding_state,
            ownership_confirmed: snapshot.ownership_confirmed,
            subordinate_loop_mode: snapshot.subordinate_loop_mode,
            loops_enabled,
            process_alive: true,
            degraded: false,
            staged_effective_config: None,
            effective_config: snapshot.effective_config,
            last_sync_error: None,
        }
    }

    pub fn sync_client(&self) -> crate::clients::nodemanage::NodeManageSyncClient {
        crate::clients::nodemanage::NodeManageSyncClient::new(
            self.base_config.nodemanage_sync_url.clone(),
        )
    }

    pub fn build_sync_request(&self, identity: &AgentIdentity) -> AgentSyncRequest {
        crate::clients::nodemanage::NodeManageSyncClient::build_request(
            &self.base_config,
            identity,
            self.config_version().map(ToString::to_string),
        )
    }

    pub fn validated_runtime_config(
        response: &AgentSyncResponse,
    ) -> std::result::Result<EffectiveRuntimeConfig, String> {
        let candidate = EffectiveRuntimeConfig::from(response);

        if candidate.heartbeat_config.interval_secs == 0 {
            return Err("heartbeat interval_secs must be greater than zero".to_string());
        }
        if candidate.job_manage_config.base_url.trim().is_empty() {
            return Err("job manage base_url must not be empty".to_string());
        }
        if candidate.sync_interval_secs == 0 {
            return Err("sync interval_secs must be greater than zero".to_string());
        }
        if candidate.task_sync_interval_secs == 0 {
            return Err("task sync interval_secs must be greater than zero".to_string());
        }

        Ok(candidate)
    }

    pub fn stage_validated_sync_response(
        &mut self,
        response: AgentSyncResponse,
        candidate: EffectiveRuntimeConfig,
    ) {
        self.latest_seen_config_version = Some(response.config_version.clone());
        self.binding_state = Some(response.binding_state.clone());
        self.local_node_id = (!response.bound_node_id.is_empty()).then(|| response.bound_node_id.clone());
        self.base_config.node_id = self.local_node_id.clone();
        self.accepted_config_version = Some(response.config_version.clone());
        self.ownership_confirmed = response.binding_state == SyncBindingState::Bound
            && response.agent_id == self.base_config.agent_id
            && !response.bound_node_id.is_empty();
        self.subordinate_loop_mode = match (self.ownership_confirmed, response.agent_run_mode) {
            (true, AgentRunMode::Active) => SubordinateLoopMode::Active,
            (true, AgentRunMode::Idle) => SubordinateLoopMode::Limited,
            (false, _) => SubordinateLoopMode::Withheld,
        };
        self.staged_effective_config = Some(candidate);
        self.loops_enabled = false;
    }

    pub fn record_invalid_accepted_sync_response(
        &mut self,
        response: AgentSyncResponse,
        error: String,
    ) {
        self.latest_seen_config_version = Some(response.config_version.clone());
        self.binding_state = Some(response.binding_state.clone());
        self.staged_effective_config = None;
        self.last_sync_error = Some(error);

        if self.effective_config.is_some() {
            self.degraded = true;
            return;
        }

        self.local_node_id = (!response.bound_node_id.is_empty()).then(|| response.bound_node_id.clone());
        self.base_config.node_id = self.local_node_id.clone();
        self.ownership_confirmed = false;
        self.subordinate_loop_mode = SubordinateLoopMode::Withheld;
        self.loops_enabled = false;
        self.degraded = false;
    }

    fn record_rejected_sync_response(&mut self, response: AgentSyncResponse) {
        self.latest_seen_config_version = Some(response.config_version.clone());
        self.binding_state = Some(response.binding_state.clone());
        self.staged_effective_config = None;
        self.last_sync_error = response
            .rejection_reason
            .or_else(|| Some("sync rejected".to_string()));
        self.ownership_confirmed = false;
        self.subordinate_loop_mode = SubordinateLoopMode::Withheld;
        self.loops_enabled = false;
        self.degraded = self.effective_config.is_some();
    }
}
