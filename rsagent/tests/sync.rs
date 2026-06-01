use std::{
    future::Future,
    io::{BufRead, BufReader, Read, Write},
    net::TcpListener,
    pin::Pin,
    sync::{Arc, Mutex},
    thread,
};

use rsagent::config::{AgentConfig, AgentRuntimeConfig};
use rsagent::{
    bootstrap::bootstrap_runtime_state,
    clients::nodemanage::{NodeManageSyncClient, NodeManageSyncTransport},
    registration::{AgentIdentity, AgentRuntimeState},
};

use anyhow::Result;
use chrono::{TimeZone, Utc};
use nodemanage::{
    AgentRunMode, AgentSyncResponse, HeartbeatConfig, JobManageConfig, MemoryNodeRepository,
    NodeManager, NoopRsAgentInstaller, SyncBindingState, TaskFilterDefaults,
};
use serde_json::{Value, json};

#[test]
fn test_agent_config_placeholder_exists() {
    let _config = AgentConfig::default();
}

#[test]
fn test_agent_config_shape_deserializes_required_fields() {
    let raw = r#"
        nodemanage_sync_url = "http://127.0.0.1:3000"
        agent_id = "agt-001"
    "#;
    let cfg: AgentRuntimeConfig = toml::from_str(raw).unwrap();
    assert_eq!(cfg.nodemanage_sync_url, "http://127.0.0.1:3000");
    assert_eq!(cfg.agent_id, "agt-001");
    assert!(cfg.node_id.is_none());
}

#[test]
fn test_agent_config_shape_optional_node_id() {
    let raw = r#"
        nodemanage_sync_url = "http://127.0.0.1:3000"
        agent_id = "agt-001"
        node_id = "node-001"
    "#;
    let cfg: AgentRuntimeConfig = toml::from_str(raw).unwrap();
    assert_eq!(cfg.node_id.as_deref(), Some("node-001"));
}

#[test]
fn test_agent_config_shape_defaults_for_runtime_knobs() {
    let raw = r#"
        nodemanage_sync_url = "http://127.0.0.1:3000"
        agent_id = "agt-001"
    "#;
    let cfg: AgentRuntimeConfig = toml::from_str(raw).unwrap();
    assert!(!cfg.data_dir.is_empty());
    assert!(cfg.sync_interval_secs > 0);
}

#[test]
fn test_sync_request_serializes_required_fields() {
    let config = AgentRuntimeConfig {
        nodemanage_sync_url: "http://127.0.0.1:3000/agent/sync".to_string(),
        agent_id: "agt-001".to_string(),
        node_id: Some("node-001".to_string()),
        data_dir: "/var/lib/rsagent".to_string(),
        sync_interval_secs: 60,
    };
    let identity = sample_identity();

    let request =
        NodeManageSyncClient::build_request(&config, &identity, Some("cfg-001".to_string()));
    let json = serde_json::to_value(&request).unwrap();

    assert_eq!(json["agent_id"], "agt-001");
    assert_eq!(json["node_id"], "node-001");
    assert_eq!(json["agent_version"], "0.1.0");
    assert_eq!(json["hostname"], "worker-01");
    assert_eq!(json["os_family"], "linux");
    assert_eq!(json["os_distribution"], "ubuntu");
    assert_eq!(json["arch"], "x86_64");
    assert_eq!(json["capabilities"], serde_json::json!(["sync", "jobs"]));
    assert_eq!(json["started_at"], "2025-01-02T03:04:05Z");
    assert_eq!(json["config_version"], "cfg-001");
}

#[test]
fn test_sync_request_omits_optional_fields_when_unknown() {
    let config = AgentRuntimeConfig {
        agent_id: "agt-001".to_string(),
        ..AgentRuntimeConfig::default()
    };
    let identity = sample_identity();

    let request = NodeManageSyncClient::build_request(&config, &identity, None);
    let json = serde_json::to_value(&request).unwrap();

    assert!(json.get("node_id").is_none());
    assert!(json.get("config_version").is_none());
}

#[test]
fn test_sync_response_parses_protocol_payload() {
    let payload = serde_json::to_string(&sample_response(
        true,
        AgentRunMode::Active,
        SyncBindingState::Bound,
        "node-001",
        "cfg-001",
        None,
    ))
    .unwrap();

    let response = NodeManageSyncClient::parse_response(&payload).unwrap();

    assert!(response.accepted);
    assert_eq!(response.agent_id, "agt-001");
    assert_eq!(response.bound_node_id, "node-001");
    assert_eq!(response.binding_state, SyncBindingState::Bound);
    assert_eq!(response.agent_run_mode, AgentRunMode::Active);
    assert_eq!(response.config_version, "cfg-001");
    assert_eq!(response.heartbeat_config.interval_secs, 15);
    assert_eq!(response.job_manage_config.base_url, "http://job-manage");
    assert_eq!(response.task_sync_interval_secs, 20);
}

#[test]
fn test_runtime_state_applies_active_sync_response() {
    let config = sample_runtime_config(None);
    let mut state = AgentRuntimeState::new(config);

    state.apply_sync_response(sample_response(
        true,
        AgentRunMode::Active,
        SyncBindingState::Bound,
        "node-001",
        "cfg-001",
        None,
    ));

    assert_eq!(state.local_node_id(), Some("node-001"));
    assert_eq!(state.config_version(), Some("cfg-001"));
    assert!(state.loops_enabled());
    assert!(!state.is_degraded());

    let effective = state.effective_config().unwrap();
    assert_eq!(effective.sync_interval_secs, 10);
    assert_eq!(effective.task_sync_interval_secs, 20);
    assert_eq!(effective.heartbeat_config.interval_secs, 15);
}

#[tokio::test]
async fn test_runtime_state_accepts_real_nodemanage_sync_response() {
    let config = sample_runtime_config(Some("node-real-sync"));
    let identity = sample_identity();
    let request = NodeManageSyncClient::build_request(&config, &identity, None);
    let manager = NodeManager::new(MemoryNodeRepository::default(), NoopRsAgentInstaller);
    let response = manager.sync_agent(request).await.unwrap();
    let mut state = AgentRuntimeState::new(config);

    state.apply_sync_response(response.clone());

    assert_eq!(state.local_node_id(), Some("node-real-sync"));
    assert_eq!(state.binding_state(), Some(&SyncBindingState::Bound));
    assert!(state.loops_enabled());

    let effective = state.effective_config().unwrap();
    assert_eq!(effective.heartbeat_config, response.heartbeat_config);
    assert_eq!(effective.job_manage_config, response.job_manage_config);
    assert_eq!(effective.sync_interval_secs, response.sync_interval_secs);
    assert_eq!(
        effective.task_sync_interval_secs,
        response.task_sync_interval_secs
    );
}

#[test]
fn test_runtime_state_applies_idle_sync_response_and_disables_loops() {
    let config = sample_runtime_config(None);
    let mut state = AgentRuntimeState::new(config);

    state.apply_sync_response(sample_response(
        true,
        AgentRunMode::Idle,
        SyncBindingState::Bound,
        "node-001",
        "cfg-002",
        None,
    ));

    assert_eq!(state.local_node_id(), Some("node-001"));
    assert_eq!(state.config_version(), Some("cfg-002"));
    assert!(!state.loops_enabled());
    assert!(!state.is_degraded());
}

#[test]
fn test_runtime_state_conflict_rejection_keeps_existing_binding_and_disables_loops() {
    let config = sample_runtime_config(Some("node-existing"));
    let mut state = AgentRuntimeState::new(config);
    state.apply_sync_response(sample_response(
        true,
        AgentRunMode::Active,
        SyncBindingState::Bound,
        "node-existing",
        "cfg-stable",
        None,
    ));

    state.apply_sync_response(sample_response(
        false,
        AgentRunMode::Idle,
        SyncBindingState::Conflict,
        "node-other",
        "cfg-other",
        Some("binding conflict"),
    ));

    assert_eq!(state.local_node_id(), Some("node-existing"));
    assert_eq!(state.config_version(), Some("cfg-stable"));
    assert_eq!(state.binding_state(), Some(&SyncBindingState::Conflict));
    assert!(!state.loops_enabled());
}

#[test]
fn test_runtime_state_unbound_rejection_keeps_process_alive_without_loops() {
    let config = sample_runtime_config(None);
    let mut state = AgentRuntimeState::new(config);

    state.apply_sync_response(sample_response(
        false,
        AgentRunMode::Idle,
        SyncBindingState::Unbound,
        "",
        "cfg-unbound",
        Some("agent not yet bound"),
    ));

    assert!(state.process_alive());
    assert!(!state.loops_enabled());
    assert_eq!(state.binding_state(), Some(&SyncBindingState::Unbound));
    assert_eq!(state.local_node_id(), None);
}

#[test]
fn test_runtime_state_temporary_sync_failure_keeps_last_good_config_and_marks_degraded() {
    let config = sample_runtime_config(Some("node-001"));
    let mut state = AgentRuntimeState::new(config);
    state.apply_sync_response(sample_response(
        true,
        AgentRunMode::Active,
        SyncBindingState::Bound,
        "node-001",
        "cfg-healthy",
        None,
    ));

    state.record_temporary_sync_failure("timeout".to_string());

    assert_eq!(state.local_node_id(), Some("node-001"));
    assert_eq!(state.config_version(), Some("cfg-healthy"));
    assert!(state.is_degraded());
    assert!(state.loops_enabled());

    let effective = state.effective_config().unwrap();
    assert_eq!(effective.config_version, "cfg-healthy");
    assert_eq!(effective.task_sync_interval_secs, 20);
}

#[test]
fn test_runtime_state_temporary_sync_failure_without_last_good_config_keeps_loops_disabled() {
    let config = sample_runtime_config(None);
    let mut state = AgentRuntimeState::new(config);

    state.record_temporary_sync_failure("timeout".to_string());

    assert!(!state.loops_enabled());
    assert!(!state.is_degraded());
    assert!(state.effective_config().is_none());
}

#[tokio::test]
async fn test_bootstrap_runtime_state_performs_initial_sync_and_enables_loops() {
    let config = sample_runtime_config(None);
    let identity = sample_identity();
    let response = sample_response(
        true,
        AgentRunMode::Active,
        SyncBindingState::Bound,
        "node-bootstrapped",
        "cfg-bootstrap",
        None,
    );
    let mut transport = RecordingNodeManageTransport::new(response);

    let (state, returned_identity) =
        bootstrap_runtime_state(config.clone(), identity.clone(), &mut transport)
            .await
            .unwrap();

    assert_eq!(returned_identity, identity);
    assert_eq!(state.local_node_id(), Some("node-bootstrapped"));
    assert_eq!(state.config_version(), Some("cfg-bootstrap"));
    assert!(state.loops_enabled());
    let effective = state.effective_config().unwrap();
    assert_eq!(effective.config_version, "cfg-bootstrap");
    assert_eq!(effective.task_sync_interval_secs, 20);

    let requests = transport.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].endpoint, config.nodemanage_sync_url);
    assert_eq!(requests[0].request.agent_id, config.agent_id);
    assert_eq!(requests[0].request.config_version, None);
}

#[tokio::test]
async fn test_bootstrap_runtime_state_populates_effective_config_and_node_binding() {
    let config = sample_runtime_config(Some("node-existing"));
    let identity = sample_identity();
    let response = sample_response(
        true,
        AgentRunMode::Idle,
        SyncBindingState::Bound,
        "node-existing",
        "cfg-after-sync",
        None,
    );
    let mut transport = RecordingNodeManageTransport::new(response);

    let (state, _) = bootstrap_runtime_state(config, identity, &mut transport)
        .await
        .unwrap();

    assert_eq!(state.local_node_id(), Some("node-existing"));
    assert_eq!(state.config_version(), Some("cfg-after-sync"));
    assert!(!state.loops_enabled());

    let effective = state.effective_config().unwrap();
    assert_eq!(effective.heartbeat_config.interval_secs, 15);
    assert_eq!(effective.job_manage_config.base_url, "http://job-manage");
    assert_eq!(effective.sync_interval_secs, 10);
}

#[tokio::test]
async fn test_reqwest_nodemanage_transport_posts_sync_request_and_parses_envelope() {
    let config = sample_runtime_config(Some("node-001"));
    let identity = sample_identity();
    let response = sample_response(
        true,
        AgentRunMode::Active,
        SyncBindingState::Bound,
        "node-001",
        "cfg-http",
        None,
    );
    let expected_request =
        NodeManageSyncClient::build_request(&config, &identity, Some("cfg-previous".to_string()));
    let server = TestHttpServer::spawn(ExpectedHttpRequest {
        method: "POST",
        target: "/agent/sync",
        body: serde_json::to_value(&expected_request).unwrap(),
        response_body: json!({
            "success": true,
            "data": response,
            "error": null
        })
        .to_string(),
    });
    let client = NodeManageSyncClient::new(format!("{}/agent/sync", server.base_url()));
    let mut transport = rsagent::clients::nodemanage::ReqwestNodeManageSyncTransport::default();

    let actual = client
        .sync(&mut transport, &expected_request)
        .await
        .unwrap();

    assert_eq!(actual.agent_id, "agt-001");
    assert_eq!(actual.bound_node_id, "node-001");
    assert_eq!(actual.config_version, "cfg-http");

    server.finish();
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RecordedSyncRequest {
    endpoint: String,
    request: nodemanage::AgentSyncRequest,
}

#[derive(Debug, Clone)]
struct RecordingNodeManageTransport {
    response: AgentSyncResponse,
    requests: Arc<Mutex<Vec<RecordedSyncRequest>>>,
}

impl RecordingNodeManageTransport {
    fn new(response: AgentSyncResponse) -> Self {
        Self {
            response,
            requests: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn requests(&self) -> Vec<RecordedSyncRequest> {
        self.requests.lock().expect("sync requests lock").clone()
    }
}

impl NodeManageSyncTransport for RecordingNodeManageTransport {
    type SyncFuture<'a>
        = Pin<Box<dyn Future<Output = Result<AgentSyncResponse>> + Send + 'a>>
    where
        Self: 'a;

    fn sync<'a>(
        &'a mut self,
        endpoint: &'a str,
        request: &'a nodemanage::AgentSyncRequest,
    ) -> Self::SyncFuture<'a> {
        self.requests
            .lock()
            .expect("sync requests lock")
            .push(RecordedSyncRequest {
                endpoint: endpoint.to_string(),
                request: request.clone(),
            });
        let response = self.response.clone();

        Box::pin(async move { Ok(response) })
    }
}

#[derive(Debug)]
struct ExpectedHttpRequest {
    method: &'static str,
    target: &'static str,
    body: Value,
    response_body: String,
}

#[derive(Debug)]
struct TestHttpServer {
    base_url: String,
    handle: Option<thread::JoinHandle<()>>,
}

impl TestHttpServer {
    fn spawn(expected: ExpectedHttpRequest) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
        let address = listener.local_addr().expect("server local addr");
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept request");
            let mut reader = BufReader::new(stream.try_clone().expect("clone stream"));

            let mut request_line = String::new();
            reader
                .read_line(&mut request_line)
                .expect("read request line");
            let request_line = request_line.trim_end();
            let mut parts = request_line.split_whitespace();
            assert_eq!(parts.next().expect("method"), expected.method);
            assert_eq!(parts.next().expect("target"), expected.target);

            let mut content_length = 0usize;
            loop {
                let mut header = String::new();
                reader.read_line(&mut header).expect("read header");
                let header = header.trim_end();
                if header.is_empty() {
                    break;
                }
                if let Some((name, value)) = header.split_once(':')
                    && name.eq_ignore_ascii_case("content-length")
                {
                    content_length = value.trim().parse().expect("content length");
                }
            }

            let mut body = vec![0; content_length];
            reader.read_exact(&mut body).expect("read body");
            let actual_body: Value = serde_json::from_slice(&body).expect("json body");
            assert_eq!(actual_body, expected.body);

            let response = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                expected.response_body.len(),
                expected.response_body
            );
            stream
                .write_all(response.as_bytes())
                .expect("write response");
            stream.flush().expect("flush response");
        });

        Self {
            base_url: format!("http://{address}"),
            handle: Some(handle),
        }
    }

    fn base_url(&self) -> &str {
        &self.base_url
    }

    fn finish(mut self) {
        self.handle
            .take()
            .expect("server thread")
            .join()
            .expect("join server thread");
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
        Utc.with_ymd_and_hms(2025, 1, 2, 3, 4, 5).unwrap(),
    )
}

fn sample_runtime_config(node_id: Option<&str>) -> AgentRuntimeConfig {
    AgentRuntimeConfig {
        nodemanage_sync_url: "http://127.0.0.1:3000/agent/sync".to_string(),
        agent_id: "agt-001".to_string(),
        node_id: node_id.map(ToString::to_string),
        data_dir: "/var/lib/rsagent".to_string(),
        sync_interval_secs: 60,
    }
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
                states: vec!["pending".to_string(), "running".to_string()],
            },
        },
        sync_interval_secs: 10,
        task_sync_interval_secs: 20,
        rejection_reason: rejection_reason.map(ToString::to_string),
    }
}
