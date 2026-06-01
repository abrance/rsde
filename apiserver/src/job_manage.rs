use std::sync::Arc;

use axum::{
    Json, Router,
    body::to_bytes,
    extract::{Query, Request, State},
    http::StatusCode,
    routing::{get, post},
};
use chrono::{DateTime, SecondsFormat, Utc};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Map, Value};

const MAX_JSON_BODY_BYTES: usize = 1024 * 1024;

#[derive(Clone)]
struct JobManageState {
    service: Arc<::job_manage::TaskSyncService>,
}

#[derive(Debug, Serialize)]
struct ApiResponse<T>
where
    T: Serialize,
{
    success: bool,
    data: Option<T>,
    error: Option<String>,
}

impl<T> ApiResponse<T>
where
    T: Serialize,
{
    fn ok(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    fn err(message: impl Into<String>) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(message.into()),
        }
    }
}

#[derive(Debug, Serialize)]
struct TaskListResponse {
    items: Vec<::job_manage::TaskResource>,
}

#[derive(Debug, Serialize)]
struct TaskApplyResponse {
    task_id: String,
    observed_state: ::job_manage::TaskObservedState,
    updated_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawTaskListQuery {
    agent_id: String,
    node_id: String,
    states: Option<String>,
    updated_after: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TaskApplyIdentityQuery {
    task_id: String,
    agent_id: String,
    node_id: String,
}

#[derive(Debug, Deserialize)]
struct TaskApplyBody {
    observed_state: Option<::job_manage::TaskObservedState>,
    claimed_at: Option<String>,
    started_at: Option<String>,
    finished_at: Option<String>,
    stdout: Option<String>,
    stderr: Option<String>,
    exit_code: Option<i32>,
    error_message: Option<String>,
    updated_at: Option<String>,
}

pub fn create_routes(service: ::job_manage::TaskSyncService) -> Router {
    Router::new()
        .route("/tasks", get(list_tasks))
        .route("/tasks:apply", post(apply_task))
        .with_state(JobManageState {
            service: Arc::new(service),
        })
}

async fn list_tasks(
    State(state): State<JobManageState>,
    request: Request,
) -> (StatusCode, Json<ApiResponse<TaskListResponse>>) {
    let raw = match parse_query::<RawTaskListQuery>(&request) {
        Ok(value) => value,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::err("invalid task list query")),
            );
        }
    };

    let query = match build_task_list_query(raw) {
        Ok(value) => value,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::err("invalid task list query")),
            );
        }
    };

    match state.service.list_tasks(&query).await {
        Ok(items) => (
            StatusCode::OK,
            Json(ApiResponse::ok(TaskListResponse { items })),
        ),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::err("internal server error")),
        ),
    }
}

async fn apply_task(
    State(state): State<JobManageState>,
    request: Request,
) -> (StatusCode, Json<ApiResponse<TaskApplyResponse>>) {
    let identity = match parse_query::<TaskApplyIdentityQuery>(&request) {
        Ok(value) => ::job_manage::TaskApplyIdentity {
            task_id: value.task_id,
            agent_id: value.agent_id,
            node_id: value.node_id,
        },
        Err(_) => {
            return bad_apply_payload_response();
        }
    };

    let task_apply_request = match parse_task_apply_request(request).await {
        Ok(value) => value,
        Err(_) => {
            return bad_apply_payload_response();
        }
    };

    match state
        .service
        .apply_task(&identity, task_apply_request)
        .await
    {
        Ok(task) => (
            StatusCode::OK,
            Json(ApiResponse::ok(TaskApplyResponse {
                task_id: task.task_id,
                observed_state: task.observed_state,
                updated_at: task.updated_at,
            })),
        ),
        Err(
            ::job_manage::TaskServiceError::TaskNotFound(_)
            | ::job_manage::TaskServiceError::TaskOwnershipMismatch { .. },
        ) => not_found_or_conflict_response(),
        Err(
            ::job_manage::TaskServiceError::RejectedServerOwnedFields(_)
            | ::job_manage::TaskServiceError::TaskModel(_),
        ) => bad_apply_payload_response(),
    }
}

fn build_task_list_query(
    raw: RawTaskListQuery,
) -> Result<::job_manage::TaskListQuery, anyhow::Error> {
    let states = raw
        .states
        .as_deref()
        .map(parse_states)
        .transpose()?
        .unwrap_or_default();

    let updated_after = raw
        .updated_after
        .as_deref()
        .map(normalize_rfc3339)
        .transpose()?;

    Ok(::job_manage::TaskListQuery {
        agent_id: raw.agent_id,
        node_id: raw.node_id,
        states,
        updated_after,
    })
}

async fn parse_task_apply_request(
    request: Request,
) -> Result<::job_manage::TaskApplyRequest, anyhow::Error> {
    let raw = parse_json_body_as_object(request).await?;
    let mut rejected_fields = Vec::new();
    let mut allowed = Map::new();

    for (key, value) in raw {
        match key.as_str() {
            "observed_state" | "claimed_at" | "started_at" | "finished_at" | "stdout"
            | "stderr" | "exit_code" | "error_message" | "updated_at" => {
                allowed.insert(key, value);
            }
            "task_id" => rejected_fields.push(::job_manage::TaskServerOwnedField::TaskId),
            "job_id" => rejected_fields.push(::job_manage::TaskServerOwnedField::JobId),
            "node_id" => rejected_fields.push(::job_manage::TaskServerOwnedField::NodeId),
            "agent_id" => rejected_fields.push(::job_manage::TaskServerOwnedField::AgentId),
            "task_type" => rejected_fields.push(::job_manage::TaskServerOwnedField::TaskType),
            "script_content" => {
                rejected_fields.push(::job_manage::TaskServerOwnedField::ScriptContent)
            }
            "command_line" => rejected_fields.push(::job_manage::TaskServerOwnedField::CommandLine),
            "interpreter" => rejected_fields.push(::job_manage::TaskServerOwnedField::Interpreter),
            "args" => rejected_fields.push(::job_manage::TaskServerOwnedField::Args),
            "env" => rejected_fields.push(::job_manage::TaskServerOwnedField::Env),
            "working_dir" => rejected_fields.push(::job_manage::TaskServerOwnedField::WorkingDir),
            "timeout_secs" => rejected_fields.push(::job_manage::TaskServerOwnedField::TimeoutSecs),
            "desired_state" => {
                rejected_fields.push(::job_manage::TaskServerOwnedField::DesiredState)
            }
            _ => return Err(anyhow::anyhow!("unknown task apply field")),
        }
    }

    let body: TaskApplyBody = serde_json::from_value(Value::Object(allowed))?;

    Ok(::job_manage::TaskApplyRequest {
        patch: ::job_manage::TaskApplyPatch {
            observed_state: body.observed_state,
            claimed_at: body
                .claimed_at
                .as_deref()
                .map(normalize_rfc3339)
                .transpose()?,
            started_at: body
                .started_at
                .as_deref()
                .map(normalize_rfc3339)
                .transpose()?,
            finished_at: body
                .finished_at
                .as_deref()
                .map(normalize_rfc3339)
                .transpose()?,
            stdout: body.stdout,
            stderr: body.stderr,
            exit_code: body.exit_code,
            error_message: body.error_message,
            updated_at: body
                .updated_at
                .as_deref()
                .map(normalize_rfc3339)
                .transpose()?,
        },
        rejected_fields,
    })
}

async fn parse_json_body_as_object(request: Request) -> Result<Map<String, Value>, anyhow::Error> {
    let (_parts, body) = request.into_parts();
    let bytes = to_bytes(body, MAX_JSON_BODY_BYTES)
        .await
        .map_err(|err| anyhow::anyhow!("invalid request body: {err}"))?;
    let value = serde_json::from_slice::<Value>(&bytes)
        .map_err(|err| anyhow::anyhow!("invalid request body: {err}"))?;

    match value {
        Value::Object(map) => Ok(map),
        _ => Err(anyhow::anyhow!("request body must be a JSON object")),
    }
}

fn parse_query<T>(request: &Request) -> Result<T, anyhow::Error>
where
    T: DeserializeOwned,
{
    Query::<T>::try_from_uri(request.uri())
        .map(|Query(value)| value)
        .map_err(|err| anyhow::anyhow!("invalid query params: {err}"))
}

fn parse_states(value: &str) -> Result<Vec<::job_manage::TaskObservedState>, anyhow::Error> {
    value
        .split(',')
        .filter(|state| !state.is_empty())
        .map(|state| {
            serde_json::from_str::<::job_manage::TaskObservedState>(&format!("\"{state}\""))
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| anyhow::anyhow!("invalid task state: {err}"))
}

fn normalize_rfc3339(value: &str) -> Result<String, anyhow::Error> {
    Ok(DateTime::parse_from_rfc3339(value)?
        .with_timezone(&Utc)
        .to_rfc3339_opts(SecondsFormat::Secs, true))
}

fn bad_apply_payload_response() -> (StatusCode, Json<ApiResponse<TaskApplyResponse>>) {
    (
        StatusCode::BAD_REQUEST,
        Json(ApiResponse::err("invalid task apply payload")),
    )
}

fn not_found_or_conflict_response() -> (StatusCode, Json<ApiResponse<TaskApplyResponse>>) {
    (
        StatusCode::NOT_FOUND,
        Json(ApiResponse::err(
            "task not found or task ownership conflict",
        )),
    )
}
