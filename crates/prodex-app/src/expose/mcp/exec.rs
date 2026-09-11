use serde_json::{Value, json};
use std::ffi::OsString;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

pub(super) const EXEC_DEFAULT_TIMEOUT_MS: u64 = 30_000;
pub(super) const EXEC_MAX_TIMEOUT_MS: u64 = 120_000;
pub(super) const EXEC_MAX_PROGRAM_BYTES: usize = 4 * 1024;
pub(super) const EXEC_MAX_ARGUMENTS: usize = 256;
pub(super) const EXEC_MAX_ARGUMENT_BYTES: usize = 16 * 1024;
pub(super) const EXEC_MAX_ARGUMENTS_BYTES: usize = 256 * 1024;
pub(super) const EXEC_MAX_CWD_BYTES: usize = 4 * 1024;
pub(super) const EXEC_MAX_ENV_ENTRIES: usize = 128;
pub(super) const EXEC_MAX_ENV_KEY_BYTES: usize = 256;
pub(super) const EXEC_MAX_ENV_VALUE_BYTES: usize = 16 * 1024;
pub(super) const EXEC_MAX_STDIN_BYTES: usize = 256 * 1024;
pub(super) const EXEC_MAX_OUTPUT_BYTES: usize = 128 * 1024;

struct ExecRequest {
    program: String,
    args: Vec<OsString>,
    cwd: PathBuf,
    env: Vec<(OsString, OsString)>,
    stdin: Vec<u8>,
    timeout: Duration,
}

struct CapturedOutput {
    bytes: Vec<u8>,
    truncated: bool,
}

pub(super) fn execute_tool(
    arguments: &Value,
    shutdown: &Arc<AtomicBool>,
    default_cwd: &Path,
) -> std::result::Result<Value, String> {
    let request = parse_request(arguments, default_cwd)?;
    run(request, shutdown)
}

fn parse_request(
    arguments: &Value,
    default_cwd: &Path,
) -> std::result::Result<ExecRequest, String> {
    let Some(object) = arguments.as_object() else {
        return Err("tool arguments must be an object".to_string());
    };
    let program = object
        .get("program")
        .and_then(Value::as_str)
        .ok_or_else(|| "program is required".to_string())?;
    validate_path_text(program, "program", EXEC_MAX_PROGRAM_BYTES, false)?;

    let mut args = Vec::new();
    let mut args_bytes: usize = 0;
    if let Some(value) = object.get("args").filter(|value| !value.is_null()) {
        let Some(values) = value.as_array() else {
            return Err("args must be an array".to_string());
        };
        if values.len() > EXEC_MAX_ARGUMENTS {
            return Err(format!(
                "args must contain at most {EXEC_MAX_ARGUMENTS} items"
            ));
        }
        for value in values {
            let Some(value) = value.as_str() else {
                return Err("args must contain only strings".to_string());
            };
            if value.len() > EXEC_MAX_ARGUMENT_BYTES || value.as_bytes().contains(&0) {
                return Err("argument contains NUL or is too large".to_string());
            }
            args_bytes = args_bytes.saturating_add(value.len());
            if args_bytes > EXEC_MAX_ARGUMENTS_BYTES {
                return Err(format!(
                    "total argument bytes must be at most {EXEC_MAX_ARGUMENTS_BYTES}"
                ));
            }
            args.push(OsString::from(value));
        }
    }

    let cwd = match object.get("cwd") {
        None | Some(Value::Null) => default_cwd.to_path_buf(),
        Some(value) => {
            let Some(value) = value.as_str() else {
                return Err("cwd must be a string".to_string());
            };
            validate_path_text(value, "cwd", EXEC_MAX_CWD_BYTES, false)?;
            PathBuf::from(value)
        }
    };

    let mut env = Vec::new();
    if let Some(value) = object.get("env").filter(|value| !value.is_null()) {
        let Some(values) = value.as_object() else {
            return Err("env must be an object".to_string());
        };
        if values.len() > EXEC_MAX_ENV_ENTRIES {
            return Err(format!(
                "env must contain at most {EXEC_MAX_ENV_ENTRIES} entries"
            ));
        }
        for (key, value) in values {
            validate_env_key(key)?;
            let Some(value) = value.as_str() else {
                return Err("env values must be strings".to_string());
            };
            if value.len() > EXEC_MAX_ENV_VALUE_BYTES || value.as_bytes().contains(&0) {
                return Err(format!("env value for {key} is too large or contains NUL"));
            }
            env.push((OsString::from(key), OsString::from(value)));
        }
    }

    let stdin = match object.get("stdin") {
        None | Some(Value::Null) => Vec::new(),
        Some(value) => {
            let Some(value) = value.as_str() else {
                return Err("stdin must be a string".to_string());
            };
            if value.len() > EXEC_MAX_STDIN_BYTES {
                return Err(format!(
                    "stdin must be at most {EXEC_MAX_STDIN_BYTES} bytes"
                ));
            }
            value.as_bytes().to_vec()
        }
    };

    let timeout_ms = match object.get("timeout_ms") {
        None | Some(Value::Null) => EXEC_DEFAULT_TIMEOUT_MS,
        Some(value) => value
            .as_u64()
            .ok_or_else(|| "timeout_ms must be a nonnegative integer".to_string())?,
    };
    if !(1..=EXEC_MAX_TIMEOUT_MS).contains(&timeout_ms) {
        return Err(format!(
            "timeout_ms must be between 1 and {EXEC_MAX_TIMEOUT_MS}"
        ));
    }

    Ok(ExecRequest {
        program: program.to_string(),
        args,
        cwd,
        env,
        stdin,
        timeout: Duration::from_millis(timeout_ms),
    })
}

fn validate_path_text(
    value: &str,
    name: &str,
    max_bytes: usize,
    allow_empty: bool,
) -> std::result::Result<(), String> {
    if (!allow_empty && value.is_empty())
        || value.len() > max_bytes
        || value.as_bytes().contains(&0)
        || value.chars().any(char::is_control)
    {
        return Err(format!(
            "{name} is empty, contains control characters, or is too large"
        ));
    }
    Ok(())
}

fn validate_env_key(key: &str) -> std::result::Result<(), String> {
    if key.is_empty()
        || key.len() > EXEC_MAX_ENV_KEY_BYTES
        || key.contains('=')
        || key.as_bytes().contains(&0)
        || key.chars().any(char::is_control)
    {
        return Err(
            "env key is empty, contains '=', control characters, or is too large".to_string(),
        );
    }
    Ok(())
}

fn run(request: ExecRequest, shutdown: &Arc<AtomicBool>) -> std::result::Result<Value, String> {
    let mut command = Command::new(&request.program);
    command
        .args(&request.args)
        .current_dir(&request.cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env_remove("CONTROL_PLANE_API_KEY");
    for (key, value) in &request.env {
        command.env(key, value);
    }
    command.env_remove("CONTROL_PLANE_API_KEY");
    crate::configure_child_process_group(&mut command, true);
    crate::configure_child_parent_death(&mut command);

    let started = Instant::now();
    let mut child = command.spawn().map_err(|error| {
        format!(
            "failed to start executable: {}",
            crate::redaction_redact_secret_like_text(&error.to_string())
        )
    })?;
    let pid = child.id();
    let Some(stdin) = child.stdin.take() else {
        kill_and_reap(&mut child);
        return Err("failed to capture executable stdin".to_string());
    };
    let Some(stdout) = child.stdout.take() else {
        kill_and_reap(&mut child);
        return Err("failed to capture executable stdout".to_string());
    };
    let Some(stderr) = child.stderr.take() else {
        kill_and_reap(&mut child);
        return Err("failed to capture executable stderr".to_string());
    };

    let input = request.stdin;
    let stdin_writer = thread::spawn(move || {
        let mut stdin = stdin;
        let _ = stdin.write_all(&input);
        let _ = stdin.flush();
    });
    let stdout_reader = thread::spawn(move || read_output(stdout));
    let stderr_reader = thread::spawn(move || read_output(stderr));

    let mut termination = None;
    let status = loop {
        if shutdown.load(Ordering::SeqCst) {
            termination = Some("cancelled");
            break terminate_and_wait(&mut child)?;
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if started.elapsed() < request.timeout => {
                thread::sleep(Duration::from_millis(10));
            }
            Ok(None) => {
                termination = Some("timeout");
                break terminate_and_wait(&mut child)?;
            }
            Err(error) => {
                kill_and_reap(&mut child);
                return Err(format!(
                    "failed to poll executable: {}",
                    crate::redaction_redact_secret_like_text(&error.to_string())
                ));
            }
        }
    };

    join_thread(stdin_writer, &mut child, "exec stdin writer")?;
    let stdout = join_reader(stdout_reader, &mut child, "exec stdout reader")?;
    let stderr = join_reader(stderr_reader, &mut child, "exec stderr reader")?;
    let (stdout, stdout_clipped) = render_output(stdout);
    let (stderr, stderr_clipped) = render_output(stderr);
    let signal = exit_signal(&status);
    let result_status = match termination {
        Some("timeout") => "timed_out",
        Some(value) => value,
        None => "completed",
    };
    Ok(json!({
        "status": result_status,
        "program": request.program,
        "arg_count": request.args.len(),
        "cwd": request.cwd.to_string_lossy(),
        "pid": pid,
        "exit_code": status.code(),
        "exit_status": crate::child_exit_code(&status),
        "signal": signal,
        "termination": termination,
        "success": termination.is_none() && status.success(),
        "duration_ms": started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
        "stdout": stdout,
        "stdout_truncated": stdout_clipped,
        "stderr": stderr,
        "stderr_truncated": stderr_clipped,
    }))
}

fn read_output(mut reader: impl Read) -> io::Result<CapturedOutput> {
    let mut bytes = Vec::with_capacity(EXEC_MAX_OUTPUT_BYTES.min(8192));
    let mut truncated = false;
    let mut buffer = [0_u8; 8192];
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        if bytes.len() < EXEC_MAX_OUTPUT_BYTES {
            let remaining = EXEC_MAX_OUTPUT_BYTES - bytes.len();
            let kept = read.min(remaining);
            bytes.extend_from_slice(&buffer[..kept]);
            truncated |= kept < read;
        } else {
            truncated = true;
        }
    }
    Ok(CapturedOutput { bytes, truncated })
}

fn render_output(output: CapturedOutput) -> (String, bool) {
    let redacted =
        crate::redaction_redact_secret_like_text(&String::from_utf8_lossy(&output.bytes));
    let text = bounded_text(&redacted, EXEC_MAX_OUTPUT_BYTES);
    (
        text,
        output.truncated || redacted.len() > EXEC_MAX_OUTPUT_BYTES,
    )
}

fn bounded_text(value: &str, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return value.to_string();
    }
    let end = value
        .char_indices()
        .take_while(|(index, _)| *index <= max_bytes)
        .map(|(index, _)| index)
        .last()
        .unwrap_or(0)
        .min(max_bytes);
    value[..end].to_string()
}

fn join_reader(
    reader: JoinHandle<io::Result<CapturedOutput>>,
    child: &mut Child,
    label: &str,
) -> std::result::Result<CapturedOutput, String> {
    join_thread(reader, child, label)?.map_err(|error| {
        format!(
            "{label} failed: {}",
            crate::redaction_redact_secret_like_text(&error.to_string())
        )
    })
}

fn join_thread<T>(
    handle: JoinHandle<T>,
    child: &mut Child,
    label: &str,
) -> std::result::Result<T, String> {
    let deadline = Instant::now() + Duration::from_secs(2);
    while !handle.is_finished() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(10));
    }
    if !handle.is_finished() {
        kill_and_reap(child);
        let deadline = Instant::now() + Duration::from_secs(2);
        while !handle.is_finished() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
    }
    if !handle.is_finished() {
        return Err(format!("{label} did not stop"));
    }
    handle
        .join()
        .map_err(|_| format!("{label} thread panicked"))
}

fn terminate_and_wait(child: &mut Child) -> std::result::Result<ExitStatus, String> {
    let _ = crate::terminate_child_process_tree(child, true);
    child.wait().map_err(|error| {
        format!(
            "failed to reap terminated executable: {}",
            crate::redaction_redact_secret_like_text(&error.to_string())
        )
    })
}

fn kill_and_reap(child: &mut Child) {
    let _ = crate::terminate_child_process_tree(child, true);
    let _ = child.wait();
}

#[cfg(unix)]
fn exit_signal(status: &ExitStatus) -> Option<i32> {
    use std::os::unix::process::ExitStatusExt;

    status.signal()
}

#[cfg(not(unix))]
fn exit_signal(_status: &ExitStatus) -> Option<i32> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;
    use std::process::Command;
    use std::sync::atomic::AtomicBool;

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
        let root =
            std::env::temp_dir().join(format!("prodex-super-exec-cwd-{}", std::process::id()));
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
}
