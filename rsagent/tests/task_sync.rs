use std::{
    collections::{BTreeMap, VecDeque},
    future::Future,
    io::{BufRead, BufReader, Read, Write},
    net::TcpListener,
    pin::Pin,
    sync::{Arc, Mutex},
    thread,
};

use anyhow::Result;
use chrono::{TimeZone, Utc};
use job_manage::{
    TaskApplyIdentity, TaskApplyPatch, TaskApplyRequest, TaskDesiredState, TaskListQuery,
    TaskObservedState, TaskResource, TaskSyncService, TaskType, models::TaskFinalResultCategory,
};
use nodemanage::{
    AgentRunMode, AgentSyncResponse, HeartbeatConfig, JobManageConfig, SyncBindingState,
    TaskFilterDefaults,
};
use rsagent::{
    clients::job_manage::{JobManageTransport, ReqwestJobManageTransport, TaskApplyAck},
    config::AgentRuntimeConfig,
    executor::ExecutionResult,
    registration::AgentRuntimeState,
    task_sync::{
        TaskDecisionDiagnostic, TaskExecutor, TaskLifecycleStage, TaskOutcomeKind, TaskSyncLoop,
        TaskSyncSkipReason, TaskSyncTick, sync_once,
    },
};
use serde_json::{Value, json};

#[test]
fn reqwest_transport_uses_job_manage_http_contract() {
    let task = queued_script_task("task-http");
    let server = TestHttpServer::spawn(vec![
        ExpectedHttpRequest {
            method: "GET",
            target: "/tasks?agent_id=agent-1&node_id=node-1&states=queued%2Crunning&updated_after=2026-05-31T08%3A00%3A00Z",
            body: None,
            response: ExpectedHttpResponse::ok(json!({
                "success": true,
                "data": { "items": [task.clone()] },
                "error": null
            })),
        },
        ExpectedHttpRequest {
            method: "POST",
            target: "/tasks:apply?task_id=task-http&agent_id=agent-1&node_id=node-1",
            body: Some(json!({
                "observed_state": "acknowledged",
                "claimed_at": "2026-05-31T08:00:00Z",
                "updated_at": "2026-05-31T08:00:00Z"
            })),
            response: ExpectedHttpResponse::ok(json!({
                "success": true,
                "data": {
                    "task_id": "task-http",
                    "observed_state": "acknowledged",
                    "updated_at": "2026-05-31T08:00:00Z"
                },
                "error": null
            })),
        },
    ]);

    let client = rsagent::clients::job_manage::JobManageSyncClient::new(server.base_url());
    let mut transport = ReqwestJobManageTransport::default();

    let tasks = client
        .list_tasks(
            &mut transport,
            &TaskListQuery {
                agent_id: "agent-1".to_string(),
                node_id: "node-1".to_string(),
                states: vec![TaskObservedState::Queued, TaskObservedState::Running],
                updated_after: Some("2026-05-31T08:00:00Z".to_string()),
            },
        )
        .unwrap();
    let applied = client
        .apply_task(
            &mut transport,
            &TaskApplyIdentity {
                task_id: "task-http".to_string(),
                agent_id: "agent-1".to_string(),
                node_id: "node-1".to_string(),
            },
            &TaskApplyPatch {
                observed_state: Some(TaskObservedState::Acknowledged),
                claimed_at: Some("2026-05-31T08:00:00Z".to_string()),
                updated_at: Some("2026-05-31T08:00:00Z".to_string()),
                ..Default::default()
            },
        )
        .unwrap();

    assert_eq!(tasks, vec![task]);
    assert_eq!(
        applied,
        TaskApplyAck {
            task_id: "task-http".to_string(),
            observed_state: TaskObservedState::Acknowledged,
            updated_at: Some("2026-05-31T08:00:00Z".to_string()),
        }
    );

    server.finish();
}

#[test]
fn reqwest_transport_accepts_optional_running_skip_and_node_plus_agent_ownership_conflicts() {
    let server = TestHttpServer::spawn(vec![
        ExpectedHttpRequest {
            method: "POST",
            target: "/tasks:apply?task_id=task-terminal&agent_id=agent-1&node_id=node-1",
            body: Some(json!({
                "observed_state": "succeeded",
                "finished_at": "2026-05-31T08:02:00Z",
                "stdout": "done",
                "stderr": "",
                "exit_code": 0,
                "updated_at": "2026-05-31T08:02:00Z"
            })),
            response: ExpectedHttpResponse::ok(json!({
                "success": true,
                "data": {
                    "task_id": "task-terminal",
                    "observed_state": "succeeded",
                    "updated_at": "2026-05-31T08:02:00Z"
                },
                "error": null
            })),
        },
        ExpectedHttpRequest {
            method: "POST",
            target: "/tasks:apply?task_id=task-terminal&agent_id=agent-1&node_id=node-2",
            body: Some(json!({
                "observed_state": "succeeded",
                "finished_at": "2026-05-31T08:02:00Z",
                "stdout": "done",
                "stderr": "",
                "exit_code": 0,
                "updated_at": "2026-05-31T08:02:00Z"
            })),
            response: ExpectedHttpResponse::status(
                404,
                "Not Found",
                json!({
                    "success": false,
                    "data": null,
                    "error": "task not found or task ownership conflict"
                }),
            ),
        },
    ]);

    let client = rsagent::clients::job_manage::JobManageSyncClient::new(server.base_url());
    let mut transport = ReqwestJobManageTransport::default();
    let terminal_patch = TaskApplyPatch {
        observed_state: Some(TaskObservedState::Succeeded),
        finished_at: Some("2026-05-31T08:02:00Z".to_string()),
        stdout: Some("done".to_string()),
        stderr: Some(String::new()),
        exit_code: Some(0),
        updated_at: Some("2026-05-31T08:02:00Z".to_string()),
        ..Default::default()
    };

    let terminal = client
        .apply_task(
            &mut transport,
            &TaskApplyIdentity {
                task_id: "task-terminal".to_string(),
                agent_id: "agent-1".to_string(),
                node_id: "node-1".to_string(),
            },
            &terminal_patch,
        )
        .unwrap();
    assert_eq!(
        terminal,
        TaskApplyAck {
            task_id: "task-terminal".to_string(),
            observed_state: TaskObservedState::Succeeded,
            updated_at: Some("2026-05-31T08:02:00Z".to_string()),
        }
    );

    let ownership_error = client
        .apply_task(
            &mut transport,
            &TaskApplyIdentity {
                task_id: "task-terminal".to_string(),
                agent_id: "agent-1".to_string(),
                node_id: "node-2".to_string(),
            },
            &terminal_patch,
        )
        .expect_err("ownership conflict should surface exact JM envelope");
    assert_eq!(
        ownership_error.to_string(),
        "job-manage request to http://127.0.0.1:0/tasks:apply returned error: task not found or task ownership conflict"
            .replace("http://127.0.0.1:0", server.base_url())
    );

    server.finish();
}

#[tokio::test]
async fn task_sync_loop_carries_updated_after_between_ticks() {
    let state = synced_runtime_state(&["queued"]);
    let transport = RecordingTransport::with_list_responses(vec![vec![], vec![]]);
    let executor = RecordingExecutor::succeeds(ExecutionResult {
        stdout: String::new(),
        stderr: String::new(),
        exit_code: Some(0),
        state: TaskObservedState::Succeeded,
        category: Some(TaskFinalResultCategory::Succeeded),
    });
    let mut loop_runner = TaskSyncLoop::new(transport, executor);

    loop_runner
        .tick(&state, "agent-1", timestamp())
        .await
        .unwrap();
    loop_runner
        .tick(
            &state,
            "agent-1",
            Utc.with_ymd_and_hms(2026, 5, 31, 8, 5, 0).unwrap(),
        )
        .await
        .unwrap();

    let transport = loop_runner.into_transport();
    assert_eq!(transport.list_calls.len(), 2);
    assert_eq!(transport.list_calls[0].query.updated_after, None);
    assert_eq!(
        transport.list_calls[1].query.updated_after.as_deref(),
        Some("2026-05-31T08:00:00Z")
    );
}

#[tokio::test]
async fn task_sync_loop_keeps_backlog_visible_across_one_task_per_tick_processing() {
    let state = synced_runtime_state(&["queued"]);
    let transport = RecordingTransport::stateful(vec![
        queued_script_task("task-1"),
        TaskResource {
            updated_at: Some("2026-05-31T07:59:00Z".to_string()),
            ..queued_script_task("task-2")
        },
    ]);
    let executor = RecordingExecutor::succeeds(ExecutionResult {
        stdout: "done".to_string(),
        stderr: String::new(),
        exit_code: Some(0),
        state: TaskObservedState::Succeeded,
        category: Some(TaskFinalResultCategory::Succeeded),
    });
    let mut loop_runner = TaskSyncLoop::new(transport, executor.clone());

    let first = loop_runner
        .tick(&state, "agent-1", timestamp())
        .await
        .unwrap();
    assert_eq!(
        first,
        TaskSyncTick::Applied {
            task_id: "task-1".to_string(),
            final_state: TaskObservedState::Succeeded,
        }
    );
    assert_eq!(
        loop_runner.last_diagnostic(),
        Some(&TaskDecisionDiagnostic {
            task_id: Some("task-1".to_string()),
            observed_state: Some(TaskObservedState::Queued),
            lifecycle_stage: TaskLifecycleStage::PublishedTerminalResult,
            outcome: TaskOutcomeKind::Succeeded,
            detail: "task completed successfully".to_string(),
        })
    );
    let second = loop_runner
        .tick(
            &state,
            "agent-1",
            Utc.with_ymd_and_hms(2026, 5, 31, 8, 5, 0).unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        second,
        TaskSyncTick::Applied {
            task_id: "task-2".to_string(),
            final_state: TaskObservedState::Succeeded,
        }
    );
    assert_eq!(
        loop_runner.last_diagnostic(),
        Some(&TaskDecisionDiagnostic {
            task_id: Some("task-2".to_string()),
            observed_state: Some(TaskObservedState::Queued),
            lifecycle_stage: TaskLifecycleStage::PublishedTerminalResult,
            outcome: TaskOutcomeKind::Succeeded,
            detail: "task completed successfully".to_string(),
        })
    );

    let transport = loop_runner.into_transport();
    assert_eq!(transport.list_calls.len(), 2);
    assert_eq!(transport.list_calls[0].query.updated_after, None);
    assert_eq!(transport.list_calls[1].query.updated_after, None);
    assert_eq!(
        executor.calls(),
        vec!["task-1".to_string(), "task-2".to_string()]
    );
}

#[tokio::test]
async fn task_sync_loop_does_not_advance_cursor_while_disabled_before_later_activation() {
    let disabled_state = AgentRuntimeState::new(AgentRuntimeConfig {
        nodemanage_sync_url: "http://127.0.0.1:3000/agent/sync".to_string(),
        agent_id: "agent-1".to_string(),
        node_id: None,
        data_dir: "/var/lib/rsagent".to_string(),
        sync_interval_secs: 60,
    });
    let enabled_state = synced_runtime_state(&["queued"]);
    let transport = RecordingTransport::with_list_responses(vec![vec![TaskResource {
        updated_at: Some("2026-05-31T08:05:00Z".to_string()),
        ..queued_script_task("task-delayed")
    }]]);
    let executor = RecordingExecutor::succeeds(ExecutionResult {
        stdout: "done".to_string(),
        stderr: String::new(),
        exit_code: Some(0),
        state: TaskObservedState::Succeeded,
        category: Some(TaskFinalResultCategory::Succeeded),
    });
    let mut loop_runner = TaskSyncLoop::new(transport, executor.clone());

    let skipped = loop_runner
        .tick(&disabled_state, "agent-1", timestamp())
        .await
        .unwrap();
    assert_eq!(
        skipped,
        TaskSyncTick::Skipped {
            reason: TaskSyncSkipReason::LoopsDisabled,
        }
    );
    assert_eq!(
        loop_runner.last_diagnostic(),
        Some(&TaskDecisionDiagnostic {
            task_id: None,
            observed_state: None,
            lifecycle_stage: TaskLifecycleStage::PollDecision,
            outcome: TaskOutcomeKind::SkippedLoopsDisabled,
            detail: "task sync skipped because subordinate loops are disabled".to_string(),
        })
    );

    let applied = loop_runner
        .tick(
            &enabled_state,
            "agent-1",
            Utc.with_ymd_and_hms(2026, 5, 31, 8, 10, 0).unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        applied,
        TaskSyncTick::Applied {
            task_id: "task-delayed".to_string(),
            final_state: TaskObservedState::Succeeded,
        }
    );

    let transport = loop_runner.into_transport();
    assert_eq!(transport.list_calls.len(), 1);
    assert_eq!(transport.list_calls[0].query.updated_after, None);
    assert_eq!(executor.calls(), vec!["task-delayed".to_string()]);
}

#[tokio::test]
async fn task_sync_loop_restart_after_outage_reuses_recovered_runtime_state_without_duplicate_claim()
 {
    let state = synced_runtime_state(&["queued", "acknowledged", "running"]);
    let transport = RecordingTransport::with_list_responses(vec![
        vec![],
        vec![acknowledged_command_task("task-recovered")],
    ]);
    let executor = RecordingExecutor::succeeds(ExecutionResult {
        stdout: "recovered".to_string(),
        stderr: String::new(),
        exit_code: Some(0),
        state: TaskObservedState::Succeeded,
        category: Some(TaskFinalResultCategory::Succeeded),
    });
    let mut loop_runner = TaskSyncLoop::new(transport, executor.clone());

    let first = loop_runner
        .tick(&state, "agent-1", timestamp())
        .await
        .unwrap();
    let second = loop_runner
        .tick(
            &state,
            "agent-1",
            Utc.with_ymd_and_hms(2026, 5, 31, 8, 5, 0).unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        first,
        TaskSyncTick::Skipped {
            reason: TaskSyncSkipReason::NoTasks,
        }
    );
    assert_eq!(
        second,
        TaskSyncTick::Applied {
            task_id: "task-recovered".to_string(),
            final_state: TaskObservedState::Succeeded,
        }
    );
    assert_eq!(
        loop_runner.last_diagnostic(),
        Some(&TaskDecisionDiagnostic {
            task_id: Some("task-recovered".to_string()),
            observed_state: Some(TaskObservedState::Acknowledged),
            lifecycle_stage: TaskLifecycleStage::PublishedTerminalResult,
            outcome: TaskOutcomeKind::Succeeded,
            detail: "task completed successfully".to_string(),
        })
    );

    let transport = loop_runner.into_transport();
    assert_eq!(transport.list_calls.len(), 2);
    assert_eq!(transport.list_calls[0].query.updated_after, None);
    assert_eq!(
        transport.list_calls[1].query.updated_after.as_deref(),
        Some("2026-05-31T08:00:00Z")
    );
    assert_eq!(executor.calls(), vec!["task-recovered".to_string()]);
    assert_eq!(transport.apply_calls.len(), 2);
    assert_eq!(
        transport.apply_calls[0].patch.observed_state,
        Some(TaskObservedState::Running)
    );
    assert_eq!(transport.apply_calls[0].patch.claimed_at, None);
    assert_eq!(
        transport.apply_calls[1].patch.observed_state,
        Some(TaskObservedState::Succeeded)
    );
}

#[tokio::test]
async fn task_sync_loop_exposes_no_tasks_poll_diagnostic() {
    let state = synced_runtime_state(&["queued"]);
    let transport = RecordingTransport::with_list_responses(vec![vec![]]);
    let executor = RecordingExecutor::succeeds(ExecutionResult {
        stdout: String::new(),
        stderr: String::new(),
        exit_code: Some(0),
        state: TaskObservedState::Succeeded,
        category: Some(TaskFinalResultCategory::Succeeded),
    });
    let mut loop_runner = TaskSyncLoop::new(transport, executor);

    let tick = loop_runner
        .tick(&state, "agent-1", timestamp())
        .await
        .unwrap();

    assert_eq!(
        tick,
        TaskSyncTick::Skipped {
            reason: TaskSyncSkipReason::NoTasks,
        }
    );
    assert_eq!(
        loop_runner.last_diagnostic(),
        Some(&TaskDecisionDiagnostic {
            task_id: None,
            observed_state: None,
            lifecycle_stage: TaskLifecycleStage::PollDecision,
            outcome: TaskOutcomeKind::NoTasksAvailable,
            detail: "task poll returned no visible tasks".to_string(),
        })
    );
}

#[tokio::test]
async fn task_sync_loop_classifies_temporary_poll_failure_with_shared_retry_policy() {
    let state = synced_runtime_state(&["queued"]);
    let transport = RecordingTransport::with_list_failures(vec!["temporary jm outage"]);
    let executor = RecordingExecutor::succeeds(ExecutionResult {
        stdout: String::new(),
        stderr: String::new(),
        exit_code: Some(0),
        state: TaskObservedState::Succeeded,
        category: Some(TaskFinalResultCategory::Succeeded),
    });
    let mut loop_runner = TaskSyncLoop::new(transport, executor);

    let error = loop_runner
        .tick(&state, "agent-1", timestamp())
        .await
        .expect_err("temporary upstream failure should bubble out");

    assert!(error.to_string().contains("temporary jm outage"));
    assert_eq!(
        loop_runner.last_retry_policy(),
        Some(&rsagent::runtime_coordinator::temporary_upstream_retry_policy(1))
    );
    let transport = loop_runner.into_transport();
    assert_eq!(transport.list_calls.len(), 1);
    assert!(transport.apply_calls.is_empty());
}

#[tokio::test]
async fn task_sync_loop_escalates_retry_backoff_without_advancing_cursor_on_repeated_poll_failures()
{
    let state = synced_runtime_state(&["queued"]);
    let transport = RecordingTransport::with_list_failures(vec![
        "temporary jm outage",
        "temporary jm outage",
        "temporary jm outage",
    ]);
    let executor = RecordingExecutor::succeeds(ExecutionResult {
        stdout: String::new(),
        stderr: String::new(),
        exit_code: Some(0),
        state: TaskObservedState::Succeeded,
        category: Some(TaskFinalResultCategory::Succeeded),
    });
    let mut loop_runner = TaskSyncLoop::new(transport, executor);

    let _ = loop_runner.tick(&state, "agent-1", timestamp()).await;
    let _ = loop_runner
        .tick(
            &state,
            "agent-1",
            Utc.with_ymd_and_hms(2026, 5, 31, 8, 5, 0).unwrap(),
        )
        .await;
    let _ = loop_runner
        .tick(
            &state,
            "agent-1",
            Utc.with_ymd_and_hms(2026, 5, 31, 8, 10, 0).unwrap(),
        )
        .await;

    assert_eq!(
        loop_runner.last_retry_policy(),
        Some(&rsagent::runtime_coordinator::temporary_upstream_retry_policy(3))
    );
    let transport = loop_runner.into_transport();
    assert_eq!(transport.list_calls.len(), 3);
    assert_eq!(transport.list_calls[0].query.updated_after, None);
    assert_eq!(transport.list_calls[1].query.updated_after, None);
    assert_eq!(transport.list_calls[2].query.updated_after, None);
}

#[tokio::test]
async fn task_sync_loop_does_not_record_temporary_upstream_for_local_payload_validation_errors() {
    let state = synced_runtime_state(&["queued"]);
    let transport = RecordingTransport::new(vec![TaskResource {
        command_line: Some("sh".to_string()),
        ..queued_script_task("task-invalid-local")
    }]);
    let executor = RecordingExecutor::succeeds(ExecutionResult {
        stdout: String::new(),
        stderr: String::new(),
        exit_code: Some(0),
        state: TaskObservedState::Succeeded,
        category: Some(TaskFinalResultCategory::Succeeded),
    });
    let mut loop_runner = TaskSyncLoop::new(transport, executor);

    let tick = loop_runner
        .tick(&state, "agent-1", timestamp())
        .await
        .unwrap();

    assert_eq!(
        tick,
        TaskSyncTick::Applied {
            task_id: "task-invalid-local".to_string(),
            final_state: TaskObservedState::Failed,
        }
    );
    assert_eq!(loop_runner.last_retry_policy(), None);
}

#[tokio::test]
async fn task_sync_loop_exposes_structured_task_lifecycle_diagnostic_for_invalid_payload_failures()
{
    let state = synced_runtime_state(&["queued"]);
    let transport = RecordingTransport::new(vec![TaskResource {
        command_line: Some("sh".to_string()),
        ..queued_script_task("task-invalid-local")
    }]);
    let executor = RecordingExecutor::succeeds(ExecutionResult {
        stdout: String::new(),
        stderr: String::new(),
        exit_code: Some(0),
        state: TaskObservedState::Succeeded,
        category: Some(TaskFinalResultCategory::Succeeded),
    });
    let mut loop_runner = TaskSyncLoop::new(transport, executor);

    let tick = loop_runner
        .tick(&state, "agent-1", timestamp())
        .await
        .unwrap();

    assert_eq!(
        tick,
        TaskSyncTick::Applied {
            task_id: "task-invalid-local".to_string(),
            final_state: TaskObservedState::Failed,
        }
    );
    assert_eq!(
        loop_runner.last_diagnostic(),
        Some(&TaskDecisionDiagnostic {
            task_id: Some("task-invalid-local".to_string()),
            observed_state: Some(TaskObservedState::Queued),
            lifecycle_stage: TaskLifecycleStage::PublishedTerminalResult,
            outcome: TaskOutcomeKind::FailedInvalidPayload,
            detail: "script task cannot include command_line".to_string(),
        })
    );
}

#[tokio::test]
async fn task_sync_loop_does_not_record_temporary_upstream_for_non_upstream_apply_errors() {
    let state = synced_runtime_state(&["queued"]);
    let transport = RecordingTransport::new(vec![queued_command_task("task-running-error")])
        .with_apply_failure(
            TaskObservedState::Running,
            "job-manage refused running transition",
        );
    let executor = RecordingExecutor::succeeds(ExecutionResult {
        stdout: String::new(),
        stderr: String::new(),
        exit_code: Some(0),
        state: TaskObservedState::Succeeded,
        category: Some(TaskFinalResultCategory::Succeeded),
    });
    let mut loop_runner = TaskSyncLoop::new(transport, executor);

    let error = loop_runner
        .tick(&state, "agent-1", timestamp())
        .await
        .expect_err("non-upstream apply failure should bubble out");

    assert!(
        error
            .to_string()
            .contains("job-manage refused running transition")
    );
    assert_eq!(loop_runner.last_retry_policy(), None);
}

#[tokio::test]
async fn task_sync_loop_does_not_record_temporary_upstream_when_effective_config_exists_but_local_node_id_is_missing()
 {
    let mut state = AgentRuntimeState::new(AgentRuntimeConfig {
        nodemanage_sync_url: "http://127.0.0.1:3000/agent/sync".to_string(),
        agent_id: "agent-1".to_string(),
        node_id: None,
        data_dir: "/var/lib/rsagent".to_string(),
        sync_interval_secs: 60,
    });
    state.apply_sync_response(AgentSyncResponse {
        accepted: true,
        agent_id: "agent-1".to_string(),
        bound_node_id: "".to_string(),
        binding_state: SyncBindingState::Bound,
        agent_run_mode: AgentRunMode::Active,
        config_version: "cfg-1".to_string(),
        heartbeat_config: HeartbeatConfig {
            version: "hb-v1".to_string(),
            data_link_id: "dl-1".to_string(),
            vm_base_url: "http://vm".to_string(),
            interval_secs: 15,
        },
        job_manage_config: JobManageConfig {
            version: "jm-v1".to_string(),
            base_url: "http://job-manage".to_string(),
            task_filter_defaults: TaskFilterDefaults {
                states: vec!["queued".to_string()],
            },
        },
        sync_interval_secs: 10,
        task_sync_interval_secs: 5,
        rejection_reason: None,
    });
    assert!(state.effective_config().is_some());
    let transport = RecordingTransport::new(vec![queued_command_task("task-missing-node")]);
    let executor = RecordingExecutor::succeeds(ExecutionResult {
        stdout: String::new(),
        stderr: String::new(),
        exit_code: Some(0),
        state: TaskObservedState::Succeeded,
        category: Some(TaskFinalResultCategory::Succeeded),
    });
    let mut loop_runner = TaskSyncLoop::new(transport, executor);

    let error = loop_runner
        .tick(&state, "agent-1", timestamp())
        .await
        .expect_err("missing local node id should bubble out");

    assert!(
        error
            .to_string()
            .contains("node_id unavailable for task sync")
    );
    assert_eq!(loop_runner.last_retry_policy(), None);
}

#[tokio::test]
async fn task_sync_loop_does_not_record_temporary_upstream_for_local_error_containing_timeout_or_unavailable_text()
 {
    let state = synced_runtime_state(&["queued"]);
    let transport = RecordingTransport::new(vec![queued_command_task("task-running-error")])
        .with_apply_failure(
            TaskObservedState::Running,
            "local guard rejected transition after timeout budget check: resource unavailable",
        );
    let executor = RecordingExecutor::succeeds(ExecutionResult {
        stdout: String::new(),
        stderr: String::new(),
        exit_code: Some(0),
        state: TaskObservedState::Succeeded,
        category: Some(TaskFinalResultCategory::Succeeded),
    });
    let mut loop_runner = TaskSyncLoop::new(transport, executor);

    let error = loop_runner
        .tick(&state, "agent-1", timestamp())
        .await
        .expect_err("local apply failure should bubble out");

    assert!(error.to_string().contains(
        "local guard rejected transition after timeout budget check: resource unavailable"
    ));
    assert_eq!(loop_runner.last_retry_policy(), None);
}

#[tokio::test]
async fn lists_tasks_with_default_filters_from_runtime_config() {
    let state = synced_runtime_state(&["queued", "running"]);
    let mut transport = RecordingTransport::new(Vec::new());
    let executor = RecordingExecutor::succeeds(ExecutionResult {
        stdout: String::new(),
        stderr: String::new(),
        exit_code: Some(0),
        state: TaskObservedState::Succeeded,
        category: Some(TaskFinalResultCategory::Succeeded),
    });

    let result = sync_once(
        &mut transport,
        &executor,
        &state,
        "agent-1",
        None,
        timestamp(),
    )
    .await
    .unwrap();

    assert_eq!(
        result,
        TaskSyncTick::Skipped {
            reason: TaskSyncSkipReason::NoTasks,
        }
    );
    assert_eq!(
        transport.list_calls,
        vec![ListCall {
            endpoint: "http://job-manage/tasks".to_string(),
            query: TaskListQuery {
                agent_id: "agent-1".to_string(),
                node_id: "node-1".to_string(),
                states: vec![TaskObservedState::Queued, TaskObservedState::Running],
                updated_after: None,
            },
        }]
    );
    assert!(transport.apply_calls.is_empty());
    assert!(executor.calls().is_empty());
}

#[tokio::test]
async fn claims_and_updates_only_one_active_task_per_tick() {
    let state = synced_runtime_state(&["queued"]);
    let tasks = vec![queued_script_task("task-1"), queued_script_task("task-2")];
    let mut transport = RecordingTransport::new(tasks);
    let executor = RecordingExecutor::succeeds(ExecutionResult {
        stdout: "done".to_string(),
        stderr: String::new(),
        exit_code: Some(0),
        state: TaskObservedState::Succeeded,
        category: Some(TaskFinalResultCategory::Succeeded),
    });

    let result = sync_once(
        &mut transport,
        &executor,
        &state,
        "agent-1",
        None,
        timestamp(),
    )
    .await
    .unwrap();

    assert_eq!(
        result,
        TaskSyncTick::Applied {
            task_id: "task-1".to_string(),
            final_state: TaskObservedState::Succeeded,
        }
    );
    assert_eq!(executor.calls(), vec!["task-1".to_string()]);
    assert_eq!(transport.apply_calls.len(), 3);
    assert_eq!(
        transport
            .apply_calls
            .iter()
            .map(|call| call.identity.task_id.as_str())
            .collect::<Vec<_>>(),
        vec!["task-1", "task-1", "task-1"]
    );
}

#[tokio::test]
async fn applies_acknowledged_running_and_terminal_updates_for_valid_tasks() {
    let state = synced_runtime_state(&["queued"]);
    let mut transport = RecordingTransport::new(vec![queued_command_task("task-1")]);
    let executor = RecordingExecutor::succeeds(ExecutionResult {
        stdout: "stdout-value".to_string(),
        stderr: "stderr-value".to_string(),
        exit_code: Some(7),
        state: TaskObservedState::Failed,
        category: Some(TaskFinalResultCategory::FailedNonZeroExit),
    });
    let now = timestamp();

    sync_once(&mut transport, &executor, &state, "agent-1", None, now)
        .await
        .unwrap();

    assert_eq!(transport.apply_calls.len(), 3);

    let acknowledged = &transport.apply_calls[0].patch;
    assert_eq!(
        acknowledged.observed_state,
        Some(TaskObservedState::Acknowledged)
    );
    assert_eq!(
        acknowledged.claimed_at.as_deref(),
        Some("2026-05-31T08:00:00Z")
    );
    assert_eq!(acknowledged.started_at, None);
    assert_eq!(acknowledged.finished_at, None);
    assert_eq!(
        acknowledged.updated_at.as_deref(),
        Some("2026-05-31T08:00:00Z")
    );

    let running = &transport.apply_calls[1].patch;
    assert_eq!(running.observed_state, Some(TaskObservedState::Running));
    assert_eq!(running.started_at.as_deref(), Some("2026-05-31T08:00:00Z"));
    assert_eq!(running.updated_at.as_deref(), Some("2026-05-31T08:00:00Z"));

    let terminal = &transport.apply_calls[2].patch;
    assert_eq!(terminal.observed_state, Some(TaskObservedState::Failed));
    assert_eq!(
        terminal.finished_at.as_deref(),
        Some("2026-05-31T08:00:00Z")
    );
    assert_eq!(terminal.stdout.as_deref(), Some("stdout-value"));
    assert_eq!(terminal.stderr.as_deref(), Some("stderr-value"));
    assert_eq!(terminal.exit_code, Some(7));
    assert_eq!(terminal.error_message, None);
    assert_eq!(terminal.updated_at.as_deref(), Some("2026-05-31T08:00:00Z"));
    assert_eq!(
        TaskFinalResultCategory::from_task_result(
            terminal.observed_state.expect("terminal state"),
            terminal.exit_code,
            terminal.error_message.as_deref(),
        ),
        Some(TaskFinalResultCategory::FailedNonZeroExit)
    );
}

#[tokio::test]
async fn rejects_invalid_mixed_payload_tasks_before_execution() {
    for (task, expected_error) in [
        (
            TaskResource {
                command_line: Some("sh".to_string()),
                ..queued_script_task("script-invalid")
            },
            "script task cannot include command_line",
        ),
        (
            TaskResource {
                script_content: Some("echo mixed".to_string()),
                ..queued_command_task("command-invalid")
            },
            "command task cannot include script_content",
        ),
    ] {
        let state = synced_runtime_state(&["queued"]);
        let mut transport = RecordingTransport::new(vec![task.clone()]);
        let executor = RecordingExecutor::succeeds(ExecutionResult {
            stdout: "should-not-run".to_string(),
            stderr: String::new(),
            exit_code: Some(0),
            state: TaskObservedState::Succeeded,
            category: Some(TaskFinalResultCategory::Succeeded),
        });

        let result = sync_once(
            &mut transport,
            &executor,
            &state,
            "agent-1",
            None,
            timestamp(),
        )
        .await
        .unwrap();

        assert_eq!(
            result,
            TaskSyncTick::Applied {
                task_id: task.task_id.clone(),
                final_state: TaskObservedState::Failed,
            }
        );
        assert!(executor.calls().is_empty());
        assert_eq!(transport.apply_calls.len(), 2);
        assert_eq!(
            transport.apply_calls[0].patch.observed_state,
            Some(TaskObservedState::Acknowledged)
        );
        assert_eq!(
            transport.apply_calls[1].patch.observed_state,
            Some(TaskObservedState::Failed)
        );
        assert_eq!(
            transport.apply_calls[1].patch.error_message.as_deref(),
            Some(format!("failed_invalid_payload: {expected_error}").as_str())
        );
        assert_eq!(
            TaskFinalResultCategory::from_task_result(
                transport.apply_calls[1]
                    .patch
                    .observed_state
                    .expect("terminal state"),
                transport.apply_calls[1].patch.exit_code,
                transport.apply_calls[1].patch.error_message.as_deref(),
            ),
            Some(TaskFinalResultCategory::FailedInvalidPayload)
        );
    }
}

#[tokio::test]
async fn maps_executor_start_failures_to_failed_executor_start_category() {
    let state = synced_runtime_state(&["queued"]);
    let mut transport = RecordingTransport::new(vec![queued_command_task("task-start-failure")]);
    let executor =
        RecordingExecutor::fails("failed to spawn task task-start-failure: missing binary");

    let result = sync_once(
        &mut transport,
        &executor,
        &state,
        "agent-1",
        None,
        timestamp(),
    )
    .await
    .unwrap();

    assert_eq!(
        result,
        TaskSyncTick::Applied {
            task_id: "task-start-failure".to_string(),
            final_state: TaskObservedState::Failed,
        }
    );
    assert_eq!(transport.apply_calls.len(), 3);
    let terminal = &transport.apply_calls[2].patch;
    assert_eq!(terminal.observed_state, Some(TaskObservedState::Failed));
    assert_eq!(terminal.exit_code, None);
    assert_eq!(
        terminal.error_message.as_deref(),
        Some("failed_executor_start: failed to spawn task task-start-failure: missing binary")
    );
    assert_eq!(
        TaskFinalResultCategory::from_task_result(
            terminal.observed_state.expect("terminal state"),
            terminal.exit_code,
            terminal.error_message.as_deref(),
        ),
        Some(TaskFinalResultCategory::FailedExecutorStart)
    );
}

#[tokio::test]
async fn maps_non_spawn_executor_failures_to_failed_executor_start_category() {
    let state = synced_runtime_state(&["queued"]);
    let mut transport =
        RecordingTransport::new(vec![queued_command_task("task-exec-setup-failure")]);
    let executor = RecordingExecutor::fails("executor output collection failed");

    let result = sync_once(
        &mut transport,
        &executor,
        &state,
        "agent-1",
        None,
        timestamp(),
    )
    .await
    .unwrap();

    assert_eq!(
        result,
        TaskSyncTick::Applied {
            task_id: "task-exec-setup-failure".to_string(),
            final_state: TaskObservedState::Failed,
        }
    );
    let terminal = &transport.apply_calls[2].patch;
    assert_eq!(terminal.observed_state, Some(TaskObservedState::Failed));
    assert_eq!(terminal.exit_code, None);
    assert_eq!(
        terminal.error_message.as_deref(),
        Some("failed_executor_start: executor output collection failed")
    );
    assert_eq!(
        TaskFinalResultCategory::from_task_result(
            terminal.observed_state.expect("terminal state"),
            terminal.exit_code,
            terminal.error_message.as_deref(),
        ),
        Some(TaskFinalResultCategory::FailedExecutorStart)
    );
}

#[tokio::test]
async fn preserves_timeout_category_in_terminal_patch() {
    let state = synced_runtime_state(&["queued"]);
    let mut transport = RecordingTransport::new(vec![queued_command_task("task-timeout")]);
    let executor = RecordingExecutor::succeeds(ExecutionResult {
        stdout: String::new(),
        stderr: String::new(),
        exit_code: None,
        state: TaskObservedState::Timeout,
        category: Some(TaskFinalResultCategory::Timeout),
    });

    let result = sync_once(
        &mut transport,
        &executor,
        &state,
        "agent-1",
        None,
        timestamp(),
    )
    .await
    .unwrap();

    assert_eq!(
        result,
        TaskSyncTick::Applied {
            task_id: "task-timeout".to_string(),
            final_state: TaskObservedState::Timeout,
        }
    );
    let terminal = &transport.apply_calls[2].patch;
    assert_eq!(terminal.observed_state, Some(TaskObservedState::Timeout));
    assert_eq!(
        TaskFinalResultCategory::from_task_result(
            terminal.observed_state.expect("terminal state"),
            terminal.exit_code,
            terminal.error_message.as_deref(),
        ),
        Some(TaskFinalResultCategory::Timeout)
    );
}

#[tokio::test]
async fn does_not_reacknowledge_task_visible_again_after_claim_timeout_or_network_loss() {
    let state = synced_runtime_state(&["queued", "acknowledged", "running"]);
    let mut transport = RecordingTransport::new(vec![acknowledged_command_task("task-claimed")]);
    let executor = RecordingExecutor::succeeds(ExecutionResult {
        stdout: "reconciled".to_string(),
        stderr: String::new(),
        exit_code: Some(0),
        state: TaskObservedState::Succeeded,
        category: Some(TaskFinalResultCategory::Succeeded),
    });

    let result = sync_once(
        &mut transport,
        &executor,
        &state,
        "agent-1",
        None,
        timestamp(),
    )
    .await
    .unwrap();

    assert_eq!(
        result,
        TaskSyncTick::Applied {
            task_id: "task-claimed".to_string(),
            final_state: TaskObservedState::Succeeded,
        }
    );
    assert_eq!(executor.calls(), vec!["task-claimed".to_string()]);
    assert_eq!(transport.apply_calls.len(), 2);
    assert_eq!(
        transport.apply_calls[0].patch.observed_state,
        Some(TaskObservedState::Running)
    );
    assert_eq!(transport.apply_calls[0].patch.claimed_at, None);
    assert_eq!(
        transport.apply_calls[1].patch.observed_state,
        Some(TaskObservedState::Succeeded)
    );
}

#[tokio::test]
async fn restart_reconciles_running_task_without_creating_a_second_active_execution() {
    let state = synced_runtime_state(&["queued", "acknowledged", "running"]);
    let mut transport = RecordingTransport::new(vec![running_command_task("task-running")]);
    let executor = RecordingExecutor::succeeds(ExecutionResult {
        stdout: "should-not-run".to_string(),
        stderr: String::new(),
        exit_code: Some(0),
        state: TaskObservedState::Succeeded,
        category: Some(TaskFinalResultCategory::Succeeded),
    });

    let result = sync_once(
        &mut transport,
        &executor,
        &state,
        "agent-1",
        None,
        timestamp(),
    )
    .await
    .unwrap();

    assert_eq!(
        result,
        TaskSyncTick::Applied {
            task_id: "task-running".to_string(),
            final_state: TaskObservedState::Failed,
        }
    );
    let transport = RecordingTransport::new(vec![running_command_task("task-running")]);
    let executor = RecordingExecutor::succeeds(ExecutionResult {
        stdout: "should-not-run".to_string(),
        stderr: String::new(),
        exit_code: Some(0),
        state: TaskObservedState::Succeeded,
        category: Some(TaskFinalResultCategory::Succeeded),
    });
    let mut loop_runner = TaskSyncLoop::new(transport, executor.clone());
    let _ = loop_runner
        .tick(&state, "agent-1", timestamp())
        .await
        .unwrap();
    assert_eq!(
        loop_runner.last_diagnostic(),
        Some(&TaskDecisionDiagnostic {
            task_id: Some("task-running".to_string()),
            observed_state: Some(TaskObservedState::Running),
            lifecycle_stage: TaskLifecycleStage::PublishedTerminalResult,
            outcome: TaskOutcomeKind::FailedExecutorStart,
            detail: "task was already running before reconciliation; skipping re-execution"
                .to_string(),
        })
    );
    let transport = loop_runner.into_transport();
    assert!(executor.calls().is_empty());
    assert_eq!(transport.apply_calls.len(), 1);
    assert_eq!(
        transport.apply_calls[0].patch.observed_state,
        Some(TaskObservedState::Failed)
    );
    assert_eq!(
        transport.apply_calls[0].patch.finished_at.as_deref(),
        Some("2026-05-31T08:00:00Z")
    );
    assert_eq!(
        transport.apply_calls[0].patch.error_message.as_deref(),
        Some(
            "failed_executor_start: task was already running before reconciliation; skipping re-execution"
        )
    );
    assert_eq!(
        TaskFinalResultCategory::from_task_result(
            transport.apply_calls[0]
                .patch
                .observed_state
                .expect("terminal state"),
            transport.apply_calls[0].patch.exit_code,
            transport.apply_calls[0].patch.error_message.as_deref(),
        ),
        Some(TaskFinalResultCategory::FailedExecutorStart)
    );
}

#[tokio::test]
async fn returns_running_apply_failure_without_executing_task() {
    let state = synced_runtime_state(&["queued"]);
    let mut transport = RecordingTransport::new(vec![queued_command_task("task-running-error")])
        .with_apply_failure(
            TaskObservedState::Running,
            "job-manage refused running transition",
        );
    let executor = RecordingExecutor::succeeds(ExecutionResult {
        stdout: "should-not-run".to_string(),
        stderr: String::new(),
        exit_code: Some(0),
        state: TaskObservedState::Succeeded,
        category: Some(TaskFinalResultCategory::Succeeded),
    });

    let error = sync_once(
        &mut transport,
        &executor,
        &state,
        "agent-1",
        None,
        timestamp(),
    )
    .await
    .expect_err("running apply failure should be returned");

    assert!(
        error
            .to_string()
            .contains("job-manage refused running transition")
    );
    assert_eq!(executor.calls(), Vec::<String>::new());
    assert_eq!(transport.apply_calls.len(), 2);
    assert_eq!(
        transport.apply_calls[0].patch.observed_state,
        Some(TaskObservedState::Acknowledged)
    );
    assert_eq!(
        transport.apply_calls[1].patch.observed_state,
        Some(TaskObservedState::Running)
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn sync_once_with_task_sync_service_publishes_final_state() {
    let state = synced_runtime_state(&["queued", "acknowledged", "running"]);
    let service = TaskSyncService::new(vec![queued_script_task("task-service")]);
    let mut transport = ServiceBackedTransport::new("http://job-manage/tasks", service.clone());
    let executor = RecordingExecutor::succeeds(ExecutionResult {
        stdout: "service-stdout".to_string(),
        stderr: String::new(),
        exit_code: Some(0),
        state: TaskObservedState::Succeeded,
        category: Some(TaskFinalResultCategory::Succeeded),
    });

    let result = sync_once(
        &mut transport,
        &executor,
        &state,
        "agent-1",
        None,
        timestamp(),
    )
    .await
    .unwrap();

    assert_eq!(
        result,
        TaskSyncTick::Applied {
            task_id: "task-service".to_string(),
            final_state: TaskObservedState::Succeeded,
        }
    );

    let stored = service
        .list_tasks(&TaskListQuery {
            agent_id: "agent-1".to_string(),
            node_id: "node-1".to_string(),
            states: vec![TaskObservedState::Succeeded],
            updated_after: None,
        })
        .await
        .unwrap();
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].observed_state, TaskObservedState::Succeeded);
    assert_eq!(stored[0].stdout.as_deref(), Some("service-stdout"));
    assert_eq!(stored[0].exit_code, Some(0));
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ListCall {
    endpoint: String,
    query: TaskListQuery,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ApplyCall {
    endpoint: String,
    identity: TaskApplyIdentity,
    patch: TaskApplyPatch,
}

#[derive(Debug, Clone)]
struct RecordingTransport {
    list_calls: Vec<ListCall>,
    apply_calls: Vec<ApplyCall>,
    list_failures: VecDeque<String>,
    list_responses: VecDeque<Vec<TaskResource>>,
    tasks: BTreeMap<String, TaskResource>,
    apply_failures: Vec<(TaskObservedState, String)>,
}

impl RecordingTransport {
    fn new(tasks: Vec<TaskResource>) -> Self {
        Self {
            list_calls: Vec::new(),
            apply_calls: Vec::new(),
            list_failures: VecDeque::new(),
            list_responses: VecDeque::from([tasks.clone()]),
            tasks: tasks
                .into_iter()
                .map(|task| (task.task_id.clone(), task))
                .collect(),
            apply_failures: Vec::new(),
        }
    }

    fn stateful(tasks: Vec<TaskResource>) -> Self {
        Self {
            list_calls: Vec::new(),
            apply_calls: Vec::new(),
            list_failures: VecDeque::new(),
            list_responses: VecDeque::new(),
            tasks: tasks
                .into_iter()
                .map(|task| (task.task_id.clone(), task))
                .collect(),
            apply_failures: Vec::new(),
        }
    }

    fn with_list_responses(list_responses: Vec<Vec<TaskResource>>) -> Self {
        let tasks = list_responses
            .iter()
            .flatten()
            .cloned()
            .map(|task| (task.task_id.clone(), task))
            .collect();

        Self {
            list_calls: Vec::new(),
            apply_calls: Vec::new(),
            list_failures: VecDeque::new(),
            list_responses: VecDeque::from(list_responses),
            tasks,
            apply_failures: Vec::new(),
        }
    }

    fn with_list_failures(messages: Vec<&str>) -> Self {
        Self {
            list_calls: Vec::new(),
            apply_calls: Vec::new(),
            list_failures: messages.into_iter().map(ToString::to_string).collect(),
            list_responses: VecDeque::new(),
            tasks: BTreeMap::new(),
            apply_failures: Vec::new(),
        }
    }

    fn with_apply_failure(mut self, state: TaskObservedState, message: impl Into<String>) -> Self {
        self.apply_failures.push((state, message.into()));
        self
    }
}

#[derive(Debug, Clone)]
struct ServiceBackedTransport {
    tasks_endpoint: String,
    apply_endpoint: String,
    service: TaskSyncService,
}

impl ServiceBackedTransport {
    fn new(base_tasks_endpoint: &str, service: TaskSyncService) -> Self {
        Self {
            tasks_endpoint: base_tasks_endpoint.to_string(),
            apply_endpoint: format!("{base_tasks_endpoint}:apply"),
            service,
        }
    }
}

impl JobManageTransport for RecordingTransport {
    fn list_tasks(&mut self, endpoint: &str, query: &TaskListQuery) -> Result<Vec<TaskResource>> {
        self.list_calls.push(ListCall {
            endpoint: endpoint.to_string(),
            query: query.clone(),
        });

        if let Some(message) = self.list_failures.pop_front() {
            return Err(anyhow::anyhow!(message));
        }

        if let Some(response) = self.list_responses.pop_front() {
            return Ok(response);
        }

        Ok(self
            .tasks
            .values()
            .filter(|task| task.agent_id == query.agent_id && task.node_id == query.node_id)
            .filter(|task| query.states.is_empty() || query.states.contains(&task.observed_state))
            .filter(|task| match query.updated_after.as_deref() {
                Some(updated_after) => task
                    .updated_at
                    .as_deref()
                    .map(|updated_at| updated_at > updated_after)
                    .unwrap_or(false),
                None => true,
            })
            .cloned()
            .collect())
    }

    fn apply_task(
        &mut self,
        endpoint: &str,
        identity: &TaskApplyIdentity,
        patch: &TaskApplyPatch,
    ) -> Result<TaskApplyAck> {
        self.apply_calls.push(ApplyCall {
            endpoint: endpoint.to_string(),
            identity: identity.clone(),
            patch: patch.clone(),
        });

        if let Some(observed_state) = patch.observed_state
            && let Some((_, message)) = self
                .apply_failures
                .iter()
                .find(|(state, _)| *state == observed_state)
        {
            return Err(anyhow::anyhow!(message.clone()));
        }

        let task = self
            .tasks
            .get_mut(&identity.task_id)
            .expect("task exists in fake transport");

        if let Some(observed_state) = patch.observed_state {
            task.observed_state = observed_state;
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
        if let Some(stdout) = &patch.stdout {
            task.stdout = Some(stdout.clone());
        }
        if let Some(stderr) = &patch.stderr {
            task.stderr = Some(stderr.clone());
        }
        if let Some(exit_code) = patch.exit_code {
            task.exit_code = Some(exit_code);
        }
        if let Some(error_message) = &patch.error_message {
            task.error_message = Some(error_message.clone());
        }
        if let Some(updated_at) = &patch.updated_at {
            task.updated_at = Some(updated_at.clone());
        }

        Ok(TaskApplyAck {
            task_id: task.task_id.clone(),
            observed_state: task.observed_state,
            updated_at: task.updated_at.clone(),
        })
    }
}

impl JobManageTransport for ServiceBackedTransport {
    fn list_tasks(&mut self, endpoint: &str, query: &TaskListQuery) -> Result<Vec<TaskResource>> {
        assert_eq!(endpoint, self.tasks_endpoint);
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current()
                .block_on(self.service.list_tasks(query))
                .map_err(|err| anyhow::anyhow!(err.to_string()))
        })
    }

    fn apply_task(
        &mut self,
        endpoint: &str,
        identity: &TaskApplyIdentity,
        patch: &TaskApplyPatch,
    ) -> Result<TaskApplyAck> {
        assert_eq!(endpoint, self.apply_endpoint);
        let task = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current()
                .block_on(self.service.apply_task(
                    identity,
                    TaskApplyRequest {
                        patch: patch.clone(),
                        rejected_fields: vec![],
                    },
                ))
                .map_err(|err| anyhow::anyhow!(err.to_string()))
        })?;

        Ok(TaskApplyAck {
            task_id: task.task_id,
            observed_state: task.observed_state,
            updated_at: task.updated_at,
        })
    }
}

#[derive(Debug)]
struct ExpectedHttpRequest {
    method: &'static str,
    target: &'static str,
    body: Option<Value>,
    response: ExpectedHttpResponse,
}

#[derive(Debug)]
struct ExpectedHttpResponse {
    status_code: u16,
    reason_phrase: &'static str,
    body: String,
}

impl ExpectedHttpResponse {
    fn ok(body: Value) -> Self {
        Self {
            status_code: 200,
            reason_phrase: "OK",
            body: body.to_string(),
        }
    }

    fn status(status_code: u16, reason_phrase: &'static str, body: Value) -> Self {
        Self {
            status_code,
            reason_phrase,
            body: body.to_string(),
        }
    }
}

#[derive(Debug)]
struct TestHttpServer {
    base_url: String,
    handle: Option<thread::JoinHandle<()>>,
}

impl TestHttpServer {
    fn spawn(expected_requests: Vec<ExpectedHttpRequest>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test http server");
        let address = listener.local_addr().expect("server local addr");
        let handle = thread::spawn(move || {
            for expected in expected_requests {
                let (mut stream, _) = listener.accept().expect("accept request");
                let mut reader = BufReader::new(stream.try_clone().expect("clone stream"));

                let mut request_line = String::new();
                reader
                    .read_line(&mut request_line)
                    .expect("read request line");
                let request_line = request_line.trim_end();
                let mut parts = request_line.split_whitespace();
                let method = parts.next().expect("request method");
                let target = parts.next().expect("request target");
                assert_eq!(method, expected.method);
                assert_eq!(target, expected.target);

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
                reader.read_exact(&mut body).expect("read request body");
                match expected.body {
                    Some(expected_body) => {
                        let actual_body: Value = serde_json::from_slice(&body).expect("json body");
                        assert_eq!(actual_body, expected_body);
                    }
                    None => assert!(body.is_empty()),
                }

                let response = format!(
                    "HTTP/1.1 {} {}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                    expected.response.status_code,
                    expected.response.reason_phrase,
                    expected.response.body.len(),
                    expected.response.body
                );
                stream
                    .write_all(response.as_bytes())
                    .expect("write response");
                stream.flush().expect("flush response");
            }
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

#[derive(Debug, Clone)]
enum RecordedExecution {
    Success(ExecutionResult),
    Failure(String),
}

#[derive(Debug, Clone)]
struct RecordingExecutor {
    calls: Arc<Mutex<Vec<String>>>,
    result: RecordedExecution,
}

impl RecordingExecutor {
    fn succeeds(result: ExecutionResult) -> Self {
        Self {
            calls: Arc::new(Mutex::new(Vec::new())),
            result: RecordedExecution::Success(result),
        }
    }

    fn fails(message: impl Into<String>) -> Self {
        Self {
            calls: Arc::new(Mutex::new(Vec::new())),
            result: RecordedExecution::Failure(message.into()),
        }
    }

    fn calls(&self) -> Vec<String> {
        self.calls.lock().expect("executor calls lock").clone()
    }
}

impl TaskExecutor for RecordingExecutor {
    type ExecuteFuture<'a>
        = Pin<Box<dyn Future<Output = Result<ExecutionResult>> + Send + 'a>>
    where
        Self: 'a;

    fn execute<'a>(&'a self, task: &'a TaskResource) -> Self::ExecuteFuture<'a> {
        self.calls
            .lock()
            .expect("executor calls lock")
            .push(task.task_id.clone());
        let result = self.result.clone();

        Box::pin(async move {
            match result {
                RecordedExecution::Success(result) => Ok(result),
                RecordedExecution::Failure(message) => Err(anyhow::anyhow!(message)),
            }
        })
    }
}

fn queued_script_task(task_id: &str) -> TaskResource {
    TaskResource {
        task_id: task_id.to_string(),
        job_id: "job-1".to_string(),
        node_id: "node-1".to_string(),
        agent_id: "agent-1".to_string(),
        task_type: TaskType::Script,
        script_content: Some("echo hello".to_string()),
        command_line: None,
        interpreter: Some("sh".to_string()),
        args: Vec::new(),
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
        updated_at: Some("2026-05-31T07:59:00Z".to_string()),
    }
}

fn queued_command_task(task_id: &str) -> TaskResource {
    TaskResource {
        task_type: TaskType::Command,
        script_content: None,
        command_line: Some("sh".to_string()),
        ..queued_script_task(task_id)
    }
}

fn acknowledged_command_task(task_id: &str) -> TaskResource {
    TaskResource {
        observed_state: TaskObservedState::Acknowledged,
        claimed_at: Some("2026-05-31T07:59:30Z".to_string()),
        updated_at: Some("2026-05-31T07:59:30Z".to_string()),
        ..queued_command_task(task_id)
    }
}

fn running_command_task(task_id: &str) -> TaskResource {
    TaskResource {
        observed_state: TaskObservedState::Running,
        claimed_at: Some("2026-05-31T07:59:30Z".to_string()),
        started_at: Some("2026-05-31T07:59:40Z".to_string()),
        updated_at: Some("2026-05-31T07:59:40Z".to_string()),
        ..queued_command_task(task_id)
    }
}

fn synced_runtime_state(default_states: &[&str]) -> AgentRuntimeState {
    let config = AgentRuntimeConfig {
        nodemanage_sync_url: "http://127.0.0.1:3000/agent/sync".to_string(),
        agent_id: "agent-1".to_string(),
        node_id: None,
        data_dir: "/var/lib/rsagent".to_string(),
        sync_interval_secs: 60,
    };
    let mut state = AgentRuntimeState::new(config);
    state.apply_sync_response(AgentSyncResponse {
        accepted: true,
        agent_id: "agent-1".to_string(),
        bound_node_id: "node-1".to_string(),
        binding_state: SyncBindingState::Bound,
        agent_run_mode: AgentRunMode::Active,
        config_version: "cfg-1".to_string(),
        heartbeat_config: HeartbeatConfig {
            version: "hb-v1".to_string(),
            data_link_id: "dl-1".to_string(),
            vm_base_url: "http://vm".to_string(),
            interval_secs: 15,
        },
        job_manage_config: JobManageConfig {
            version: "jm-v1".to_string(),
            base_url: "http://job-manage".to_string(),
            task_filter_defaults: TaskFilterDefaults {
                states: default_states
                    .iter()
                    .map(|state| state.to_string())
                    .collect(),
            },
        },
        sync_interval_secs: 10,
        task_sync_interval_secs: 5,
        rejection_reason: None,
    });
    state
}

fn timestamp() -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 5, 31, 8, 0, 0).unwrap()
}
