use std::collections::VecDeque;

use anyhow::{Result, anyhow};
use chrono::{Duration, TimeZone, Utc};
use nodemanage::{
    AgentRunMode, AgentSyncResponse, HeartbeatConfig, JobManageConfig, SyncBindingState,
    TaskFilterDefaults,
};
use rsagent::{
    clients::victoria_metrics::VictoriaMetricsTransport,
    config::AgentRuntimeConfig,
    heartbeat::{HeartbeatReporter, HeartbeatSkipReason, HeartbeatTick, build_heartbeat_payload},
    registration::{AgentIdentity, AgentRuntimeState},
};

#[test]
fn heartbeat_payload_assembly_uses_identity_runtime_and_sync_config() {
    let state = synced_runtime_state(30);
    let identity = sample_identity();
    let timestamp = Utc.with_ymd_and_hms(2025, 1, 2, 3, 4, 5).unwrap();

    let payload = build_heartbeat_payload(&state, "agt-001", &identity, timestamp).unwrap();

    assert_eq!(
        payload,
        "rsagent_heartbeat,node_id=node-001,agent_id=agt-001,agent_version=0.1.0,data_link_id=dl-001,status=alive value=1i 1735787045000000000"
    );
}

#[test]
fn heartbeat_reporter_sends_on_schedule() {
    let state = synced_runtime_state(60);
    let identity = sample_identity();
    let start = Utc.with_ymd_and_hms(2025, 1, 2, 3, 4, 5).unwrap();
    let mut reporter = HeartbeatReporter::new(RecordingTransport::default());

    let first = reporter.tick(start, &state, "agt-001", &identity).unwrap();
    let second = reporter
        .tick(start + Duration::seconds(30), &state, "agt-001", &identity)
        .unwrap();
    let third = reporter
        .tick(start + Duration::seconds(60), &state, "agt-001", &identity)
        .unwrap();

    assert!(matches!(first, HeartbeatTick::Sent { .. }));
    assert_eq!(
        second,
        HeartbeatTick::Skipped {
            reason: HeartbeatSkipReason::IntervalNotElapsed,
        }
    );
    assert!(matches!(third, HeartbeatTick::Sent { .. }));

    let transport = reporter.into_transport();
    assert_eq!(transport.requests.len(), 2);
    assert_eq!(
        transport.requests[0].endpoint,
        "http://victoriametrics:8428/api/v2/write"
    );
    assert_eq!(
        transport.requests[0].payload,
        "rsagent_heartbeat,node_id=node-001,agent_id=agt-001,agent_version=0.1.0,data_link_id=dl-001,status=alive value=1i 1735787045000000000"
    );
    assert_eq!(
        transport.requests[1].payload,
        "rsagent_heartbeat,node_id=node-001,agent_id=agt-001,agent_version=0.1.0,data_link_id=dl-001,status=alive value=1i 1735787105000000000"
    );
}

#[test]
fn heartbeat_reporter_resets_schedule_when_heartbeat_destination_changes() {
    let initial_state = synced_runtime_state_with_response(sample_response(
        "node-001",
        "cfg-001",
        "dl-001",
        "http://victoriametrics-a:8428",
        60,
    ));
    let updated_state = synced_runtime_state_with_response(sample_response(
        "node-001",
        "cfg-001",
        "dl-002",
        "http://victoriametrics-b:8428",
        60,
    ));
    let identity = sample_identity();
    let start = Utc.with_ymd_and_hms(2025, 1, 2, 3, 4, 5).unwrap();
    let mut reporter = HeartbeatReporter::new(RecordingTransport::default());

    let first = reporter
        .tick(start, &initial_state, "agt-001", &identity)
        .unwrap();
    let second = reporter
        .tick(
            start + Duration::seconds(1),
            &updated_state,
            "agt-001",
            &identity,
        )
        .unwrap();

    assert!(matches!(first, HeartbeatTick::Sent { .. }));
    assert!(matches!(second, HeartbeatTick::Sent { .. }));

    let transport = reporter.into_transport();
    assert_eq!(transport.requests.len(), 2);
    assert_eq!(
        transport.requests[1].endpoint,
        "http://victoriametrics-b:8428/api/v2/write"
    );
    assert!(
        transport.requests[1]
            .payload
            .contains("data_link_id=dl-002")
    );
}

#[test]
fn heartbeat_reporter_resets_schedule_when_bound_node_changes_without_config_version_change() {
    let initial_state = synced_runtime_state_with_response(sample_response(
        "node-001",
        "cfg-001",
        "dl-001",
        "http://victoriametrics:8428",
        60,
    ));
    let rebound_state = synced_runtime_state_with_response(sample_response(
        "node-002",
        "cfg-001",
        "dl-001",
        "http://victoriametrics:8428",
        60,
    ));
    let identity = sample_identity();
    let start = Utc.with_ymd_and_hms(2025, 1, 2, 3, 4, 5).unwrap();
    let mut reporter = HeartbeatReporter::new(RecordingTransport::default());

    let first = reporter
        .tick(start, &initial_state, "agt-001", &identity)
        .unwrap();
    let second = reporter
        .tick(
            start + Duration::seconds(1),
            &rebound_state,
            "agt-001",
            &identity,
        )
        .unwrap();

    assert!(matches!(first, HeartbeatTick::Sent { .. }));
    assert!(matches!(second, HeartbeatTick::Sent { .. }));

    let transport = reporter.into_transport();
    assert_eq!(transport.requests.len(), 2);
    assert!(transport.requests[1].payload.contains("node_id=node-002"));
}

#[test]
fn heartbeat_reporter_marks_itself_degraded_after_write_failure() {
    let state = synced_runtime_state(60);
    let identity = sample_identity();
    let timestamp = Utc.with_ymd_and_hms(2025, 1, 2, 3, 4, 5).unwrap();
    let mut reporter = HeartbeatReporter::new(FailingTransport);

    let error = reporter
        .tick(timestamp, &state, "agt-001", &identity)
        .unwrap_err();

    assert!(reporter.is_degraded());
    assert!(error.to_string().contains("temporary write failure"));
}

#[test]
fn heartbeat_reporter_retries_temporary_write_failure_before_succeeding() {
    let state = synced_runtime_state(60);
    let identity = sample_identity();
    let timestamp = Utc.with_ymd_and_hms(2025, 1, 2, 3, 4, 5).unwrap();
    let mut reporter = HeartbeatReporter::new(ScriptedTransport::new(vec![
        Err("temporary write failure 1"),
        Ok(()),
    ]));

    let tick = reporter
        .tick(timestamp, &state, "agt-001", &identity)
        .unwrap();

    assert!(matches!(tick, HeartbeatTick::Sent { .. }));
    assert!(!reporter.is_degraded());
    let transport = reporter.into_transport();
    assert_eq!(transport.requests.len(), 2);
}

#[test]
fn heartbeat_reporter_stops_after_retry_budget_exhaustion() {
    let state = synced_runtime_state(60);
    let identity = sample_identity();
    let timestamp = Utc.with_ymd_and_hms(2025, 1, 2, 3, 4, 5).unwrap();
    let mut reporter = HeartbeatReporter::new(ScriptedTransport::new(vec![
        Err("temporary write failure 1"),
        Err("temporary write failure 2"),
        Err("temporary write failure 3"),
    ]));

    let error = reporter
        .tick(timestamp, &state, "agt-001", &identity)
        .unwrap_err();

    assert!(reporter.is_degraded());
    assert!(error.to_string().contains("temporary write failure 3"));
    let transport = reporter.into_transport();
    assert_eq!(transport.requests.len(), 3);
}

#[test]
fn heartbeat_reporter_reset_clears_last_sent_at_and_degraded_state() {
    let state = synced_runtime_state(60);
    let identity = sample_identity();
    let timestamp = Utc.with_ymd_and_hms(2025, 1, 2, 3, 4, 5).unwrap();
    let mut reporter = HeartbeatReporter::new(RecordingTransport::default());

    reporter
        .tick(timestamp, &state, "agt-001", &identity)
        .unwrap();
    reporter.reset();

    let next = reporter
        .tick(
            timestamp + Duration::seconds(1),
            &state,
            "agt-001",
            &identity,
        )
        .unwrap();

    assert!(matches!(next, HeartbeatTick::Sent { .. }));
    assert!(!reporter.is_degraded());
}

#[test]
fn heartbeat_reporter_recover_clears_degraded_without_requiring_config_change() {
    let state = synced_runtime_state(60);
    let identity = sample_identity();
    let timestamp = Utc.with_ymd_and_hms(2025, 1, 2, 3, 4, 5).unwrap();
    let mut reporter = HeartbeatReporter::new(FailingTransport);

    let _ = reporter.tick(timestamp, &state, "agt-001", &identity);
    assert!(reporter.is_degraded());

    reporter.recover();

    assert!(!reporter.is_degraded());
}

#[test]
fn heartbeat_reporter_still_emits_if_called_with_last_known_good_config_while_loops_disabled() {
    let mut state = synced_runtime_state(60);
    state.apply_sync_response(AgentSyncResponse {
        accepted: false,
        agent_id: "agt-001".to_string(),
        bound_node_id: "node-other".to_string(),
        binding_state: SyncBindingState::Conflict,
        agent_run_mode: AgentRunMode::Idle,
        config_version: "cfg-other".to_string(),
        heartbeat_config: HeartbeatConfig {
            version: "hb-v2".to_string(),
            data_link_id: "dl-other".to_string(),
            vm_base_url: "http://victoriametrics-other:8428".to_string(),
            interval_secs: 5,
        },
        job_manage_config: JobManageConfig {
            version: "jm-v2".to_string(),
            base_url: "http://job-manage-other".to_string(),
            task_filter_defaults: TaskFilterDefaults {
                states: vec!["queued".to_string()],
            },
        },
        sync_interval_secs: 5,
        task_sync_interval_secs: 5,
        rejection_reason: Some("binding conflict".to_string()),
    });
    let identity = sample_identity();
    let timestamp = Utc.with_ymd_and_hms(2025, 1, 2, 3, 4, 5).unwrap();
    let mut reporter = HeartbeatReporter::new(RecordingTransport::default());

    let tick = reporter
        .tick(timestamp, &state, "agt-001", &identity)
        .unwrap();

    assert!(matches!(tick, HeartbeatTick::Sent { .. }));
    assert!(!state.loops_enabled());
    assert!(state.effective_config().is_some());
}

#[test]
fn heartbeat_reporter_skips_when_effective_config_is_missing_even_if_process_is_alive() {
    let config = AgentRuntimeConfig {
        nodemanage_sync_url: "http://127.0.0.1:3000/agent/sync".to_string(),
        agent_id: "agt-001".to_string(),
        node_id: None,
        data_dir: "/var/lib/rsagent".to_string(),
        sync_interval_secs: 60,
    };
    let state = AgentRuntimeState::new(config);
    let identity = sample_identity();
    let timestamp = Utc.with_ymd_and_hms(2025, 1, 2, 3, 4, 5).unwrap();
    let mut reporter = HeartbeatReporter::new(RecordingTransport::default());

    let tick = reporter
        .tick(timestamp, &state, "agt-001", &identity)
        .unwrap();

    assert_eq!(
        tick,
        HeartbeatTick::Skipped {
            reason: HeartbeatSkipReason::MissingConfig,
        }
    );
    assert!(state.process_alive());
    assert!(!state.loops_enabled());
    assert!(state.effective_config().is_none());
}

#[derive(Debug, Default)]
struct RecordingTransport {
    requests: Vec<RecordedRequest>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RecordedRequest {
    endpoint: String,
    payload: String,
}

impl VictoriaMetricsTransport for RecordingTransport {
    fn post_line_protocol(&mut self, endpoint: &str, payload: &str) -> Result<()> {
        self.requests.push(RecordedRequest {
            endpoint: endpoint.to_string(),
            payload: payload.to_string(),
        });
        Ok(())
    }
}

#[derive(Debug)]
struct FailingTransport;

impl VictoriaMetricsTransport for FailingTransport {
    fn post_line_protocol(&mut self, _endpoint: &str, _payload: &str) -> Result<()> {
        Err(anyhow!("temporary write failure"))
    }
}

#[derive(Debug)]
struct ScriptedTransport {
    requests: Vec<RecordedRequest>,
    outcomes: VecDeque<std::result::Result<(), String>>,
}

impl ScriptedTransport {
    fn new(outcomes: Vec<std::result::Result<(), &str>>) -> Self {
        Self {
            requests: Vec::new(),
            outcomes: outcomes
                .into_iter()
                .map(|outcome| outcome.map_err(|error| error.to_string()))
                .collect(),
        }
    }
}

impl VictoriaMetricsTransport for ScriptedTransport {
    fn post_line_protocol(&mut self, endpoint: &str, payload: &str) -> Result<()> {
        self.requests.push(RecordedRequest {
            endpoint: endpoint.to_string(),
            payload: payload.to_string(),
        });

        match self.outcomes.pop_front() {
            Some(Ok(())) => Ok(()),
            Some(Err(error)) => Err(anyhow!(error)),
            None => Err(anyhow!("scripted transport exhausted")),
        }
    }
}

fn sample_identity() -> AgentIdentity {
    AgentIdentity::new(
        "0.1.0".to_string(),
        "worker-01".to_string(),
        "linux".to_string(),
        "ubuntu".to_string(),
        "x86_64".to_string(),
        vec!["sync".to_string(), "jobs".to_string()],
        Utc.with_ymd_and_hms(2025, 1, 2, 3, 0, 0).unwrap(),
    )
}

fn synced_runtime_state(heartbeat_interval_secs: u64) -> AgentRuntimeState {
    let config = AgentRuntimeConfig {
        nodemanage_sync_url: "http://127.0.0.1:3000/agent/sync".to_string(),
        agent_id: "agt-001".to_string(),
        node_id: None,
        data_dir: "/var/lib/rsagent".to_string(),
        sync_interval_secs: 60,
    };
    let mut state = AgentRuntimeState::new(config);
    state.apply_sync_response(sample_default_response(heartbeat_interval_secs));
    state
}

fn synced_runtime_state_with_response(response: AgentSyncResponse) -> AgentRuntimeState {
    let config = AgentRuntimeConfig {
        nodemanage_sync_url: "http://127.0.0.1:3000/agent/sync".to_string(),
        agent_id: "agt-001".to_string(),
        node_id: None,
        data_dir: "/var/lib/rsagent".to_string(),
        sync_interval_secs: 60,
    };
    let mut state = AgentRuntimeState::new(config);
    state.apply_sync_response(response);
    state
}

fn sample_default_response(heartbeat_interval_secs: u64) -> AgentSyncResponse {
    sample_response_with_values(
        "node-001",
        "cfg-001",
        "dl-001",
        "http://victoriametrics:8428",
        heartbeat_interval_secs,
    )
}

fn sample_response(
    bound_node_id: &str,
    config_version: &str,
    data_link_id: &str,
    vm_base_url: &str,
    heartbeat_interval_secs: u64,
) -> AgentSyncResponse {
    sample_response_with_values(
        bound_node_id,
        config_version,
        data_link_id,
        vm_base_url,
        heartbeat_interval_secs,
    )
}

fn sample_response_with_values(
    bound_node_id: &str,
    config_version: &str,
    data_link_id: &str,
    vm_base_url: &str,
    heartbeat_interval_secs: u64,
) -> AgentSyncResponse {
    AgentSyncResponse {
        accepted: true,
        agent_id: "agt-001".to_string(),
        bound_node_id: bound_node_id.to_string(),
        binding_state: SyncBindingState::Bound,
        agent_run_mode: AgentRunMode::Active,
        config_version: config_version.to_string(),
        heartbeat_config: HeartbeatConfig {
            version: "hb-v1".to_string(),
            data_link_id: data_link_id.to_string(),
            vm_base_url: vm_base_url.to_string(),
            interval_secs: heartbeat_interval_secs,
        },
        job_manage_config: JobManageConfig {
            version: "jm-v1".to_string(),
            base_url: "http://job-manage".to_string(),
            task_filter_defaults: TaskFilterDefaults {
                states: vec!["queued".to_string(), "running".to_string()],
            },
        },
        sync_interval_secs: 30,
        task_sync_interval_secs: 10,
        rejection_reason: None,
    }
}
