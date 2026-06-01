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
