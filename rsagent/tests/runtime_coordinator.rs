use chrono::{TimeZone, Utc};
use nodemanage::{
    AgentRunMode, AgentSyncResponse, HeartbeatConfig, JobManageConfig, SyncBindingState,
    TaskFilterDefaults,
};
use rsagent::{
    bootstrap::converge_runtime_mainline_after_sync,
    config::AgentRuntimeConfig,
    config_sync::SyncOutcome,
    registration::{AgentIdentity, AgentRuntimeState},
};

#[test]
fn coordinator_disables_subordinate_loops_when_only_accepted_config_exists() {
    let mut state = sample_runtime_state();
    state.stage_accepted_sync_response(sample_response(
        true,
        AgentRunMode::Active,
        SyncBindingState::Bound,
        "node-001",
        "cfg-staged",
        None,
    ));

    let decision = rsagent::runtime_coordinator::evaluate_subordinate_loops(&state);

    assert!(!decision.run_heartbeat);
    assert!(!decision.run_task_sync);
    assert!(decision.keep_process_alive);
}

#[test]
fn coordinator_promotion_boundary_makes_staged_config_effective() {
    let mut state = sample_runtime_state();
    state.stage_accepted_sync_response(sample_response(
        true,
        AgentRunMode::Active,
        SyncBindingState::Bound,
        "node-001",
        "cfg-staged",
        None,
    ));

    rsagent::runtime_coordinator::promote_staged_config_after_loop_switch(&mut state);

    let decision = rsagent::runtime_coordinator::evaluate_subordinate_loops(&state);

    assert!(decision.run_heartbeat);
    assert!(decision.run_task_sync);
    assert!(state.effective_config().is_some());
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
fn coordinator_keeps_process_alive_and_disables_subordinate_loops_in_no_valid_config_state() {
    let mut state = sample_runtime_state();
    state.record_rejected_or_invalid_config(
        Some("cfg-invalid".to_string()),
        Some(SyncBindingState::Bound),
        "job manage base_url must not be empty".to_string(),
    );

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
        retry_policy: None,
    };

    let effects = rsagent::runtime_coordinator::effects_from_sync_outcome(&outcome);

    assert!(effects.reset_heartbeat);
    assert!(effects.rebuild_sync_interval);
    assert!(!effects.rebuild_task_sync_interval);
    assert!(effects.rebuild_heartbeat_interval);
}

#[test]
fn coordinator_effects_rebuild_all_intervals_for_first_accepted_config() {
    let outcome = rsagent::config_sync::SyncOutcome {
        config_changed: true,
        heartbeat_reset_required: true,
        sync_interval_changed: true,
        task_sync_interval_changed: true,
        heartbeat_interval_changed: true,
        retry_policy: None,
    };

    let effects = rsagent::runtime_coordinator::effects_from_sync_outcome(&outcome);

    assert!(effects.rebuild_sync_interval);
    assert!(effects.rebuild_task_sync_interval);
    assert!(effects.rebuild_heartbeat_interval);
}

#[test]
fn coordinator_rebind_convergence_returns_through_shared_sync_control_path() {
    let mut state = AgentRuntimeState::new(AgentRuntimeConfig {
        node_id: Some("node-001".to_string()),
        agent_id: "agt-new".to_string(),
        ..sample_runtime_config()
    });
    state.apply_sync_response(sample_response_with_agent_id(
        false,
        AgentRunMode::Idle,
        SyncBindingState::Conflict,
        "node-001",
        "cfg-conflict",
        Some("rebind required"),
        "agt-old",
    ));

    state.stage_accepted_sync_response(sample_response_with_agent_id(
        true,
        AgentRunMode::Active,
        SyncBindingState::Bound,
        "node-001",
        "cfg-rebound",
        None,
        "agt-new",
    ));

    let effects = converge_runtime_mainline_after_sync(
        &mut state,
        &SyncOutcome {
            config_changed: true,
            heartbeat_reset_required: true,
            sync_interval_changed: true,
            task_sync_interval_changed: true,
            heartbeat_interval_changed: true,
            retry_policy: None,
        },
    );

    assert!(effects.reset_heartbeat);
    assert!(state.loops_enabled());
    assert_eq!(state.local_node_id(), Some("node-001"));
    assert_eq!(state.config_version(), Some("cfg-rebound"));
}

#[test]
fn coordinator_restart_reenters_shared_sync_control_path_without_special_case() {
    let mut restarted = AgentRuntimeState::new(AgentRuntimeConfig {
        node_id: Some("node-001".to_string()),
        ..sample_runtime_config()
    });
    restarted.stage_accepted_sync_response(sample_response(
        true,
        AgentRunMode::Active,
        SyncBindingState::Bound,
        "node-001",
        "cfg-restart",
        None,
    ));

    let effects = converge_runtime_mainline_after_sync(
        &mut restarted,
        &SyncOutcome {
            config_changed: true,
            heartbeat_reset_required: true,
            sync_interval_changed: true,
            task_sync_interval_changed: true,
            heartbeat_interval_changed: true,
            retry_policy: None,
        },
    );

    assert!(effects.rebuild_sync_interval);
    assert!(effects.rebuild_heartbeat_interval);
    assert!(effects.rebuild_task_sync_interval);
    assert!(restarted.loops_enabled());
    assert_eq!(restarted.local_node_id(), Some("node-001"));
    assert_eq!(restarted.config_version(), Some("cfg-restart"));
}

#[test]
fn coordinator_restart_from_recovered_last_known_good_state_keeps_known_sync_heartbeat_task_path() {
    let mut restarted = AgentRuntimeState::new(AgentRuntimeConfig {
        node_id: Some("node-001".to_string()),
        ..sample_runtime_config()
    });
    restarted.apply_sync_response(sample_response(
        true,
        AgentRunMode::Active,
        SyncBindingState::Bound,
        "node-001",
        "cfg-restart",
        None,
    ));
    restarted.record_temporary_sync_failure("upstream unavailable".to_string());

    let decision = rsagent::runtime_coordinator::evaluate_subordinate_loops(&restarted);
    let intervals = rsagent::runtime_coordinator::loop_intervals(&restarted, 60);

    assert!(decision.run_heartbeat);
    assert!(decision.run_task_sync);
    assert!(decision.keep_process_alive);
    assert_eq!(intervals.sync_interval_secs, 10);
    assert_eq!(intervals.heartbeat_interval_secs, 15);
    assert_eq!(intervals.task_sync_interval_secs, 20);
    assert!(restarted.is_degraded());
    assert_eq!(restarted.config_version(), Some("cfg-restart"));
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

#[test]
fn coordinator_exposes_shared_temporary_upstream_retry_backoff_policy() {
    assert_eq!(
        rsagent::runtime_coordinator::temporary_upstream_retry_policy(1),
        rsagent::runtime_coordinator::RetryBackoffPolicy {
            failure_class: rsagent::runtime_coordinator::FailureClass::TemporaryUpstream,
            consecutive_failures: 1,
            backoff_level: 0,
        }
    );
    assert_eq!(
        rsagent::runtime_coordinator::temporary_upstream_retry_policy(4),
        rsagent::runtime_coordinator::RetryBackoffPolicy {
            failure_class: rsagent::runtime_coordinator::FailureClass::TemporaryUpstream,
            consecutive_failures: 4,
            backoff_level: 3,
        }
    );

    let second = rsagent::runtime_coordinator::next_temporary_upstream_retry_policy(Some(
        &rsagent::runtime_coordinator::temporary_upstream_retry_policy(1),
    ));
    let third = rsagent::runtime_coordinator::next_temporary_upstream_retry_policy(Some(&second));

    assert_eq!(
        second,
        rsagent::runtime_coordinator::temporary_upstream_retry_policy(2)
    );
    assert_eq!(
        third,
        rsagent::runtime_coordinator::temporary_upstream_retry_policy(3)
    );
}

#[test]
fn coordinator_keeps_loop_decisions_coherent_under_repeated_temporary_sync_failure() {
    let mut state = sample_runtime_state();
    state.apply_sync_response(sample_response(
        true,
        AgentRunMode::Active,
        SyncBindingState::Bound,
        "node-001",
        "cfg-healthy",
        None,
    ));
    state.record_temporary_sync_failure("timeout-1".to_string());
    state.record_temporary_sync_failure("timeout-2".to_string());

    let outcome = SyncOutcome {
        config_changed: false,
        heartbeat_reset_required: false,
        sync_interval_changed: false,
        task_sync_interval_changed: false,
        heartbeat_interval_changed: false,
        retry_policy: Some(rsagent::runtime_coordinator::temporary_upstream_retry_policy(2)),
    };
    let decision = rsagent::runtime_coordinator::evaluate_subordinate_loops(&state);
    let effects = rsagent::runtime_coordinator::effects_from_sync_outcome(&outcome);

    assert!(decision.run_heartbeat);
    assert!(decision.run_task_sync);
    assert!(decision.keep_process_alive);
    assert_eq!(
        effects.retry_policy,
        Some(rsagent::runtime_coordinator::temporary_upstream_retry_policy(2))
    );
    assert!(!effects.rebuild_sync_interval);
    assert!(!effects.rebuild_heartbeat_interval);
    assert!(!effects.rebuild_task_sync_interval);
}

fn sample_runtime_state() -> AgentRuntimeState {
    AgentRuntimeState::new(sample_runtime_config())
}

fn sample_runtime_config() -> AgentRuntimeConfig {
    AgentRuntimeConfig {
        nodemanage_sync_url: "http://127.0.0.1:3000/agent/sync".to_string(),
        agent_id: "agt-001".to_string(),
        node_id: None,
        data_dir: "/var/lib/rsagent".to_string(),
        sync_interval_secs: 60,
    }
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
    sample_response_with_agent_id(
        accepted,
        agent_run_mode,
        binding_state,
        bound_node_id,
        config_version,
        rejection_reason,
        "agt-001",
    )
}

fn sample_response_with_agent_id(
    accepted: bool,
    agent_run_mode: AgentRunMode,
    binding_state: SyncBindingState,
    bound_node_id: &str,
    config_version: &str,
    rejection_reason: Option<&str>,
    agent_id: &str,
) -> AgentSyncResponse {
    AgentSyncResponse {
        accepted,
        agent_id: agent_id.to_string(),
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
