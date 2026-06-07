use chrono::Utc;
use std::{fs, path::PathBuf};

use anyhow::Result;

use crate::{
    clients::nodemanage::NodeManageSyncTransport,
    config::AgentRuntimeConfig,
    config_sync::{SyncOutcome, run_sync_once},
    registration::{AgentIdentity, AgentRuntimeState, DurableRuntimeSnapshot},
    runtime_coordinator::{
        RuntimeCoordinatorEffects, effects_from_sync_outcome,
        promote_staged_config_after_loop_switch,
    },
};

const DURABLE_RUNTIME_STATE_FILE: &str = "runtime-state.json";

pub fn converge_runtime_mainline_after_sync(
    state: &mut AgentRuntimeState,
    outcome: &SyncOutcome,
) -> RuntimeCoordinatorEffects {
    let effects = effects_from_sync_outcome(outcome);
    promote_staged_config_after_loop_switch(state);
    effects
}

pub async fn bootstrap_runtime_state<T>(
    config: AgentRuntimeConfig,
    identity: AgentIdentity,
    transport: &mut T,
) -> Result<(AgentRuntimeState, AgentIdentity)>
where
    T: NodeManageSyncTransport,
{
    let mut state = load_durable_runtime_state(&config)?
        .unwrap_or_else(|| AgentRuntimeState::new(config.clone()));
    let outcome = run_sync_once(&mut state, &identity, transport).await?;
    let _ = converge_runtime_mainline_after_sync(&mut state, &outcome);
    persist_durable_runtime_state(&state)?;
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

pub fn persist_durable_runtime_state(state: &AgentRuntimeState) -> Result<()> {
    let Some(snapshot) = state.durable_snapshot() else {
        return Ok(());
    };

    let path = durable_runtime_state_path(state.data_dir());
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let payload = serde_json::to_vec_pretty(&snapshot)?;
    fs::write(path, payload)?;
    Ok(())
}

pub fn load_durable_runtime_state(
    config: &AgentRuntimeConfig,
) -> Result<Option<AgentRuntimeState>> {
    let path = durable_runtime_state_path(&config.data_dir);
    if !path.exists() {
        return Ok(None);
    }

    let payload = fs::read(path)?;
    let snapshot: DurableRuntimeSnapshot = serde_json::from_slice(&payload)?;
    Ok(Some(AgentRuntimeState::restore_durable_snapshot(
        config.clone(),
        snapshot,
    )))
}

fn durable_runtime_state_path(data_dir: &str) -> PathBuf {
    PathBuf::from(data_dir).join(DURABLE_RUNTIME_STATE_FILE)
}
