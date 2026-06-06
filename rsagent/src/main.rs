use std::{env, fs, time::Duration};

use anyhow::Result;
use rsagent::{
    bootstrap::{bootstrap_runtime_state, default_identity},
    clients::{
        nodemanage::ReqwestNodeManageSyncTransport,
        victoria_metrics::ReqwestVictoriaMetricsTransport,
    },
    config::AgentRuntimeConfig,
    config_sync::run_sync_once,
    heartbeat::HeartbeatReporter,
    runtime_coordinator::{effects_from_sync_outcome, evaluate_subordinate_loops, loop_intervals},
    task_sync::TaskSyncLoop,
};
use tracing::{error, info, warn};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let config = load_runtime_config()?;
    let identity = default_identity();
    let agent_id = config.agent_id.clone();
    let default_sync_interval_secs = config.sync_interval_secs;
    let mut sync_transport = ReqwestNodeManageSyncTransport::default();
    let (mut state, identity) =
        bootstrap_runtime_state(config, identity, &mut sync_transport).await?;

    info!(agent_version = %identity.agent_version, agent_id = %agent_id, "rsagent starting");

    if !state.loops_enabled() {
        warn!("subordinate loops disabled until nodemanage sync provides active runtime config");
    }

    let mut heartbeat_reporter = HeartbeatReporter::new(ReqwestVictoriaMetricsTransport::default());
    let mut loop_runner = TaskSyncLoop::production();
    let initial_intervals = loop_intervals(&state, default_sync_interval_secs);
    let mut sync_interval =
        tokio::time::interval(Duration::from_secs(initial_intervals.sync_interval_secs));
    let mut heartbeat_interval = tokio::time::interval(Duration::from_secs(
        initial_intervals.heartbeat_interval_secs,
    ));
    let mut task_sync_interval = tokio::time::interval(Duration::from_secs(
        initial_intervals.task_sync_interval_secs,
    ));

    loop {
        tokio::select! {
            _ = sync_interval.tick() => {
                let now = chrono::Utc::now();
                match run_sync_once(&mut state, &identity, &mut sync_transport).await {
                    Ok(outcome) => {
                        let effects = effects_from_sync_outcome(&outcome);
                        if effects.reset_heartbeat {
                            heartbeat_reporter.reset();
                        }

                        if effects.rebuild_sync_interval
                            || effects.rebuild_heartbeat_interval
                            || effects.rebuild_task_sync_interval
                        {
                            let intervals = loop_intervals(&state, default_sync_interval_secs);
                            if effects.rebuild_sync_interval {
                                sync_interval = tokio::time::interval(Duration::from_secs(intervals.sync_interval_secs));
                            }
                            if effects.rebuild_heartbeat_interval {
                                heartbeat_interval = tokio::time::interval(Duration::from_secs(intervals.heartbeat_interval_secs));
                            }
                            if effects.rebuild_task_sync_interval {
                                task_sync_interval = tokio::time::interval(Duration::from_secs(intervals.task_sync_interval_secs));
                            }
                        }

                        info!(?outcome, degraded = state.is_degraded(), loops_enabled = state.loops_enabled(), sync_error = ?state.last_sync_error(), at = %now, "config sync tick completed");
                    }
                    Err(error) => error!(error = %error, "config sync tick failed"),
                }
            }
            _ = heartbeat_interval.tick() => {
                let decision = evaluate_subordinate_loops(&state);
                if !decision.keep_process_alive {
                    warn!("runtime coordinator requested process exit before heartbeat tick");
                    break;
                }
                if !decision.run_heartbeat {
                    continue;
                }

                match heartbeat_reporter.tick(chrono::Utc::now(), &state, &agent_id, &identity) {
                    Ok(tick) => info!(?tick, "heartbeat tick completed"),
                    Err(error) => error!(error = %error, "heartbeat tick failed"),
                }
            }
            _ = task_sync_interval.tick() => {
                let decision = evaluate_subordinate_loops(&state);
                if !decision.keep_process_alive {
                    warn!("runtime coordinator requested process exit before task sync tick");
                    break;
                }
                if !decision.run_task_sync {
                    continue;
                }

                match loop_runner
                    .tick(&state, &agent_id, chrono::Utc::now())
                    .await
                {
                    Ok(tick) => info!(?tick, "task sync tick completed"),
                    Err(error) => error!(error = %error, "task sync tick failed"),
                }
            }
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
