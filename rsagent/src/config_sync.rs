use anyhow::Result;
use tokio::time::sleep;
use tracing::{error, info, warn};

use crate::{
    clients::nodemanage::NodeManageSyncTransport,
    registration::{AgentIdentity, AgentRuntimeState},
    retry::{transport_max_attempts, transport_retry_delay},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncOutcome {
    pub config_changed: bool,
    pub heartbeat_reset_required: bool,
    pub sync_interval_changed: bool,
    pub task_sync_interval_changed: bool,
    pub heartbeat_interval_changed: bool,
}

pub async fn run_sync_once<T>(
    state: &mut AgentRuntimeState,
    identity: &AgentIdentity,
    transport: &mut T,
) -> Result<SyncOutcome>
where
    T: NodeManageSyncTransport,
{
    let previous = state.effective_config().cloned();
    let was_degraded = state.is_degraded();
    let previously_running_loops = state.loops_enabled();
    let client = state.sync_client();
    let request = state.build_sync_request(identity);

    let response = match sync_with_retry(&client, transport, &request).await {
        Ok(response) => response,
        Err(error) => {
            let error_message = error.to_string();
            state.record_transient_sync_failure(error.to_string());
            if previous.is_none() && state.is_degraded() {
                warn!(
                    error = %error_message,
                    max_attempts = transport_max_attempts(),
                    loops_enabled = state.loops_enabled(),
                    degraded = state.is_degraded(),
                    "config sync remains in startup degraded state waiting for first accepted runtime config"
                );
            }
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
        state.apply_sync_response(response);
        return Ok(SyncOutcome {
            config_changed: false,
            heartbeat_reset_required: false,
            sync_interval_changed: false,
            task_sync_interval_changed: false,
            heartbeat_interval_changed: false,
        });
    }

    let config_changed = state.config_version() != Some(response.config_version.as_str());
    let node_binding_changed = state.local_node_id() != Some(response.bound_node_id.as_str());
    let sync_interval_changed = previous.as_ref().map_or(true, |config| {
        config.sync_interval_secs != response.sync_interval_secs
    });
    let task_sync_interval_changed = previous.as_ref().map_or(true, |config| {
        config.task_sync_interval_secs != response.task_sync_interval_secs
    });
    let heartbeat_interval_changed = previous.as_ref().map_or(true, |config| {
        config.heartbeat_config.interval_secs != response.heartbeat_config.interval_secs
    });
    let heartbeat_destination_changed = previous.as_ref().map_or(true, |config| {
        config.heartbeat_config.data_link_id != response.heartbeat_config.data_link_id
            || config.heartbeat_config.vm_base_url != response.heartbeat_config.vm_base_url
    });

    state.apply_sync_response(response);
    let loops_reenabled = !previously_running_loops && state.loops_enabled();

    if was_degraded && !state.is_degraded() {
        info!(
            config_version = ?state.config_version(),
            loops_enabled = state.loops_enabled(),
            "config sync recovered local runtime from degraded state"
        );
    }

    Ok(SyncOutcome {
        config_changed,
        heartbeat_reset_required: was_degraded
            || loops_reenabled
            || config_changed
            || node_binding_changed
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
) -> Result<nodemanage::AgentSyncResponse>
where
    T: NodeManageSyncTransport,
{
    let mut attempt = 0;

    loop {
        match client.sync(transport, request).await {
            Ok(response) => {
                if attempt > 0 {
                    info!(
                        operation = "config_sync",
                        retries = attempt,
                        recovered_on_attempt = attempt + 1,
                        max_attempts = transport_max_attempts(),
                        "config sync transport recovered after retry"
                    );
                }
                return Ok(response);
            }
            Err(error) => match transport_retry_delay(attempt) {
                Some(delay) => {
                    warn!(
                        operation = "config_sync",
                        failed_attempt = attempt + 1,
                        next_attempt = attempt + 2,
                        max_attempts = transport_max_attempts(),
                        delay_ms = delay.as_millis() as u64,
                        error = %error,
                        "config sync transport failed; retrying"
                    );
                    attempt += 1;
                    sleep(delay).await;
                }
                None => {
                    error!(
                        operation = "config_sync",
                        failed_attempt = attempt + 1,
                        max_attempts = transport_max_attempts(),
                        error = %error,
                        "config sync transport failed; retry budget exhausted"
                    );
                    return Err(error);
                }
            },
        }
    }
}
