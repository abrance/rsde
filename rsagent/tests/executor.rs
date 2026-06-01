use std::{
    collections::BTreeMap,
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use job_manage::{TaskDesiredState, TaskObservedState, TaskResource, TaskType};
use rsagent::executor::{ExecutionResult, LocalTaskExecutor};

#[tokio::test]
async fn executes_script_tasks_with_interpreter_args_and_env() {
    let task = task_resource(TaskType::Script);
    let task = TaskResource {
        script_content: Some("printf '%s|%s' \"$1\" \"$TASK_ENV\"".to_string()),
        interpreter: Some("sh".to_string()),
        args: vec!["arg-value".to_string()],
        env: BTreeMap::from([("TASK_ENV".to_string(), "env-value".to_string())]),
        timeout_secs: Some(5),
        ..task
    };

    let result = LocalTaskExecutor.execute(&task).await.unwrap();

    assert_execution(
        &result,
        "arg-value|env-value",
        "",
        Some(0),
        TaskObservedState::Succeeded,
    );
}

#[tokio::test]
async fn executes_command_tasks_in_the_requested_working_directory() {
    let working_dir = unique_temp_dir("executor-command");
    fs::create_dir_all(&working_dir).unwrap();

    let task = TaskResource {
        command_line: Some("sh".to_string()),
        args: vec!["-c".to_string(), "printf '%s' \"$PWD\"".to_string()],
        working_dir: Some(working_dir.display().to_string()),
        timeout_secs: Some(5),
        ..task_resource(TaskType::Command)
    };

    let result = LocalTaskExecutor.execute(&task).await.unwrap();

    assert_execution(
        &result,
        &working_dir.display().to_string(),
        "",
        Some(0),
        TaskObservedState::Succeeded,
    );
}

#[tokio::test]
async fn captures_stdout_stderr_and_failed_exit_code_for_command_tasks() {
    let task = TaskResource {
        command_line: Some("sh".to_string()),
        args: vec![
            "-c".to_string(),
            "printf 'cmd-out'; >&2 printf 'cmd-err'; exit 7".to_string(),
        ],
        timeout_secs: Some(5),
        ..task_resource(TaskType::Command)
    };

    let result = LocalTaskExecutor.execute(&task).await.unwrap();

    assert_execution(
        &result,
        "cmd-out",
        "cmd-err",
        Some(7),
        TaskObservedState::Failed,
    );
}

#[tokio::test]
async fn marks_timed_out_tasks_with_timeout_terminal_state() {
    let task = TaskResource {
        command_line: Some("sh".to_string()),
        args: vec!["-c".to_string(), "sleep 2".to_string()],
        timeout_secs: Some(1),
        ..task_resource(TaskType::Command)
    };

    let result = LocalTaskExecutor.execute(&task).await.unwrap();

    assert_execution(&result, "", "", None, TaskObservedState::Timeout);
}

#[tokio::test]
async fn rejects_script_tasks_when_command_line_is_also_populated() {
    let task = TaskResource {
        script_content: Some("printf 'script-only'".to_string()),
        command_line: Some("sh".to_string()),
        ..task_resource(TaskType::Script)
    };

    let error = LocalTaskExecutor.execute(&task).await.unwrap_err();

    assert!(
        error
            .to_string()
            .contains("script task cannot include command_line")
    );
}

#[tokio::test]
async fn rejects_command_tasks_when_script_content_is_also_populated() {
    let task = TaskResource {
        command_line: Some("sh".to_string()),
        script_content: Some("printf 'command-only'".to_string()),
        ..task_resource(TaskType::Command)
    };

    let error = LocalTaskExecutor.execute(&task).await.unwrap_err();

    assert!(
        error
            .to_string()
            .contains("command task cannot include script_content")
    );
}

fn assert_execution(
    result: &ExecutionResult,
    stdout: &str,
    stderr: &str,
    exit_code: Option<i32>,
    state: TaskObservedState,
) {
    assert_eq!(result.stdout, stdout);
    assert_eq!(result.stderr, stderr);
    assert_eq!(result.exit_code, exit_code);
    assert_eq!(result.state, state);
}

fn task_resource(task_type: TaskType) -> TaskResource {
    TaskResource {
        task_id: format!("task-{}", unique_suffix()),
        job_id: "job-001".to_string(),
        node_id: "node-001".to_string(),
        agent_id: "agent-001".to_string(),
        task_type,
        script_content: None,
        command_line: None,
        interpreter: None,
        args: Vec::new(),
        env: BTreeMap::new(),
        working_dir: None,
        timeout_secs: None,
        desired_state: TaskDesiredState::Queued,
        observed_state: TaskObservedState::Acknowledged,
        stdout: None,
        stderr: None,
        exit_code: None,
        started_at: None,
        finished_at: None,
        error_message: None,
        claimed_at: None,
        updated_at: None,
    }
}

fn unique_temp_dir(prefix: &str) -> PathBuf {
    std::env::temp_dir().join(format!("{prefix}-{}", unique_suffix()))
}

fn unique_suffix() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos()
}
