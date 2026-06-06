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
    registration::{AgentIdentity, AgentRuntimeState},
};

#[test]
fn placeholder_until_config_sync_module_exists() {
    let _ = sample_runtime_state();
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
    assert!(!state.is_degraded());
    assert_eq!(state.last_sync_error(), None);
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
    assert_eq!(state.config_version(), Some("cfg-v1"));
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
    assert!(!state.is_degraded());
    assert_eq!(state.binding_state(), Some(&SyncBindingState::Conflict));
    assert_eq!(state.last_sync_error(), Some("binding conflict"));
}

#[derive(Debug)]
struct RecordingTransport {
    response: Option<AgentSyncResponse>,
    error: Option<String>,
    requests: Arc<Mutex<Vec<nodemanage::AgentSyncRequest>>>,
}

impl RecordingTransport {
    fn success(response: AgentSyncResponse) -> Self {
        Self {
            response: Some(response),
            error: None,
            requests: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn temporary_failure(message: &str) -> Self {
        Self {
            response: None,
            error: Some(message.to_string()),
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
        let response = self.response.clone();
        let error = self.error.clone();
        Box::pin(async move {
            match (response, error) {
                (Some(response), None) => Ok(response),
                (None, Some(error)) => Err(anyhow!(error)),
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
