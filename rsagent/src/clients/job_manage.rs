use anyhow::{Context, Result, anyhow};
use job_manage::{
    TaskApplyIdentity, TaskApplyPatch, TaskListQuery, TaskObservedState, TaskResource,
};
use serde::{Deserialize, Serialize};

pub trait JobManageTransport {
    fn list_tasks(&mut self, endpoint: &str, query: &TaskListQuery) -> Result<Vec<TaskResource>>;

    fn apply_task(
        &mut self,
        endpoint: &str,
        identity: &TaskApplyIdentity,
        patch: &TaskApplyPatch,
    ) -> Result<TaskApplyAck>;
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct TaskApplyAck {
    pub task_id: String,
    pub observed_state: TaskObservedState,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobManageSyncClient {
    base_url: String,
}

impl JobManageSyncClient {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
        }
    }

    pub fn tasks_endpoint(&self) -> String {
        format!("{}/tasks", self.base_url.trim_end_matches('/'))
    }

    pub fn apply_endpoint(&self) -> String {
        format!("{}/tasks:apply", self.base_url.trim_end_matches('/'))
    }

    pub fn list_tasks<T>(
        &self,
        transport: &mut T,
        query: &TaskListQuery,
    ) -> Result<Vec<TaskResource>>
    where
        T: JobManageTransport,
    {
        transport.list_tasks(&self.tasks_endpoint(), query)
    }

    pub fn apply_task<T>(
        &self,
        transport: &mut T,
        identity: &TaskApplyIdentity,
        patch: &TaskApplyPatch,
    ) -> Result<TaskApplyAck>
    where
        T: JobManageTransport,
    {
        transport.apply_task(&self.apply_endpoint(), identity, patch)
    }
}

pub struct ReqwestJobManageTransport {
    client: reqwest::blocking::Client,
}

impl Default for ReqwestJobManageTransport {
    fn default() -> Self {
        Self {
            client: reqwest::blocking::Client::new(),
        }
    }
}

impl JobManageTransport for ReqwestJobManageTransport {
    fn list_tasks(&mut self, endpoint: &str, query: &TaskListQuery) -> Result<Vec<TaskResource>> {
        let mut request = self.client.get(endpoint).query(&[
            ("agent_id", query.agent_id.as_str()),
            ("node_id", query.node_id.as_str()),
        ]);

        let states = encode_states(&query.states)?;
        if !states.is_empty() {
            request = request.query(&[("states", states.as_str())]);
        }

        if let Some(updated_after) = query.updated_after.as_deref() {
            request = request.query(&[("updated_after", updated_after)]);
        }

        let response = request
            .send()
            .with_context(|| format!("failed to GET task list from {endpoint}"))?;

        let envelope: TaskListEnvelope = decode_response(response, endpoint)?;
        envelope
            .data
            .map(|data| data.items)
            .ok_or_else(|| anyhow!("job-manage task list response missing data"))
    }

    fn apply_task(
        &mut self,
        endpoint: &str,
        identity: &TaskApplyIdentity,
        patch: &TaskApplyPatch,
    ) -> Result<TaskApplyAck> {
        let response = self
            .client
            .post(endpoint)
            .query(&[
                ("task_id", identity.task_id.as_str()),
                ("agent_id", identity.agent_id.as_str()),
                ("node_id", identity.node_id.as_str()),
            ])
            .json(&TaskApplyBody::from(patch))
            .send()
            .with_context(|| format!("failed to POST task apply to {endpoint}"))?;

        let envelope: TaskApplyEnvelope = decode_response(response, endpoint)?;
        envelope
            .data
            .ok_or_else(|| anyhow!("job-manage task apply response missing data"))
    }
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct TaskListEnvelope {
    success: bool,
    data: Option<TaskListData>,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TaskListData {
    items: Vec<TaskResource>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct TaskApplyEnvelope {
    success: bool,
    data: Option<TaskApplyAck>,
    error: Option<String>,
}

#[derive(Debug, Serialize)]
struct TaskApplyBody {
    #[serde(skip_serializing_if = "Option::is_none")]
    observed_state: Option<TaskObservedState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    claimed_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    started_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    finished_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stdout: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stderr: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    exit_code: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error_message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    updated_at: Option<String>,
}

impl From<&TaskApplyPatch> for TaskApplyBody {
    fn from(value: &TaskApplyPatch) -> Self {
        Self {
            observed_state: value.observed_state,
            claimed_at: value.claimed_at.clone(),
            started_at: value.started_at.clone(),
            finished_at: value.finished_at.clone(),
            stdout: value.stdout.clone(),
            stderr: value.stderr.clone(),
            exit_code: value.exit_code,
            error_message: value.error_message.clone(),
            updated_at: value.updated_at.clone(),
        }
    }
}

fn encode_states(states: &[TaskObservedState]) -> Result<String> {
    states
        .iter()
        .map(|state| {
            serde_json::to_string(state)
                .map(|value| value.trim_matches('"').to_string())
                .context("failed to encode task observed state")
        })
        .collect::<Result<Vec<_>>>()
        .map(|values| values.join(","))
}

fn decode_response<T>(response: reqwest::blocking::Response, endpoint: &str) -> Result<T>
where
    T: for<'de> Deserialize<'de>,
{
    let status = response.status();
    let body = response
        .text()
        .with_context(|| format!("failed to read job-manage response body from {endpoint}"))?;

    if !status.is_success() {
        return Err(anyhow!(
            "job-manage request to {endpoint} failed with status {status}: {body}"
        ));
    }

    let value: serde_json::Value = serde_json::from_str(&body)
        .with_context(|| format!("invalid JSON response from {endpoint}"))?;

    if let Some(success) = value.get("success").and_then(|value| value.as_bool())
        && !success
    {
        let error = value
            .get("error")
            .and_then(|value| value.as_str())
            .unwrap_or("unknown job-manage error");
        return Err(anyhow!(
            "job-manage request to {endpoint} returned error: {error}"
        ));
    }

    serde_json::from_value(value)
        .with_context(|| format!("failed to decode response from {endpoint}"))
}
