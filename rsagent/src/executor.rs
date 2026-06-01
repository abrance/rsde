use std::{ffi::OsStr, process::Stdio, time::Duration};

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

#[derive(Debug, Default, Clone, Copy)]
pub struct LocalTaskExecutor;

impl LocalTaskExecutor {
    pub async fn execute(&self, task: &TaskResource) -> Result<ExecutionResult> {
        let mut command = build_command(task)?;
        command.stdout(Stdio::piped()).stderr(Stdio::piped());

        let mut child = command
            .spawn()
            .with_context(|| format!("failed to spawn task {}", task.task_id))?;

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

fn build_command(task: &TaskResource) -> Result<Command> {
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

fn validate_task_payload(task: &TaskResource) -> Result<()> {
    match task.task_type {
        TaskType::Script => {
            if populated_field(task.command_line.as_deref()) {
                anyhow::bail!("script task cannot include command_line");
            }
        }
        TaskType::Command => {
            if populated_field(task.script_content.as_deref()) {
                anyhow::bail!("command task cannot include script_content");
            }
        }
    }

    Ok(())
}

fn build_script_command(task: &TaskResource) -> Result<Command> {
    let script = task
        .script_content
        .as_deref()
        .filter(|value| !value.is_empty())
        .context("script task missing script_content")?;
    let interpreter = task.interpreter.as_deref().unwrap_or("sh");

    let mut command = Command::new(interpreter);
    command.arg("-c").arg(script).arg("rsagent");
    append_args(&mut command, &task.args);
    Ok(command)
}

fn build_command_task(task: &TaskResource) -> Result<Command> {
    let command_line = task
        .command_line
        .as_deref()
        .filter(|value| !value.is_empty())
        .context("command task missing command_line")?;

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
