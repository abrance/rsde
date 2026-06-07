use std::{error::Error as StdError, ffi::OsStr, fmt, process::Stdio, time::Duration};

use anyhow::{Context, Result};
use job_manage::{TaskObservedState, TaskResource, TaskType};
use tokio::{
    io::AsyncReadExt,
    process::{Child, ChildStderr, ChildStdout, Command},
    task::JoinHandle,
    time,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
    pub state: TaskObservedState,
}

impl ExecutionResult {
    pub fn classification(&self) -> ExecutionClassification {
        match self.state {
            TaskObservedState::Timeout => ExecutionClassification::TimedOut,
            TaskObservedState::Succeeded if self.exit_code == Some(0) => {
                ExecutionClassification::Succeeded
            }
            _ => ExecutionClassification::NonZeroExit,
        }
    }

    pub fn normalized_state(&self) -> TaskObservedState {
        self.classification().observed_state()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionClassification {
    Succeeded,
    NonZeroExit,
    TimedOut,
}

impl ExecutionClassification {
    pub fn observed_state(self) -> TaskObservedState {
        match self {
            Self::Succeeded => TaskObservedState::Succeeded,
            Self::NonZeroExit => TaskObservedState::Failed,
            Self::TimedOut => TaskObservedState::Timeout,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionErrorKind {
    InvalidPayload,
    SpawnFailed,
}

#[derive(Debug)]
pub struct ExecutionError {
    kind: ExecutionErrorKind,
    task_id: String,
    message: String,
    source: Option<Box<dyn StdError + Send + Sync>>,
}

impl ExecutionError {
    pub fn invalid_payload(task_id: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            kind: ExecutionErrorKind::InvalidPayload,
            task_id: task_id.into(),
            message: message.into(),
            source: None,
        }
    }

    pub fn spawn_failed(task_id: impl Into<String>) -> Self {
        let task_id = task_id.into();
        Self {
            kind: ExecutionErrorKind::SpawnFailed,
            message: format!("failed to spawn task {task_id}"),
            task_id,
            source: None,
        }
    }

    fn spawn_failed_with_source(task_id: impl Into<String>, source: std::io::Error) -> Self {
        let task_id = task_id.into();
        Self {
            kind: ExecutionErrorKind::SpawnFailed,
            message: format!("failed to spawn task {task_id}"),
            task_id,
            source: Some(Box::new(source)),
        }
    }

    pub fn kind(&self) -> ExecutionErrorKind {
        self.kind
    }

    pub fn task_id(&self) -> &str {
        &self.task_id
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for ExecutionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl StdError for ExecutionError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        self.source
            .as_deref()
            .map(|source| source as &(dyn StdError + 'static))
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct LocalTaskExecutor;

impl LocalTaskExecutor {
    pub async fn execute(&self, task: &TaskResource) -> Result<ExecutionResult> {
        let mut command = build_command(task).map_err(anyhow::Error::from)?;
        command.stdout(Stdio::piped()).stderr(Stdio::piped());

        let mut child = command.spawn().map_err(|error| {
            ExecutionError::spawn_failed_with_source(task.task_id.clone(), error)
        })?;

        let stdout_handle = spawn_stdout_reader(child.stdout.take());
        let stderr_handle = spawn_stderr_reader(child.stderr.take());

        let completion = wait_for_completion(task.timeout_secs, &mut child).await?;
        let stdout = join_output(stdout_handle).await?;
        let stderr = join_output(stderr_handle).await?;

        Ok(ExecutionResult {
            stdout,
            stderr,
            exit_code: completion.exit_code,
            state: completion.state,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ProcessCompletion {
    state: TaskObservedState,
    exit_code: Option<i32>,
}

fn build_command(task: &TaskResource) -> std::result::Result<Command, ExecutionError> {
    validate_task_payload(task)?;

    let mut command = match task.task_type {
        TaskType::Script => build_script_command(task)?,
        TaskType::Command => build_command_task(task)?,
    };

    if let Some(working_dir) = task.working_dir.as_deref() {
        command.current_dir(working_dir);
    }

    if !task.env.is_empty() {
        command.envs(
            task.env
                .iter()
                .map(|(key, value)| (key.as_str(), value.as_str())),
        );
    }

    Ok(command)
}

fn validate_task_payload(task: &TaskResource) -> std::result::Result<(), ExecutionError> {
    match task.task_type {
        TaskType::Script => {
            if populated_field(task.command_line.as_deref()) {
                return Err(ExecutionError::invalid_payload(
                    task.task_id.clone(),
                    "script task cannot include command_line",
                ));
            }
        }
        TaskType::Command => {
            if populated_field(task.script_content.as_deref()) {
                return Err(ExecutionError::invalid_payload(
                    task.task_id.clone(),
                    "command task cannot include script_content",
                ));
            }
        }
    }

    Ok(())
}

fn build_script_command(task: &TaskResource) -> std::result::Result<Command, ExecutionError> {
    let script = task
        .script_content
        .as_deref()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            ExecutionError::invalid_payload(
                task.task_id.clone(),
                "script task missing script_content",
            )
        })?;
    let interpreter = task.interpreter.as_deref().unwrap_or("sh");

    let mut command = Command::new(interpreter);
    command.arg("-c").arg(script).arg("rsagent");
    append_args(&mut command, &task.args);
    Ok(command)
}

fn build_command_task(task: &TaskResource) -> std::result::Result<Command, ExecutionError> {
    let command_line = task
        .command_line
        .as_deref()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            ExecutionError::invalid_payload(
                task.task_id.clone(),
                "command task missing command_line",
            )
        })?;

    let mut command = Command::new(command_line);
    append_args(&mut command, &task.args);
    Ok(command)
}

fn append_args(command: &mut Command, args: &[String]) {
    command.args(args.iter().map(OsStr::new));
}

fn populated_field(value: Option<&str>) -> bool {
    value.is_some_and(|value| !value.is_empty())
}

async fn wait_for_completion(
    timeout_secs: Option<u64>,
    child: &mut Child,
) -> Result<ProcessCompletion> {
    let wait_future = child.wait();

    let status = if let Some(timeout_secs) = timeout_secs {
        match time::timeout(Duration::from_secs(timeout_secs), wait_future).await {
            Ok(status) => Some(status?),
            Err(_) => {
                child
                    .kill()
                    .await
                    .context("failed to kill timed out task process")?;
                child
                    .wait()
                    .await
                    .context("failed to reap timed out task process")?;
                None
            }
        }
    } else {
        Some(wait_future.await?)
    };

    Ok(match status {
        Some(status) if status.success() => ProcessCompletion {
            state: TaskObservedState::Succeeded,
            exit_code: status.code(),
        },
        Some(status) => ProcessCompletion {
            state: TaskObservedState::Failed,
            exit_code: status.code(),
        },
        None => ProcessCompletion {
            state: TaskObservedState::Timeout,
            exit_code: None,
        },
    })
}

fn spawn_stdout_reader(stdout: Option<ChildStdout>) -> JoinHandle<Result<String>> {
    tokio::spawn(read_output(stdout))
}

fn spawn_stderr_reader(stderr: Option<ChildStderr>) -> JoinHandle<Result<String>> {
    tokio::spawn(read_output(stderr))
}

async fn read_output<R>(reader: Option<R>) -> Result<String>
where
    R: tokio::io::AsyncRead + Unpin,
{
    let Some(mut reader) = reader else {
        return Ok(String::new());
    };

    let mut buffer = Vec::new();
    reader.read_to_end(&mut buffer).await?;
    Ok(String::from_utf8_lossy(&buffer).into_owned())
}

async fn join_output(handle: JoinHandle<Result<String>>) -> Result<String> {
    handle.await.context("output collection task failed")?
}
