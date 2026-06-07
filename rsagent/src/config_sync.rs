use anyhow::Result;
use nodemanage::SyncBindingState;

use crate::{
    clients::nodemanage::NodeManageSyncTransport,
    registration::{AgentIdentity, AgentRuntimeState},
    runtime_coordinator::{RetryBackoffPolicy, next_temporary_upstream_retry_policy},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncOutcome {
    pub config_changed: bool,
    pub heartbeat_reset_required: bool,
    pub sync_interval_changed: bool,
    pub task_sync_interval_changed: bool,
    pub heartbeat_interval_changed: bool,
    pub retry_policy: Option<RetryBackoffPolicy>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigSyncHealthTransition {
    EnteredDegraded,
    Recovered,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigSyncDiagnostic {
    pub transition: ConfigSyncHealthTransition,
    pub config_version: Option<String>,
    pub binding_state: Option<SyncBindingState>,
    pub reason: Option<String>,
    pub retry_policy: Option<RetryBackoffPolicy>,
}

pub struct ConfigSyncLoop<T> {
    transport: T,
    last_retry_policy: Option<RetryBackoffPolicy>,
    last_tick_diagnostic: Option<ConfigSyncDiagnostic>,
}

impl<T> ConfigSyncLoop<T>
where
    T: NodeManageSyncTransport,
{
    pub fn new(transport: T) -> Self {
        Self {
            transport,
            last_retry_policy: None,
            last_tick_diagnostic: None,
        }
    }

    pub async fn tick(
        &mut self,
        state: &mut AgentRuntimeState,
        identity: &AgentIdentity,
    ) -> Result<SyncOutcome> {
        let was_degraded = state.is_degraded();
        let outcome = run_sync_once_with_retry_policy(
            state,
            identity,
            &mut self.transport,
            self.last_retry_policy.as_ref(),
        )
        .await?;

        self.last_tick_diagnostic = diagnose_sync_transition(was_degraded, state, &outcome);
        self.last_retry_policy = outcome.retry_policy.clone();

        Ok(outcome)
    }

    pub fn last_retry_policy(&self) -> Option<&RetryBackoffPolicy> {
        self.last_retry_policy.as_ref()
    }

    pub fn last_tick_diagnostic(&self) -> Option<&ConfigSyncDiagnostic> {
        self.last_tick_diagnostic.as_ref()
    }

    pub fn into_transport(self) -> T {
        self.transport
    }
}

pub fn diagnose_sync_transition(
    was_degraded: bool,
    state: &AgentRuntimeState,
    outcome: &SyncOutcome,
) -> Option<ConfigSyncDiagnostic> {
    let is_degraded = state.is_degraded();
    let transition = match (was_degraded, is_degraded) {
        (false, true) => ConfigSyncHealthTransition::EnteredDegraded,
        (true, false) => ConfigSyncHealthTransition::Recovered,
        _ => return None,
    };

    Some(ConfigSyncDiagnostic {
        transition,
        config_version: state.config_version().map(ToString::to_string),
        binding_state: state.binding_state().cloned(),
        reason: state.last_sync_error().map(ToString::to_string),
        retry_policy: outcome.retry_policy.clone(),
    })
}

pub async fn run_sync_once<T>(
    state: &mut AgentRuntimeState,
    identity: &AgentIdentity,
    transport: &mut T,
) -> Result<SyncOutcome>
where
    T: NodeManageSyncTransport,
{
    run_sync_once_with_retry_policy(state, identity, transport, None).await
}

async fn run_sync_once_with_retry_policy<T>(
    state: &mut AgentRuntimeState,
    identity: &AgentIdentity,
    transport: &mut T,
    previous_retry_policy: Option<&RetryBackoffPolicy>,
) -> Result<SyncOutcome>
where
    T: NodeManageSyncTransport,
{
    let previous = state.effective_config().cloned();
    let was_degraded = state.is_degraded();
    let client = state.sync_client();
    let request = state.build_sync_request(identity);

    let response = match client.sync(transport, &request).await {
        Ok(response) => response,
        Err(error) => {
            let error_message = error.to_string();
            let retry_policy = next_temporary_upstream_retry_policy(previous_retry_policy);
            state.record_temporary_sync_failure(error_message);
            return Ok(SyncOutcome {
                config_changed: false,
                heartbeat_reset_required: false,
                sync_interval_changed: false,
                task_sync_interval_changed: false,
                heartbeat_interval_changed: false,
                retry_policy: Some(retry_policy),
            });
        }
    };

    if !response.accepted {
        state.apply_sync_response(response);
        return Ok(SyncOutcome {
            config_changed: false,
            heartbeat_reset_required: false,
            sync_interval_changed: false,
            task_sync_interval_changed: false,
            heartbeat_interval_changed: false,
            retry_policy: None,
        });
    }

    let candidate = match AgentRuntimeState::validated_runtime_config(&response) {
        Ok(candidate) => candidate,
        Err(error) => {
            state.record_invalid_accepted_sync_response(response, error);
            return Ok(SyncOutcome {
                config_changed: false,
                heartbeat_reset_required: false,
                sync_interval_changed: false,
                task_sync_interval_changed: false,
                heartbeat_interval_changed: false,
                retry_policy: None,
            });
        }
    };

    let config_changed = previous
        .as_ref()
        .map(|config| config.config_version.as_str())
        != Some(candidate.config_version.as_str());
    let sync_interval_changed = previous
        .as_ref()
        .map(|config| config.sync_interval_secs != candidate.sync_interval_secs)
        .unwrap_or(true);
    let task_sync_interval_changed = previous
        .as_ref()
        .map(|config| config.task_sync_interval_secs != candidate.task_sync_interval_secs)
        .unwrap_or(true);
    let heartbeat_interval_changed = previous
        .as_ref()
        .map(|config| {
            config.heartbeat_config.interval_secs != candidate.heartbeat_config.interval_secs
        })
        .unwrap_or(true);
    let heartbeat_destination_changed = previous
        .as_ref()
        .map(|config| {
            config.heartbeat_config.data_link_id != candidate.heartbeat_config.data_link_id
                || config.heartbeat_config.vm_base_url != candidate.heartbeat_config.vm_base_url
        })
        .unwrap_or(true);

    state.stage_validated_sync_response(response, candidate);

    Ok(SyncOutcome {
        config_changed,
        heartbeat_reset_required: was_degraded
            || heartbeat_interval_changed
            || heartbeat_destination_changed,
        sync_interval_changed,
        task_sync_interval_changed,
        heartbeat_interval_changed,
        retry_policy: None,
    })
}
