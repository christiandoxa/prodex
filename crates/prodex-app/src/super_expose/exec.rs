use super::logging::ExposeAuditLog;
use super::optional_tools::ExposeOptionalTools;
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
pub(super) const EXEC_MAX_ARGUMENTS: usize = 256;
pub(super) const EXEC_MAX_ARGUMENTS_BYTES: usize = 256 * 1024;
pub(super) const EXEC_MAX_CWD_BYTES: usize = 4 * 1024;
pub(super) const EXEC_MAX_ENV_ENTRIES: usize = 128;
pub(super) const EXEC_MAX_STDIN_BYTES: usize = 256 * 1024;
pub(super) const EXEC_MAX_OUTPUT_BYTES: usize = 128 * 1024;
const EXEC_LOG_COMMAND_MAX_BYTES: usize = 2 * 1024;

struct ExecRequest {
    requested_program: String,
    program: PathBuf,
    args: Vec<OsString>,
    cwd: PathBuf,
    env: Vec<(OsString, OsString)>,
    stdin: Vec<u8>,
    timeout: Duration,
    optional_tool: Option<prodex_optional_tools::OptionalToolId>,
}

struct CapturedOutput {
    bytes: Vec<u8>,
    truncated: bool,
}

pub(super) fn execute_direct(
    arguments: &Value,
    default_cwd: &Path,
    audit: &ExposeAuditLog,
    optional_tools: &ExposeOptionalTools,
) -> std::result::Result<Value, String> {
    let request = match parse_request(arguments, default_cwd, optional_tools) {
        Ok(request) => request,
        Err(error) => {
            audit.event(
                "super_expose_exec_rejected",
                [
                    crate::runtime_proxy_log_field(
                        "program",
                        safe_program_label(arguments.get("program").and_then(Value::as_str)),
                    ),
                    crate::runtime_proxy_log_field(
                        "reason",
                        crate::redaction_redact_secret_like_text(&error),
                    ),
                ],
            );
            return Err(error);
        }
    };
    let program = safe_program_label(Some(&request.requested_program));
    let command = exec_log_command_preview(&request);
    let cwd = bounded_text(
        &crate::redaction_redact_secret_like_text(&request.cwd.to_string_lossy()),
        EXEC_MAX_CWD_BYTES,
    );
    audit.event(
        "super_expose_exec_started",
        [
            crate::runtime_proxy_log_field("program", program.clone()),
            crate::runtime_proxy_log_field("command", command.clone()),
            crate::runtime_proxy_log_field("cwd", cwd),
            crate::runtime_proxy_log_field("arg_count", request.args.len().to_string()),
            crate::runtime_proxy_log_field("env_count", request.env.len().to_string()),
            crate::runtime_proxy_log_field("stdin_bytes", request.stdin.len().to_string()),
            crate::runtime_proxy_log_field("timeout_ms", request.timeout.as_millis().to_string()),
        ],
    );
    let shutdown = Arc::new(AtomicBool::new(false));
    let result = run(request, &shutdown, optional_tools);
    let fields = match result.as_ref() {
        Ok(value) => vec![
            crate::runtime_proxy_log_field("program", program),
            crate::runtime_proxy_log_field("command", command),
            crate::runtime_proxy_log_field(
                "success",
                value
                    .get("success")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
                    .to_string(),
            ),
            crate::runtime_proxy_log_field(
                "status",
                value
                    .get("status")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown"),
            ),
            crate::runtime_proxy_log_field(
                "exit_status",
                value
                    .get("exit_status")
                    .map(Value::to_string)
                    .unwrap_or_else(|| "null".to_string()),
            ),
            crate::runtime_proxy_log_field(
                "duration_ms",
                value
                    .get("duration_ms")
                    .and_then(Value::as_u64)
                    .unwrap_or_default()
                    .to_string(),
            ),
            crate::runtime_proxy_log_field(
                "stdout_truncated",
                value
                    .get("stdout_truncated")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
                    .to_string(),
            ),
            crate::runtime_proxy_log_field(
                "stderr_truncated",
                value
                    .get("stderr_truncated")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
                    .to_string(),
            ),
        ],
        Err(error) => vec![
            crate::runtime_proxy_log_field("program", program),
            crate::runtime_proxy_log_field("command", command),
            crate::runtime_proxy_log_field("success", "false"),
            crate::runtime_proxy_log_field(
                "reason",
                crate::redaction_redact_secret_like_text(error),
            ),
        ],
    };
    audit.event("super_expose_exec_completed", fields);
    result
}

fn safe_program_label(program: Option<&str>) -> String {
    program
        .and_then(|program| {
            Path::new(program)
                .file_name()
                .and_then(|name| name.to_str())
        })
        .unwrap_or("program")
        .to_string()
}

fn exec_log_command_preview(request: &ExecRequest) -> String {
    let mut rendered = Vec::with_capacity(request.args.len().saturating_add(1));
    rendered.push(exec_log_argument(&request.requested_program, false));
    let mut redact_next = false;
    for argument in &request.args {
        let raw = argument.to_string_lossy();
        if redact_next {
            rendered.push("<redacted>".to_string());
            redact_next = false;
            continue;
        }
        if exec_log_secret_flag(&raw) {
            if raw.contains('=') {
                let key = raw.split_once('=').map_or(raw.as_ref(), |(key, _)| key);
                rendered.push(format!("{key}=<redacted>"));
            } else {
                rendered.push(raw.to_string());
                redact_next = true;
            }
            continue;
        }
        rendered.push(exec_log_argument(&raw, true));
    }
    bounded_text(
        &crate::redaction_redact_secret_like_text(&rendered.join(" ")),
        EXEC_LOG_COMMAND_MAX_BYTES,
    )
}

fn exec_log_secret_flag(value: &str) -> bool {
    prodex_mojo_core::redaction::redaction_cli_flag_sensitive(value)
        .expect("compiled Mojo expose command flag policy returned invalid output")
}

fn exec_log_argument(value: &str, quote: bool) -> String {
    let redacted = crate::redaction_redact_secret_like_text(value);
    if quote && redacted.chars().any(char::is_whitespace) {
        serde_json::to_string(&redacted).unwrap_or_else(|_| "\"<redacted>\"".to_string())
    } else {
        redacted
    }
}

#[cfg(test)]
pub(super) fn execute_tool(
    arguments: &Value,
    shutdown: &Arc<AtomicBool>,
    default_cwd: &Path,
) -> std::result::Result<Value, String> {
    let optional_tools = ExposeOptionalTools::empty_for_tests();
    let request = parse_request(arguments, default_cwd, &optional_tools)?;
    run(request, shutdown, &optional_tools)
}

#[cfg(test)]
pub(super) fn execute_tool_with_optional_tools(
    arguments: &Value,
    shutdown: &Arc<AtomicBool>,
    default_cwd: &Path,
    optional_tools: &ExposeOptionalTools,
) -> std::result::Result<Value, String> {
    let request = parse_request(arguments, default_cwd, optional_tools)?;
    run(request, shutdown, optional_tools)
}

fn parse_request(
    arguments: &Value,
    default_cwd: &Path,
    optional_tools: &ExposeOptionalTools,
) -> std::result::Result<ExecRequest, String> {
    let Some(object) = arguments.as_object() else {
        return Err("tool arguments must be an object".to_string());
    };
    let program = object
        .get("program")
        .and_then(Value::as_str)
        .ok_or_else(|| "program is required".to_string())?;
    ensure_exec_validation(
        prodex_mojo_core::rich::super_expose_exec_validate_text(
            program,
            prodex_mojo_core::rich::SuperExposeExecTextKind::Program,
        ),
        "program is empty, contains control characters, or is too large",
    )?;
    let resolved = optional_tools.resolve_exec(program, parse_args(object.get("args"))?)?;

    Ok(ExecRequest {
        requested_program: program.to_string(),
        program: resolved.program,
        args: resolved.args,
        cwd: parse_cwd(object.get("cwd"), default_cwd)?,
        env: parse_env(object.get("env"))?,
        stdin: parse_stdin(object.get("stdin"))?,
        timeout: parse_timeout(object.get("timeout_ms"))?,
        optional_tool: resolved.optional_tool,
    })
}

fn parse_args(value: Option<&Value>) -> std::result::Result<Vec<OsString>, String> {
    let Some(value) = value.filter(|value| !value.is_null()) else {
        return Ok(Vec::new());
    };
    let Some(values) = value.as_array() else {
        return Err("args must be an array".to_string());
    };
    ensure_exec_validation(
        prodex_mojo_core::rich::super_expose_exec_validate_count(
            values.len(),
            prodex_mojo_core::rich::SuperExposeExecCountKind::Arguments,
        ),
        format!("args must contain at most {EXEC_MAX_ARGUMENTS} items"),
    )?;
    let mut args = Vec::with_capacity(values.len());
    let mut args_bytes = 0;
    for value in values {
        let Some(value) = value.as_str() else {
            return Err("args must contain only strings".to_string());
        };
        let (validation, next_bytes) =
            prodex_mojo_core::rich::super_expose_exec_validate_argument(value, args_bytes)
                .map_err(exec_validation_error)?;
        match validation {
            prodex_mojo_core::rich::SuperExposeExecValidation::Valid => {
                args_bytes = next_bytes;
            }
            prodex_mojo_core::rich::SuperExposeExecValidation::ArgumentLimit => {
                return Err("argument contains NUL or is too large".to_string());
            }
            prodex_mojo_core::rich::SuperExposeExecValidation::TotalArgumentLimit => {
                return Err(format!(
                    "total argument bytes must be at most {EXEC_MAX_ARGUMENTS_BYTES}"
                ));
            }
            _ => return Err(exec_validation_error_code(validation)),
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
            ensure_exec_validation(
                prodex_mojo_core::rich::super_expose_exec_validate_text(
                    value,
                    prodex_mojo_core::rich::SuperExposeExecTextKind::Cwd,
                ),
                "cwd is empty, contains control characters, or is too large",
            )?;
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
    ensure_exec_validation(
        prodex_mojo_core::rich::super_expose_exec_validate_count(
            values.len(),
            prodex_mojo_core::rich::SuperExposeExecCountKind::Environment,
        ),
        format!("env must contain at most {EXEC_MAX_ENV_ENTRIES} entries"),
    )?;
    for (key, value) in values {
        let key_validation = prodex_mojo_core::rich::super_expose_exec_validate_env_entry(key, "")
            .map_err(exec_validation_error)?;
        if !matches!(
            key_validation,
            prodex_mojo_core::rich::SuperExposeExecValidation::Valid
        ) {
            return Err(
                "env key is empty, contains '=', control characters, or is too large".to_string(),
            );
        }
        let Some(value) = value.as_str() else {
            return Err("env values must be strings".to_string());
        };
        let validation = prodex_mojo_core::rich::super_expose_exec_validate_env_entry(key, value)
            .map_err(exec_validation_error)?;
        match validation {
            prodex_mojo_core::rich::SuperExposeExecValidation::Valid => {}
            prodex_mojo_core::rich::SuperExposeExecValidation::EnvironmentKey => {
                return Err(
                    "env key is empty, contains '=', control characters, or is too large"
                        .to_string(),
                );
            }
            prodex_mojo_core::rich::SuperExposeExecValidation::EnvironmentValue => {
                return Err(format!("env value for {key} is too large or contains NUL"));
            }
            _ => return Err(exec_validation_error_code(validation)),
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
            ensure_exec_validation(
                prodex_mojo_core::rich::super_expose_exec_validate_stdin(value.len()),
                format!("stdin must be at most {EXEC_MAX_STDIN_BYTES} bytes"),
            )?;
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
    ensure_exec_validation(
        prodex_mojo_core::rich::super_expose_exec_validate_timeout(timeout_ms),
        format!("timeout_ms must be between 1 and {EXEC_MAX_TIMEOUT_MS}"),
    )?;
    Ok(Duration::from_millis(timeout_ms))
}

fn ensure_exec_validation(
    validation: Result<
        prodex_mojo_core::rich::SuperExposeExecValidation,
        prodex_mojo_core::MojoError,
    >,
    invalid_message: impl Into<String>,
) -> std::result::Result<(), String> {
    match validation.map_err(exec_validation_error)? {
        prodex_mojo_core::rich::SuperExposeExecValidation::Valid => Ok(()),
        _ => Err(invalid_message.into()),
    }
}

fn exec_validation_error(error: prodex_mojo_core::MojoError) -> String {
    format!("exec request validation failed: {error:?}")
}

fn exec_validation_error_code(
    validation: prodex_mojo_core::rich::SuperExposeExecValidation,
) -> String {
    format!("exec request validation returned unexpected result: {validation:?}")
}

fn run(
    request: ExecRequest,
    shutdown: &Arc<AtomicBool>,
    optional_tools: &ExposeOptionalTools,
) -> std::result::Result<Value, String> {
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
    let requested_path = request
        .env
        .iter()
        .find(|(key, _)| key.to_string_lossy().eq_ignore_ascii_case("PATH"))
        .map(|(_, value)| value.as_os_str());
    optional_tools.apply_environment(&mut command, requested_path);
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
        "program": request.requested_program,
        "optional_tool": request.optional_tool.map(|tool| tool.as_str()),
        "available_optional_tools": optional_tools.available_ids(),
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
