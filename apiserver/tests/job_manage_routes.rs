use std::collections::BTreeMap;

use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Method, Request, StatusCode},
};
use job_manage::{TaskDesiredState, TaskObservedState, TaskResource, TaskSyncService, TaskType};
use serde_json::{Value, json};
use tower::ServiceExt;

fn build_app(tasks: Vec<TaskResource>) -> Router {
    Router::new().nest(
        "/api/job-manage/v1",
        apiserver::job_manage::create_routes(TaskSyncService::new(tasks)),
    )
}

fn sample_task(
    task_id: &str,
    agent_id: &str,
    node_id: &str,
    observed_state: TaskObservedState,
    updated_at: &str,
) -> TaskResource {
    TaskResource {
        task_id: task_id.to_string(),
        job_id: "job-1".to_string(),
        node_id: node_id.to_string(),
        agent_id: agent_id.to_string(),
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
        updated_at: Some(updated_at.to_string()),
    }
}

fn make_json_request(method: Method, path: &str, body: Value) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .expect("request")
}

async fn read_json(response: axum::response::Response) -> Value {
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read body");
    serde_json::from_slice(&bytes).expect("json body")
}

#[tokio::test]
async fn get_tasks_filters_by_agent_node_states_and_updated_after() {
    let app = build_app(vec![
        sample_task(
            "task-queued",
            "agent-1",
            "node-1",
            TaskObservedState::Queued,
            "2026-05-31T00:00:00Z",
        ),
        sample_task(
            "task-running",
            "agent-1",
            "node-1",
            TaskObservedState::Running,
            "2026-05-31T01:00:00Z",
        ),
        sample_task(
            "task-other-node",
            "agent-1",
            "node-2",
            TaskObservedState::Running,
            "2026-05-31T02:00:00Z",
        ),
        sample_task(
            "task-other-agent",
            "agent-2",
            "node-1",
            TaskObservedState::Failed,
            "2026-05-31T03:00:00Z",
        ),
    ]);

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri(
                    "/api/job-manage/v1/tasks?agent_id=agent-1&node_id=node-1&states=running,failed&updated_after=2026-05-31T00:30:00Z",
                )
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = read_json(response).await;
    assert_eq!(
        body,
        json!({
            "success": true,
            "data": {
                "items": [
                    {
                        "task_id": "task-running",
                        "job_id": "job-1",
                        "node_id": "node-1",
                        "agent_id": "agent-1",
                        "task_type": "script",
                        "script_content": "echo hello",
                        "command_line": null,
                        "interpreter": "/bin/bash",
                        "args": ["-lc", "echo hello"],
                        "env": {"RUST_LOG": "info"},
                        "working_dir": "/tmp/work",
                        "timeout_secs": 300,
                        "desired_state": "queued",
                        "observed_state": "running",
                        "stdout": null,
                        "stderr": null,
                        "exit_code": null,
                        "started_at": null,
                        "finished_at": null,
                        "error_message": null,
                        "claimed_at": null,
                        "updated_at": "2026-05-31T01:00:00Z"
                    }
                ]
            },
            "error": null
        })
    );
}

#[tokio::test]
async fn post_apply_uses_query_identity_and_partial_body_only() {
    let app = build_app(vec![sample_task(
        "task-apply",
        "agent-1",
        "node-1",
        TaskObservedState::Queued,
        "2026-05-31T00:00:00Z",
    )]);

    let response = app
        .clone()
        .oneshot(make_json_request(
            Method::POST,
            "/api/job-manage/v1/tasks:apply?task_id=task-apply&agent_id=agent-1&node_id=node-1",
            json!({
                "observed_state": "acknowledged",
                "claimed_at": "2026-05-31T00:05:00Z",
                "stdout": "hello",
                "updated_at": "2026-05-31T00:05:00Z"
            }),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = read_json(response).await;
    assert_eq!(
        body,
        json!({
            "success": true,
            "data": {
                "task_id": "task-apply",
                "observed_state": "acknowledged",
                "updated_at": "2026-05-31T00:05:00Z"
            },
            "error": null
        })
    );

    let list = app
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/api/job-manage/v1/tasks?agent_id=agent-1&node_id=node-1&states=acknowledged")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list.status(), StatusCode::OK);
    let list_body = read_json(list).await;
    assert_eq!(list_body["data"]["items"][0]["task_id"], "task-apply");
    assert_eq!(
        list_body["data"]["items"][0]["observed_state"],
        "acknowledged"
    );
    assert_eq!(list_body["data"]["items"][0]["stdout"], "hello");
    assert_eq!(list_body["data"]["items"][0]["desired_state"], "queued");
}

#[tokio::test]
async fn post_apply_rejects_server_owned_fields_and_invalid_state_with_exact_envelope() {
    let app = build_app(vec![sample_task(
        "task-invalid",
        "agent-1",
        "node-1",
        TaskObservedState::Queued,
        "2026-05-31T00:00:00Z",
    )]);

    let server_owned = app
        .clone()
        .oneshot(make_json_request(
            Method::POST,
            "/api/job-manage/v1/tasks:apply?task_id=task-invalid&agent_id=agent-1&node_id=node-1",
            json!({
                "task_id": "task-invalid",
                "observed_state": "acknowledged",
                "updated_at": "2026-05-31T00:01:00Z"
            }),
        ))
        .await
        .unwrap();

    assert_eq!(server_owned.status(), StatusCode::BAD_REQUEST);
    let server_owned_body = read_json(server_owned).await;
    assert_eq!(
        server_owned_body,
        json!({
            "success": false,
            "data": null,
            "error": "invalid task apply payload"
        })
    );

    let invalid_state = app
        .oneshot(make_json_request(
            Method::POST,
            "/api/job-manage/v1/tasks:apply?task_id=task-invalid&agent_id=agent-1&node_id=node-1",
            json!({
                "observed_state": "running",
                "updated_at": "2026-05-31T00:02:00Z"
            }),
        ))
        .await
        .unwrap();

    assert_eq!(invalid_state.status(), StatusCode::BAD_REQUEST);
    let invalid_state_body = read_json(invalid_state).await;
    assert_eq!(
        invalid_state_body,
        json!({
            "success": false,
            "data": null,
            "error": "invalid task apply payload"
        })
    );
}

#[tokio::test]
async fn post_apply_collapses_not_found_and_ownership_conflict_to_exact_envelope() {
    let app = build_app(vec![sample_task(
        "task-owned",
        "agent-1",
        "node-1",
        TaskObservedState::Queued,
        "2026-05-31T00:00:00Z",
    )]);

    let missing = app
        .clone()
        .oneshot(make_json_request(
            Method::POST,
            "/api/job-manage/v1/tasks:apply?task_id=missing-task&agent_id=agent-1&node_id=node-1",
            json!({
                "observed_state": "acknowledged",
                "updated_at": "2026-05-31T00:01:00Z"
            }),
        ))
        .await
        .unwrap();
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);
    let missing_body = read_json(missing).await;
    assert_eq!(
        missing_body,
        json!({
            "success": false,
            "data": null,
            "error": "task not found or task ownership conflict"
        })
    );

    let ownership_conflict = app
        .oneshot(make_json_request(
            Method::POST,
            "/api/job-manage/v1/tasks:apply?task_id=task-owned&agent_id=agent-2&node_id=node-1",
            json!({
                "observed_state": "acknowledged",
                "updated_at": "2026-05-31T00:01:00Z"
            }),
        ))
        .await
        .unwrap();
    assert_eq!(ownership_conflict.status(), StatusCode::NOT_FOUND);
    let ownership_conflict_body = read_json(ownership_conflict).await;
    assert_eq!(
        ownership_conflict_body,
        json!({
            "success": false,
            "data": null,
            "error": "task not found or task ownership conflict"
        })
    );
}

#[tokio::test]
async fn task_routes_persist_rsagent_style_terminal_updates() {
    let app = build_app(vec![sample_task(
        "task-terminal",
        "agent-1",
        "node-1",
        TaskObservedState::Queued,
        "2026-05-31T00:00:00Z",
    )]);

    let list = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/api/job-manage/v1/tasks?agent_id=agent-1&node_id=node-1&states=queued,acknowledged,running")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list.status(), StatusCode::OK);
    let list_body = read_json(list).await;
    assert_eq!(list_body["data"]["items"][0]["task_id"], "task-terminal");

    for (observed_state, extra_fields, updated_at) in [
        (
            "acknowledged",
            json!({"claimed_at": "2026-05-31T00:01:00Z"}),
            "2026-05-31T00:01:00Z",
        ),
        (
            "running",
            json!({"started_at": "2026-05-31T00:02:00Z"}),
            "2026-05-31T00:02:00Z",
        ),
        (
            "succeeded",
            json!({
                "finished_at": "2026-05-31T00:03:00Z",
                "stdout": "done",
                "stderr": "",
                "exit_code": 0
            }),
            "2026-05-31T00:03:00Z",
        ),
    ] {
        let mut payload = serde_json::Map::new();
        payload.insert("observed_state".to_string(), json!(observed_state));
        payload.insert("updated_at".to_string(), json!(updated_at));
        if let Value::Object(extra) = extra_fields {
            payload.extend(extra);
        }

        let response = app
            .clone()
            .oneshot(make_json_request(
                Method::POST,
                "/api/job-manage/v1/tasks:apply?task_id=task-terminal&agent_id=agent-1&node_id=node-1",
                Value::Object(payload),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    let stored = app
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/api/job-manage/v1/tasks?agent_id=agent-1&node_id=node-1&states=succeeded")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(stored.status(), StatusCode::OK);
    let stored_body = read_json(stored).await;
    assert_eq!(
        stored_body["data"]["items"][0]["observed_state"],
        "succeeded"
    );
    assert_eq!(stored_body["data"]["items"][0]["stdout"], "done");
    assert_eq!(stored_body["data"]["items"][0]["exit_code"], 0);
    assert_eq!(stored_body["data"]["items"][0]["desired_state"], "queued");
}
