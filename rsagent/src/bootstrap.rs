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
    let mut state = AgentRuntimeState::new(config.clone());
    let client = state.sync_client();
    let request = crate::clients::nodemanage::NodeManageSyncClient::build_request(
        &config,
        &identity,
        state.config_version().map(ToString::to_string),
    );
    let response = client.sync(transport, &request).await?;
    state.apply_sync_response(response);
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
