use std::{
    fs,
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{anyhow, Result};
use chrono::{TimeZone, Utc};
use nodemanage::{
    AgentRunMode, AgentSyncResponse, HeartbeatConfig, JobManageConfig, SyncBindingState,
    TaskFilterDefaults,
};
use rsagent::{
    clients::nodemanage::NodeManageSyncTransport,
    config::AgentRuntimeConfig,
    config_sync::{ConfigSyncTransition, diagnose_sync_transition},
    registration::{AgentIdentity, AgentRuntimeState, RuntimeSyncState},
    runtime_coordinator::TransientRetryPolicy,
};

#[test]
fn placeholder_until_config_sync_module_exists() {
    let _ = sample_runtime_state();
}

#[tokio::test]
async fn run_sync_once_first_accepted_sync_marks_loop_intervals_changed_from_bootstrap_defaults() {
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
    assert!(outcome.heartbeat_reset_required);
    assert!(outcome.sync_interval_changed);
    assert!(outcome.task_sync_interval_changed);
    assert!(outcome.heartbeat_interval_changed);
    assert!(state.loops_enabled());
    assert_eq!(state.config_version(), Some("cfg-v1"));
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
async fn run_sync_once_emits_degraded_transition_diagnostic_for_last_good_temporary_failure() {
    let mut state = sample_runtime_state();
    state.apply_sync_response(sample_response(
        true,
        AgentRunMode::Active,
        SyncBindingState::Bound,
        "node-001",
        "cfg-v1",
        None,
    ));
    let previous = state.clone();
    let mut transport = RecordingTransport::temporary_failure("timeout");

    let outcome =
        rsagent::config_sync::run_sync_once(&mut state, &sample_identity(), &mut transport)
            .await
            .unwrap();
    let diagnostic = diagnose_sync_transition(&previous, &state, &outcome);

    assert_eq!(diagnostic.transition, ConfigSyncTransition::Degraded);
    assert_eq!(diagnostic.sync_state, RuntimeSyncState::TemporaryFailure);
    assert!(diagnostic.detail.contains("last good config"));
}

#[tokio::test]
async fn run_sync_once_emits_recovered_transition_diagnostic_after_degraded_state_recovers() {
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
    let previous = state.clone();
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
    let diagnostic = diagnose_sync_transition(&previous, &state, &outcome);

    assert_eq!(diagnostic.transition, ConfigSyncTransition::Recovered);
    assert_eq!(diagnostic.sync_state, RuntimeSyncState::Accepted);
    assert!(diagnostic.detail.contains("recovered"));
}

#[tokio::test]
async fn run_sync_once_retries_transient_sync_failure_within_same_tick() {
    let mut state = sample_runtime_state();
    let requests = Arc::new(Mutex::new(Vec::new()));
    let mut transport = RecordingTransport::outcomes(
        vec![
            Err("temporary sync timeout".to_string()),
            Ok(sample_response(
                true,
                AgentRunMode::Active,
                SyncBindingState::Bound,
                "node-001",
                "cfg-v1",
                None,
            )),
        ],
        requests.clone(),
    );

    let outcome = rsagent::config_sync::run_sync_once_with_retry_policy(
        &mut state,
        &sample_identity(),
        &mut transport,
        &TransientRetryPolicy::new(vec![Duration::ZERO]),
    )
    .await
    .unwrap();

    assert!(outcome.config_changed);
    assert!(state.loops_enabled());
    assert_eq!(requests.lock().expect("requests lock").len(), 2);
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
    assert_eq!(state.sync_state(), RuntimeSyncState::TemporaryFailure);
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
    assert!(outcome.heartbeat_reset_required);
    assert!(!state.loops_enabled());
    assert!(!state.is_degraded());
    assert_eq!(state.binding_state(), Some(&SyncBindingState::Conflict));
    assert_eq!(state.last_sync_error(), Some("binding conflict"));
    assert_eq!(state.sync_state(), RuntimeSyncState::ExplicitDenial);
}

#[tokio::test]
async fn run_sync_once_requests_heartbeat_reset_when_destination_changes_without_version_change() {
    let mut state = sample_runtime_state();
    state.apply_sync_response(sample_response(
        true,
        AgentRunMode::Active,
        SyncBindingState::Bound,
        "node-001",
        "cfg-v1",
        None,
    ));

    let mut response = sample_response(
        true,
        AgentRunMode::Active,
        SyncBindingState::Bound,
        "node-001",
        "cfg-v1",
        None,
    );
    response.heartbeat_config.data_link_id = "dl-002".to_string();
    let mut transport = RecordingTransport::success(response);

    let outcome =
        rsagent::config_sync::run_sync_once(&mut state, &sample_identity(), &mut transport)
            .await
            .unwrap();

    assert!(!outcome.config_changed);
    assert!(outcome.heartbeat_reset_required);
    assert!(!outcome.sync_interval_changed);
    assert!(!outcome.task_sync_interval_changed);
    assert!(!outcome.heartbeat_interval_changed);
}

#[tokio::test]
async fn run_sync_once_noop_accepted_sync_does_not_rebuild_loops() {
    let mut state = sample_runtime_state();
    state.apply_sync_response(sample_response(
        true,
        AgentRunMode::Active,
        SyncBindingState::Bound,
        "node-001",
        "cfg-v1",
        None,
    ));
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
    assert!(!outcome.heartbeat_reset_required);
    assert!(!outcome.sync_interval_changed);
    assert!(!outcome.task_sync_interval_changed);
    assert!(!outcome.heartbeat_interval_changed);
}

#[tokio::test]
async fn run_sync_once_uses_last_accepted_config_version_after_temporary_failure() {
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
    let requests = Arc::new(Mutex::new(Vec::new()));
    let mut transport = RecordingTransport::success_with_requests(
        sample_response(
            true,
            AgentRunMode::Active,
            SyncBindingState::Bound,
            "node-001",
            "cfg-v1",
            None,
        ),
        requests.clone(),
    );

    let outcome =
        rsagent::config_sync::run_sync_once(&mut state, &sample_identity(), &mut transport)
            .await
            .unwrap();

    assert!(!outcome.config_changed);
    let requests = requests.lock().expect("requests lock");
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].config_version.as_deref(), Some("cfg-v1"));
    assert_eq!(state.sync_state(), RuntimeSyncState::Accepted);
}

#[tokio::test]
async fn run_sync_once_does_not_mutate_live_state_when_accepted_sync_persistence_fails() {
    let mut state = sample_runtime_state_in(unwritable_data_dir());
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
        AgentRunMode::Idle,
        SyncBindingState::Bound,
        "node-002",
        "cfg-v2",
        None,
    );
    updated.sync_interval_secs = 99;
    let mut transport = RecordingTransport::success(updated);

    let error = rsagent::config_sync::run_sync_once(&mut state, &sample_identity(), &mut transport)
        .await
        .expect_err("accepted sync should fail when identity persistence fails");

    assert!(
        error
            .to_string()
            .contains("failed to create rsagent data dir")
    );
    assert_eq!(state.local_node_id(), Some("node-001"));
    assert_eq!(state.config_version(), Some("cfg-v1"));
    assert!(state.loops_enabled());
    assert_eq!(state.sync_state(), RuntimeSyncState::Accepted);
    assert_eq!(
        state
            .effective_config()
            .expect("effective config")
            .sync_interval_secs,
        10
    );
}

#[derive(Debug)]
struct RecordingTransport {
    outcomes: std::collections::VecDeque<std::result::Result<AgentSyncResponse, String>>,
    requests: Arc<Mutex<Vec<nodemanage::AgentSyncRequest>>>,
}

impl RecordingTransport {
    fn success(response: AgentSyncResponse) -> Self {
        Self {
            outcomes: std::collections::VecDeque::from([Ok(response)]),
            requests: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn temporary_failure(message: &str) -> Self {
        Self {
            outcomes: std::collections::VecDeque::from([Err(message.to_string())]),
            requests: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn success_with_requests(
        response: AgentSyncResponse,
        requests: Arc<Mutex<Vec<nodemanage::AgentSyncRequest>>>,
    ) -> Self {
        Self {
            outcomes: std::collections::VecDeque::from([Ok(response)]),
            requests,
        }
    }

    fn outcomes(
        outcomes: Vec<std::result::Result<AgentSyncResponse, String>>,
        requests: Arc<Mutex<Vec<nodemanage::AgentSyncRequest>>>,
    ) -> Self {
        Self {
            outcomes: std::collections::VecDeque::from(outcomes),
            requests,
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
        let outcome = self.outcomes.pop_front();
        Box::pin(async move {
            match outcome {
                Some(Ok(response)) => Ok(response),
                Some(Err(error)) => Err(anyhow!(error)),
                None => Err(anyhow!("recording transport misconfigured")),
            }
        })
    }
}

fn sample_runtime_state() -> AgentRuntimeState {
    sample_runtime_state_in(writable_data_dir("config-sync-state"))
}

fn sample_runtime_state_in(data_dir: String) -> AgentRuntimeState {
    AgentRuntimeState::new(AgentRuntimeConfig {
        nodemanage_sync_url: "http://127.0.0.1:3000/agent/sync".to_string(),
        agent_id: "agt-001".to_string(),
        node_id: None,
        data_dir,
        sync_interval_secs: 60,
    })
}

fn unwritable_data_dir() -> String {
    let path = std::env::temp_dir().join(format!(
        "rsagent-config-sync-file-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("unix epoch")
            .as_nanos()
    ));
    fs::write(&path, "not a directory").expect("write blocking file");
    path.to_string_lossy().into_owned()
}

fn writable_data_dir(prefix: &str) -> String {
    let path = std::env::temp_dir().join(format!(
        "rsagent-{prefix}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("unix epoch")
            .as_nanos()
    ));
    fs::create_dir_all(&path).expect("create writable data dir");
    path.to_string_lossy().into_owned()
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
