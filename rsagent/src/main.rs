use std::{env, fs, time::Duration};

use anyhow::Result;
use rsagent::{
    bootstrap::{bootstrap_runtime_state, default_identity},
    clients::nodemanage::ReqwestNodeManageSyncTransport,
    config::AgentRuntimeConfig,
    task_sync::TaskSyncLoop,
};
use tracing::{error, info, warn};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let config = load_runtime_config()?;
    let identity = default_identity();
    let agent_id = config.agent_id.clone();
    let mut sync_transport = ReqwestNodeManageSyncTransport::default();
    let (state, identity) = bootstrap_runtime_state(config, identity, &mut sync_transport).await?;

    info!(agent_version = %identity.agent_version, agent_id = %agent_id, "rsagent starting");

    if !state.loops_enabled() {
        warn!("task sync loop disabled until nodemanage sync provides active runtime config");
        return Ok(());
    }

    let interval_secs = state
        .effective_config()
        .map(|config| config.task_sync_interval_secs)
        .unwrap_or(60);
    let mut loop_runner = TaskSyncLoop::production();
    let mut interval = tokio::time::interval(Duration::from_secs(interval_secs));

    loop {
        interval.tick().await;
        match loop_runner
            .tick(&state, &agent_id, chrono::Utc::now())
            .await
        {
            Ok(tick) => info!(?tick, "task sync tick completed"),
            Err(error) => error!(error = %error, "task sync tick failed"),
        }

        if !state.loops_enabled() {
            warn!("task sync loop disabled; exiting main loop");
            break;
        }
    }

    Ok(())
}

fn load_runtime_config() -> Result<AgentRuntimeConfig> {
    let config_path = env::var("RSAGENT_CONFIG").ok();

    match config_path {
        Some(path) => {
            let content = fs::read_to_string(path)?;
            Ok(toml::from_str(&content)?)
        }
        None => Ok(AgentRuntimeConfig::default()),
    }
}
