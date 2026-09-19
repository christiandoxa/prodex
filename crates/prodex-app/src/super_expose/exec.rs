use super::{bounded_redacted_text, logging::ExposeAuditLog};
use crate::{configure_child_process_group, terminate_child_process_tree};
use serde_json::{Value, json};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const OUTPUT_MAX_BYTES: usize = 256 * 1024;
const TIMEOUT_MAX_MS: u64 = 120_000;

pub(super) fn execute_direct(
    arguments: &Value,
    workspace: &Path,
    audit: &ExposeAuditLog,
) -> std::result::Result<Value, String> {
    let program = required_string(arguments, "program", 4096)?;
    let args = parse_args(arguments)?;
    let cwd = optional_string(arguments, "cwd")?
        .map(PathBuf::from)
        .unwrap_or_else(|| workspace.to_path_buf());
    let timeout_ms = arguments
        .get("timeout_ms")
        .filter(|value| !value.is_null())
        .map(|value| {
            value
                .as_u64()
                .filter(|value| (1..=TIMEOUT_MAX_MS).contains(value))
                .ok_or_else(|| "timeout_ms is outside the supported range".to_string())
        })
        .transpose()?
        .unwrap_or(30_000);
    let stdin = optional_string(arguments, "stdin")?
        .unwrap_or("")
        .as_bytes()
        .to_vec();
    let env_count = arguments
        .get("env")
        .and_then(Value::as_object)
        .map_or(0, serde_json::Map::len);
    let cwd_kind = if arguments.get("cwd").is_some_and(|value| !value.is_null()) {
        "custom"
    } else {
        "workspace"
    };
    let program_label = Path::new(&program)
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("program")
        .to_string();
    audit.event(
        "super_expose_exec_started",
        [
            crate::runtime_proxy_log_field("program", program_label.clone()),
            crate::runtime_proxy_log_field("arg_count", args.len().to_string()),
            crate::runtime_proxy_log_field("cwd", cwd_kind),
            crate::runtime_proxy_log_field("env_count", env_count.to_string()),
            crate::runtime_proxy_log_field("stdin_bytes", stdin.len().to_string()),
            crate::runtime_proxy_log_field("timeout_ms", timeout_ms.to_string()),
        ],
    );

    let mut command = Command::new(&program);
    command
        .args(&args)
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    apply_env(arguments, &mut command)?;
    configure_child_process_group(&mut command, true);

    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            audit.event(
                "super_expose_exec_completed",
                [
                    crate::runtime_proxy_log_field("program", program_label),
                    crate::runtime_proxy_log_field("success", "false"),
                    crate::runtime_proxy_log_field("spawn_failed", "true"),
                    crate::runtime_proxy_log_field("timed_out", "false"),
                ],
            );
            return Err(format!("spawn failed: {error}"));
        }
    };
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    if let Some(mut child_stdin) = child.stdin.take() {
        let _ = child_stdin.write_all(&stdin);
    }
    let stdout_reader = stdout.map(spawn_reader);
    let stderr_reader = stderr.map(spawn_reader);
    let deadline = Instant::now() + Duration::from_millis(timeout_ms);
    let (status, timed_out) = loop {
        match child.try_wait() {
            Ok(Some(status)) => break (Some(status), false),
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(20)),
            Ok(None) => {
                let _ = terminate_child_process_tree(&mut child, true);
                break (child.wait().ok(), true);
            }
            Err(_) => break (None, false),
        }
    };
    let stdout = stdout_reader
        .and_then(|reader| reader.join().ok())
        .unwrap_or_default();
    let stderr = stderr_reader
        .and_then(|reader| reader.join().ok())
        .unwrap_or_default();
    let success = status.is_some_and(|status| status.success()) && !timed_out;
    audit.event(
        "super_expose_exec_completed",
        [
            crate::runtime_proxy_log_field("program", program_label),
            crate::runtime_proxy_log_field("success", success.to_string()),
            crate::runtime_proxy_log_field("spawn_failed", "false"),
            crate::runtime_proxy_log_field("timed_out", timed_out.to_string()),
            crate::runtime_proxy_log_field(
                "exit_code",
                status
                    .as_ref()
                    .and_then(std::process::ExitStatus::code)
                    .map_or_else(|| "none".to_string(), |code| code.to_string()),
            ),
            crate::runtime_proxy_log_field("stdout_bytes", stdout.len().to_string()),
            crate::runtime_proxy_log_field("stderr_bytes", stderr.len().to_string()),
        ],
    );
    Ok(json!({
        "program": program,
        "arg_count": args.len(),
        "exit_code": status.as_ref().and_then(std::process::ExitStatus::code),
        "success": success,
        "timed_out": timed_out,
        "stdout": bounded_redacted_text(&stdout, OUTPUT_MAX_BYTES),
        "stderr": bounded_redacted_text(&stderr, OUTPUT_MAX_BYTES)
    }))
}

fn parse_args(arguments: &Value) -> std::result::Result<Vec<String>, String> {
    let Some(value) = arguments.get("args").filter(|value| !value.is_null()) else {
        return Ok(Vec::new());
    };
    let values = value
        .as_array()
        .ok_or_else(|| "args must be an array".to_string())?;
    if values.len() > 256 {
        return Err("too many command arguments".to_string());
    }
    values
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::to_string)
                .ok_or_else(|| "args entries must be strings".to_string())
        })
        .collect()
}

fn apply_env(arguments: &Value, command: &mut Command) -> std::result::Result<(), String> {
    let Some(value) = arguments.get("env").filter(|value| !value.is_null()) else {
        return Ok(());
    };
    let env = value
        .as_object()
        .ok_or_else(|| "env must be an object".to_string())?;
    if env.len() > 256 {
        return Err("too many environment entries".to_string());
    }
    for (name, value) in env {
        let value = value
            .as_str()
            .ok_or_else(|| "environment values must be strings".to_string())?;
        command.env(name, value);
    }
    Ok(())
}

fn spawn_reader(reader: impl Read + Send + 'static) -> thread::JoinHandle<Vec<u8>> {
    thread::spawn(move || {
        let mut output = Vec::new();
        let _ = reader
            .take(OUTPUT_MAX_BYTES as u64 + 1)
            .read_to_end(&mut output);
        output
    })
}

fn required_string(
    arguments: &Value,
    name: &str,
    max_bytes: usize,
) -> std::result::Result<String, String> {
    arguments
        .get(name)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty() && value.len() <= max_bytes)
        .map(str::to_string)
        .ok_or_else(|| format!("{name} is required or too large"))
}

fn optional_string<'a>(
    arguments: &'a Value,
    name: &str,
) -> std::result::Result<Option<&'a str>, String> {
    let Some(value) = arguments.get(name) else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    value
        .as_str()
        .map(Some)
        .ok_or_else(|| format!("{name} must be a string"))
}
