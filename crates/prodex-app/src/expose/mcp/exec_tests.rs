use super::{EXEC_MAX_OUTPUT_BYTES, execute_tool};
use serde_json::{Value, json};
use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::thread;
use std::time::Duration;

fn run(arguments: Value, cwd: &Path) -> Value {
    execute_tool(&arguments, &Arc::new(AtomicBool::new(false)), cwd).unwrap()
}

#[cfg(unix)]
fn shell(script: &str) -> (String, Vec<String>) {
    ("sh".to_string(), vec!["-c".to_string(), script.to_string()])
}

#[cfg(windows)]
fn shell(script: &str) -> (String, Vec<String>) {
    (
        "cmd.exe".to_string(),
        vec!["/C".to_string(), script.to_string()],
    )
}

#[test]
fn direct_command_returns_bounded_structured_result() {
    let (program, args) = shell(if cfg!(windows) {
        "echo direct"
    } else {
        "printf direct"
    });
    let result = run(json!({"program": program, "args": args}), Path::new("."));
    assert_eq!(result["status"], "completed");
    assert_eq!(result["success"], true);
    assert_eq!(result["exit_code"], 0);
    assert_eq!(result["arg_count"], 2);
    assert_eq!(
        result["stdout"],
        "direct\n".replace("\n", if cfg!(windows) { "\r\n" } else { "" })
    );
}

#[test]
fn python_execution_is_covered_when_python_is_available() {
    let program = if cfg!(windows) { "python" } else { "python3" };
    if Command::new(program).arg("--version").status().is_err() {
        return;
    }
    let result = run(
        json!({"program": program, "args": ["-c", "print('python-ok')"]}),
        Path::new("."),
    );
    assert_eq!(result["success"], true);
    assert_eq!(result["stdout"], "python-ok\n");
}

#[test]
fn nonzero_exit_is_returned_without_losing_output() {
    let (program, args) = shell(if cfg!(windows) {
        "echo failed & exit /b 7"
    } else {
        "printf failed; printf failed >&2; exit 7"
    });
    let result = run(json!({"program": program, "args": args}), Path::new("."));
    assert_eq!(result["status"], "completed");
    assert_eq!(result["success"], false);
    assert_eq!(result["exit_code"], 7);
    assert!(result["stdout"].as_str().unwrap().contains("failed"));
    assert!(result["stderr"].as_str().unwrap().contains("failed"));
}

#[test]
fn cwd_and_environment_overrides_reach_the_direct_child() {
    let root = std::env::temp_dir().join(format!("prodex-super-exec-cwd-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let (program, args) = shell(if cfg!(windows) {
        "cd & echo %PRODEX_EXEC_TEST%"
    } else {
        "pwd; printf %s \"$PRODEX_EXEC_TEST\""
    });
    let result = run(
        json!({"program": program, "args": args, "cwd": root, "env": {"PRODEX_EXEC_TEST": "env-ok"}}),
        Path::new("."),
    );
    assert_eq!(result["success"], true);
    assert!(result["stdout"].as_str().unwrap().contains("env-ok"));
    assert!(
        result["stdout"]
            .as_str()
            .unwrap()
            .contains(&root.to_string_lossy().to_string())
    );
    fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn stdin_is_written_and_closed() {
    let result = run(
        json!({"program": "sh", "args": ["-c", "cat"], "stdin": "stdin-ok"}),
        Path::new("."),
    );
    assert_eq!(result["success"], true);
    assert_eq!(result["stdout"], "stdin-ok");
}

#[cfg(unix)]
#[test]
fn timeout_kills_the_process_group() {
    let root =
        std::env::temp_dir().join(format!("prodex-super-exec-timeout-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let pid_file = root.join("child.pid");
    let result = run(
        json!({
            "program": "sh",
            "args": ["-c", "sleep 30 & echo $! > \"$1\"; wait", "sh", pid_file],
            "timeout_ms": 50
        }),
        &root,
    );
    assert_eq!(result["status"], "timed_out");
    assert_eq!(result["termination"], "timeout");
    let child_pid = fs::read_to_string(&pid_file)
        .unwrap()
        .trim()
        .parse::<i32>()
        .unwrap();
    for _ in 0..100 {
        if unsafe { libc::kill(child_pid, 0) } != 0 {
            fs::remove_dir_all(root).unwrap();
            return;
        }
        thread::sleep(Duration::from_millis(10));
    }
    panic!("timed-out child process survived process-group cleanup: {child_pid}");
}

#[test]
fn output_is_redacted_and_bounded() {
    let program = if cfg!(windows) { "python" } else { "python3" };
    if Command::new(program).arg("--version").status().is_err() {
        return;
    }
    let result = run(
        json!({"program": program, "args": ["-c", "print('Authorization: Bearer fixture-exec-token ' + 'x' * 300000, end='')"]}),
        Path::new("."),
    );
    assert_eq!(result["success"], true);
    assert_eq!(result["stdout_truncated"], true);
    let stdout = result["stdout"].as_str().unwrap();
    assert!(stdout.len() <= EXEC_MAX_OUTPUT_BYTES);
    assert!(stdout.contains("Bearer <redacted>"));
    assert!(!stdout.contains("fixture-exec-token"));
}

#[test]
fn invalid_requests_and_no_session_dependency_fail_or_succeed_locally() {
    assert!(
        execute_tool(
            &json!({}),
            &Arc::new(AtomicBool::new(false)),
            Path::new(".")
        )
        .is_err()
    );
    let result = run(
        json!({"program": "echo", "args": ["standalone"]}),
        Path::new("."),
    );
    assert_eq!(result["success"], true);
    assert!(result.get("run_id").is_none());
    assert!(result.get("thread_id").is_none());
}
