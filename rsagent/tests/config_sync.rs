use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
};

use anyhow::{Result, anyhow};
use chrono::{TimeZone, Utc};
use nodemanage::{
    AgentRunMode, AgentSyncResponse, HeartbeatConfig, JobManageConfig, SyncBindingState,
    TaskFilterDefaults,
};
use rsagent::{
    clients::nodemanage::NodeManageSyncTransport,
    config::AgentRuntimeConfig,
    config_sync::{
        ConfigSyncDiagnostic, ConfigSyncHealthTransition, ConfigSyncLoop, diagnose_sync_transition,
    },
    registration::{AgentIdentity, AgentRuntimeState, SubordinateLoopMode},
};

#[test]
fn staged_accepted_config_only_becomes_effective_after_promotion() {
    let mut state = sample_runtime_state();

    state.stage_accepted_sync_response(sample_response(
        true,
        AgentRunMode::Active,
        SyncBindingState::Bound,
        "node-001",
        "cfg-v1",
        None,
    ));

    assert_eq!(state.latest_seen_config_version(), Some("cfg-v1"));
    assert_eq!(state.accepted_config_version(), Some("cfg-v1"));
    assert!(state.effective_config().is_none());

    state.promote_staged_config_to_effective();

    let effective = state.effective_config().expect("effective config");
    assert_eq!(effective.config_version, "cfg-v1");
}

#[tokio::test]
async fn run_sync_once_stages_config_without_making_it_effective_until_promotion_boundary() {
    let mut state = sample_runtime_state();
    let mut transport = RecordingTransport::success(sample_response(
        true,
        AgentRunMode::Active,
        SyncBindingState::Bound,
        "node-001",
        "cfg-v1",
        None,
    ));

    let outcome =
        rsagent::config_sync::run_sync_once(&mut state, &sample_identity(), &mut transport)
            .await
            .unwrap();

    assert!(outcome.config_changed);
    assert!(outcome.sync_interval_changed);
    assert!(outcome.task_sync_interval_changed);
    assert!(outcome.heartbeat_interval_changed);
    assert!(outcome.heartbeat_reset_required);
    assert_eq!(state.latest_seen_config_version(), Some("cfg-v1"));
    assert_eq!(state.accepted_config_version(), Some("cfg-v1"));
    assert!(state.effective_config().is_none());
    assert!(!state.loops_enabled());

    rsagent::runtime_coordinator::promote_staged_config_after_loop_switch(&mut state);

    let effective = state.effective_config().expect("effective config");
    assert_eq!(effective.config_version, "cfg-v1");
    assert!(state.loops_enabled());
}

#[tokio::test]
async fn run_sync_once_updates_runtime_state_and_reports_interval_changes() {
    let mut state = sample_runtime_state();
    state.apply_sync_response(sample_response(
        true,
        AgentRunMode::Active,
        SyncBindingState::Bound,
        "node-001",
        "cfg-v1",
        None,
    ));

    let mut updated = sample_response(
        true,
        AgentRunMode::Active,
        SyncBindingState::Bound,
        "node-001",
        "cfg-v2",
        None,
    );
    updated.heartbeat_config.interval_secs = 45;
    updated.task_sync_interval_secs = 77;
    updated.sync_interval_secs = 99;
    let mut transport = RecordingTransport::success(updated);

    let outcome =
        rsagent::config_sync::run_sync_once(&mut state, &sample_identity(), &mut transport)
            .await
            .unwrap();

    assert!(outcome.config_changed);
    assert!(outcome.heartbeat_reset_required);
    assert!(outcome.sync_interval_changed);
    assert!(outcome.task_sync_interval_changed);
    assert!(outcome.heartbeat_interval_changed);
    assert!(!state.is_degraded());
    assert_eq!(state.config_version(), Some("cfg-v2"));

    let effective_before = state.effective_config().expect("previous effective config");
    assert_eq!(effective_before.config_version, "cfg-v1");

    rsagent::runtime_coordinator::promote_staged_config_after_loop_switch(&mut state);

    assert!(!state.is_degraded());
    let effective_after = state.effective_config().expect("promoted effective config");
    assert_eq!(effective_after.config_version, "cfg-v2");
}

#[tokio::test]
async fn run_sync_once_does_not_reset_heartbeat_for_version_only_change() {
    let mut state = sample_runtime_state();
    state.apply_sync_response(sample_response(
        true,
        AgentRunMode::Active,
        SyncBindingState::Bound,
        "node-001",
        "cfg-v1",
        None,
    ));

    let mut version_only = sample_response(
        true,
        AgentRunMode::Active,
        SyncBindingState::Bound,
        "node-001",
        "cfg-v2",
        None,
    );
    version_only.sync_interval_secs = 10;
    version_only.task_sync_interval_secs = 20;
    version_only.heartbeat_config.interval_secs = 15;
    let mut transport = RecordingTransport::success(version_only);

    let outcome =
        rsagent::config_sync::run_sync_once(&mut state, &sample_identity(), &mut transport)
            .await
            .unwrap();

    assert!(outcome.config_changed);
    assert!(!outcome.sync_interval_changed);
    assert!(!outcome.task_sync_interval_changed);
    assert!(!outcome.heartbeat_interval_changed);
    assert!(!outcome.heartbeat_reset_required);
}

#[tokio::test]
async fn run_sync_once_clears_temporary_degraded_state_when_config_version_is_unchanged() {
    let mut state = sample_runtime_state();
    state.apply_sync_response(sample_response(
        true,
        AgentRunMode::Active,
        SyncBindingState::Bound,
        "node-001",
        "cfg-v1",
        None,
    ));
    state.record_temporary_sync_failure("timeout".to_string());

    let mut transport = RecordingTransport::success(sample_response(
        true,
        AgentRunMode::Active,
        SyncBindingState::Bound,
        "node-001",
        "cfg-v1",
        None,
    ));

    let outcome =
        rsagent::config_sync::run_sync_once(&mut state, &sample_identity(), &mut transport)
            .await
            .unwrap();

    assert!(!outcome.config_changed);
    assert!(outcome.heartbeat_reset_required);
    assert!(state.is_degraded());
    assert_eq!(state.last_sync_error(), Some("timeout"));

    rsagent::runtime_coordinator::promote_staged_config_after_loop_switch(&mut state);

    assert!(!state.is_degraded());
    assert_eq!(state.last_sync_error(), None);
}

#[tokio::test]
async fn config_sync_loop_exposes_recovered_transition_diagnostic_after_later_success() {
    let mut state = sample_runtime_state();
    state.apply_sync_response(sample_response(
        true,
        AgentRunMode::Active,
        SyncBindingState::Bound,
        "node-001",
        "cfg-v1",
        None,
    ));
    state.record_temporary_sync_failure("timeout".to_string());
    let was_degraded = state.is_degraded();
    let mut transport = RecordingTransport::success(sample_response(
        true,
        AgentRunMode::Active,
        SyncBindingState::Bound,
        "node-001",
        "cfg-v1",
        None,
    ));

    let outcome =
        rsagent::config_sync::run_sync_once(&mut state, &sample_identity(), &mut transport)
            .await
            .unwrap();

    assert!(!outcome.config_changed);
    rsagent::runtime_coordinator::promote_staged_config_after_loop_switch(&mut state);
    assert_eq!(
        diagnose_sync_transition(was_degraded, &state, &outcome),
        Some(ConfigSyncDiagnostic {
            transition: ConfigSyncHealthTransition::Recovered,
            config_version: Some("cfg-v1".to_string()),
            binding_state: Some(SyncBindingState::Bound),
            reason: None,
            retry_policy: None,
        })
    );
}

#[tokio::test]
async fn run_sync_once_temporary_failure_keeps_last_good_config_and_marks_degraded() {
    let mut state = sample_runtime_state();
    state.apply_sync_response(sample_response(
        true,
        AgentRunMode::Active,
        SyncBindingState::Bound,
        "node-001",
        "cfg-v1",
        None,
    ));
    let mut transport = RecordingTransport::temporary_failure("timeout");

    let outcome =
        rsagent::config_sync::run_sync_once(&mut state, &sample_identity(), &mut transport)
            .await
            .unwrap();

    assert!(!outcome.config_changed);
    assert!(!outcome.heartbeat_reset_required);
    assert!(state.loops_enabled());
    assert!(state.is_degraded());
    assert_eq!(state.latest_seen_config_version(), Some("cfg-v1"));
    assert_eq!(state.config_version(), Some("cfg-v1"));
}

#[tokio::test]
async fn config_sync_loop_exposes_degraded_transition_diagnostic_for_last_good_runtime() {
    let mut state = sample_runtime_state();
    state.apply_sync_response(sample_response(
        true,
        AgentRunMode::Active,
        SyncBindingState::Bound,
        "node-001",
        "cfg-v1",
        None,
    ));
    let mut loop_runner = ConfigSyncLoop::new(RecordingTransport::temporary_failure("timeout"));

    let outcome = loop_runner
        .tick(&mut state, &sample_identity())
        .await
        .unwrap();

    assert_eq!(
        loop_runner.last_tick_diagnostic(),
        Some(&ConfigSyncDiagnostic {
            transition: ConfigSyncHealthTransition::EnteredDegraded,
            config_version: Some("cfg-v1".to_string()),
            binding_state: Some(SyncBindingState::Bound),
            reason: Some("timeout".to_string()),
            retry_policy: outcome.retry_policy,
        })
    );
}

#[tokio::test]
async fn run_sync_once_classifies_temporary_upstream_failure_with_shared_retry_policy() {
    let mut state = sample_runtime_state();
    state.apply_sync_response(sample_response(
        true,
        AgentRunMode::Active,
        SyncBindingState::Bound,
        "node-001",
        "cfg-v1",
        None,
    ));
    let mut transport = RecordingTransport::temporary_failure("timeout");

    let outcome =
        rsagent::config_sync::run_sync_once(&mut state, &sample_identity(), &mut transport)
            .await
            .unwrap();

    assert_eq!(
        outcome.retry_policy,
        Some(rsagent::runtime_coordinator::temporary_upstream_retry_policy(1))
    );
    assert!(state.loops_enabled());
    assert!(state.is_degraded());
    assert_eq!(state.config_version(), Some("cfg-v1"));
}

#[tokio::test]
async fn run_sync_once_escalates_retry_backoff_on_repeated_temporary_upstream_failures() {
    let mut state = sample_runtime_state();
    state.apply_sync_response(sample_response(
        true,
        AgentRunMode::Active,
        SyncBindingState::Bound,
        "node-001",
        "cfg-v1",
        None,
    ));
    let mut loop_runner = ConfigSyncLoop::new(RecordingTransport::temporary_failures(vec![
        "timeout",
        "timeout",
        "different timeout wording",
    ]));

    let first = loop_runner
        .tick(&mut state, &sample_identity())
        .await
        .unwrap();
    let second = loop_runner
        .tick(&mut state, &sample_identity())
        .await
        .unwrap();
    let third = loop_runner
        .tick(&mut state, &sample_identity())
        .await
        .unwrap();

    assert_eq!(
        first.retry_policy,
        Some(rsagent::runtime_coordinator::temporary_upstream_retry_policy(1))
    );
    assert_eq!(
        second.retry_policy,
        Some(rsagent::runtime_coordinator::temporary_upstream_retry_policy(2))
    );
    assert_eq!(
        third.retry_policy,
        Some(rsagent::runtime_coordinator::temporary_upstream_retry_policy(3))
    );
    assert_eq!(
        loop_runner.last_retry_policy(),
        Some(&rsagent::runtime_coordinator::temporary_upstream_retry_policy(3))
    );
    assert!(state.loops_enabled());
    assert!(state.is_degraded());
}

#[tokio::test]
async fn run_sync_once_without_last_good_config_disables_loops_after_temporary_failure() {
    let mut state = sample_runtime_state();
    let mut transport = RecordingTransport::temporary_failure("timeout");

    let outcome =
        rsagent::config_sync::run_sync_once(&mut state, &sample_identity(), &mut transport)
            .await
            .unwrap();

    assert!(!outcome.config_changed);
    assert!(!state.loops_enabled());
    assert!(!state.is_degraded());
    assert!(state.effective_config().is_none());
}

#[tokio::test]
async fn run_sync_once_applies_explicit_denial_without_temporary_failure_semantics() {
    let mut state = sample_runtime_state();
    state.apply_sync_response(sample_response(
        true,
        AgentRunMode::Active,
        SyncBindingState::Bound,
        "node-001",
        "cfg-v1",
        None,
    ));
    state.record_temporary_sync_failure("timeout".to_string());

    let mut transport = RecordingTransport::success(sample_response(
        false,
        AgentRunMode::Idle,
        SyncBindingState::Conflict,
        "node-other",
        "cfg-denied",
        Some("binding conflict"),
    ));

    let outcome =
        rsagent::config_sync::run_sync_once(&mut state, &sample_identity(), &mut transport)
            .await
            .unwrap();

    assert!(!outcome.config_changed);
    assert!(!outcome.heartbeat_reset_required);
    assert!(!state.loops_enabled());
    assert!(state.is_degraded());
    assert_eq!(state.binding_state(), Some(&SyncBindingState::Conflict));
    assert_eq!(state.subordinate_loop_mode(), SubordinateLoopMode::Withheld);
    assert_eq!(state.latest_seen_config_version(), Some("cfg-denied"));
    assert_eq!(state.accepted_config_version(), Some("cfg-v1"));
    assert_eq!(state.last_sync_error(), Some("binding conflict"));

    let effective = state.effective_config().expect("last known good config");
    assert_eq!(effective.config_version, "cfg-v1");
}

#[tokio::test]
async fn run_sync_once_invalid_candidate_config_does_not_replace_last_known_good() {
    let mut state = sample_runtime_state();
    state.apply_sync_response(sample_response(
        true,
        AgentRunMode::Active,
        SyncBindingState::Bound,
        "node-001",
        "cfg-v1",
        None,
    ));

    let mut invalid = sample_response(
        true,
        AgentRunMode::Active,
        SyncBindingState::Bound,
        "node-001",
        "cfg-v2",
        None,
    );
    invalid.heartbeat_config.interval_secs = 0;
    let mut transport = RecordingTransport::success(invalid);

    let outcome =
        rsagent::config_sync::run_sync_once(&mut state, &sample_identity(), &mut transport)
            .await
            .unwrap();

    assert!(!outcome.config_changed);
    assert!(!outcome.heartbeat_reset_required);
    assert!(state.loops_enabled());
    assert!(state.is_degraded());
    assert_eq!(state.latest_seen_config_version(), Some("cfg-v2"));
    assert_eq!(state.accepted_config_version(), Some("cfg-v1"));
    assert_eq!(state.config_version(), Some("cfg-v1"));
    assert_eq!(
        state.last_sync_error(),
        Some("heartbeat interval_secs must be greater than zero")
    );

    let effective = state.effective_config().expect("last known good config");
    assert_eq!(effective.config_version, "cfg-v1");
    assert_eq!(effective.heartbeat_config.interval_secs, 15);
}

#[tokio::test]
async fn run_sync_once_invalid_initial_config_leaves_runtime_without_effective_config() {
    let mut state = sample_runtime_state();
    let mut invalid = sample_response(
        true,
        AgentRunMode::Active,
        SyncBindingState::Bound,
        "node-001",
        "cfg-v1",
        None,
    );
    invalid.job_manage_config.base_url = "".to_string();
    let mut transport = RecordingTransport::success(invalid);

    let outcome =
        rsagent::config_sync::run_sync_once(&mut state, &sample_identity(), &mut transport)
            .await
            .unwrap();

    assert!(!outcome.config_changed);
    assert!(!state.loops_enabled());
    assert!(!state.is_degraded());
    assert_eq!(state.latest_seen_config_version(), Some("cfg-v1"));
    assert_eq!(state.accepted_config_version(), None);
    assert!(state.effective_config().is_none());
    assert_eq!(
        state.last_sync_error(),
        Some("job manage base_url must not be empty")
    );
}

#[tokio::test]
async fn run_sync_once_idle_acceptance_keeps_config_but_limits_subordinate_loops() {
    let mut state = sample_runtime_state();
    let mut transport = RecordingTransport::success(sample_response(
        true,
        AgentRunMode::Idle,
        SyncBindingState::Bound,
        "node-001",
        "cfg-limited",
        None,
    ));

    let outcome =
        rsagent::config_sync::run_sync_once(&mut state, &sample_identity(), &mut transport)
            .await
            .unwrap();

    assert!(outcome.config_changed);
    assert!(!state.loops_enabled());
    assert_eq!(state.subordinate_loop_mode(), SubordinateLoopMode::Limited);
    assert_eq!(state.accepted_config_version(), Some("cfg-limited"));
    assert!(state.effective_config().is_none());

    rsagent::runtime_coordinator::promote_staged_config_after_loop_switch(&mut state);

    let effective = state.effective_config().unwrap();
    assert_eq!(effective.config_version, "cfg-limited");
}

#[tokio::test]
async fn run_sync_once_accepted_untrusted_binding_does_not_activate_loops() {
    let mut state = sample_runtime_state();
    let mut transport = RecordingTransport::success(sample_response(
        true,
        AgentRunMode::Active,
        SyncBindingState::Conflict,
        "node-other",
        "cfg-v2",
        None,
    ));

    let outcome =
        rsagent::config_sync::run_sync_once(&mut state, &sample_identity(), &mut transport)
            .await
            .unwrap();

    assert!(outcome.config_changed);
    assert!(!state.loops_enabled());
    assert_eq!(state.subordinate_loop_mode(), SubordinateLoopMode::Withheld);
    assert_eq!(state.binding_state(), Some(&SyncBindingState::Conflict));
    assert_eq!(state.accepted_config_version(), Some("cfg-v2"));
    assert!(state.effective_config().is_none());

    rsagent::runtime_coordinator::promote_staged_config_after_loop_switch(&mut state);

    let effective = state.effective_config().unwrap();
    assert_eq!(effective.config_version, "cfg-v2");
}

#[tokio::test]
async fn run_sync_once_rejected_sync_keeps_local_identity_boundary_for_next_request() {
    let mut state = AgentRuntimeState::new(AgentRuntimeConfig {
        nodemanage_sync_url: "http://127.0.0.1:3000/agent/sync".to_string(),
        agent_id: "agt-local".to_string(),
        node_id: Some("node-001".to_string()),
        data_dir: "/var/lib/rsagent".to_string(),
        sync_interval_secs: 60,
    });
    let mut transport = RecordingTransport::success(sample_response_with_agent_id(
        false,
        AgentRunMode::Idle,
        SyncBindingState::Conflict,
        "node-001",
        "cfg-denied",
        Some("rebind required"),
        "agt-previous",
    ));

    rsagent::config_sync::run_sync_once(&mut state, &sample_identity(), &mut transport)
        .await
        .unwrap();

    let request = state.build_sync_request(&sample_identity());
    assert_eq!(request.agent_id, "agt-local");
    assert_eq!(request.node_id.as_deref(), Some("node-001"));
    assert_eq!(state.binding_state(), Some(&SyncBindingState::Conflict));
}

#[tokio::test]
async fn run_sync_once_does_not_infer_runtime_ownership_from_node_match_alone() {
    let mut state = AgentRuntimeState::new(AgentRuntimeConfig {
        nodemanage_sync_url: "http://127.0.0.1:3000/agent/sync".to_string(),
        agent_id: "agt-local".to_string(),
        node_id: Some("node-001".to_string()),
        data_dir: "/var/lib/rsagent".to_string(),
        sync_interval_secs: 60,
    });
    let mut transport = RecordingTransport::success(sample_response_with_agent_id(
        true,
        AgentRunMode::Active,
        SyncBindingState::Bound,
        "node-001",
        "cfg-v1",
        None,
        "agt-other",
    ));

    let outcome =
        rsagent::config_sync::run_sync_once(&mut state, &sample_identity(), &mut transport)
            .await
            .unwrap();

    assert!(outcome.config_changed);
    assert!(!state.loops_enabled());
    assert_eq!(state.subordinate_loop_mode(), SubordinateLoopMode::Withheld);
}

#[derive(Debug)]
struct RecordingTransport {
    responses: Vec<AgentSyncResponse>,
    errors: std::collections::VecDeque<String>,
    requests: Arc<Mutex<Vec<nodemanage::AgentSyncRequest>>>,
}

impl RecordingTransport {
    fn success(response: AgentSyncResponse) -> Self {
        Self {
            responses: vec![response],
            errors: std::collections::VecDeque::new(),
            requests: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn temporary_failure(message: &str) -> Self {
        Self::temporary_failures(vec![message])
    }

    fn temporary_failures(messages: Vec<&str>) -> Self {
        Self {
            responses: Vec::new(),
            errors: messages.into_iter().map(ToString::to_string).collect(),
            requests: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl NodeManageSyncTransport for RecordingTransport {
    type SyncFuture<'a>
        = Pin<Box<dyn Future<Output = Result<AgentSyncResponse>> + Send + 'a>>
    where
        Self: 'a;

    fn sync<'a>(
        &'a mut self,
        _endpoint: &'a str,
        request: &'a nodemanage::AgentSyncRequest,
    ) -> Self::SyncFuture<'a> {
        self.requests
            .lock()
            .expect("requests lock")
            .push(request.clone());
        let response = self.responses.first().cloned();
        if self.responses.len() > 1 {
            self.responses.remove(0);
        }
        let error = self.errors.pop_front();
        Box::pin(async move {
            match (response, error) {
                (_, Some(error)) => Err(anyhow!(error)),
                (Some(response), None) => Ok(response),
                _ => Err(anyhow!("recording transport misconfigured")),
            }
        })
    }
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
