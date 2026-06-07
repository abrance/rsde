use std::{
    collections::{BTreeMap, VecDeque},
    env,
    future::Future,
    io::{BufRead, BufReader, Read, Write},
    net::TcpListener,
    path::PathBuf,
    pin::Pin,
    sync::{Arc, Mutex},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
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
    executor::{ExecutionError, ExecutionResult},
    registration::AgentRuntimeState,
    runtime_coordinator::TransientRetryPolicy,
    task_sync::{
        TaskDecisionOutcome, TaskDecisionStage, TaskExecutor, TaskSyncLoop, TaskSyncSkipReason,
        TaskSyncTick, sync_once,
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
async fn task_sync_loop_keeps_unprocessed_visible_task_reachable_on_later_tick() {
    let state = synced_runtime_state(&["queued"]);
    let first_tick_tasks = vec![
        TaskResource {
            updated_at: Some("2026-05-31T08:01:00Z".to_string()),
            ..queued_script_task("task-1")
        },
        TaskResource {
            updated_at: Some("2026-05-31T08:02:00Z".to_string()),
            ..queued_script_task("task-2")
        },
    ];
    let second_tick_tasks = vec![TaskResource {
        updated_at: Some("2026-05-31T08:02:00Z".to_string()),
        ..queued_script_task("task-2")
    }];
    let transport =
        RecordingTransport::with_list_responses(vec![first_tick_tasks, second_tick_tasks]);
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
            task_id: "task-1".to_string(),
            final_state: TaskObservedState::Succeeded,
        }
    );
    assert_eq!(
        second,
        TaskSyncTick::Applied {
            task_id: "task-2".to_string(),
            final_state: TaskObservedState::Succeeded,
        }
    );

    let transport = loop_runner.into_transport();
    assert_eq!(transport.list_calls.len(), 2);
    assert_eq!(transport.list_calls[0].query.updated_after, None);
    assert_eq!(
        transport.list_calls[1].query.updated_after.as_deref(),
        Some("2026-05-31T08:01:00Z")
    );
    assert_eq!(
        executor.calls(),
        vec!["task-1".to_string(), "task-2".to_string()]
    );
}

#[tokio::test]
async fn task_sync_loop_retries_failed_terminal_replay_before_processing_sibling_pending_task() {
    let data_dir = writable_data_dir("task-loop-replay-priority");
    let state = synced_runtime_state_in(&data_dir, &["queued", "running"]);
    let first_tick_tasks = vec![
        TaskResource {
            updated_at: Some("2026-05-31T08:01:00Z".to_string()),
            ..queued_command_task("task-1")
        },
        TaskResource {
            updated_at: Some("2026-05-31T08:02:00Z".to_string()),
            ..queued_command_task("task-2")
        },
    ];
    let transport = RecordingTransport::with_list_responses(vec![first_tick_tasks]);
    let apply_outcomes = vec![
        ApplyOutcome::Success,
        ApplyOutcome::Success,
        ApplyOutcome::Fail,
    ];
    let mut loop_runner = TaskSyncLoop::new(
        RecordingTransport {
            apply_outcomes: VecDeque::from(apply_outcomes),
            ..transport
        },
        RecordingExecutor::succeeds(ExecutionResult {
            stdout: "done".to_string(),
            stderr: String::new(),
            exit_code: Some(0),
            state: TaskObservedState::Succeeded,
        }),
    );

    let first_error = loop_runner
        .tick(&state, "agent-1", timestamp())
        .await
        .unwrap_err();
    assert!(first_error.to_string().contains("simulated apply failure"));

    let replay = loop_runner
        .tick(
            &state,
            "agent-1",
            Utc.with_ymd_and_hms(2026, 5, 31, 8, 5, 0).unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        replay,
        TaskSyncTick::Applied {
            task_id: "task-1".to_string(),
            final_state: TaskObservedState::Succeeded,
        }
    );

    let transport = loop_runner.into_transport();
    assert_eq!(transport.apply_calls.len(), 4);
    assert_eq!(
        transport
            .apply_calls
            .iter()
            .map(|call| call.identity.task_id.as_str())
            .collect::<Vec<_>>(),
        vec!["task-1", "task-1", "task-1", "task-1"]
    );
}

#[tokio::test]
async fn task_sync_loop_running_replay_removes_persisted_file_and_later_tick_cannot_reuse_stale_data()
 {
    let data_dir = writable_data_dir("task-loop-running-replay-cleanup");
    let state = synced_runtime_state_in(&data_dir, &["running", "queued"]);
    let replay_path = PathBuf::from(&data_dir).join("task-replay.toml");
    let running_task = TaskResource {
        observed_state: TaskObservedState::Running,
        claimed_at: Some("2026-05-31T08:00:00Z".to_string()),
        started_at: Some("2026-05-31T08:00:00Z".to_string()),
        updated_at: Some("2026-05-31T08:01:00Z".to_string()),
        ..queued_command_task("task-running-replay")
    };
    let sibling_task = TaskResource {
        updated_at: Some("2026-05-31T08:02:00Z".to_string()),
        ..queued_command_task("task-after-replay")
    };

    let mut first_transport = RecordingTransport::with_apply_outcomes(
        vec![queued_command_task("task-running-replay")],
        vec![
            ApplyOutcome::Success,
            ApplyOutcome::Success,
            ApplyOutcome::Fail,
        ],
    );
    let first_executor = RecordingExecutor::succeeds(ExecutionResult {
        stdout: "persisted-stdout".to_string(),
        stderr: String::new(),
        exit_code: Some(0),
        state: TaskObservedState::Succeeded,
    });
    let first_error = sync_once(
        &mut first_transport,
        &first_executor,
        &state,
        "agent-1",
        None,
        timestamp(),
    )
    .await
    .unwrap_err();
    assert!(first_error.to_string().contains("simulated apply failure"));
    assert!(replay_path.exists());

    let transport = RecordingTransport::with_list_responses(vec![
        vec![running_task, sibling_task.clone()],
        vec![sibling_task],
    ]);
    let executor = RecordingExecutor::succeeds(ExecutionResult {
        stdout: "fresh-stdout".to_string(),
        stderr: String::new(),
        exit_code: Some(0),
        state: TaskObservedState::Succeeded,
    });
    let mut loop_runner = TaskSyncLoop::new(transport, executor.clone());

    let replay_tick = loop_runner
        .tick(&state, "agent-1", timestamp())
        .await
        .unwrap();
    assert_eq!(
        replay_tick,
        TaskSyncTick::Applied {
            task_id: "task-running-replay".to_string(),
            final_state: TaskObservedState::Succeeded,
        }
    );
    assert!(!replay_path.exists());

    let later_tick = loop_runner
        .tick(
            &state,
            "agent-1",
            Utc.with_ymd_and_hms(2026, 5, 31, 8, 5, 0).unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        later_tick,
        TaskSyncTick::Applied {
            task_id: "task-after-replay".to_string(),
            final_state: TaskObservedState::Succeeded,
        }
    );

    let transport = loop_runner.into_transport();
    assert_eq!(transport.apply_calls.len(), 4);
    assert_eq!(
        transport.apply_calls[0].identity.task_id,
        "task-running-replay"
    );
    assert_eq!(
        transport.apply_calls[0].patch.stdout.as_deref(),
        Some("persisted-stdout")
    );
    assert_eq!(
        transport.apply_calls[3].identity.task_id,
        "task-after-replay"
    );
    assert_eq!(
        transport.apply_calls[3].patch.stdout.as_deref(),
        Some("fresh-stdout")
    );
    assert_eq!(executor.calls(), vec!["task-after-replay".to_string()]);
}

#[tokio::test]
async fn task_sync_loop_records_recovery_diagnostic_when_replaying_persisted_terminal_patch() {
    let data_dir = writable_data_dir("task-loop-replay-diagnostic");
    let state = synced_runtime_state_in(&data_dir, &["running"]);
    let running_task = TaskResource {
        observed_state: TaskObservedState::Running,
        claimed_at: Some("2026-05-31T08:00:00Z".to_string()),
        started_at: Some("2026-05-31T08:00:00Z".to_string()),
        updated_at: Some("2026-05-31T08:01:00Z".to_string()),
        ..queued_command_task("task-replay-diagnostic")
    };

    let mut first_transport = RecordingTransport::with_apply_outcomes(
        vec![queued_command_task("task-replay-diagnostic")],
        vec![
            ApplyOutcome::Success,
            ApplyOutcome::Success,
            ApplyOutcome::Fail,
        ],
    );
    let first_executor = RecordingExecutor::succeeds(ExecutionResult {
        stdout: "persisted-stdout".to_string(),
        stderr: String::new(),
        exit_code: Some(0),
        state: TaskObservedState::Succeeded,
    });
    let first_error = sync_once(
        &mut first_transport,
        &first_executor,
        &state,
        "agent-1",
        None,
        timestamp(),
    )
    .await
    .unwrap_err();
    assert!(first_error.to_string().contains("simulated apply failure"));

    let transport = RecordingTransport::with_list_responses(vec![vec![running_task]]);
    let executor = RecordingExecutor::fails("executor must not rerun replay task");
    let mut loop_runner = TaskSyncLoop::new(transport, executor);

    let replay_tick = loop_runner
        .tick(&state, "agent-1", timestamp())
        .await
        .unwrap();
    let diagnostic = loop_runner
        .last_tick_diagnostic()
        .expect("diagnostic after replay recovery")
        .clone();

    assert_eq!(
        replay_tick,
        TaskSyncTick::Applied {
            task_id: "task-replay-diagnostic".to_string(),
            final_state: TaskObservedState::Succeeded,
        }
    );
    assert_eq!(diagnostic.stage, TaskDecisionStage::Replay);
    assert_eq!(diagnostic.outcome, TaskDecisionOutcome::Recovered);
    assert_eq!(
        diagnostic.task_id.as_deref(),
        Some("task-replay-diagnostic")
    );
    assert!(diagnostic.detail.contains("persisted terminal patch"));
}

#[tokio::test]
async fn task_sync_loop_retries_transient_list_failure_within_same_tick() {
    let state = synced_runtime_state(&["queued"]);
    let transport = RecordingTransport::with_list_outcomes(vec![
        ListOutcome::TransientFail("temporary list failure".to_string()),
        ListOutcome::Success(vec![]),
    ]);
    let executor = RecordingExecutor::succeeds(ExecutionResult {
        stdout: String::new(),
        stderr: String::new(),
        exit_code: Some(0),
        state: TaskObservedState::Succeeded,
    });
    let mut loop_runner = TaskSyncLoop::with_retry_policy(
        transport,
        executor,
        TransientRetryPolicy::new(vec![Duration::ZERO]),
    );

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
    assert_eq!(loop_runner.into_transport().list_calls.len(), 2);
}

#[tokio::test]
async fn task_sync_loop_retries_transient_terminal_apply_failure_within_same_tick() {
    let data_dir = writable_data_dir("task-apply-retry");
    let state = synced_runtime_state_in(&data_dir, &["queued"]);
    let transport = RecordingTransport {
        apply_outcomes: VecDeque::from(vec![
            ApplyOutcome::Success,
            ApplyOutcome::Success,
            ApplyOutcome::TransientFail("temporary apply failure".to_string()),
            ApplyOutcome::Success,
        ]),
        ..RecordingTransport::with_list_outcomes(vec![ListOutcome::Success(vec![
            queued_command_task("task-apply-retry"),
        ])])
    };
    let executor = RecordingExecutor::succeeds(ExecutionResult {
        stdout: "done".to_string(),
        stderr: String::new(),
        exit_code: Some(0),
        state: TaskObservedState::Succeeded,
    });
    let mut loop_runner = TaskSyncLoop::with_retry_policy(
        transport,
        executor,
        TransientRetryPolicy::new(vec![Duration::ZERO]),
    );

    let tick = loop_runner
        .tick(&state, "agent-1", timestamp())
        .await
        .unwrap();
    let transport = loop_runner.into_transport();

    assert_eq!(
        tick,
        TaskSyncTick::Applied {
            task_id: "task-apply-retry".to_string(),
            final_state: TaskObservedState::Succeeded,
        }
    );
    assert_eq!(transport.apply_calls.len(), 4);
    assert!(!PathBuf::from(&data_dir).join("task-replay.toml").exists());
}

#[tokio::test(flavor = "current_thread")]
async fn task_sync_loop_async_tick_offloads_blocking_work() {
    let state = synced_runtime_state(&["queued"]);
    let transport = RecordingTransport::with_list_outcomes(vec![
        ListOutcome::TransientFail("temporary list failure".to_string()),
        ListOutcome::Success(vec![queued_command_task("task-async")]),
    ]);
    let executor = RecordingExecutor::succeeds(ExecutionResult {
        stdout: "done".to_string(),
        stderr: String::new(),
        exit_code: Some(0),
        state: TaskObservedState::Succeeded,
    });
    let loop_runner = TaskSyncLoop::with_retry_policy(
        transport,
        executor,
        TransientRetryPolicy::new(vec![Duration::from_millis(1)]),
    );

    let (yielded, tick) = tokio::join!(
        async {
            tokio::task::yield_now().await;
            "yielded"
        },
        loop_runner.tick_async(state, "agent-1".to_string(), timestamp())
    );

    assert_eq!(yielded, "yielded");
    assert_eq!(
        tick.1.unwrap(),
        TaskSyncTick::Applied {
            task_id: "task-async".to_string(),
            final_state: TaskObservedState::Succeeded,
        }
    );
}

#[tokio::test]
async fn task_sync_loop_prefers_visible_active_task_before_claiming_new_work() {
    let state = synced_runtime_state(&["queued", "running"]);
    let first_tick_tasks = vec![
        TaskResource {
            updated_at: Some("2026-05-31T08:01:00Z".to_string()),
            ..queued_script_task("task-queued")
        },
        TaskResource {
            observed_state: TaskObservedState::Running,
            started_at: Some("2026-05-31T08:00:30Z".to_string()),
            updated_at: Some("2026-05-31T08:02:00Z".to_string()),
            ..queued_script_task("task-running")
        },
    ];
    let transport = RecordingTransport::with_list_responses(vec![first_tick_tasks]);
    let executor = RecordingExecutor::succeeds(ExecutionResult {
        stdout: "done".to_string(),
        stderr: String::new(),
        exit_code: Some(0),
        state: TaskObservedState::Succeeded,
    });
    let mut loop_runner = TaskSyncLoop::new(transport, executor.clone());

    let result = loop_runner
        .tick(&state, "agent-1", timestamp())
        .await
        .unwrap();

    assert_eq!(
        result,
        TaskSyncTick::Applied {
            task_id: "task-running".to_string(),
            final_state: TaskObservedState::Succeeded,
        }
    );
    assert_eq!(executor.calls(), vec!["task-running".to_string()]);

    let transport = loop_runner.into_transport();
    assert_eq!(transport.list_calls.len(), 1);
    assert_eq!(transport.apply_calls.len(), 1);
    assert_eq!(transport.apply_calls[0].identity.task_id, "task-running");
}

#[tokio::test]
async fn task_sync_loop_prefers_dispatched_task_before_queued_new_work() {
    let state = synced_runtime_state(&["queued", "dispatched"]);
    let first_tick_tasks = vec![
        TaskResource {
            updated_at: Some("2026-05-31T08:01:00Z".to_string()),
            ..queued_script_task("task-queued")
        },
        TaskResource {
            observed_state: TaskObservedState::Dispatched,
            updated_at: Some("2026-05-31T08:02:00Z".to_string()),
            ..queued_script_task("task-dispatched")
        },
    ];
    let transport = RecordingTransport::with_list_responses(vec![first_tick_tasks]);
    let executor = RecordingExecutor::succeeds(ExecutionResult {
        stdout: "done".to_string(),
        stderr: String::new(),
        exit_code: Some(0),
        state: TaskObservedState::Succeeded,
    });
    let mut loop_runner = TaskSyncLoop::new(transport, executor.clone());

    let result = loop_runner
        .tick(&state, "agent-1", timestamp())
        .await
        .unwrap();

    assert_eq!(
        result,
        TaskSyncTick::Applied {
            task_id: "task-dispatched".to_string(),
            final_state: TaskObservedState::Succeeded,
        }
    );
    assert_eq!(executor.calls(), vec!["task-dispatched".to_string()]);

    let transport = loop_runner.into_transport();
    assert_eq!(transport.list_calls.len(), 1);
    assert_eq!(transport.apply_calls.len(), 3);
    assert_eq!(transport.apply_calls[0].identity.task_id, "task-dispatched");
}

#[tokio::test]
async fn task_sync_loop_keeps_same_timestamp_visible_task_reachable_on_later_tick() {
    let state = synced_runtime_state(&["queued"]);
    let visible_tasks = vec![
        TaskResource {
            updated_at: Some("2026-05-31T08:01:00Z".to_string()),
            ..queued_script_task("task-1")
        },
        TaskResource {
            updated_at: Some("2026-05-31T08:01:00Z".to_string()),
            ..queued_script_task("task-2")
        },
    ];
    let transport = RecordingTransport::with_list_responses(vec![visible_tasks]);
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
            task_id: "task-1".to_string(),
            final_state: TaskObservedState::Succeeded,
        }
    );
    assert_eq!(
        second,
        TaskSyncTick::Applied {
            task_id: "task-2".to_string(),
            final_state: TaskObservedState::Succeeded,
        }
    );

    let transport = loop_runner.into_transport();
    assert_eq!(transport.list_calls.len(), 1);
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
async fn lists_tasks_with_default_filters_from_runtime_config() {
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
}

#[tokio::test]
async fn dispatched_task_is_acknowledged_before_execution() {
    let data_dir = writable_data_dir("task-dispatched");
    let state = synced_runtime_state_in(&data_dir, &["dispatched"]);
    let mut transport = RecordingTransport::new(vec![task_with_observed_state(
        queued_command_task("task-dispatched"),
        "dispatched",
    )]);
    let executor = RecordingExecutor::succeeds(ExecutionResult {
        stdout: "dispatch-stdout".to_string(),
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
            task_id: "task-dispatched".to_string(),
            final_state: TaskObservedState::Succeeded,
        }
    );
    assert_eq!(executor.calls(), vec!["task-dispatched".to_string()]);
    assert_eq!(transport.apply_calls.len(), 3);
    assert_eq!(
        transport.apply_calls[0].patch.observed_state,
        Some(TaskObservedState::Acknowledged)
    );
    assert_eq!(
        transport.apply_calls[0].patch.claimed_at.as_deref(),
        Some("2026-05-31T08:00:00Z")
    );
    assert_eq!(
        transport.apply_calls[1].patch.observed_state,
        Some(TaskObservedState::Running)
    );
    assert_eq!(
        transport.apply_calls[2].patch.observed_state,
        Some(TaskObservedState::Succeeded)
    );
}

#[tokio::test]
async fn rediscovered_running_task_replays_persisted_terminal_patch_without_reexecution() {
    let data_dir = writable_data_dir("task-replay");
    let state = synced_runtime_state_in(&data_dir, &["dispatched", "running"]);
    let mut first_transport = RecordingTransport::with_apply_outcomes(
        vec![task_with_observed_state(
            queued_command_task("task-running"),
            "dispatched",
        )],
        vec![
            ApplyOutcome::Success,
            ApplyOutcome::Success,
            ApplyOutcome::Fail,
        ],
    );
    let first_executor = RecordingExecutor::succeeds(ExecutionResult {
        stdout: "persisted-stdout".to_string(),
        stderr: "persisted-stderr".to_string(),
        exit_code: Some(17),
        state: TaskObservedState::Failed,
    });

    let first_error = sync_once(
        &mut first_transport,
        &first_executor,
        &state,
        "agent-1",
        None,
        timestamp(),
    )
    .await
    .unwrap_err();
    assert!(first_error.to_string().contains("simulated apply failure"));
    assert_eq!(first_executor.calls(), vec!["task-running".to_string()]);

    let mut replay_transport = RecordingTransport::new(vec![TaskResource {
        observed_state: TaskObservedState::Running,
        claimed_at: Some("2026-05-31T08:00:00Z".to_string()),
        started_at: Some("2026-05-31T08:00:00Z".to_string()),
        ..queued_command_task("task-running")
    }]);
    let replay_executor = RecordingExecutor::fails("executor must not rerun visible running task");

    let result = sync_once(
        &mut replay_transport,
        &replay_executor,
        &state,
        "agent-1",
        None,
        Utc.with_ymd_and_hms(2026, 5, 31, 8, 5, 0).unwrap(),
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
    assert!(replay_executor.calls().is_empty());
    assert_eq!(replay_transport.apply_calls.len(), 1);
    let replay_patch = &replay_transport.apply_calls[0].patch;
    assert_eq!(replay_patch.observed_state, Some(TaskObservedState::Failed));
    assert_eq!(replay_patch.stdout.as_deref(), Some("persisted-stdout"));
    assert_eq!(replay_patch.stderr.as_deref(), Some("persisted-stderr"));
    assert_eq!(replay_patch.exit_code, Some(17));
    assert_eq!(
        replay_patch.finished_at.as_deref(),
        Some("2026-05-31T08:00:00Z")
    );
}

#[tokio::test]
async fn successful_replay_removes_persisted_patch_and_later_tick_does_not_reuse_stale_terminal_data()
 {
    let data_dir = writable_data_dir("task-replay-cleanup");
    let state = synced_runtime_state_in(&data_dir, &["dispatched", "running", "queued"]);
    let replay_path = PathBuf::from(&data_dir).join("task-replay.toml");

    let mut first_transport = RecordingTransport::with_apply_outcomes(
        vec![task_with_observed_state(
            queued_command_task("task-cleanup"),
            "dispatched",
        )],
        vec![
            ApplyOutcome::Success,
            ApplyOutcome::Success,
            ApplyOutcome::Fail,
        ],
    );
    let first_executor = RecordingExecutor::succeeds(ExecutionResult {
        stdout: "persisted-stdout".to_string(),
        stderr: "persisted-stderr".to_string(),
        exit_code: Some(17),
        state: TaskObservedState::Failed,
    });

    let first_error = sync_once(
        &mut first_transport,
        &first_executor,
        &state,
        "agent-1",
        None,
        timestamp(),
    )
    .await
    .unwrap_err();
    assert!(first_error.to_string().contains("simulated apply failure"));
    assert!(replay_path.exists());

    let mut replay_transport = RecordingTransport::new(vec![TaskResource {
        observed_state: TaskObservedState::Running,
        claimed_at: Some("2026-05-31T08:00:00Z".to_string()),
        started_at: Some("2026-05-31T08:00:00Z".to_string()),
        ..queued_command_task("task-cleanup")
    }]);
    let replay_executor = RecordingExecutor::fails("executor must not rerun visible running task");

    let replay_result = sync_once(
        &mut replay_transport,
        &replay_executor,
        &state,
        "agent-1",
        None,
        Utc.with_ymd_and_hms(2026, 5, 31, 8, 5, 0).unwrap(),
    )
    .await
    .unwrap();

    assert_eq!(
        replay_result,
        TaskSyncTick::Applied {
            task_id: "task-cleanup".to_string(),
            final_state: TaskObservedState::Failed,
        }
    );
    assert!(replay_executor.calls().is_empty());
    assert!(!replay_path.exists());

    let mut later_transport = RecordingTransport::new(vec![queued_command_task("task-later")]);
    let later_executor = RecordingExecutor::succeeds(ExecutionResult {
        stdout: "later-stdout".to_string(),
        stderr: String::new(),
        exit_code: Some(0),
        state: TaskObservedState::Succeeded,
    });

    let later_result = sync_once(
        &mut later_transport,
        &later_executor,
        &state,
        "agent-1",
        None,
        Utc.with_ymd_and_hms(2026, 5, 31, 8, 10, 0).unwrap(),
    )
    .await
    .unwrap();

    assert_eq!(
        later_result,
        TaskSyncTick::Applied {
            task_id: "task-later".to_string(),
            final_state: TaskObservedState::Succeeded,
        }
    );
    assert_eq!(later_executor.calls(), vec!["task-later".to_string()]);
    let later_terminal = &later_transport.apply_calls[2].patch;
    assert_eq!(later_terminal.stdout.as_deref(), Some("later-stdout"));
    assert_eq!(later_terminal.stderr.as_deref(), Some(""));
    assert_eq!(later_terminal.exit_code, Some(0));
    assert_eq!(later_terminal.error_message, None);
}

#[tokio::test]
async fn corrupt_replay_file_is_discarded_and_later_task_sync_can_proceed() {
    let data_dir = writable_data_dir("task-replay-corrupt");
    let state = synced_runtime_state_in(&data_dir, &["running", "queued"]);
    let replay_path = PathBuf::from(&data_dir).join("task-replay.toml");
    std::fs::write(&replay_path, "not-valid-toml = [").expect("write corrupt replay file");

    let tasks = vec![
        TaskResource {
            observed_state: TaskObservedState::Running,
            claimed_at: Some("2026-05-31T08:00:00Z".to_string()),
            started_at: Some("2026-05-31T08:00:00Z".to_string()),
            updated_at: Some("2026-05-31T08:01:00Z".to_string()),
            ..queued_command_task("task-corrupt-running")
        },
        TaskResource {
            updated_at: Some("2026-05-31T08:02:00Z".to_string()),
            ..queued_command_task("task-after-corrupt")
        },
    ];
    let transport = RecordingTransport::with_list_responses(vec![
        tasks,
        vec![queued_command_task("task-after-corrupt")],
    ]);
    let executor = RecordingExecutor::succeeds(ExecutionResult {
        stdout: "fresh-stdout".to_string(),
        stderr: String::new(),
        exit_code: Some(0),
        state: TaskObservedState::Succeeded,
    });
    let mut loop_runner = TaskSyncLoop::new(transport, executor.clone());

    let first = loop_runner
        .tick(&state, "agent-1", timestamp())
        .await
        .unwrap();
    assert_eq!(
        first,
        TaskSyncTick::Applied {
            task_id: "task-corrupt-running".to_string(),
            final_state: TaskObservedState::Succeeded,
        }
    );
    assert!(!replay_path.exists());

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
            task_id: "task-after-corrupt".to_string(),
            final_state: TaskObservedState::Succeeded,
        }
    );

    let transport = loop_runner.into_transport();
    assert_eq!(
        executor.calls(),
        vec![
            "task-corrupt-running".to_string(),
            "task-after-corrupt".to_string()
        ]
    );
    let last_patch = transport.apply_calls.last().expect("terminal patch");
    assert_eq!(last_patch.identity.task_id, "task-after-corrupt");
    assert_eq!(last_patch.patch.stdout.as_deref(), Some("fresh-stdout"));
}

#[tokio::test]
async fn spawn_failure_patch_is_stable_and_replayed_without_reexecution() {
    let data_dir = writable_data_dir("task-spawn-replay");
    let state = synced_runtime_state_in(&data_dir, &["queued", "running"]);
    let task = TaskResource {
        command_line: Some(format!(
            "missing-binary-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        )),
        ..queued_command_task("task-spawn-replay")
    };
    let expected_error = "failed to spawn task task-spawn-replay";
    let mut first_transport = RecordingTransport::with_apply_outcomes(
        vec![task.clone()],
        vec![
            ApplyOutcome::Success,
            ApplyOutcome::Success,
            ApplyOutcome::Fail,
        ],
    );
    let first_executor = rsagent::executor::LocalTaskExecutor;

    let first_error = sync_once(
        &mut first_transport,
        &first_executor,
        &state,
        "agent-1",
        None,
        timestamp(),
    )
    .await
    .unwrap_err();

    assert!(first_error.to_string().contains("simulated apply failure"));
    assert_eq!(first_transport.apply_calls.len(), 3);
    let first_patch = &first_transport.apply_calls[2].patch;
    assert_eq!(first_patch.observed_state, Some(TaskObservedState::Failed));
    assert_eq!(first_patch.error_message.as_deref(), Some(expected_error));
    assert_eq!(first_patch.stdout, None);
    assert_eq!(first_patch.stderr, None);
    assert_eq!(first_patch.exit_code, None);

    let mut replay_transport = RecordingTransport::new(vec![TaskResource {
        observed_state: TaskObservedState::Running,
        claimed_at: Some("2026-05-31T08:00:00Z".to_string()),
        started_at: Some("2026-05-31T08:00:00Z".to_string()),
        ..task
    }]);
    let replay_executor = RecordingExecutor::fails("executor must not rerun spawn failure replay");

    let replay = sync_once(
        &mut replay_transport,
        &replay_executor,
        &state,
        "agent-1",
        None,
        Utc.with_ymd_and_hms(2026, 5, 31, 8, 5, 0).unwrap(),
    )
    .await
    .unwrap();

    assert_eq!(
        replay,
        TaskSyncTick::Applied {
            task_id: "task-spawn-replay".to_string(),
            final_state: TaskObservedState::Failed,
        }
    );
    assert!(replay_executor.calls().is_empty());
    assert_eq!(replay_transport.apply_calls.len(), 1);
    let replay_patch = &replay_transport.apply_calls[0].patch;
    assert_eq!(replay_patch.observed_state, Some(TaskObservedState::Failed));
    assert_eq!(replay_patch.error_message.as_deref(), Some(expected_error));
    assert_eq!(replay_patch.stdout, None);
    assert_eq!(replay_patch.stderr, None);
    assert_eq!(replay_patch.exit_code, None);
}

#[tokio::test]
async fn structured_spawn_failure_error_maps_to_stable_terminal_patch() {
    let state = synced_runtime_state(&["queued"]);
    let mut transport = RecordingTransport::new(vec![queued_command_task("task-structured-spawn")]);
    let executor = StructuredErrorExecutor::spawn_failed();

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
            task_id: "task-structured-spawn".to_string(),
            final_state: TaskObservedState::Failed,
        }
    );
    assert_eq!(transport.apply_calls.len(), 3);
    let terminal = &transport.apply_calls[2].patch;
    assert_eq!(terminal.observed_state, Some(TaskObservedState::Failed));
    assert_eq!(
        terminal.error_message.as_deref(),
        Some("failed to spawn task task-structured-spawn")
    );
    assert_eq!(terminal.stdout, None);
    assert_eq!(terminal.stderr, None);
    assert_eq!(terminal.exit_code, None);
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

#[tokio::test(flavor = "multi_thread")]
async fn sync_once_with_task_sync_service_completes_rediscovered_running_task() {
    let state = synced_runtime_state(&["running"]);
    let service = TaskSyncService::new(vec![TaskResource {
        observed_state: TaskObservedState::Running,
        claimed_at: Some("2026-05-31T07:55:00Z".to_string()),
        started_at: Some("2026-05-31T07:56:00Z".to_string()),
        ..queued_script_task("task-service-running")
    }]);
    let mut transport = ServiceBackedTransport::new("http://job-manage/tasks", service.clone());
    let executor = RecordingExecutor::succeeds(ExecutionResult {
        stdout: "service-replayed".to_string(),
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
            task_id: "task-service-running".to_string(),
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
    assert_eq!(stored[0].task_id, "task-service-running");
    assert_eq!(stored[0].observed_state, TaskObservedState::Succeeded);
    assert_eq!(stored[0].stdout.as_deref(), Some("service-replayed"));
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
    list_outcomes: VecDeque<ListOutcome>,
    tasks: BTreeMap<String, TaskResource>,
    apply_outcomes: VecDeque<ApplyOutcome>,
}

impl RecordingTransport {
    fn new(tasks: Vec<TaskResource>) -> Self {
        Self {
            list_calls: Vec::new(),
            apply_calls: Vec::new(),
            list_outcomes: VecDeque::from([ListOutcome::Success(tasks.clone())]),
            tasks: tasks
                .into_iter()
                .map(|task| (task.task_id.clone(), task))
                .collect(),
            apply_outcomes: VecDeque::new(),
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
            list_outcomes: list_responses
                .into_iter()
                .map(ListOutcome::Success)
                .collect(),
            tasks,
            apply_outcomes: VecDeque::new(),
        }
    }

    fn with_list_outcomes(list_outcomes: Vec<ListOutcome>) -> Self {
        let tasks = list_outcomes
            .iter()
            .filter_map(|outcome| match outcome {
                ListOutcome::Success(tasks) => Some(tasks.iter()),
                ListOutcome::TransientFail(_) => None,
            })
            .flatten()
            .cloned()
            .map(|task| (task.task_id.clone(), task))
            .collect();

        Self {
            list_calls: Vec::new(),
            apply_calls: Vec::new(),
            list_outcomes: VecDeque::from(list_outcomes),
            tasks,
            apply_outcomes: VecDeque::new(),
        }
    }

    fn with_apply_outcomes(tasks: Vec<TaskResource>, apply_outcomes: Vec<ApplyOutcome>) -> Self {
        let mut transport = Self::new(tasks);
        transport.apply_outcomes = VecDeque::from(apply_outcomes);
        transport
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
        match self
            .list_outcomes
            .pop_front()
            .unwrap_or(ListOutcome::Success(vec![]))
        {
            ListOutcome::Success(tasks) => Ok(tasks),
            ListOutcome::TransientFail(message) => Err(anyhow::anyhow!(message)),
        }
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

        match self.apply_outcomes.pop_front() {
            Some(ApplyOutcome::Fail) => {
                return Err(anyhow::anyhow!("simulated apply failure"));
            }
            Some(ApplyOutcome::TransientFail(message)) => {
                return Err(anyhow::anyhow!(message));
            }
            Some(ApplyOutcome::Success) | None => {}
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

#[derive(Debug, Clone, PartialEq, Eq)]
enum ApplyOutcome {
    Success,
    Fail,
    TransientFail(String),
}

#[derive(Debug, Clone)]
enum ListOutcome {
    Success(Vec<TaskResource>),
    TransientFail(String),
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

#[derive(Debug, Clone, Copy)]
struct StructuredErrorExecutor;

impl StructuredErrorExecutor {
    fn spawn_failed() -> Self {
        Self
    }
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

impl TaskExecutor for StructuredErrorExecutor {
    type ExecuteFuture<'a>
        = Pin<Box<dyn Future<Output = Result<ExecutionResult>> + Send + 'a>>
    where
        Self: 'a;

    fn execute<'a>(&'a self, task: &'a TaskResource) -> Self::ExecuteFuture<'a> {
        let task_id = task.task_id.clone();
        Box::pin(async move { Err(ExecutionError::spawn_failed(task_id).into()) })
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
    let data_dir = writable_data_dir("task-sync-state");
    synced_runtime_state_in(&data_dir, default_states)
}

fn synced_runtime_state_in(data_dir: &str, default_states: &[&str]) -> AgentRuntimeState {
    let config = AgentRuntimeConfig {
        nodemanage_sync_url: "http://127.0.0.1:3000/agent/sync".to_string(),
        agent_id: "agent-1".to_string(),
        node_id: None,
        data_dir: data_dir.to_string(),
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

fn task_with_observed_state(task: TaskResource, observed_state: &str) -> TaskResource {
    let mut value = serde_json::to_value(task).expect("serialize task");
    value["observed_state"] = Value::String(observed_state.to_string());
    serde_json::from_value(value).expect("deserialize task with observed_state")
}

fn writable_data_dir(prefix: &str) -> String {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    let path: PathBuf = env::temp_dir().join(format!(
        "rsagent-task-sync-{prefix}-{}-{unique}",
        std::process::id()
    ));
    std::fs::create_dir_all(&path).expect("create temp task sync dir");
    path.display().to_string()
}

fn timestamp() -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 5, 31, 8, 0, 0).unwrap()
}
