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
    state.record_transient_sync_failure("timeout".to_string());

    let decision = rsagent::runtime_coordinator::evaluate_subordinate_loops(&state);

    assert!(decision.run_heartbeat);
    assert!(decision.run_task_sync);
    assert!(decision.keep_process_alive);
}

#[test]
fn coordinator_keeps_last_good_intervals_while_runtime_is_temporarily_degraded() {
    let mut state = sample_runtime_state();
    state.apply_sync_response(sample_response(
        true,
        AgentRunMode::Active,
        SyncBindingState::Bound,
        "node-001",
        "cfg-healthy",
        None,
    ));
    state.record_transient_sync_failure("timeout".to_string());

    let intervals = rsagent::runtime_coordinator::loop_intervals(&state, 60);

    assert_eq!(intervals.sync_interval_secs, 10);
    assert_eq!(intervals.heartbeat_interval_secs, 15);
    assert_eq!(intervals.task_sync_interval_secs, 20);
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
fn coordinator_parks_subordinate_loop_intervals_after_explicit_denial() {
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

    let intervals = rsagent::runtime_coordinator::loop_intervals(&state, 60);

    assert_eq!(intervals.sync_interval_secs, 10);
    assert_eq!(
        intervals.heartbeat_interval_secs, 60,
        "disabled heartbeat loop should park on the base interval"
    );
    assert_eq!(
        intervals.task_sync_interval_secs, 60,
        "disabled task-sync loop should park on the base interval"
    );
}

#[test]
fn coordinator_parks_subordinate_loop_intervals_for_accepted_idle_runtime() {
    let mut state = sample_runtime_state();
    state.apply_sync_response(sample_response(
        true,
        AgentRunMode::Idle,
        SyncBindingState::Bound,
        "node-001",
        "cfg-idle",
        None,
    ));

    let decision = rsagent::runtime_coordinator::evaluate_subordinate_loops(&state);
    let intervals = rsagent::runtime_coordinator::loop_intervals(&state, 60);

    assert!(!decision.run_heartbeat);
    assert!(!decision.run_task_sync);
    assert!(decision.keep_process_alive);
    assert_eq!(intervals.sync_interval_secs, 10);
    assert_eq!(intervals.heartbeat_interval_secs, 60);
    assert_eq!(intervals.task_sync_interval_secs, 60);
}

#[test]
fn coordinator_keeps_subordinate_loops_disabled_after_rejection_then_transport_failure() {
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
    state.record_transient_sync_failure("timeout".to_string());

    let decision = rsagent::runtime_coordinator::evaluate_subordinate_loops(&state);

    assert!(!decision.run_heartbeat);
    assert!(!decision.run_task_sync);
    assert!(decision.keep_process_alive);
}

#[test]
fn restart_like_reconstruction_preserves_only_local_identity_before_next_sync() {
    let state = AgentRuntimeState::new(AgentRuntimeConfig {
        nodemanage_sync_url: "http://127.0.0.1:3000/agent/sync".to_string(),
        agent_id: "agt-001".to_string(),
        node_id: Some("node-persisted".to_string()),
        data_dir: "/var/lib/rsagent".to_string(),
        sync_interval_secs: 60,
    });

    let decision = rsagent::runtime_coordinator::evaluate_subordinate_loops(&state);

    assert_eq!(state.local_node_id(), Some("node-persisted"));
    assert_eq!(state.config_version(), None);
    assert_eq!(state.binding_state(), None);
    assert!(state.effective_config().is_none());
    assert!(!state.is_degraded());
    assert!(!state.loops_enabled());
    assert!(!decision.run_heartbeat);
    assert!(!decision.run_task_sync);
    assert!(decision.keep_process_alive);
}

#[test]
fn restart_like_reconstruction_reconverges_after_next_accepted_sync() {
    let mut state = AgentRuntimeState::new(AgentRuntimeConfig {
        nodemanage_sync_url: "http://127.0.0.1:3000/agent/sync".to_string(),
        agent_id: "agt-001".to_string(),
        node_id: Some("node-persisted".to_string()),
        data_dir: "/var/lib/rsagent".to_string(),
        sync_interval_secs: 60,
    });

    state.apply_sync_response(sample_response(
        true,
        AgentRunMode::Active,
        SyncBindingState::Bound,
        "node-rebound",
        "cfg-recovered",
        None,
    ));

    let decision = rsagent::runtime_coordinator::evaluate_subordinate_loops(&state);

    assert_eq!(state.local_node_id(), Some("node-rebound"));
    assert_eq!(state.config_version(), Some("cfg-recovered"));
    assert_eq!(state.binding_state(), Some(&SyncBindingState::Bound));
    assert!(!state.is_degraded());
    assert!(state.loops_enabled());
    assert!(decision.run_heartbeat);
    assert!(decision.run_task_sync);
    assert!(decision.keep_process_alive);
}

#[test]
fn reconstructed_runtime_state_only_preserves_local_identity_before_resync() {
    let state = AgentRuntimeState::new(AgentRuntimeConfig {
        nodemanage_sync_url: "http://127.0.0.1:3000/agent/sync".to_string(),
        agent_id: "agt-001".to_string(),
        node_id: Some("node-persisted".to_string()),
        data_dir: "/var/lib/rsagent".to_string(),
        sync_interval_secs: 60,
    });

    let decision = rsagent::runtime_coordinator::evaluate_subordinate_loops(&state);

    assert_eq!(state.local_node_id(), Some("node-persisted"));
    assert_eq!(state.config_version(), None);
    assert!(state.binding_state().is_none());
    assert!(state.effective_config().is_none());
    assert!(!state.is_degraded());
    assert!(!decision.run_heartbeat);
    assert!(!decision.run_task_sync);
    assert!(decision.keep_process_alive);
}

#[test]
fn reconstructed_runtime_state_reconverges_after_later_accepted_sync() {
    let mut state = AgentRuntimeState::new(AgentRuntimeConfig {
        nodemanage_sync_url: "http://127.0.0.1:3000/agent/sync".to_string(),
        agent_id: "agt-001".to_string(),
        node_id: Some("node-persisted".to_string()),
        data_dir: "/var/lib/rsagent".to_string(),
        sync_interval_secs: 60,
    });

    state.apply_sync_response(sample_response(
        true,
        AgentRunMode::Active,
        SyncBindingState::Bound,
        "node-persisted",
        "cfg-recovered",
        None,
    ));

    let decision = rsagent::runtime_coordinator::evaluate_subordinate_loops(&state);

    assert_eq!(state.local_node_id(), Some("node-persisted"));
    assert_eq!(state.config_version(), Some("cfg-recovered"));
    assert_eq!(state.binding_state(), Some(&SyncBindingState::Bound));
    assert!(!state.is_degraded());
    assert!(state.effective_config().is_some());
    assert!(decision.run_heartbeat);
    assert!(decision.run_task_sync);
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
fn coordinator_keeps_rebuild_matrix_isolated_to_the_changed_loop() {
    let outcome = rsagent::config_sync::SyncOutcome {
        config_changed: true,
        heartbeat_reset_required: false,
        sync_interval_changed: false,
        task_sync_interval_changed: true,
        heartbeat_interval_changed: false,
    };

    let effects = rsagent::runtime_coordinator::effects_from_sync_outcome(&outcome);

    assert!(!effects.reset_heartbeat);
    assert!(!effects.rebuild_sync_interval);
    assert!(effects.rebuild_task_sync_interval);
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
