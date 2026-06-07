use anyhow::Result;

use crate::{
    clients::nodemanage::NodeManageSyncTransport,
    registration::{AgentIdentity, AgentRuntimeState, RuntimeSyncState},
    runtime_coordinator::{
        TransientRetryPolicy, default_transient_retry_policy, is_transient_error,
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncOutcome {
    pub config_changed: bool,
    pub heartbeat_reset_required: bool,
    pub sync_interval_changed: bool,
    pub task_sync_interval_changed: bool,
    pub heartbeat_interval_changed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigSyncTransition {
    Stable,
    Degraded,
    Recovered,
    Disabled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigSyncDiagnostic {
    pub transition: ConfigSyncTransition,
    pub sync_state: RuntimeSyncState,
    pub loops_enabled: bool,
    pub degraded: bool,
    pub detail: String,
}

pub fn diagnose_sync_transition(
    previous: &AgentRuntimeState,
    current: &AgentRuntimeState,
    outcome: &SyncOutcome,
) -> ConfigSyncDiagnostic {
    let (transition, detail) = if current.sync_state() == RuntimeSyncState::TemporaryFailure
        && current.is_degraded()
    {
        (
            ConfigSyncTransition::Degraded,
            format!(
                "temporary sync failure preserved last good config; heartbeat_reset_required={}",
                outcome.heartbeat_reset_required
            ),
        )
    } else if previous.sync_state() == RuntimeSyncState::TemporaryFailure
        && current.sync_state() == RuntimeSyncState::Accepted
    {
        (
            ConfigSyncTransition::Recovered,
            format!(
                "sync recovered into accepted runtime config {}; heartbeat_reset_required={}",
                current.config_version().unwrap_or("unknown"),
                outcome.heartbeat_reset_required
            ),
        )
    } else if !current.loops_enabled() && current.effective_config().is_none() {
        (
            ConfigSyncTransition::Disabled,
            "sync has not produced an executable runtime config yet".to_string(),
        )
    } else {
        (
            ConfigSyncTransition::Stable,
            format!(
                "sync state {:?}; config_changed={}; heartbeat_reset_required={}",
                current.sync_state(),
                outcome.config_changed,
                outcome.heartbeat_reset_required
            ),
        )
    };

    ConfigSyncDiagnostic {
        transition,
        sync_state: current.sync_state(),
        loops_enabled: current.loops_enabled(),
        degraded: current.is_degraded(),
        detail,
    }
}

pub async fn run_sync_once<T>(
    state: &mut AgentRuntimeState,
    identity: &AgentIdentity,
    transport: &mut T,
) -> Result<SyncOutcome>
where
    T: NodeManageSyncTransport,
{
    run_sync_once_with_retry_policy(
        state,
        identity,
        transport,
        &default_transient_retry_policy(),
    )
    .await
}

pub async fn run_sync_once_with_retry_policy<T>(
    state: &mut AgentRuntimeState,
    identity: &AgentIdentity,
    transport: &mut T,
    retry_policy: &TransientRetryPolicy,
) -> Result<SyncOutcome>
where
    T: NodeManageSyncTransport,
{
    let previous = state.effective_config().cloned();
    let was_degraded = state.is_degraded();
    let client = state.sync_client();
    let request = state.build_sync_request(identity);

    let response = match sync_with_retry(&client, transport, &request, retry_policy).await {
        Ok(response) => response,
        Err(error) => {
            state.record_temporary_sync_failure(error.to_string());
            return Ok(SyncOutcome {
                config_changed: false,
                heartbeat_reset_required: false,
                sync_interval_changed: false,
                task_sync_interval_changed: false,
                heartbeat_interval_changed: false,
            });
        }
    };

    if !response.accepted {
        let heartbeat_reset_required = previous.is_some() || was_degraded;
        state.apply_sync_response(response);
        return Ok(SyncOutcome {
            config_changed: false,
            heartbeat_reset_required,
            sync_interval_changed: false,
            task_sync_interval_changed: false,
            heartbeat_interval_changed: false,
        });
    }

    let config_changed = state.config_version() != Some(response.config_version.as_str());
    let sync_interval_changed = previous
        .as_ref()
        .map(|config| config.sync_interval_secs != response.sync_interval_secs)
        .unwrap_or(config_changed);
    let task_sync_interval_changed = previous
        .as_ref()
        .map(|config| config.task_sync_interval_secs != response.task_sync_interval_secs)
        .unwrap_or(config_changed);
    let heartbeat_interval_changed = previous
        .as_ref()
        .map(|config| {
            config.heartbeat_config.interval_secs != response.heartbeat_config.interval_secs
        })
        .unwrap_or(config_changed);
    let heartbeat_destination_changed = previous.as_ref().is_some_and(|config| {
        config.heartbeat_config.data_link_id != response.heartbeat_config.data_link_id
            || config.heartbeat_config.vm_base_url != response.heartbeat_config.vm_base_url
    });

    let next_state = state.applied_sync_response(response);
    next_state.persist_local_identity()?;
    *state = next_state;

    Ok(SyncOutcome {
        config_changed,
        heartbeat_reset_required: was_degraded
            || config_changed
            || heartbeat_interval_changed
            || heartbeat_destination_changed,
        sync_interval_changed,
        task_sync_interval_changed,
        heartbeat_interval_changed,
    })
}

async fn sync_with_retry<T>(
    client: &crate::clients::nodemanage::NodeManageSyncClient,
    transport: &mut T,
    request: &nodemanage::AgentSyncRequest,
    retry_policy: &TransientRetryPolicy,
) -> Result<nodemanage::AgentSyncResponse>
where
    T: NodeManageSyncTransport,
{
    let mut attempt = 0usize;
    loop {
        match client.sync(transport, request).await {
            Ok(response) => return Ok(response),
            Err(error)
                if attempt < retry_policy.backoff_delays().len() && is_transient_error(&error) =>
            {
                tokio::time::sleep(retry_policy.backoff_delays()[attempt]).await;
                attempt += 1;
            }
            Err(error) => return Err(error),
        }
    }
}
