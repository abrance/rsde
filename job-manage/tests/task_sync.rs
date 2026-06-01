use std::collections::BTreeMap;

use job_manage::{
    JobManageError, TaskApplyIdentity, TaskApplyPatch, TaskApplyRequest, TaskDesiredState,
    TaskListQuery, TaskObservedState, TaskResource, TaskServerOwnedField, TaskServiceError,
    TaskSyncService, TaskType,
};

fn sample_task_resource(observed_state: TaskObservedState) -> TaskResource {
    TaskResource {
        task_id: "task-1".to_string(),
        job_id: "job-1".to_string(),
        node_id: "node-1".to_string(),
        agent_id: "agent-1".to_string(),
        task_type: TaskType::Script,
        script_content: Some("echo hello".to_string()),
        command_line: None,
        interpreter: Some("/bin/bash".to_string()),
        args: vec!["-lc".to_string(), "echo hello".to_string()],
        env: BTreeMap::from([("RUST_LOG".to_string(), "info".to_string())]),
        working_dir: Some("/tmp/work".to_string()),
        timeout_secs: Some(300),
        desired_state: TaskDesiredState::Queued,
        observed_state,
        stdout: None,
        stderr: None,
        exit_code: None,
        started_at: None,
        finished_at: None,
        error_message: None,
        claimed_at: None,
        updated_at: Some("2026-05-31T00:00:00Z".to_string()),
    }
}

fn sample_task(
    task_id: &str,
    agent_id: &str,
    node_id: &str,
    observed_state: TaskObservedState,
    updated_at: Option<&str>,
) -> TaskResource {
    let mut task = sample_task_resource(observed_state);
    task.task_id = task_id.to_string();
    task.agent_id = agent_id.to_string();
    task.node_id = node_id.to_string();
    task.updated_at = updated_at.map(str::to_string);
    task
}

#[test]
fn task_resource_exposes_first_phase_shape() {
    let task = sample_task_resource(TaskObservedState::Queued);

    assert_eq!(task.task_id, "task-1");
    assert_eq!(task.job_id, "job-1");
    assert_eq!(task.node_id, "node-1");
    assert_eq!(task.agent_id, "agent-1");
    assert_eq!(task.task_type, TaskType::Script);
    assert_eq!(task.script_content.as_deref(), Some("echo hello"));
    assert_eq!(task.command_line, None);
    assert_eq!(task.interpreter.as_deref(), Some("/bin/bash"));
    assert_eq!(task.args, vec!["-lc", "echo hello"]);
    assert_eq!(task.env.get("RUST_LOG").map(String::as_str), Some("info"));
    assert_eq!(task.working_dir.as_deref(), Some("/tmp/work"));
    assert_eq!(task.timeout_secs, Some(300));
    assert_eq!(task.desired_state, TaskDesiredState::Queued);
    assert_eq!(task.observed_state, TaskObservedState::Queued);
    assert_eq!(task.stdout, None);
    assert_eq!(task.stderr, None);
    assert_eq!(task.exit_code, None);
    assert_eq!(task.started_at, None);
    assert_eq!(task.finished_at, None);
    assert_eq!(task.error_message, None);
    assert_eq!(task.claimed_at, None);
    assert_eq!(task.updated_at.as_deref(), Some("2026-05-31T00:00:00Z"));
}

#[test]
fn transitioning_observed_state_preserves_desired_state() {
    let mut task = sample_task_resource(TaskObservedState::Queued);

    task.transition_observed_state(TaskObservedState::Acknowledged)
        .unwrap();

    assert_eq!(task.desired_state, TaskDesiredState::Queued);
    assert_eq!(task.observed_state, TaskObservedState::Acknowledged);
}

#[test]
fn observed_state_only_allows_first_phase_legal_transitions() {
    assert!(TaskObservedState::Queued.can_transition_to(TaskObservedState::Acknowledged));
    assert!(TaskObservedState::Acknowledged.can_transition_to(TaskObservedState::Running));
    assert!(TaskObservedState::Acknowledged.can_transition_to(TaskObservedState::Succeeded));
    assert!(TaskObservedState::Acknowledged.can_transition_to(TaskObservedState::Failed));
    assert!(TaskObservedState::Acknowledged.can_transition_to(TaskObservedState::Timeout));
    assert!(TaskObservedState::Running.can_transition_to(TaskObservedState::Succeeded));
    assert!(TaskObservedState::Running.can_transition_to(TaskObservedState::Failed));
    assert!(TaskObservedState::Running.can_transition_to(TaskObservedState::Timeout));

    assert!(!TaskObservedState::Queued.can_transition_to(TaskObservedState::Running));
    assert!(!TaskObservedState::Queued.can_transition_to(TaskObservedState::Succeeded));
    assert!(!TaskObservedState::Running.can_transition_to(TaskObservedState::Acknowledged));
}

#[test]
fn transition_rejects_illegal_non_terminal_progression() {
    let mut task = sample_task_resource(TaskObservedState::Queued);

    let err = task
        .transition_observed_state(TaskObservedState::Running)
        .unwrap_err();

    assert_eq!(
        err,
        JobManageError::InvalidTaskObservedStateTransition {
            from: TaskObservedState::Queued,
            to: TaskObservedState::Running,
        }
    );
    assert_eq!(task.observed_state, TaskObservedState::Queued);
}

#[test]
fn terminal_states_cannot_regress_to_non_terminal_states() {
    for terminal_state in [
        TaskObservedState::Succeeded,
        TaskObservedState::Failed,
        TaskObservedState::Timeout,
    ] {
        let mut task = sample_task_resource(terminal_state);

        let err = task
            .transition_observed_state(TaskObservedState::Running)
            .unwrap_err();

        assert_eq!(
            err,
            JobManageError::InvalidTaskObservedStateTransition {
                from: terminal_state,
                to: TaskObservedState::Running,
            }
        );
        assert_eq!(task.observed_state, terminal_state);
    }
}

#[tokio::test]
async fn list_filters_by_agent_node_states_and_updated_after() {
    let service = TaskSyncService::new(vec![
        sample_task(
            "task-queued",
            "agent-1",
            "node-1",
            TaskObservedState::Queued,
            Some("2026-05-31T00:00:00Z"),
        ),
        sample_task(
            "task-running",
            "agent-1",
            "node-1",
            TaskObservedState::Running,
            Some("2026-05-31T01:00:00Z"),
        ),
        sample_task(
            "task-other-node",
            "agent-1",
            "node-2",
            TaskObservedState::Running,
            Some("2026-05-31T02:00:00Z"),
        ),
        sample_task(
            "task-other-agent",
            "agent-2",
            "node-1",
            TaskObservedState::Failed,
            Some("2026-05-31T03:00:00Z"),
        ),
    ]);

    let tasks = service
        .list_tasks(&TaskListQuery {
            agent_id: "agent-1".to_string(),
            node_id: "node-1".to_string(),
            states: vec![TaskObservedState::Running, TaskObservedState::Failed],
            updated_after: Some("2026-05-31T00:30:00Z".to_string()),
        })
        .await
        .unwrap();

    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].task_id, "task-running");
}

#[tokio::test]
async fn apply_updates_only_fields_present_in_partial_patch() {
    let mut task = sample_task(
        "task-apply",
        "agent-1",
        "node-1",
        TaskObservedState::Queued,
        Some("2026-05-31T00:00:00Z"),
    );
    task.stderr = Some("keep-me".to_string());

    let service = TaskSyncService::new(vec![task]);
    let identity = TaskApplyIdentity {
        task_id: "task-apply".to_string(),
        agent_id: "agent-1".to_string(),
        node_id: "node-1".to_string(),
    };

    let updated = service
        .apply_task(
            &identity,
            TaskApplyRequest {
                patch: TaskApplyPatch {
                    observed_state: Some(TaskObservedState::Acknowledged),
                    claimed_at: Some("2026-05-31T00:05:00Z".to_string()),
                    stdout: Some("hello".to_string()),
                    updated_at: Some("2026-05-31T00:05:00Z".to_string()),
                    ..Default::default()
                },
                rejected_fields: vec![],
            },
        )
        .await
        .unwrap();

    assert_eq!(updated.desired_state, TaskDesiredState::Queued);
    assert_eq!(updated.observed_state, TaskObservedState::Acknowledged);
    assert_eq!(updated.claimed_at.as_deref(), Some("2026-05-31T00:05:00Z"));
    assert_eq!(updated.stdout.as_deref(), Some("hello"));
    assert_eq!(updated.stderr.as_deref(), Some("keep-me"));
    assert_eq!(updated.started_at, None);
    assert_eq!(updated.finished_at, None);
}

#[tokio::test]
async fn repeated_apply_with_same_payload_is_idempotent() {
    let service = TaskSyncService::new(vec![sample_task(
        "task-idempotent",
        "agent-1",
        "node-1",
        TaskObservedState::Queued,
        Some("2026-05-31T00:00:00Z"),
    )]);
    let identity = TaskApplyIdentity {
        task_id: "task-idempotent".to_string(),
        agent_id: "agent-1".to_string(),
        node_id: "node-1".to_string(),
    };
    let request = TaskApplyRequest {
        patch: TaskApplyPatch {
            observed_state: Some(TaskObservedState::Acknowledged),
            claimed_at: Some("2026-05-31T00:01:00Z".to_string()),
            updated_at: Some("2026-05-31T00:01:00Z".to_string()),
            ..Default::default()
        },
        rejected_fields: vec![],
    };

    let first = service
        .apply_task(&identity, request.clone())
        .await
        .unwrap();
    let second = service.apply_task(&identity, request).await.unwrap();

    assert_eq!(first, second);
}

#[tokio::test]
async fn apply_rejects_server_owned_fields() {
    let service = TaskSyncService::new(vec![sample_task(
        "task-server-owned",
        "agent-1",
        "node-1",
        TaskObservedState::Queued,
        Some("2026-05-31T00:00:00Z"),
    )]);

    let err = service
        .apply_task(
            &TaskApplyIdentity {
                task_id: "task-server-owned".to_string(),
                agent_id: "agent-1".to_string(),
                node_id: "node-1".to_string(),
            },
            TaskApplyRequest {
                patch: TaskApplyPatch {
                    observed_state: Some(TaskObservedState::Acknowledged),
                    ..Default::default()
                },
                rejected_fields: vec![
                    TaskServerOwnedField::DesiredState,
                    TaskServerOwnedField::ScriptContent,
                ],
            },
        )
        .await
        .unwrap_err();

    assert_eq!(
        err,
        TaskServiceError::RejectedServerOwnedFields(vec![
            TaskServerOwnedField::DesiredState,
            TaskServerOwnedField::ScriptContent,
        ])
    );
}

#[tokio::test]
async fn apply_rejects_terminal_state_regression() {
    let service = TaskSyncService::new(vec![sample_task(
        "task-terminal",
        "agent-1",
        "node-1",
        TaskObservedState::Succeeded,
        Some("2026-05-31T00:00:00Z"),
    )]);

    let err = service
        .apply_task(
            &TaskApplyIdentity {
                task_id: "task-terminal".to_string(),
                agent_id: "agent-1".to_string(),
                node_id: "node-1".to_string(),
            },
            TaskApplyRequest {
                patch: TaskApplyPatch {
                    observed_state: Some(TaskObservedState::Running),
                    ..Default::default()
                },
                rejected_fields: vec![],
            },
        )
        .await
        .unwrap_err();

    assert_eq!(
        err,
        TaskServiceError::TaskModel(JobManageError::InvalidTaskObservedStateTransition {
            from: TaskObservedState::Succeeded,
            to: TaskObservedState::Running,
        })
    );
}

#[tokio::test]
async fn apply_distinguishes_not_found_and_ownership_mismatch() {
    let service = TaskSyncService::new(vec![sample_task(
        "task-owned",
        "agent-1",
        "node-1",
        TaskObservedState::Queued,
        Some("2026-05-31T00:00:00Z"),
    )]);

    let missing = service
        .apply_task(
            &TaskApplyIdentity {
                task_id: "missing-task".to_string(),
                agent_id: "agent-1".to_string(),
                node_id: "node-1".to_string(),
            },
            TaskApplyRequest::default(),
        )
        .await
        .unwrap_err();
    assert_eq!(
        missing,
        TaskServiceError::TaskNotFound("missing-task".to_string())
    );

    let mismatch = service
        .apply_task(
            &TaskApplyIdentity {
                task_id: "task-owned".to_string(),
                agent_id: "agent-2".to_string(),
                node_id: "node-1".to_string(),
            },
            TaskApplyRequest::default(),
        )
        .await
        .unwrap_err();
    assert_eq!(
        mismatch,
        TaskServiceError::TaskOwnershipMismatch {
            task_id: "task-owned".to_string(),
            agent_id: "agent-2".to_string(),
            node_id: "node-1".to_string(),
        }
    );
}
