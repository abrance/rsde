use std::{
    collections::{BTreeMap, VecDeque},
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Result, anyhow};
use chrono::{Duration, TimeZone, Utc};
use job_manage::{
    TaskApplyIdentity, TaskApplyPatch, TaskDesiredState, TaskListQuery, TaskObservedState,
    TaskResource, TaskType,
};
use nodemanage::{
    AgentRunMode, AgentSyncResponse, HeartbeatConfig, JobManageConfig, SyncBindingState,
    TaskFilterDefaults,
};
use rsagent::{
    bootstrap::bootstrap_runtime_state,
    clients::{
        job_manage::{JobManageTransport, TaskApplyAck},
        nodemanage::NodeManageSyncTransport,
        victoria_metrics::VictoriaMetricsTransport,
    },
    config::AgentRuntimeConfig,
    config_sync::{ConfigSyncTransition, diagnose_sync_transition, run_sync_once},
    heartbeat::{HeartbeatDiagnosticTransition, HeartbeatReporter},
    registration::AgentIdentity,
    task_sync::{TaskDecisionOutcome, TaskDecisionStage, TaskExecutor, TaskSyncLoop, TaskSyncTick},
};

#[tokio::test]
async fn restart_outage_recovery_restores_sync_heartbeat_and_task_capability() {
    let data_dir = writable_data_dir("restart-recovery");
    let config = AgentRuntimeConfig::installer_bootstrap(
        "http://127.0.0.1:3000/agent/sync".to_string(),
        data_dir,
    );
    let identity = sample_identity();
    let mut first_transport = RecordingNodeManageTransport::success(sample_sync_response("cfg-v1"));

    let (first_state, _) =
        bootstrap_runtime_state(config.clone(), identity.clone(), &mut first_transport)
            .await
            .unwrap();
    assert!(first_state.loops_enabled());

    let mut outage_transport = RecordingNodeManageTransport::temporary_failure("timeout");
    let (mut restarted_state, _) =
        bootstrap_runtime_state(config, identity.clone(), &mut outage_transport)
            .await
            .unwrap();
    assert!(!restarted_state.loops_enabled());
    assert_eq!(restarted_state.local_node_id(), Some("node-001"));

    let previous = restarted_state.clone();
    let mut recovery_transport =
        RecordingNodeManageTransport::success(sample_sync_response("cfg-v1"));
    let outcome = run_sync_once(&mut restarted_state, &identity, &mut recovery_transport)
        .await
        .unwrap();
    let sync_diagnostic = diagnose_sync_transition(&previous, &restarted_state, &outcome);

    let mut heartbeat = HeartbeatReporter::new(RecordingVmTransport::default());
    let heartbeat_tick = heartbeat
        .tick(
            timestamp() + Duration::seconds(61),
            &restarted_state,
            restarted_state.agent_id(),
            &identity,
        )
        .unwrap();

    let transport =
        RecordingJobManageTransport::with_list_responses(vec![vec![queued_command_task(
            "task-after-restart",
        )]]);
    let executor = RecordingExecutor::succeeds();
    let mut task_loop = TaskSyncLoop::new(transport, executor);
    let task_tick = task_loop
        .tick(&restarted_state, restarted_state.agent_id(), timestamp())
        .await
        .unwrap();
    let task_diagnostic = task_loop
        .last_tick_diagnostic()
        .expect("task diagnostic after restart recovery")
        .clone();

    assert_eq!(sync_diagnostic.transition, ConfigSyncTransition::Recovered);
    assert!(matches!(
        heartbeat_tick,
        rsagent::heartbeat::HeartbeatTick::Sent { .. }
    ));
    assert_eq!(
        heartbeat
            .last_diagnostic()
            .expect("heartbeat diagnostic after restart recovery")
            .transition,
        HeartbeatDiagnosticTransition::Sent
    );
    assert_eq!(
        task_tick,
        TaskSyncTick::Applied {
            task_id: "task-after-restart".to_string(),
            final_state: TaskObservedState::Succeeded,
        }
    );
    assert_eq!(task_diagnostic.stage, TaskDecisionStage::Execute);
    assert_eq!(task_diagnostic.outcome, TaskDecisionOutcome::Completed);
}

#[derive(Debug, Clone, Default)]
struct RecordingVmTransport {
    payloads: Vec<String>,
}

impl VictoriaMetricsTransport for RecordingVmTransport {
    fn post_line_protocol(&mut self, _endpoint: &str, payload: &str) -> Result<()> {
        self.payloads.push(payload.to_string());
        Ok(())
    }
}

#[derive(Debug, Clone)]
struct RecordingNodeManageTransport {
    response: Option<AgentSyncResponse>,
    error: Option<String>,
    requests: Arc<Mutex<Vec<nodemanage::AgentSyncRequest>>>,
}

impl RecordingNodeManageTransport {
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

impl NodeManageSyncTransport for RecordingNodeManageTransport {
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
            .expect("sync requests lock")
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct ApplyCall {
    identity: TaskApplyIdentity,
    patch: TaskApplyPatch,
}

#[derive(Debug, Clone)]
struct RecordingJobManageTransport {
    list_responses: VecDeque<Vec<TaskResource>>,
    tasks: BTreeMap<String, TaskResource>,
    apply_calls: Vec<ApplyCall>,
}

impl RecordingJobManageTransport {
    fn with_list_responses(list_responses: Vec<Vec<TaskResource>>) -> Self {
        let tasks = list_responses
            .iter()
            .flatten()
            .cloned()
            .map(|task| (task.task_id.clone(), task))
            .collect();
        Self {
            list_responses: VecDeque::from(list_responses),
            tasks,
            apply_calls: Vec::new(),
        }
    }
}

impl JobManageTransport for RecordingJobManageTransport {
    fn list_tasks(&mut self, _endpoint: &str, _query: &TaskListQuery) -> Result<Vec<TaskResource>> {
        Ok(self.list_responses.pop_front().unwrap_or_default())
    }

    fn apply_task(
        &mut self,
        _endpoint: &str,
        identity: &TaskApplyIdentity,
        patch: &TaskApplyPatch,
    ) -> Result<TaskApplyAck> {
        self.apply_calls.push(ApplyCall {
            identity: identity.clone(),
            patch: patch.clone(),
        });
        let task = self.tasks.get_mut(&identity.task_id).expect("task exists");
        if let Some(observed_state) = patch.observed_state {
            task.observed_state = observed_state;
        }
        if let Some(stdout) = &patch.stdout {
            task.stdout = Some(stdout.clone());
        }
        if let Some(stderr) = &patch.stderr {
            task.stderr = Some(stderr.clone());
        }
        if let Some(exit_code) = patch.exit_code {
            task.exit_code = Some(exit_code);
        }
        if let Some(updated_at) = &patch.updated_at {
            task.updated_at = Some(updated_at.clone());
        }
        if let Some(claimed_at) = &patch.claimed_at {
            task.claimed_at = Some(claimed_at.clone());
        }
        if let Some(started_at) = &patch.started_at {
            task.started_at = Some(started_at.clone());
        }
        if let Some(finished_at) = &patch.finished_at {
            task.finished_at = Some(finished_at.clone());
        }

        Ok(TaskApplyAck {
            task_id: task.task_id.clone(),
            observed_state: task.observed_state,
            updated_at: task.updated_at.clone(),
        })
    }
}

#[derive(Debug, Clone, Copy)]
struct RecordingExecutor;

impl RecordingExecutor {
    fn succeeds() -> Self {
        Self
    }
}

impl TaskExecutor for RecordingExecutor {
    type ExecuteFuture<'a>
        = Pin<Box<dyn Future<Output = Result<rsagent::executor::ExecutionResult>> + Send + 'a>>
    where
        Self: 'a;

    fn execute<'a>(&'a self, _task: &'a TaskResource) -> Self::ExecuteFuture<'a> {
        Box::pin(async move {
            Ok(rsagent::executor::ExecutionResult {
                stdout: "done".to_string(),
                stderr: String::new(),
                exit_code: Some(0),
                state: TaskObservedState::Succeeded,
            })
        })
    }
}

fn queued_command_task(task_id: &str) -> TaskResource {
    TaskResource {
        task_id: task_id.to_string(),
        job_id: "job-1".to_string(),
        node_id: "node-001".to_string(),
        agent_id: "agt-001".to_string(),
        task_type: TaskType::Command,
        script_content: None,
        command_line: Some("sh".to_string()),
        interpreter: None,
        args: vec!["-c".to_string(), "printf 'done'".to_string()],
        env: BTreeMap::new(),
        working_dir: None,
        timeout_secs: Some(30),
        desired_state: TaskDesiredState::Queued,
        observed_state: TaskObservedState::Queued,
        stdout: None,
        stderr: None,
        exit_code: None,
        started_at: None,
        finished_at: None,
        error_message: None,
        claimed_at: None,
        updated_at: Some("2026-05-31T08:00:00Z".to_string()),
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

fn sample_sync_response(config_version: &str) -> AgentSyncResponse {
    AgentSyncResponse {
        accepted: true,
        agent_id: "agt-001".to_string(),
        bound_node_id: "node-001".to_string(),
        binding_state: SyncBindingState::Bound,
        agent_run_mode: AgentRunMode::Active,
        config_version: config_version.to_string(),
        heartbeat_config: HeartbeatConfig {
            version: "hb-v1".to_string(),
            data_link_id: "dl-1".to_string(),
            vm_base_url: "http://victoriametrics:8428".to_string(),
            interval_secs: 60,
        },
        job_manage_config: JobManageConfig {
            version: "jm-v1".to_string(),
            base_url: "http://job-manage".to_string(),
            task_filter_defaults: TaskFilterDefaults {
                states: vec!["queued".to_string(), "running".to_string()],
            },
        },
        sync_interval_secs: 30,
        task_sync_interval_secs: 5,
        rejection_reason: None,
    }
}

fn writable_data_dir(prefix: &str) -> String {
    let path = std::env::temp_dir().join(format!(
        "rsagent-restart-recovery-{prefix}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos()
    ));
    std::fs::create_dir_all(&path).expect("create temp dir");
    path.to_string_lossy().into_owned()
}

fn timestamp() -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 5, 31, 8, 0, 0).unwrap()
}
