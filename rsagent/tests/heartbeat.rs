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
        HeartbeatDiagnosticTransition, HeartbeatReporter, HeartbeatSkipReason, HeartbeatTick,
        build_heartbeat_payload,
    },
    registration::{AgentIdentity, AgentRuntimeState},
    runtime_coordinator::TransientRetryPolicy,
};
use std::{
    future::Future,
    pin::Pin,
    time::{SystemTime, UNIX_EPOCH},
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
fn heartbeat_reporter_successful_write_recovers_without_manual_reset() {
    let state = synced_runtime_state(60);
    let identity = sample_identity();
    let timestamp = Utc.with_ymd_and_hms(2025, 1, 2, 3, 4, 5).unwrap();
    let mut reporter = HeartbeatReporter::with_retry_policy(
        SometimesFailingTransport::new(vec![Err(anyhow!("temporary write failure")), Ok(())]),
        TransientRetryPolicy::new(vec![]),
    );

    let _ = reporter.tick(timestamp, &state, "agt-001", &identity);
    assert!(reporter.is_degraded());

    let recovered = reporter
        .tick(
            timestamp + Duration::seconds(61),
            &state,
            "agt-001",
            &identity,
        )
        .unwrap();

    assert!(matches!(recovered, HeartbeatTick::Sent { .. }));
    assert!(!reporter.is_degraded());
}

#[test]
fn heartbeat_reporter_records_degraded_and_recovered_diagnostics_across_network_flap() {
    let state = synced_runtime_state(60);
    let identity = sample_identity();
    let timestamp = Utc.with_ymd_and_hms(2025, 1, 2, 3, 4, 5).unwrap();
    let mut reporter = HeartbeatReporter::with_retry_policy(
        SometimesFailingTransport::new(vec![Err(anyhow!("temporary write failure")), Ok(())]),
        TransientRetryPolicy::new(vec![]),
    );

    let _ = reporter.tick(timestamp, &state, "agt-001", &identity);
    let degraded = reporter
        .last_diagnostic()
        .expect("diagnostic after degraded heartbeat write")
        .clone();
    let _ = reporter.tick(
        timestamp + Duration::seconds(61),
        &state,
        "agt-001",
        &identity,
    );
    let recovered = reporter
        .last_diagnostic()
        .expect("diagnostic after recovered heartbeat write")
        .clone();

    assert_eq!(degraded.transition, HeartbeatDiagnosticTransition::Degraded);
    assert!(degraded.detail.contains("temporary write failure"));
    assert_eq!(
        recovered.transition,
        HeartbeatDiagnosticTransition::Recovered
    );
    assert!(recovered.detail.contains("recovered"));
}

#[test]
fn heartbeat_reporter_retries_transient_write_failure_within_same_tick() {
    let state = synced_runtime_state(60);
    let identity = sample_identity();
    let timestamp = Utc.with_ymd_and_hms(2025, 1, 2, 3, 4, 5).unwrap();
    let mut reporter = HeartbeatReporter::with_retry_policy(
        SometimesFailingTransport::new(vec![Err(anyhow!("temporary write failure")), Ok(())]),
        TransientRetryPolicy::new(vec![std::time::Duration::ZERO]),
    );

    let tick = reporter
        .tick(timestamp, &state, "agt-001", &identity)
        .unwrap();

    assert!(matches!(tick, HeartbeatTick::Sent { .. }));
    assert_eq!(reporter.into_transport().attempts, 2);
}

#[tokio::test(flavor = "current_thread")]
async fn heartbeat_reporter_async_tick_offloads_blocking_work() {
    let state = synced_runtime_state(60);
    let identity = sample_identity();
    let timestamp = Utc.with_ymd_and_hms(2025, 1, 2, 3, 4, 5).unwrap();
    let reporter = HeartbeatReporter::with_retry_policy(
        SometimesFailingTransport::new(vec![Err(anyhow!("temporary write failure")), Ok(())]),
        TransientRetryPolicy::new(vec![std::time::Duration::from_millis(1)]),
    );

    let (yielded, tick) = tokio::join!(
        async {
            tokio::task::yield_now().await;
            "yielded"
        },
        reporter.tick_async(timestamp, state, "agt-001".to_string(), identity)
    );

    assert_eq!(yielded, "yielded");
    assert!(matches!(tick.1.unwrap(), HeartbeatTick::Sent { .. }));
}

#[tokio::test]
async fn heartbeat_reporter_healthy_sync_recovers_after_write_failure_without_config_change() {
    let mut state = AgentRuntimeState::new(AgentRuntimeConfig {
        nodemanage_sync_url: "http://127.0.0.1:3000/agent/sync".to_string(),
        agent_id: "agt-001".to_string(),
        node_id: None,
        data_dir: writable_data_dir("heartbeat-sync-recovery"),
        sync_interval_secs: 60,
    });
    state.apply_sync_response(sample_response(60));
    let identity = sample_identity();
    let timestamp = Utc.with_ymd_and_hms(2025, 1, 2, 3, 4, 5).unwrap();
    let mut reporter = HeartbeatReporter::new(SometimesFailingTransport::new(vec![Err(anyhow!(
        "temporary write failure",
    ))]));

    let _ = reporter.tick(timestamp, &state, "agt-001", &identity);
    assert!(reporter.is_degraded());

    let mut sync_transport = RecordingSyncTransport::success(sample_response(60));
    let outcome = run_sync_once(&mut state, &identity, &mut sync_transport)
        .await
        .unwrap();
    assert!(!outcome.heartbeat_reset_required);

    rsagent::heartbeat::reconcile_after_sync(&mut reporter, &outcome, &state);

    assert!(!reporter.is_degraded());
    let diagnostic = reporter
        .last_diagnostic()
        .expect("diagnostic after sync-based recovery");
    assert_eq!(
        diagnostic.transition,
        HeartbeatDiagnosticTransition::Recovered
    );
    assert!(diagnostic.detail.contains("sync"));
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
struct SometimesFailingTransport {
    outcomes: std::vec::IntoIter<Result<()>>,
    attempts: usize,
}

impl SometimesFailingTransport {
    fn new(outcomes: Vec<Result<()>>) -> Self {
        Self {
            outcomes: outcomes.into_iter(),
            attempts: 0,
        }
    }
}

impl VictoriaMetricsTransport for SometimesFailingTransport {
    fn post_line_protocol(&mut self, _endpoint: &str, _payload: &str) -> Result<()> {
        self.attempts += 1;
        self.outcomes
            .next()
            .unwrap_or_else(|| Err(anyhow!("missing test outcome")))
    }
}

#[derive(Debug)]
struct RecordingSyncTransport {
    response: AgentSyncResponse,
}

impl RecordingSyncTransport {
    fn success(response: AgentSyncResponse) -> Self {
        Self { response }
    }
}

impl NodeManageSyncTransport for RecordingSyncTransport {
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

fn writable_data_dir(prefix: &str) -> String {
    std::env::temp_dir()
        .join(format!(
            "rsagent-{prefix}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("unix epoch")
                .as_nanos()
        ))
        .to_string_lossy()
        .into_owned()
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
    AgentSyncResponse {
        accepted: true,
        agent_id: "agt-001".to_string(),
        bound_node_id: "node-001".to_string(),
        binding_state: SyncBindingState::Bound,
        agent_run_mode: AgentRunMode::Active,
        config_version: "cfg-001".to_string(),
        heartbeat_config: HeartbeatConfig {
            version: "hb-v1".to_string(),
            data_link_id: "dl-001".to_string(),
            vm_base_url: "http://victoriametrics:8428".to_string(),
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
