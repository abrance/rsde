use std::{future::Future, pin::Pin};

use anyhow::{Result, anyhow};
use chrono::{Duration, TimeZone, Utc};
use nodemanage::{
    AgentRunMode, AgentSyncResponse, HeartbeatConfig, JobManageConfig, SyncBindingState,
    TaskFilterDefaults,
};
use rsagent::{
    clients::nodemanage::NodeManageSyncTransport,
    clients::victoria_metrics::VictoriaMetricsTransport,
    config::AgentRuntimeConfig,
    config_sync::run_sync_once,
    heartbeat::{
        HeartbeatDiagnostic, HeartbeatHealthTransition, HeartbeatReporter, HeartbeatSkipReason,
        HeartbeatTick, apply_sync_refresh, build_heartbeat_payload,
    },
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
fn heartbeat_payload_prefers_bound_node_identity_dimensions() {
    let mut state = AgentRuntimeState::new(AgentRuntimeConfig {
        nodemanage_sync_url: "http://127.0.0.1:3000/agent/sync".to_string(),
        agent_id: "agt-001".to_string(),
        node_id: Some("stale-node".to_string()),
        data_dir: "/var/lib/rsagent".to_string(),
        sync_interval_secs: 60,
    });
    let identity = sample_identity();
    let timestamp = Utc.with_ymd_and_hms(2025, 1, 2, 3, 4, 5).unwrap();

    state.apply_sync_response(sample_response(30));

    let payload = build_heartbeat_payload(&state, "agt-001", &identity, timestamp).unwrap();

    assert!(payload.contains("node_id=node-001"));
    assert!(!payload.contains("node_id=stale-node"));
    assert!(payload.contains("agent_id=agt-001"));
    assert!(payload.contains("agent_version=0.1.0"));
    assert!(payload.contains("data_link_id=dl-001"));
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
fn heartbeat_reporter_exposes_degraded_transition_diagnostic_after_write_failure() {
    let state = synced_runtime_state(60);
    let identity = sample_identity();
    let timestamp = Utc.with_ymd_and_hms(2025, 1, 2, 3, 4, 5).unwrap();
    let mut reporter = HeartbeatReporter::new(FailingTransport);

    let _ = reporter
        .tick(timestamp, &state, "agt-001", &identity)
        .unwrap_err();

    assert_eq!(
        reporter.take_diagnostic(),
        Some(HeartbeatDiagnostic {
            transition: HeartbeatHealthTransition::EnteredDegraded,
            node_id: Some("node-001".to_string()),
            config_version: Some("cfg-001".to_string()),
            reason: Some("temporary write failure".to_string()),
            retry_policy: Some(rsagent::runtime_coordinator::temporary_upstream_retry_policy(1)),
        })
    );
    assert_eq!(reporter.take_diagnostic(), None);
}

#[test]
fn heartbeat_reporter_classifies_temporary_write_failure_with_shared_retry_policy() {
    let state = synced_runtime_state(60);
    let identity = sample_identity();
    let timestamp = Utc.with_ymd_and_hms(2025, 1, 2, 3, 4, 5).unwrap();
    let mut reporter = HeartbeatReporter::new(FailingTransport);

    let _ = reporter
        .tick(timestamp, &state, "agt-001", &identity)
        .unwrap_err();

    assert_eq!(
        reporter.last_retry_policy(),
        Some(&rsagent::runtime_coordinator::temporary_upstream_retry_policy(1))
    );
    assert!(reporter.is_degraded());
}

#[test]
fn heartbeat_reporter_escalates_retry_backoff_on_repeated_temporary_write_failures() {
    let state = synced_runtime_state(60);
    let identity = sample_identity();
    let timestamp = Utc.with_ymd_and_hms(2025, 1, 2, 3, 4, 5).unwrap();
    let mut reporter = HeartbeatReporter::new(FailingTransport);

    let _ = reporter.tick(timestamp, &state, "agt-001", &identity);
    let _ = reporter.tick(
        timestamp + Duration::seconds(60),
        &state,
        "agt-001",
        &identity,
    );
    let _ = reporter.tick(
        timestamp + Duration::seconds(120),
        &state,
        "agt-001",
        &identity,
    );

    assert_eq!(
        reporter.last_retry_policy(),
        Some(&rsagent::runtime_coordinator::temporary_upstream_retry_policy(3))
    );
    assert!(reporter.is_degraded());
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

#[tokio::test]
async fn heartbeat_sync_refresh_resets_reporter_on_destination_change() {
    let mut state = synced_runtime_state(60);
    let identity = sample_identity();
    let start = Utc.with_ymd_and_hms(2025, 1, 2, 3, 4, 5).unwrap();
    let mut reporter = HeartbeatReporter::new(RecordingTransport::default());

    reporter.tick(start, &state, "agt-001", &identity).unwrap();

    let mut sync_transport = StaticSyncTransport::accepted(sample_response_with_heartbeat(
        "cfg-002",
        "dl-002",
        "http://victoriametrics-alt:8428",
        60,
    ));

    let outcome = run_sync_once(&mut state, &identity, &mut sync_transport)
        .await
        .unwrap();

    assert!(outcome.heartbeat_reset_required);
    assert!(!outcome.heartbeat_interval_changed);

    apply_sync_refresh(&mut reporter, &mut state, outcome.heartbeat_reset_required);

    let next = reporter
        .tick(start + Duration::seconds(1), &state, "agt-001", &identity)
        .unwrap();

    assert!(matches!(next, HeartbeatTick::Sent { .. }));
    let transport = reporter.into_transport();
    assert_eq!(transport.requests.len(), 2);
    assert_eq!(
        transport.requests[1].endpoint,
        "http://victoriametrics-alt:8428/api/v2/write"
    );
    assert!(
        transport.requests[1]
            .payload
            .contains("data_link_id=dl-002")
    );
    assert_eq!(
        state
            .effective_config()
            .unwrap()
            .heartbeat_config
            .data_link_id,
        "dl-002"
    );
}

#[tokio::test]
async fn heartbeat_sync_refresh_resets_reporter_on_interval_change() {
    let mut state = synced_runtime_state(60);
    let identity = sample_identity();
    let start = Utc.with_ymd_and_hms(2025, 1, 2, 3, 4, 5).unwrap();
    let mut reporter = HeartbeatReporter::new(RecordingTransport::default());

    reporter.tick(start, &state, "agt-001", &identity).unwrap();

    let mut sync_transport = StaticSyncTransport::accepted(sample_response_with_heartbeat(
        "cfg-002",
        "dl-001",
        "http://victoriametrics:8428",
        300,
    ));

    let outcome = run_sync_once(&mut state, &identity, &mut sync_transport)
        .await
        .unwrap();

    assert!(outcome.heartbeat_reset_required);
    assert!(outcome.heartbeat_interval_changed);

    apply_sync_refresh(&mut reporter, &mut state, outcome.heartbeat_reset_required);

    let next = reporter
        .tick(start + Duration::seconds(1), &state, "agt-001", &identity)
        .unwrap();

    assert!(matches!(next, HeartbeatTick::Sent { .. }));
    assert_eq!(
        state
            .effective_config()
            .unwrap()
            .heartbeat_config
            .interval_secs,
        300
    );
    assert!(!reporter.is_degraded());
}

#[test]
fn heartbeat_reporter_clears_degraded_after_later_success_without_corrupting_runtime_state() {
    let state = synced_runtime_state(60);
    let identity = sample_identity();
    let expected_config = state.effective_config().cloned();
    let expected_node_id = state.local_node_id().map(ToString::to_string);
    let timestamp = Utc.with_ymd_and_hms(2025, 1, 2, 3, 4, 5).unwrap();
    let mut reporter = HeartbeatReporter::new(FlakyTransport::fail_then_succeed(1));

    let error = reporter
        .tick(timestamp, &state, "agt-001", &identity)
        .unwrap_err();

    assert!(error.to_string().contains("temporary write failure"));
    assert!(reporter.is_degraded());
    assert_eq!(state.effective_config(), expected_config.as_ref());
    assert_eq!(state.local_node_id(), expected_node_id.as_deref());
    assert!(!state.is_degraded());

    let next = reporter
        .tick(
            timestamp + Duration::seconds(60),
            &state,
            "agt-001",
            &identity,
        )
        .unwrap();

    assert!(matches!(next, HeartbeatTick::Sent { .. }));
    assert!(!reporter.is_degraded());
    assert_eq!(state.effective_config(), expected_config.as_ref());
    assert_eq!(state.local_node_id(), expected_node_id.as_deref());
    assert!(!state.is_degraded());
}

#[test]
fn heartbeat_reporter_exposes_recovery_transition_diagnostic_after_later_success() {
    let state = synced_runtime_state(60);
    let identity = sample_identity();
    let timestamp = Utc.with_ymd_and_hms(2025, 1, 2, 3, 4, 5).unwrap();
    let mut reporter = HeartbeatReporter::new(FlakyTransport::fail_then_succeed(1));

    let _ = reporter.tick(timestamp, &state, "agt-001", &identity);
    let _ = reporter
        .tick(
            timestamp + Duration::seconds(60),
            &state,
            "agt-001",
            &identity,
        )
        .unwrap();

    assert_eq!(
        reporter.last_diagnostic(),
        Some(&HeartbeatDiagnostic {
            transition: HeartbeatHealthTransition::Recovered,
            node_id: Some("node-001".to_string()),
            config_version: Some("cfg-001".to_string()),
            reason: None,
            retry_policy: None,
        })
    );
}

#[test]
fn heartbeat_reporter_take_diagnostic_is_one_shot_after_recovery_success_path() {
    let state = synced_runtime_state(60);
    let identity = sample_identity();
    let timestamp = Utc.with_ymd_and_hms(2025, 1, 2, 3, 4, 5).unwrap();
    let mut reporter = HeartbeatReporter::new(FlakyTransport::fail_then_succeed(1));

    let _ = reporter.tick(timestamp, &state, "agt-001", &identity);
    let _ = reporter
        .tick(
            timestamp + Duration::seconds(60),
            &state,
            "agt-001",
            &identity,
        )
        .unwrap();

    assert_eq!(
        reporter.take_diagnostic(),
        Some(HeartbeatDiagnostic {
            transition: HeartbeatHealthTransition::Recovered,
            node_id: Some("node-001".to_string()),
            config_version: Some("cfg-001".to_string()),
            reason: None,
            retry_policy: None,
        })
    );
    assert_eq!(reporter.take_diagnostic(), None);
}

#[test]
fn heartbeat_reporter_clears_recovery_diagnostic_on_interval_not_elapsed_skip() {
    let state = synced_runtime_state(60);
    let identity = sample_identity();
    let timestamp = Utc.with_ymd_and_hms(2025, 1, 2, 3, 4, 5).unwrap();
    let mut reporter = HeartbeatReporter::new(FlakyTransport::fail_then_succeed(1));

    let _ = reporter.tick(timestamp, &state, "agt-001", &identity);
    let _ = reporter
        .tick(
            timestamp + Duration::seconds(60),
            &state,
            "agt-001",
            &identity,
        )
        .unwrap();
    assert!(matches!(
        reporter.last_diagnostic(),
        Some(HeartbeatDiagnostic {
            transition: HeartbeatHealthTransition::Recovered,
            ..
        })
    ));

    let skipped = reporter
        .tick(
            timestamp + Duration::seconds(61),
            &state,
            "agt-001",
            &identity,
        )
        .unwrap();

    assert_eq!(
        skipped,
        HeartbeatTick::Skipped {
            reason: HeartbeatSkipReason::IntervalNotElapsed,
        }
    );
    assert_eq!(reporter.last_diagnostic(), None);
}

#[test]
fn heartbeat_reporter_clears_recovery_diagnostic_on_missing_config_skip() {
    let state = synced_runtime_state(60);
    let missing_config_state = AgentRuntimeState::new(AgentRuntimeConfig {
        nodemanage_sync_url: "http://127.0.0.1:3000/agent/sync".to_string(),
        agent_id: "agt-001".to_string(),
        node_id: None,
        data_dir: "/var/lib/rsagent".to_string(),
        sync_interval_secs: 60,
    });
    let identity = sample_identity();
    let timestamp = Utc.with_ymd_and_hms(2025, 1, 2, 3, 4, 5).unwrap();
    let mut reporter = HeartbeatReporter::new(FlakyTransport::fail_then_succeed(1));

    let _ = reporter.tick(timestamp, &state, "agt-001", &identity);
    let _ = reporter
        .tick(
            timestamp + Duration::seconds(60),
            &state,
            "agt-001",
            &identity,
        )
        .unwrap();
    assert!(matches!(
        reporter.last_diagnostic(),
        Some(HeartbeatDiagnostic {
            transition: HeartbeatHealthTransition::Recovered,
            ..
        })
    ));

    let skipped = reporter
        .tick(
            timestamp + Duration::seconds(120),
            &missing_config_state,
            "agt-001",
            &identity,
        )
        .unwrap();

    assert_eq!(
        skipped,
        HeartbeatTick::Skipped {
            reason: HeartbeatSkipReason::MissingConfig,
        }
    );
    assert_eq!(reporter.last_diagnostic(), None);
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
struct FlakyTransport {
    failures_remaining: usize,
}

impl FlakyTransport {
    fn fail_then_succeed(failures_remaining: usize) -> Self {
        Self { failures_remaining }
    }
}

impl VictoriaMetricsTransport for FlakyTransport {
    fn post_line_protocol(&mut self, _endpoint: &str, _payload: &str) -> Result<()> {
        if self.failures_remaining > 0 {
            self.failures_remaining -= 1;
            return Err(anyhow!("temporary write failure"));
        }

        Ok(())
    }
}

#[derive(Debug)]
struct StaticSyncTransport {
    response: AgentSyncResponse,
}

impl StaticSyncTransport {
    fn accepted(response: AgentSyncResponse) -> Self {
        Self { response }
    }
}

impl NodeManageSyncTransport for StaticSyncTransport {
    type SyncFuture<'a>
        = Pin<Box<dyn Future<Output = Result<AgentSyncResponse>> + Send + 'a>>
    where
        Self: 'a;

    fn sync<'a>(
        &'a mut self,
        _endpoint: &'a str,
        _request: &'a nodemanage::AgentSyncRequest,
    ) -> Self::SyncFuture<'a> {
        let response = self.response.clone();
        Box::pin(async move { Ok(response) })
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
    state.apply_sync_response(sample_response(heartbeat_interval_secs));
    state
}

fn sample_response(heartbeat_interval_secs: u64) -> AgentSyncResponse {
    sample_response_with_heartbeat(
        "cfg-001",
        "dl-001",
        "http://victoriametrics:8428",
        heartbeat_interval_secs,
    )
}

fn sample_response_with_heartbeat(
    config_version: &str,
    data_link_id: &str,
    vm_base_url: &str,
    heartbeat_interval_secs: u64,
) -> AgentSyncResponse {
    AgentSyncResponse {
        accepted: true,
        agent_id: "agt-001".to_string(),
        bound_node_id: "node-001".to_string(),
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
