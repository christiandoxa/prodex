use anyhow::{Context, Result, bail};
use prodex_cli::{PingCommands, PingOpenaiArgs};
use serde_json::Value;
use std::ffi::OsString;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use terminal_ui::print_stdout_line;

use super::command_output_with_timeout;

const PING_PROMPT: &str = "ping";
const PING_TIMEOUT: Duration = Duration::from_secs(45);
const PING_OUTPUT_MAX_BYTES: usize = 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PingStatus {
    Pass,
    AuthFailed,
    DnsFailed,
    TlsFailed,
    Timeout,
    RateLimited,
    QuotaExhausted,
    UpstreamOverloaded,
    ModelUnavailable,
    ProtocolFailed,
    TurnFailed,
    ProcessFailed,
    Cancelled,
    UnexpectedResponse,
}

impl PingStatus {
    fn is_failure(self) -> bool {
        self != Self::Pass
    }
}

#[derive(Debug)]
struct PingResult {
    model: Option<String>,
    status: PingStatus,
    detail: &'static str,
    latency_ms: Option<u128>,
}

#[derive(Debug)]
struct PingValidationFailure {
    status: PingStatus,
    detail: &'static str,
}

pub(crate) fn handle_ping(command: PingCommands) -> Result<()> {
    match command {
        PingCommands::Openai(args) => handle_ping_openai(args),
    }
}

fn handle_ping_openai(args: PingOpenaiArgs) -> Result<()> {
    validate_ping_args(&args)?;
    let started = Instant::now();
    let output = match run_ping_command(&args) {
        Ok(output) => output,
        Err(error) => {
            let (status, detail) = if error.to_string().contains("timed out") {
                (PingStatus::Timeout, "OpenAI application ping timed out")
            } else {
                (
                    PingStatus::ProcessFailed,
                    "OpenAI application ping process failed",
                )
            };
            let result = PingResult {
                model: args.model.clone(),
                status,
                detail,
                latency_ms: Some(started.elapsed().as_millis()),
            };
            render_ping_result(&result, args.json)?;
            bail!("OpenAI application ping failed: {detail}");
        }
    };
    let result = ping_result_from_output(&output, args.model.clone(), started);
    render_ping_result(&result, args.json)?;
    if result.status.is_failure() {
        bail!("OpenAI application ping failed: {}", result.detail);
    }
    Ok(())
}

fn validate_ping_args(args: &PingOpenaiArgs) -> Result<()> {
    if let Some(base_url) = args.base_url.as_deref() {
        crate::validate_credential_free_http_url(base_url, "ping upstream base URL")?;
    }
    for (name, value) in [
        ("--model", args.model.as_deref()),
        ("--profile", args.profile.as_deref()),
    ] {
        if value.is_some_and(|value| value.trim().is_empty() || value.chars().any(char::is_control))
        {
            bail!("{name} must be nonempty and contain no control characters");
        }
    }
    Ok(())
}

fn ping_result_from_output(output: &Output, model: Option<String>, started: Instant) -> PingResult {
    let (status, detail) = match validate_ping_output(output) {
        Ok(()) if output.status.success() => (PingStatus::Pass, "valid model response received"),
        Ok(()) => classify_failure(output),
        Err(failure) => (failure.status, failure.detail),
    };
    PingResult {
        model,
        status,
        detail,
        latency_ms: Some(started.elapsed().as_millis()),
    }
}

fn ping_command_args(args: &PingOpenaiArgs) -> Vec<OsString> {
    let mut command_args = vec![OsString::from("run")];
    if let Some(profile) = args.profile.as_deref() {
        command_args.extend([OsString::from("--profile"), OsString::from(profile)]);
    }
    if let Some(base_url) = args.base_url.as_deref() {
        command_args.extend([OsString::from("--base-url"), OsString::from(base_url)]);
    }
    if args.no_proxy {
        command_args.push(OsString::from("--no-proxy"));
    }
    command_args.extend([
        OsString::from("--sandbox"),
        OsString::from("read-only"),
        OsString::from("--ephemeral"),
        OsString::from("--ignore-user-config"),
        OsString::from("--skip-git-repo-check"),
        OsString::from("-c"),
        OsString::from("model_provider=\"openai\""),
    ]);
    if let Some(model) = args.model.as_deref() {
        command_args.extend([OsString::from("--model"), OsString::from(model)]);
    }
    command_args.extend([
        OsString::from("--json"),
        OsString::from("exec"),
        OsString::from(PING_PROMPT),
    ]);
    command_args
}

fn run_ping_command(args: &PingOpenaiArgs) -> Result<Output> {
    let cwd = create_ping_cwd()?;
    let result = {
        let mut command = Command::new(
            std::env::current_exe().context("failed to resolve Prodex executable for ping")?,
        );
        command.args(ping_command_args(args)).current_dir(&cwd);
        command_output_with_timeout(
            &mut command,
            PING_TIMEOUT,
            PING_OUTPUT_MAX_BYTES,
            "OpenAI application ping",
        )
    };
    let cleanup = fs::remove_dir_all(&cwd);
    match (result, cleanup) {
        (Ok(output), Ok(())) => Ok(output),
        (Err(error), Ok(())) => Err(error),
        (Ok(_), Err(error)) => Err(error).context("failed to clean the diagnostic directory"),
        (Err(error), Err(cleanup_error)) => Err(error).context(format!(
            "failed to clean the diagnostic directory: {cleanup_error}"
        )),
    }
}

fn create_ping_cwd() -> Result<PathBuf> {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    for attempt in 0..16u32 {
        let path = std::env::temp_dir().join(format!(
            "prodex-ping-{}-{stamp}-{attempt}",
            std::process::id()
        ));
        match fs::create_dir(&path) {
            Ok(()) => {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    fs::set_permissions(&path, fs::Permissions::from_mode(0o700))?;
                }
                return Ok(path);
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error).context("failed to create diagnostic directory"),
        }
    }
    bail!("could not allocate a private diagnostic directory")
}

#[derive(Default)]
struct PingValidationState {
    thread_started: bool,
    turn_started: bool,
    turn_completed: bool,
    agent_message_completed: bool,
    final_message: Option<String>,
}

impl PingValidationState {
    fn finish(self) -> std::result::Result<(), PingValidationFailure> {
        if !self.thread_started || !self.turn_started || !self.turn_completed {
            return Err(PingValidationFailure {
                status: PingStatus::ProtocolFailed,
                detail: "Codex did not complete a structured turn",
            });
        }
        if !self.agent_message_completed
            || self
                .final_message
                .as_deref()
                .is_none_or(|message| message.trim().is_empty())
        {
            return Err(PingValidationFailure {
                status: PingStatus::UnexpectedResponse,
                detail: "completed turn did not return a model response",
            });
        }
        Ok(())
    }
}

fn validate_ping_output(output: &Output) -> std::result::Result<(), PingValidationFailure> {
    let mut state = PingValidationState::default();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let event: Value = serde_json::from_str(line).map_err(|_| PingValidationFailure {
            status: PingStatus::ProtocolFailed,
            detail: "Codex JSONL output was malformed",
        })?;
        validate_ping_event(&event, &mut state)?;
    }
    state.finish()
}

fn validate_ping_event(
    event: &Value,
    state: &mut PingValidationState,
) -> std::result::Result<(), PingValidationFailure> {
    match event.get("type").and_then(Value::as_str) {
        Some("thread.started") => state.thread_started = true,
        Some("turn.started") => state.turn_started = true,
        Some("turn.completed") => state.turn_completed = true,
        Some("turn.failed") => return ping_turn_failure(event),
        Some("error") => return ping_event_failure(event),
        Some(event_type) if is_ping_item_event(event_type) => {
            return validate_ping_item(event_type, event, state);
        }
        Some(_) | None => {
            return Err(PingValidationFailure {
                status: PingStatus::ProtocolFailed,
                detail: "Codex JSONL output contained an unsupported event",
            });
        }
    }
    Ok(())
}

fn is_ping_item_event(event_type: &str) -> bool {
    matches!(
        event_type,
        "item.started" | "item.updated" | "item.completed"
    )
}

fn ping_turn_failure(event: &Value) -> std::result::Result<(), PingValidationFailure> {
    let status = classify_failure_text(
        event
            .get("error")
            .and_then(|error| error.get("message"))
            .and_then(Value::as_str)
            .unwrap_or("turn failed"),
    )
    .0;
    Err(PingValidationFailure {
        status: if status == PingStatus::ProcessFailed {
            PingStatus::TurnFailed
        } else {
            status
        },
        detail: "Codex turn failed",
    })
}

fn ping_event_failure(event: &Value) -> std::result::Result<(), PingValidationFailure> {
    Err(PingValidationFailure {
        status: classify_failure_text(
            event
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("Codex error"),
        )
        .0,
        detail: "Codex reported an unrecoverable error",
    })
}

fn validate_ping_item(
    event_type: &str,
    event: &Value,
    state: &mut PingValidationState,
) -> std::result::Result<(), PingValidationFailure> {
    let item = event.get("item").ok_or(PingValidationFailure {
        status: PingStatus::ProtocolFailed,
        detail: "Codex JSONL item event had no item",
    })?;
    match item.get("type").and_then(Value::as_str) {
        Some("agent_message") => {
            if event_type == "item.completed" {
                state.agent_message_completed = true;
                if let Some(text) = item.get("text").and_then(Value::as_str) {
                    state.final_message = Some(text.to_string());
                }
            }
            Ok(())
        }
        Some("reasoning") => Ok(()),
        Some(_) | None => Err(PingValidationFailure {
            status: PingStatus::ProtocolFailed,
            detail: "diagnostic turn emitted tool or unsupported activity",
        }),
    }
}

fn classify_failure(output: &Output) -> (PingStatus, &'static str) {
    let text = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    classify_failure_text(&text)
}

type PingFailureMatcher = fn(&str) -> bool;
type PingFailureRule = (PingFailureMatcher, PingStatus, &'static str);

const PING_FAILURE_RULES: &[PingFailureRule] = &[
    (
        is_quota_failure,
        PingStatus::QuotaExhausted,
        "OpenAI reported quota exhaustion",
    ),
    (
        is_auth_failure,
        PingStatus::AuthFailed,
        "OpenAI authentication failed",
    ),
    (
        is_rate_limit_failure,
        PingStatus::RateLimited,
        "OpenAI temporarily rate limited the profile",
    ),
    (
        is_overload_failure,
        PingStatus::UpstreamOverloaded,
        "OpenAI upstream is temporarily unavailable",
    ),
    (
        is_dns_failure,
        PingStatus::DnsFailed,
        "OpenAI hostname resolution failed",
    ),
    (
        is_tls_failure,
        PingStatus::TlsFailed,
        "OpenAI TLS connection failed",
    ),
    (
        is_model_failure,
        PingStatus::ModelUnavailable,
        "selected OpenAI model is unavailable",
    ),
    (
        is_cancelled_failure,
        PingStatus::Cancelled,
        "OpenAI ping was cancelled",
    ),
    (
        is_timeout_failure,
        PingStatus::Timeout,
        "OpenAI diagnostic timed out",
    ),
];

fn classify_failure_text(text: &str) -> (PingStatus, &'static str) {
    let lower = text.to_ascii_lowercase();
    PING_FAILURE_RULES
        .iter()
        .find_map(|(matches, status, detail)| matches(&lower).then_some((*status, *detail)))
        .unwrap_or((PingStatus::ProcessFailed, "Codex diagnostic process failed"))
}

fn contains_any(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| text.contains(needle))
}

fn is_quota_failure(text: &str) -> bool {
    contains_any(
        text,
        &[
            "insufficient_quota",
            "usage_limit_reached",
            "quota exceeded",
        ],
    )
}

fn is_auth_failure(text: &str) -> bool {
    contains_any(
        text,
        &["401", "unauthorized", "invalid api key", "authentication"],
    )
}

fn is_rate_limit_failure(text: &str) -> bool {
    contains_any(text, &["429", "rate_limit", "rate limit"])
}

fn is_overload_failure(text: &str) -> bool {
    contains_any(
        text,
        &["503", "502", "504", "overloaded", "temporarily unavailable"],
    )
}

fn is_dns_failure(text: &str) -> bool {
    contains_any(
        text,
        &["dns", "resolve", "name or service not known", "getaddrinfo"],
    )
}

fn is_tls_failure(text: &str) -> bool {
    contains_any(text, &["tls", "certificate", "handshake"])
}

fn is_model_failure(text: &str) -> bool {
    contains_any(text, &["unsupported model", "model_not_found"])
}

fn is_cancelled_failure(text: &str) -> bool {
    text.contains("cancel")
}

fn is_timeout_failure(text: &str) -> bool {
    contains_any(text, &["timeout", "timed out"])
}

fn render_ping_result(result: &PingResult, json: bool) -> Result<()> {
    if json {
        let value = serde_json::json!({
            "provider": "openai",
            "status": if result.status == PingStatus::Pass { "ok" } else { "failed" },
            "model": result.model,
            "latency_ms": result.latency_ms,
            "detail": result.detail,
        });
        print_stdout_line(&serde_json::to_string(&value)?)?;
        return Ok(());
    }
    let status = if result.status == PingStatus::Pass {
        "ok"
    } else {
        "failed"
    };
    print_stdout_line(&format!("OpenAI ping: {status}"))?;
    print_stdout_line(&format!(
        "latency: {}ms",
        result.latency_ms.unwrap_or_default()
    ))?;
    print_stdout_line(&format!(
        "model: {}",
        result.model.as_deref().unwrap_or("configured/default")
    ))?;
    print_stdout_line("provider: openai")?;
    if result.status.is_failure() {
        print_stdout_line(&format!("reason: {}", result.detail))?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "ping_tests.rs"]
mod tests;
