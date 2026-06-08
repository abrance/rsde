use crate::{config_sync::SyncOutcome, registration::AgentRuntimeState};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubordinateLoopDecision {
    pub run_heartbeat: bool,
    pub run_task_sync: bool,
    pub keep_process_alive: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeCoordinatorEffects {
    pub reset_heartbeat: bool,
    pub rebuild_sync_interval: bool,
    pub rebuild_task_sync_interval: bool,
    pub rebuild_heartbeat_interval: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoopIntervals {
    pub sync_interval_secs: u64,
    pub heartbeat_interval_secs: u64,
    pub task_sync_interval_secs: u64,
}

pub fn evaluate_subordinate_loops(state: &AgentRuntimeState) -> SubordinateLoopDecision {
    let enabled = state.loops_enabled() && state.effective_config().is_some();
    SubordinateLoopDecision {
        run_heartbeat: enabled,
        run_task_sync: enabled,
        keep_process_alive: true,
    }
}

pub fn effects_from_sync_outcome(outcome: &SyncOutcome) -> RuntimeCoordinatorEffects {
    RuntimeCoordinatorEffects {
        reset_heartbeat: outcome.heartbeat_reset_required,
        rebuild_sync_interval: outcome.sync_interval_changed,
        rebuild_task_sync_interval: outcome.task_sync_interval_changed,
        rebuild_heartbeat_interval: outcome.heartbeat_interval_changed,
    }
}

pub fn loop_intervals(state: &AgentRuntimeState, default_sync_interval_secs: u64) -> LoopIntervals {
    match state.effective_config() {
        Some(config) => {
            let decision = evaluate_subordinate_loops(state);

            LoopIntervals {
                sync_interval_secs: config.sync_interval_secs,
                heartbeat_interval_secs: if decision.run_heartbeat {
                    config.heartbeat_config.interval_secs
                } else {
                    default_sync_interval_secs
                },
                task_sync_interval_secs: if decision.run_task_sync {
                    config.task_sync_interval_secs
                } else {
                    default_sync_interval_secs
                },
            }
        }
        None => LoopIntervals {
            sync_interval_secs: default_sync_interval_secs,
            heartbeat_interval_secs: default_sync_interval_secs,
            task_sync_interval_secs: default_sync_interval_secs,
        },
    }
}
