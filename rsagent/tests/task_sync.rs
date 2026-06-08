use std::{
    collections::{BTreeMap, VecDeque},
    future::Future,
    io::{BufRead, BufReader, Read, Write},
    net::TcpListener,
    pin::Pin,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use anyhow::Result;
use chrono::{TimeZone, Utc};
use job_manage::{
    TaskApplyIdentity, TaskApplyPatch, TaskApplyRequest, TaskDesiredState, TaskListQuery,
    TaskObservedState, TaskResource, TaskSyncService, TaskType,
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
    task_sync::{TaskExecutor, TaskSyncLoop, TaskSyncSkipReason, TaskSyncTick, sync_once},
};
use serde_json::{Value, json};

#[tokio::test]
async fn reqwest_transport_uses_job_manage_http_contract() {
    let task = queued_script_task("task-http");
    let server = TestHttpServer::spawn(vec![
        ExpectedHttpRequest {
            method: "GET",
            target: "/tasks?agent_id=agent-1&node_id=node-1&states=queued%2Crunning&updated_after=2026-05-31T08%3A00%3A00Z",
            body: None,
            response_body: json!({
                "success": true,
                "data": { "items": [task.clone()] },
                "error": null
            })
            .to_string(),
        },
        ExpectedHttpRequest {
            method: "POST",
            target: "/tasks:apply?task_id=task-http&agent_id=agent-1&node_id=node-1",
            body: Some(json!({
                "observed_state": "acknowledged",
                "claimed_at": "2026-05-31T08:00:00Z",
                "updated_at": "2026-05-31T08:00:00Z"
            })),
            response_body: json!({
                "success": true,
                "data": {
                    "task_id": "task-http",
                    "observed_state": "acknowledged",
                    "updated_at": "2026-05-31T08:00:00Z"
                },
                "error": null
            })
            .to_string(),
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
        .await
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
        .await
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

#[tokio::test]
async fn task_sync_loop_carries_updated_after_between_ticks() {
    let state = synced_runtime_state(&["queued"]);
    let transport = RecordingTransport::with_list_responses(vec![vec![], vec![]]);
    let executor = RecordingExecutor::succeeds(ExecutionResult {
        stdout: String::new(),
        stderr: String::new(),
        exit_code: Some(0),
        state: TaskObservedState::Succeeded,
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
async fn task_sync_retries_list_transport_failure_before_claiming_task() {
    let state = synced_runtime_state(&["queued"]);
    let mut transport = RecordingTransport::with_list_results(vec![
        Err("temporary list failure 1"),
        Ok(vec![queued_script_task("task-retry-list")]),
    ]);
    let executor = RecordingExecutor::succeeds(ExecutionResult {
        stdout: "done".to_string(),
        stderr: String::new(),
        exit_code: Some(0),
        state: TaskObservedState::Succeeded,
    });

    let tick = sync_once(
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
        tick,
        TaskSyncTick::Applied {
            task_id: "task-retry-list".to_string(),
            final_state: TaskObservedState::Succeeded,
        }
    );
    assert_eq!(transport.list_calls.len(), 2);
    assert_eq!(executor.calls(), vec!["task-retry-list".to_string()]);
}

#[tokio::test]
async fn task_sync_retries_apply_transport_failure_without_reexecuting_work() {
    let state = synced_runtime_state(&["queued"]);
    let mut transport = RecordingTransport::new(vec![queued_script_task("task-retry-apply")]);
    transport.apply_outcomes = VecDeque::from([
        Err("temporary apply failure 1".to_string()),
        Ok(TaskObservedState::Acknowledged),
        Ok(TaskObservedState::Running),
        Err("temporary apply failure 2".to_string()),
        Ok(TaskObservedState::Succeeded),
    ]);
    let executor = RecordingExecutor::succeeds(ExecutionResult {
        stdout: "done".to_string(),
        stderr: String::new(),
        exit_code: Some(0),
        state: TaskObservedState::Succeeded,
    });

    let tick = sync_once(
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
        tick,
        TaskSyncTick::Applied {
            task_id: "task-retry-apply".to_string(),
            final_state: TaskObservedState::Succeeded,
        }
    );
    assert_eq!(executor.calls(), vec!["task-retry-apply".to_string()]);
    assert_eq!(transport.apply_calls.len(), 5);
    assert_eq!(
        transport
            .apply_calls
            .iter()
            .map(|call| call.patch.observed_state)
            .collect::<Vec<_>>(),
        vec![
            Some(TaskObservedState::Acknowledged),
            Some(TaskObservedState::Acknowledged),
            Some(TaskObservedState::Running),
            Some(TaskObservedState::Succeeded),
            Some(TaskObservedState::Succeeded),
        ]
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn task_sync_retries_acknowledged_apply_after_persisted_response_loss_without_duplicate_execution()
 {
    let state = synced_runtime_state(&["queued"]);
    let service = TaskSyncService::new(vec![queued_script_task("task-ack-response-loss")]);
    let mut transport = PersistingApplyErrorTransport::new("http://job-manage/tasks", service);
    transport.apply_persisted_errors =
        VecDeque::from([Some("lost acknowledged apply response".to_string())]);
    let executor = RecordingExecutor::succeeds(ExecutionResult {
        stdout: "done".to_string(),
        stderr: String::new(),
        exit_code: Some(0),
        state: TaskObservedState::Succeeded,
    });

    let tick = sync_once(
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
        tick,
        TaskSyncTick::Applied {
            task_id: "task-ack-response-loss".to_string(),
            final_state: TaskObservedState::Succeeded,
        }
    );
    assert_eq!(executor.calls(), vec!["task-ack-response-loss".to_string()]);
    assert_eq!(transport.apply_calls.len(), 4);
    assert_eq!(
        transport
            .apply_calls
            .iter()
            .map(|call| call.patch.observed_state)
            .collect::<Vec<_>>(),
        vec![
            Some(TaskObservedState::Acknowledged),
            Some(TaskObservedState::Acknowledged),
            Some(TaskObservedState::Running),
            Some(TaskObservedState::Succeeded),
        ]
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn task_sync_retries_running_apply_after_persisted_response_loss_without_duplicate_execution()
{
    let state = synced_runtime_state(&["queued"]);
    let service = TaskSyncService::new(vec![queued_script_task("task-running-response-loss")]);
    let mut transport = PersistingApplyErrorTransport::new("http://job-manage/tasks", service);
    transport.apply_persisted_errors =
        VecDeque::from([None, Some("lost running apply response".to_string())]);
    let executor = RecordingExecutor::succeeds(ExecutionResult {
        stdout: "done".to_string(),
        stderr: String::new(),
        exit_code: Some(0),
        state: TaskObservedState::Succeeded,
    });

    let tick = sync_once(
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
        tick,
        TaskSyncTick::Applied {
            task_id: "task-running-response-loss".to_string(),
            final_state: TaskObservedState::Succeeded,
        }
    );
    assert_eq!(
        executor.calls(),
        vec!["task-running-response-loss".to_string()]
    );
    assert_eq!(transport.apply_calls.len(), 4);
    assert_eq!(
        transport
            .apply_calls
            .iter()
            .map(|call| call.patch.observed_state)
            .collect::<Vec<_>>(),
        vec![
            Some(TaskObservedState::Acknowledged),
            Some(TaskObservedState::Running),
            Some(TaskObservedState::Running),
            Some(TaskObservedState::Succeeded),
        ]
    );
}

#[tokio::test]
async fn task_sync_stops_after_list_retry_budget_exhaustion_without_execution() {
    let state = synced_runtime_state(&["queued"]);
    let mut transport = RecordingTransport::with_list_results(vec![
        Err("temporary list failure 1"),
        Err("temporary list failure 2"),
        Err("temporary list failure 3"),
    ]);
    let executor = RecordingExecutor::succeeds(ExecutionResult {
        stdout: "done".to_string(),
        stderr: String::new(),
        exit_code: Some(0),
        state: TaskObservedState::Succeeded,
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
    .unwrap_err();

    assert!(error.to_string().contains("temporary list failure 3"));
    assert_eq!(transport.list_calls.len(), 3);
    assert!(transport.apply_calls.is_empty());
    assert!(executor.calls().is_empty());
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
async fn lists_tasks_with_default_filters_and_active_recovery_states_from_runtime_config() {
    let state = synced_runtime_state(&["queued", "running"]);
    let mut transport = RecordingTransport::new(Vec::new());
    let executor = RecordingExecutor::succeeds(ExecutionResult {
        stdout: String::new(),
        stderr: String::new(),
        exit_code: Some(0),
        state: TaskObservedState::Succeeded,
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
                states: vec![
                    TaskObservedState::Queued,
                    TaskObservedState::Running,
                    TaskObservedState::Acknowledged,
                ],
                updated_after: None,
            },
        }]
    );
    assert!(transport.apply_calls.is_empty());
    assert!(executor.calls().is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn skips_stranded_active_tasks_after_restart_when_defaults_only_include_queued() {
    let state = synced_runtime_state(&["queued"]);
    let service = TaskSyncService::new(vec![TaskResource {
        observed_state: TaskObservedState::Acknowledged,
        claimed_at: Some("2026-05-31T07:58:00Z".to_string()),
        updated_at: Some("2026-05-31T07:59:00Z".to_string()),
        ..queued_script_task("task-stranded")
    }]);
    let mut transport = ObservedServiceTransport::new("http://job-manage/tasks", service);
    let executor = RecordingExecutor::succeeds(ExecutionResult {
        stdout: "should-not-run".to_string(),
        stderr: String::new(),
        exit_code: Some(0),
        state: TaskObservedState::Succeeded,
    });

    let tick = sync_once(
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
        tick,
        TaskSyncTick::Skipped {
            reason: TaskSyncSkipReason::ExistingActiveTask {
                task_id: "task-stranded".to_string(),
                observed_state: TaskObservedState::Acknowledged,
            },
        }
    );
    assert_eq!(transport.list_calls.len(), 1);
    assert_eq!(
        transport.list_calls[0].query.states,
        vec![
            TaskObservedState::Queued,
            TaskObservedState::Acknowledged,
            TaskObservedState::Running,
        ]
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
    let running_started_at = parse_timestamp(running.started_at.as_deref().expect("started_at"));
    assert!(running_started_at >= parse_timestamp("2026-05-31T08:00:00Z"));
    assert_eq!(running.updated_at.as_deref(), running.started_at.as_deref());

    let terminal = &transport.apply_calls[2].patch;
    assert_eq!(terminal.observed_state, Some(TaskObservedState::Failed));
    let terminal_finished_at =
        parse_timestamp(terminal.finished_at.as_deref().expect("finished_at"));
    assert!(terminal_finished_at >= running_started_at);
    assert_eq!(terminal.stdout.as_deref(), Some("stdout-value"));
    assert_eq!(terminal.stderr.as_deref(), Some("stderr-value"));
    assert_eq!(terminal.exit_code, Some(7));
    assert_eq!(terminal.error_message, None);
    assert_eq!(
        terminal.updated_at.as_deref(),
        terminal.finished_at.as_deref()
    );
}

#[tokio::test]
async fn skips_preclaimed_active_tasks_without_regressing_state_or_execution() {
    for observed_state in [TaskObservedState::Acknowledged, TaskObservedState::Running] {
        let state = synced_runtime_state(&["queued", "acknowledged", "running"]);
        let mut transport = RecordingTransport::new(vec![TaskResource {
            observed_state,
            claimed_at: Some("2026-05-31T07:58:00Z".to_string()),
            started_at: matches!(observed_state, TaskObservedState::Running)
                .then(|| "2026-05-31T07:59:00Z".to_string()),
            updated_at: Some("2026-05-31T07:59:00Z".to_string()),
            ..queued_script_task("task-active")
        }]);
        let executor = RecordingExecutor::succeeds(ExecutionResult {
            stdout: "should-not-run".to_string(),
            stderr: String::new(),
            exit_code: Some(0),
            state: TaskObservedState::Succeeded,
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

        match result {
            TaskSyncTick::Skipped { reason } => {
                assert_eq!(
                    format!("{reason:?}"),
                    format!(
                        "ExistingActiveTask {{ task_id: \"task-active\", observed_state: {observed_state:?} }}"
                    )
                );
            }
            other => panic!("expected active-task skip, got {other:?}"),
        }
        assert!(transport.apply_calls.is_empty());
        assert!(executor.calls().is_empty());
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn task_sync_loop_does_not_advance_cursor_while_backend_keeps_returning_active_task() {
    let state = synced_runtime_state(&["queued", "acknowledged", "running"]);
    let service = TaskSyncService::new(vec![
        TaskResource {
            observed_state: TaskObservedState::Running,
            claimed_at: Some("2026-05-31T07:58:00Z".to_string()),
            started_at: Some("2026-05-31T07:59:00Z".to_string()),
            updated_at: Some("2026-05-31T07:59:00Z".to_string()),
            ..queued_script_task("task-running")
        },
        TaskResource {
            updated_at: Some("2026-05-31T08:01:00Z".to_string()),
            ..queued_script_task("task-queued")
        },
    ]);
    let transport = ObservedServiceTransport::new("http://job-manage/tasks", service);
    let executor = RecordingExecutor::succeeds(ExecutionResult {
        stdout: "should-not-run".to_string(),
        stderr: String::new(),
        exit_code: Some(0),
        state: TaskObservedState::Succeeded,
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

    for tick in [first, second] {
        match tick {
            TaskSyncTick::Skipped { reason } => assert_eq!(
                format!("{reason:?}"),
                "ExistingActiveTask { task_id: \"task-running\", observed_state: Running }"
            ),
            other => panic!("expected active-task skip, got {other:?}"),
        }
    }

    let transport = loop_runner.into_transport();
    assert_eq!(transport.list_calls.len(), 2);
    assert_eq!(transport.list_calls[0].query.updated_after, None);
    assert_eq!(transport.list_calls[1].query.updated_after, None);
    assert!(transport.apply_calls.is_empty());
    assert!(executor.calls().is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn task_sync_loop_processes_remaining_queued_work_on_later_ticks() {
    let state = synced_runtime_state(&["queued"]);
    let service = TaskSyncService::new(vec![
        TaskResource {
            updated_at: Some("2026-05-31T07:58:00Z".to_string()),
            ..queued_script_task("task-older")
        },
        TaskResource {
            updated_at: Some("2026-05-31T07:59:00Z".to_string()),
            ..queued_script_task("task-newer")
        },
    ]);
    let transport = ServiceBackedTransport::new("http://job-manage/tasks", service);
    let executor = RecordingExecutor::succeeds(ExecutionResult {
        stdout: "done".to_string(),
        stderr: String::new(),
        exit_code: Some(0),
        state: TaskObservedState::Succeeded,
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
        TaskSyncTick::Applied {
            task_id: "task-older".to_string(),
            final_state: TaskObservedState::Succeeded,
        }
    );
    assert_eq!(
        second,
        TaskSyncTick::Applied {
            task_id: "task-newer".to_string(),
            final_state: TaskObservedState::Succeeded,
        }
    );
    assert_eq!(
        executor.calls(),
        vec!["task-older".to_string(), "task-newer".to_string()]
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
            Some(expected_error)
        );
    }
}

#[tokio::test]
async fn validation_failure_final_result_patch_omits_runtime_fields() {
    let state = synced_runtime_state(&["queued"]);
    let mut transport = RecordingTransport::new(vec![TaskResource {
        command_line: Some("sh".to_string()),
        ..queued_script_task("task-invalid")
    }]);
    let executor = RecordingExecutor::succeeds(ExecutionResult {
        stdout: "should-not-run".to_string(),
        stderr: String::new(),
        exit_code: Some(0),
        state: TaskObservedState::Succeeded,
    });

    sync_once(
        &mut transport,
        &executor,
        &state,
        "agent-1",
        None,
        timestamp(),
    )
    .await
    .unwrap();

    let terminal = &transport.apply_calls[1].patch;
    assert_eq!(terminal.observed_state, Some(TaskObservedState::Failed));
    assert_eq!(terminal.started_at, None);
    assert_eq!(terminal.finished_at, None);
    assert_eq!(terminal.stdout, None);
    assert_eq!(terminal.stderr, None);
    assert_eq!(terminal.exit_code, None);
    assert_eq!(
        terminal.error_message.as_deref(),
        Some("script task cannot include command_line")
    );
}

#[tokio::test]
async fn executor_error_final_result_patch_uses_error_message_and_real_finish_time() {
    let state = synced_runtime_state(&["queued"]);
    let mut transport = RecordingTransport::new(vec![queued_script_task("task-spawn")]);
    let executor = RecordingExecutor::fails_after(
        Duration::from_millis(1_100),
        "task spawn failure for task task-spawn while spawning `missing-binary`",
    );

    sync_once(
        &mut transport,
        &executor,
        &state,
        "agent-1",
        None,
        Utc::now(),
    )
    .await
    .unwrap();

    let running = &transport.apply_calls[1].patch;
    let terminal = &transport.apply_calls[2].patch;
    assert_eq!(terminal.observed_state, Some(TaskObservedState::Failed));
    assert_eq!(terminal.stdout, None);
    assert_eq!(terminal.stderr, None);
    assert_eq!(terminal.exit_code, None);
    assert_eq!(
        terminal.error_message.as_deref(),
        Some("task spawn failure for task task-spawn while spawning `missing-binary`")
    );
    assert!(
        parse_timestamp(terminal.finished_at.as_deref().expect("finished_at"))
            > parse_timestamp(running.started_at.as_deref().expect("started_at"))
    );
}

#[tokio::test]
async fn non_zero_exit_final_result_patch_preserves_process_fields_and_real_finish_time() {
    let state = synced_runtime_state(&["queued"]);
    let mut transport = RecordingTransport::new(vec![queued_command_task("task-nonzero")]);
    let executor = RecordingExecutor::succeeds_after(
        Duration::from_millis(1_100),
        ExecutionResult {
            stdout: "stdout-value".to_string(),
            stderr: "stderr-value".to_string(),
            exit_code: Some(7),
            state: TaskObservedState::Failed,
        },
    );

    sync_once(
        &mut transport,
        &executor,
        &state,
        "agent-1",
        None,
        Utc::now(),
    )
    .await
    .unwrap();

    let running = &transport.apply_calls[1].patch;
    let terminal = &transport.apply_calls[2].patch;
    assert_eq!(terminal.observed_state, Some(TaskObservedState::Failed));
    assert_eq!(terminal.stdout.as_deref(), Some("stdout-value"));
    assert_eq!(terminal.stderr.as_deref(), Some("stderr-value"));
    assert_eq!(terminal.exit_code, Some(7));
    assert_eq!(terminal.error_message, None);
    assert!(
        parse_timestamp(terminal.finished_at.as_deref().expect("finished_at"))
            > parse_timestamp(running.started_at.as_deref().expect("started_at"))
    );
}

#[tokio::test]
async fn timeout_final_result_patch_preserves_timeout_shape_and_real_finish_time() {
    let state = synced_runtime_state(&["queued"]);
    let mut transport = RecordingTransport::new(vec![queued_command_task("task-timeout")]);
    let executor = RecordingExecutor::succeeds_after(
        Duration::from_millis(1_100),
        ExecutionResult {
            stdout: "partial-stdout".to_string(),
            stderr: "partial-stderr".to_string(),
            exit_code: None,
            state: TaskObservedState::Timeout,
        },
    );

    sync_once(
        &mut transport,
        &executor,
        &state,
        "agent-1",
        None,
        Utc::now(),
    )
    .await
    .unwrap();

    let running = &transport.apply_calls[1].patch;
    let terminal = &transport.apply_calls[2].patch;
    assert_eq!(terminal.observed_state, Some(TaskObservedState::Timeout));
    assert_eq!(terminal.stdout.as_deref(), Some("partial-stdout"));
    assert_eq!(terminal.stderr.as_deref(), Some("partial-stderr"));
    assert_eq!(terminal.exit_code, None);
    assert_eq!(terminal.error_message, None);
    assert!(
        parse_timestamp(terminal.finished_at.as_deref().expect("finished_at"))
            > parse_timestamp(running.started_at.as_deref().expect("started_at"))
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
    list_outcomes: VecDeque<std::result::Result<Vec<TaskResource>, String>>,
    apply_outcomes: VecDeque<std::result::Result<TaskObservedState, String>>,
    tasks: BTreeMap<String, TaskResource>,
}

impl RecordingTransport {
    fn new(tasks: Vec<TaskResource>) -> Self {
        Self {
            list_calls: Vec::new(),
            apply_calls: Vec::new(),
            list_outcomes: VecDeque::from([Ok(tasks.clone())]),
            apply_outcomes: VecDeque::new(),
            tasks: tasks
                .into_iter()
                .map(|task| (task.task_id.clone(), task))
                .collect(),
        }
    }

    fn with_list_responses(list_responses: Vec<Vec<TaskResource>>) -> Self {
        Self::with_list_results(list_responses.into_iter().map(Ok).collect())
    }

    fn with_list_results(list_outcomes: Vec<std::result::Result<Vec<TaskResource>, &str>>) -> Self {
        let tasks = list_outcomes
            .iter()
            .filter_map(|outcome| outcome.as_ref().ok())
            .flatten()
            .cloned()
            .map(|task| (task.task_id.clone(), task))
            .collect();

        let list_outcomes = list_outcomes
            .into_iter()
            .map(|outcome| outcome.map_err(|error| error.to_string()))
            .collect();

        Self {
            list_calls: Vec::new(),
            apply_calls: Vec::new(),
            list_outcomes,
            apply_outcomes: VecDeque::new(),
            tasks,
        }
    }
}

#[derive(Debug, Clone)]
struct ServiceBackedTransport {
    tasks_endpoint: String,
    apply_endpoint: String,
    service: TaskSyncService,
}

#[derive(Debug, Clone)]
struct ObservedServiceTransport {
    tasks_endpoint: String,
    apply_endpoint: String,
    service: TaskSyncService,
    list_calls: Vec<ListCall>,
    apply_calls: Vec<ApplyCall>,
}

#[derive(Debug, Clone)]
struct PersistingApplyErrorTransport {
    tasks_endpoint: String,
    apply_endpoint: String,
    service: TaskSyncService,
    list_calls: Vec<ListCall>,
    apply_calls: Vec<ApplyCall>,
    apply_persisted_errors: VecDeque<Option<String>>,
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

impl ObservedServiceTransport {
    fn new(base_tasks_endpoint: &str, service: TaskSyncService) -> Self {
        Self {
            tasks_endpoint: base_tasks_endpoint.to_string(),
            apply_endpoint: format!("{base_tasks_endpoint}:apply"),
            service,
            list_calls: Vec::new(),
            apply_calls: Vec::new(),
        }
    }
}

impl PersistingApplyErrorTransport {
    fn new(base_tasks_endpoint: &str, service: TaskSyncService) -> Self {
        Self {
            tasks_endpoint: base_tasks_endpoint.to_string(),
            apply_endpoint: format!("{base_tasks_endpoint}:apply"),
            service,
            list_calls: Vec::new(),
            apply_calls: Vec::new(),
            apply_persisted_errors: VecDeque::new(),
        }
    }
}

impl JobManageTransport for RecordingTransport {
    type ListTasksFuture<'a>
        = Pin<Box<dyn Future<Output = Result<Vec<TaskResource>>> + Send + 'a>>
    where
        Self: 'a;

    fn list_tasks<'a>(
        &'a mut self,
        endpoint: &'a str,
        query: &'a TaskListQuery,
    ) -> Self::ListTasksFuture<'a> {
        self.list_calls.push(ListCall {
            endpoint: endpoint.to_string(),
            query: query.clone(),
        });
        let result = match self.list_outcomes.pop_front() {
            Some(Ok(tasks)) => Ok(tasks),
            Some(Err(error)) => Err(anyhow::anyhow!(error)),
            None => Ok(Vec::new()),
        };
        Box::pin(async move { result })
    }

    type ApplyTaskFuture<'a>
        = Pin<Box<dyn Future<Output = Result<TaskApplyAck>> + Send + 'a>>
    where
        Self: 'a;

    fn apply_task<'a>(
        &'a mut self,
        endpoint: &'a str,
        identity: &'a TaskApplyIdentity,
        patch: &'a TaskApplyPatch,
    ) -> Self::ApplyTaskFuture<'a> {
        self.apply_calls.push(ApplyCall {
            endpoint: endpoint.to_string(),
            identity: identity.clone(),
            patch: patch.clone(),
        });

        if let Some(outcome) = self.apply_outcomes.pop_front() {
            let result = match outcome {
                Ok(observed_state) => Ok(TaskApplyAck {
                    task_id: identity.task_id.clone(),
                    observed_state,
                    updated_at: patch.updated_at.clone(),
                }),
                Err(error) => Err(anyhow::anyhow!(error)),
            };
            return Box::pin(async move { result });
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

        let result = Ok(TaskApplyAck {
            task_id: task.task_id.clone(),
            observed_state: task.observed_state,
            updated_at: task.updated_at.clone(),
        });
        Box::pin(async move { result })
    }
}

impl JobManageTransport for ServiceBackedTransport {
    type ListTasksFuture<'a>
        = Pin<Box<dyn Future<Output = Result<Vec<TaskResource>>> + Send + 'a>>
    where
        Self: 'a;

    fn list_tasks<'a>(
        &'a mut self,
        endpoint: &'a str,
        query: &'a TaskListQuery,
    ) -> Self::ListTasksFuture<'a> {
        assert_eq!(endpoint, self.tasks_endpoint);
        let service = self.service.clone();
        let query = query.clone();
        Box::pin(async move {
            service
                .list_tasks(&query)
                .await
                .map_err(|err| anyhow::anyhow!(err.to_string()))
        })
    }

    type ApplyTaskFuture<'a>
        = Pin<Box<dyn Future<Output = Result<TaskApplyAck>> + Send + 'a>>
    where
        Self: 'a;

    fn apply_task<'a>(
        &'a mut self,
        endpoint: &'a str,
        identity: &'a TaskApplyIdentity,
        patch: &'a TaskApplyPatch,
    ) -> Self::ApplyTaskFuture<'a> {
        assert_eq!(endpoint, self.apply_endpoint);
        let service = self.service.clone();
        let identity = identity.clone();
        let patch = patch.clone();
        Box::pin(async move {
            let task = service
                .apply_task(
                    &identity,
                    TaskApplyRequest {
                        patch: patch.clone(),
                        rejected_fields: vec![],
                    },
                )
                .await
                .map_err(|err| anyhow::anyhow!(err.to_string()))?;

            Ok(TaskApplyAck {
                task_id: task.task_id,
                observed_state: task.observed_state,
                updated_at: task.updated_at,
            })
        })
    }
}

impl JobManageTransport for ObservedServiceTransport {
    type ListTasksFuture<'a>
        = Pin<Box<dyn Future<Output = Result<Vec<TaskResource>>> + Send + 'a>>
    where
        Self: 'a;

    fn list_tasks<'a>(
        &'a mut self,
        endpoint: &'a str,
        query: &'a TaskListQuery,
    ) -> Self::ListTasksFuture<'a> {
        assert_eq!(endpoint, self.tasks_endpoint);
        self.list_calls.push(ListCall {
            endpoint: endpoint.to_string(),
            query: query.clone(),
        });
        let service = self.service.clone();
        let query = query.clone();
        Box::pin(async move {
            service
                .list_tasks(&query)
                .await
                .map_err(|err| anyhow::anyhow!(err.to_string()))
        })
    }

    type ApplyTaskFuture<'a>
        = Pin<Box<dyn Future<Output = Result<TaskApplyAck>> + Send + 'a>>
    where
        Self: 'a;

    fn apply_task<'a>(
        &'a mut self,
        endpoint: &'a str,
        identity: &'a TaskApplyIdentity,
        patch: &'a TaskApplyPatch,
    ) -> Self::ApplyTaskFuture<'a> {
        assert_eq!(endpoint, self.apply_endpoint);
        self.apply_calls.push(ApplyCall {
            endpoint: endpoint.to_string(),
            identity: identity.clone(),
            patch: patch.clone(),
        });
        let service = self.service.clone();
        let identity = identity.clone();
        let patch = patch.clone();
        Box::pin(async move {
            let task = service
                .apply_task(
                    &identity,
                    TaskApplyRequest {
                        patch: patch.clone(),
                        rejected_fields: vec![],
                    },
                )
                .await
                .map_err(|err| anyhow::anyhow!(err.to_string()))?;

            Ok(TaskApplyAck {
                task_id: task.task_id,
                observed_state: task.observed_state,
                updated_at: task.updated_at,
            })
        })
    }
}

impl JobManageTransport for PersistingApplyErrorTransport {
    type ListTasksFuture<'a>
        = Pin<Box<dyn Future<Output = Result<Vec<TaskResource>>> + Send + 'a>>
    where
        Self: 'a;

    fn list_tasks<'a>(
        &'a mut self,
        endpoint: &'a str,
        query: &'a TaskListQuery,
    ) -> Self::ListTasksFuture<'a> {
        assert_eq!(endpoint, self.tasks_endpoint);
        self.list_calls.push(ListCall {
            endpoint: endpoint.to_string(),
            query: query.clone(),
        });
        let service = self.service.clone();
        let query = query.clone();
        Box::pin(async move {
            service
                .list_tasks(&query)
                .await
                .map_err(|err| anyhow::anyhow!(err.to_string()))
        })
    }

    type ApplyTaskFuture<'a>
        = Pin<Box<dyn Future<Output = Result<TaskApplyAck>> + Send + 'a>>
    where
        Self: 'a;

    fn apply_task<'a>(
        &'a mut self,
        endpoint: &'a str,
        identity: &'a TaskApplyIdentity,
        patch: &'a TaskApplyPatch,
    ) -> Self::ApplyTaskFuture<'a> {
        assert_eq!(endpoint, self.apply_endpoint);
        self.apply_calls.push(ApplyCall {
            endpoint: endpoint.to_string(),
            identity: identity.clone(),
            patch: patch.clone(),
        });
        let service = self.service.clone();
        let identity = identity.clone();
        let patch = patch.clone();
        let error = self.apply_persisted_errors.pop_front();
        Box::pin(async move {
            let task = service
                .apply_task(
                    &identity,
                    TaskApplyRequest {
                        patch: patch.clone(),
                        rejected_fields: vec![],
                    },
                )
                .await
                .map_err(|err| anyhow::anyhow!(err.to_string()))?;

            match error {
                Some(Some(message)) => Err(anyhow::anyhow!(message)),
                Some(None) | None => Ok(TaskApplyAck {
                    task_id: task.task_id,
                    observed_state: task.observed_state,
                    updated_at: task.updated_at,
                }),
            }
        })
    }
}

#[derive(Debug)]
struct ExpectedHttpRequest {
    method: &'static str,
    target: &'static str,
    body: Option<Value>,
    response_body: String,
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
                    "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                    expected.response_body.len(),
                    expected.response_body
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
    DelayedSuccess {
        delay: Duration,
        result: ExecutionResult,
    },
    DelayedError {
        delay: Duration,
        message: String,
    },
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

    fn succeeds_after(delay: Duration, result: ExecutionResult) -> Self {
        Self {
            calls: Arc::new(Mutex::new(Vec::new())),
            result: RecordedExecution::DelayedSuccess { delay, result },
        }
    }

    fn fails_after(delay: Duration, message: impl Into<String>) -> Self {
        Self {
            calls: Arc::new(Mutex::new(Vec::new())),
            result: RecordedExecution::DelayedError {
                delay,
                message: message.into(),
            },
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
                RecordedExecution::DelayedSuccess { delay, result } => {
                    tokio::time::sleep(delay).await;
                    Ok(result)
                }
                RecordedExecution::DelayedError { delay, message } => {
                    tokio::time::sleep(delay).await;
                    Err(anyhow::anyhow!(message))
                }
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

fn parse_timestamp(value: &str) -> chrono::DateTime<Utc> {
    chrono::DateTime::parse_from_rfc3339(value)
        .expect("valid timestamp")
        .with_timezone(&Utc)
}
