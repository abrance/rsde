use chrono::Utc;

use anyhow::Result;

use crate::{
    clients::nodemanage::NodeManageSyncTransport,
    config::AgentRuntimeConfig,
    registration::{AgentIdentity, AgentRuntimeState},
};

pub async fn bootstrap_runtime_state<T>(
    config: AgentRuntimeConfig,
    identity: AgentIdentity,
    transport: &mut T,
) -> Result<(AgentRuntimeState, AgentIdentity)>
where
    T: NodeManageSyncTransport,
{
    let config = config.resolve_local_identity()?;
    config.persist_local_identity()?;

    let mut state = AgentRuntimeState::new(config.clone());
    let client = state.sync_client();
    let request = state.build_sync_request(&identity);
    match client.sync(transport, &request).await {
        Ok(response) => {
            let next_state = state.applied_sync_response(response);
            if next_state.sync_state() == crate::registration::RuntimeSyncState::Accepted {
                next_state.persist_local_identity()?;
            }
            state = next_state;
        }
        Err(error) => state.record_temporary_sync_failure(error.to_string()),
    }
    Ok((state, identity))
}

pub fn default_identity() -> AgentIdentity {
    AgentIdentity::new(
        env!("CARGO_PKG_VERSION").to_string(),
        "unknown-host".to_string(),
        std::env::consts::OS.to_string(),
        std::env::consts::OS.to_string(),
        std::env::consts::ARCH.to_string(),
        vec!["sync".to_string()],
        Utc::now(),
    )
}
