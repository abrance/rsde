use anyhow::Result;

use crate::{
    clients::nodemanage::NodeManageSyncTransport,
    registration::{AgentIdentity, AgentRuntimeState},
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
    let client = state.sync_client();
    let request = state.build_sync_request(identity);

    let response = match client.sync(transport, &request).await {
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
    let sync_interval_changed = previous
        .as_ref()
        .is_some_and(|config| config.sync_interval_secs != response.sync_interval_secs);
    let task_sync_interval_changed = previous
        .as_ref()
        .is_some_and(|config| config.task_sync_interval_secs != response.task_sync_interval_secs);
    let heartbeat_interval_changed = previous.as_ref().is_some_and(|config| {
        config.heartbeat_config.interval_secs != response.heartbeat_config.interval_secs
    });
    let heartbeat_destination_changed = previous.as_ref().is_some_and(|config| {
        config.heartbeat_config.data_link_id != response.heartbeat_config.data_link_id
            || config.heartbeat_config.vm_base_url != response.heartbeat_config.vm_base_url
    });

    state.apply_sync_response(response);

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
