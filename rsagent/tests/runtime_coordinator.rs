use chrono::{TimeZone, Utc};
use nodemanage::{
    AgentRunMode, AgentSyncResponse, HeartbeatConfig, JobManageConfig, SyncBindingState,
    TaskFilterDefaults,
};
use rsagent::{
    config::AgentRuntimeConfig,
    registration::{AgentIdentity, AgentRuntimeState},
};

#[test]
fn placeholder_until_runtime_coordinator_exists() {
    let _ = sample_runtime_state();
}

#[test]
fn coordinator_allows_heartbeat_and_task_sync_when_loops_enabled() {
    let mut state = sample_runtime_state();
    state.apply_sync_response(sample_response(
        true,
        AgentRunMode::Active,
        SyncBindingState::Bound,
        "node-001",
        "cfg-001",
        None,
    ));

    let decision = rsagent::runtime_coordinator::evaluate_subordinate_loops(&state);

    assert!(decision.run_heartbeat);
    assert!(decision.run_task_sync);
    assert!(decision.keep_process_alive);
}

#[test]
fn coordinator_keeps_process_alive_when_subordinate_loops_are_disabled() {
    let state = sample_runtime_state();

    let decision = rsagent::runtime_coordinator::evaluate_subordinate_loops(&state);

    assert!(!decision.run_heartbeat);
    assert!(!decision.run_task_sync);
    assert!(decision.keep_process_alive);
}

#[test]
fn coordinator_treats_degraded_last_good_runtime_as_executable() {
    let mut state = sample_runtime_state();
    state.apply_sync_response(sample_response(
        true,
        AgentRunMode::Active,
        SyncBindingState::Bound,
        "node-001",
        "cfg-healthy",
        None,
    ));
    state.record_temporary_sync_failure("timeout".to_string());

    let decision = rsagent::runtime_coordinator::evaluate_subordinate_loops(&state);

    assert!(decision.run_heartbeat);
    assert!(decision.run_task_sync);
    assert!(decision.keep_process_alive);
}

#[test]
fn coordinator_disables_subordinate_loops_after_explicit_denial() {
    let mut state = sample_runtime_state();
    state.apply_sync_response(sample_response(
        true,
        AgentRunMode::Active,
        SyncBindingState::Bound,
        "node-001",
        "cfg-healthy",
        None,
    ));
    state.apply_sync_response(sample_response(
        false,
        AgentRunMode::Idle,
        SyncBindingState::Conflict,
        "node-other",
        "cfg-denied",
        Some("binding conflict"),
    ));

    let decision = rsagent::runtime_coordinator::evaluate_subordinate_loops(&state);

    assert!(!decision.run_heartbeat);
    assert!(!decision.run_task_sync);
    assert!(decision.keep_process_alive);
}

#[test]
fn coordinator_translates_sync_outcome_into_runtime_effects() {
    let outcome = rsagent::config_sync::SyncOutcome {
        config_changed: true,
        heartbeat_reset_required: true,
        sync_interval_changed: true,
        task_sync_interval_changed: false,
        heartbeat_interval_changed: true,
    };

    let effects = rsagent::runtime_coordinator::effects_from_sync_outcome(&outcome);

    assert!(effects.reset_heartbeat);
    assert!(effects.rebuild_sync_interval);
    assert!(!effects.rebuild_task_sync_interval);
    assert!(effects.rebuild_heartbeat_interval);
}

#[test]
fn coordinator_marks_first_accepted_sync_as_rebuild_for_all_loop_intervals() {
    let outcome = rsagent::config_sync::SyncOutcome {
        config_changed: true,
        heartbeat_reset_required: true,
        sync_interval_changed: true,
        task_sync_interval_changed: true,
        heartbeat_interval_changed: true,
    };

    let effects = rsagent::runtime_coordinator::effects_from_sync_outcome(&outcome);

    assert!(effects.reset_heartbeat);
    assert!(effects.rebuild_sync_interval);
    assert!(effects.rebuild_task_sync_interval);
    assert!(effects.rebuild_heartbeat_interval);
}

#[test]
fn coordinator_noop_sync_outcome_does_not_rebuild_any_loop() {
    let outcome = rsagent::config_sync::SyncOutcome {
        config_changed: false,
        heartbeat_reset_required: false,
        sync_interval_changed: false,
        task_sync_interval_changed: false,
        heartbeat_interval_changed: false,
    };

    let effects = rsagent::runtime_coordinator::effects_from_sync_outcome(&outcome);

    assert!(!effects.reset_heartbeat);
    assert!(!effects.rebuild_sync_interval);
    assert!(!effects.rebuild_task_sync_interval);
    assert!(!effects.rebuild_heartbeat_interval);
}

#[test]
fn loop_intervals_use_runtime_config_when_available() {
    let mut state = sample_runtime_state();
    state.apply_sync_response(sample_response(
        true,
        AgentRunMode::Active,
        SyncBindingState::Bound,
        "node-001",
        "cfg-001",
        None,
    ));

    let intervals = rsagent::runtime_coordinator::loop_intervals(&state, 60);

    assert_eq!(intervals.sync_interval_secs, 10);
    assert_eq!(intervals.heartbeat_interval_secs, 15);
    assert_eq!(intervals.task_sync_interval_secs, 20);
}

#[test]
fn loop_intervals_normalize_zero_sync_driven_values_to_non_zero() {
    let mut state = sample_runtime_state();
    let mut response = sample_response(
        true,
        AgentRunMode::Active,
        SyncBindingState::Bound,
        "node-001",
        "cfg-zero",
        None,
    );
    response.sync_interval_secs = 0;
    response.task_sync_interval_secs = 0;
    response.heartbeat_config.interval_secs = 0;
    state.apply_sync_response(response);

    let intervals = rsagent::runtime_coordinator::loop_intervals(&state, 60);

    assert_eq!(intervals.sync_interval_secs, 1);
    assert_eq!(intervals.heartbeat_interval_secs, 1);
    assert_eq!(intervals.task_sync_interval_secs, 1);
}

#[test]
fn loop_intervals_fall_back_to_base_sync_interval_without_effective_config() {
    let state = sample_runtime_state();

    let intervals = rsagent::runtime_coordinator::loop_intervals(&state, 60);

    assert_eq!(intervals.sync_interval_secs, 60);
    assert_eq!(intervals.heartbeat_interval_secs, 60);
    assert_eq!(intervals.task_sync_interval_secs, 60);
}

fn sample_runtime_state() -> AgentRuntimeState {
    AgentRuntimeState::new(AgentRuntimeConfig {
        nodemanage_sync_url: "http://127.0.0.1:3000/agent/sync".to_string(),
        agent_id: "agt-001".to_string(),
        node_id: None,
        data_dir: "/var/lib/rsagent".to_string(),
        sync_interval_secs: 60,
    })
}

#[allow(dead_code)]
fn sample_identity() -> AgentIdentity {
    AgentIdentity::new(
        "0.1.0".to_string(),
        "worker-01".to_string(),
        "linux".to_string(),
        "ubuntu".to_string(),
        "x86_64".to_string(),
        vec!["sync".to_string(), "jobs".to_string()],
        Utc.with_ymd_and_hms(2025, 1, 2, 3, 4, 5).unwrap(),
    )
}

fn sample_response(
    accepted: bool,
    agent_run_mode: AgentRunMode,
    binding_state: SyncBindingState,
    bound_node_id: &str,
    config_version: &str,
    rejection_reason: Option<&str>,
) -> AgentSyncResponse {
    AgentSyncResponse {
        accepted,
        agent_id: "agt-001".to_string(),
        bound_node_id: bound_node_id.to_string(),
        binding_state,
        agent_run_mode,
        config_version: config_version.to_string(),
        heartbeat_config: HeartbeatConfig {
            version: "hb-v1".to_string(),
            data_link_id: "dl-001".to_string(),
            vm_base_url: "http://vm".to_string(),
            interval_secs: 15,
        },
        job_manage_config: JobManageConfig {
            version: "jm-v1".to_string(),
            base_url: "http://job-manage".to_string(),
            task_filter_defaults: TaskFilterDefaults {
                states: vec!["queued".to_string(), "running".to_string()],
            },
        },
        sync_interval_secs: 10,
        task_sync_interval_secs: 20,
        rejection_reason: rejection_reason.map(ToString::to_string),
    }
}
