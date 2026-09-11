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

    Ok(ExecRequest {
        program: program.to_string(),
        args: parse_args(object.get("args"))?,
        cwd: parse_cwd(object.get("cwd"), default_cwd)?,
        env: parse_env(object.get("env"))?,
        stdin: parse_stdin(object.get("stdin"))?,
        timeout: parse_timeout(object.get("timeout_ms"))?,
    })
}

fn parse_args(value: Option<&Value>) -> std::result::Result<Vec<OsString>, String> {
    let mut args = Vec::new();
    let mut args_bytes: usize = 0;
    let Some(value) = value.filter(|value| !value.is_null()) else {
        return Ok(args);
    };
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
    Ok(args)
}

fn parse_cwd(value: Option<&Value>, default_cwd: &Path) -> std::result::Result<PathBuf, String> {
    Ok(match value {
        None | Some(Value::Null) => default_cwd.to_path_buf(),
        Some(value) => {
            let Some(value) = value.as_str() else {
                return Err("cwd must be a string".to_string());
            };
            validate_path_text(value, "cwd", EXEC_MAX_CWD_BYTES, false)?;
            PathBuf::from(value)
        }
    })
}

fn parse_env(value: Option<&Value>) -> std::result::Result<Vec<(OsString, OsString)>, String> {
    let mut env = Vec::new();
    let Some(value) = value.filter(|value| !value.is_null()) else {
        return Ok(env);
    };
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
    Ok(env)
}

fn parse_stdin(value: Option<&Value>) -> std::result::Result<Vec<u8>, String> {
    Ok(match value {
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
    })
}

fn parse_timeout(value: Option<&Value>) -> std::result::Result<Duration, String> {
    let timeout_ms = match value {
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
    Ok(Duration::from_millis(timeout_ms))
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
#[path = "exec_tests.rs"]
mod tests;
