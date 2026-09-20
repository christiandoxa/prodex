use anyhow::{Context, Result, bail};
use prodex_cli::{PingCommands, PingOpenaiArgs};
use serde_json::Value;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::process::Output;
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::{Duration, Instant};
use terminal_ui::print_stdout_line;

use super::{AppPaths, AppState, AppStateIoExt, ProfileProvider};

#[path = "ping_output.rs"]
mod ping_output;
#[path = "ping_process.rs"]
mod ping_process;
#[path = "ping_workers.rs"]
mod ping_workers;
use ping_output::{render_ping_result, render_ping_summary};
use ping_process::{classify_ping_process_error, ping_output_failure_detail, run_ping_command};
use ping_workers::{collect_ping_results, probe_ping_worker};

const PING_PROMPT: &str = "hello";
const PING_TIMEOUT: Duration = Duration::from_secs(45);
const PING_OUTPUT_MAX_BYTES: usize = 1024 * 1024;
const PING_MAX_CONCURRENCY: usize = 4;

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
    SpawnFailed,
    Cancelled,
    UnexpectedResponse,
}

impl PingStatus {
    fn is_failure(self) -> bool {
        self != Self::Pass
    }

    fn label(self) -> &'static str {
        match self {
            Self::Pass => "OK",
            Self::AuthFailed => "AUTH_FAILED",
            Self::DnsFailed => "DNS_FAILED",
            Self::TlsFailed => "TLS_FAILED",
            Self::Timeout => "TIMEOUT",
            Self::RateLimited => "RATE_LIMITED",
            Self::QuotaExhausted => "EXHAUSTED",
            Self::UpstreamOverloaded => "UPSTREAM_OVERLOADED",
            Self::ModelUnavailable => "MODEL_UNAVAILABLE",
            Self::ProtocolFailed => "PROTOCOL_FAILED",
            Self::TurnFailed => "TURN_FAILED",
            Self::ProcessFailed => "PROCESS_FAILED",
            Self::SpawnFailed => "SPAWN_FAILED",
            Self::Cancelled => "CANCELLED",
            Self::UnexpectedResponse => "UNEXPECTED_RESPONSE",
        }
    }

    fn json_label(self) -> &'static str {
        match self {
            Self::Pass => "ok",
            Self::AuthFailed => "auth_failed",
            Self::DnsFailed => "dns_failed",
            Self::TlsFailed => "tls_failed",
            Self::Timeout => "timeout",
            Self::RateLimited => "rate_limited",
            Self::QuotaExhausted => "exhausted",
            Self::UpstreamOverloaded => "upstream_overloaded",
            Self::ModelUnavailable => "model_unavailable",
            Self::ProtocolFailed => "protocol_failed",
            Self::TurnFailed => "turn_failed",
            Self::ProcessFailed => "process_failed",
            Self::SpawnFailed => "spawn_failed",
            Self::Cancelled => "cancelled",
            Self::UnexpectedResponse => "unexpected_response",
        }
    }

    fn is_temporary(self) -> bool {
        matches!(
            self,
            Self::DnsFailed
                | Self::TlsFailed
                | Self::Timeout
                | Self::RateLimited
                | Self::UpstreamOverloaded
        )
    }
}

#[derive(Debug, Clone)]
struct PingResult {
    profile: String,
    model: Option<String>,
    effective_model: Option<String>,
    status: PingStatus,
    detail: String,
    first_response_latency_ms: Option<u128>,
    latency_ms: Option<u128>,
}

#[derive(Clone)]
struct PingProbeOptions {
    model: Option<String>,
    base_url: Option<String>,
    no_proxy: bool,
}

#[derive(Clone)]
struct PingTarget {
    name: String,
    codex_home: PathBuf,
}

#[derive(Debug)]
struct PingValidationFailure {
    status: PingStatus,
    detail: String,
}

pub(crate) fn handle_ping(command: PingCommands) -> Result<()> {
    match command {
        PingCommands::Openai(args) => handle_ping_openai(args),
    }
}

fn handle_ping_openai(args: PingOpenaiArgs) -> Result<()> {
    validate_ping_args(&args)?;
    let paths = AppPaths::discover()?;
    let targets = ping_targets(&paths, args.profile.as_deref())?;
    let started = Instant::now();
    if !args.json {
        print_stdout_line("OpenAI application ping")?;
    }
    let options = PingProbeOptions {
        model: args.model.clone(),
        base_url: args.base_url.clone(),
        no_proxy: args.no_proxy,
    };
    let results = probe_ping_targets(targets, options, args.json)?;
    render_ping_summary(&results, started.elapsed(), args.json)?;
    if results.is_empty() || results.iter().any(|result| result.status.is_failure()) {
        bail!("OpenAI application ping failed: one or more profiles are not healthy");
    }
    Ok(())
}

fn ping_targets(paths: &AppPaths, requested: Option<&str>) -> Result<Vec<PingTarget>> {
    let state = AppState::load(paths)?;
    if let Some(name) = requested {
        let profile = state
            .profiles
            .get(name)
            .with_context(|| format!("OpenAI profile '{name}' is not configured"))?;
        if !matches!(&profile.provider, ProfileProvider::Openai) {
            bail!("profile '{name}' is not configured for OpenAI")
        }
        return Ok(vec![PingTarget {
            name: name.to_string(),
            codex_home: profile.codex_home.clone(),
        }]);
    }

    Ok(state
        .profiles
        .iter()
        .filter(|(_, profile)| matches!(&profile.provider, ProfileProvider::Openai))
        .map(|(name, profile)| PingTarget {
            name: name.clone(),
            codex_home: profile.codex_home.clone(),
        })
        .collect())
}

fn probe_ping_targets(
    targets: Vec<PingTarget>,
    options: PingProbeOptions,
    json: bool,
) -> Result<Vec<PingResult>> {
    if targets.is_empty() {
        return Ok(Vec::new());
    }
    let target_count = targets.len();
    let worker_count = thread::available_parallelism()
        .map(|count| count.get())
        .unwrap_or(2)
        .clamp(1, PING_MAX_CONCURRENCY)
        .min(target_count);
    let queue = Arc::new(Mutex::new(VecDeque::from(targets)));
    let (sender, receiver) = mpsc::sync_channel(worker_count);
    let mut results = Vec::with_capacity(target_count);

    thread::scope(|scope| {
        for _ in 0..worker_count {
            let queue = Arc::clone(&queue);
            let sender = sender.clone();
            let options = options.clone();
            scope.spawn(move || probe_ping_worker(&queue, sender, &options));
        }
        drop(sender);
        results = collect_ping_results(&receiver, target_count, json)?;
        Ok::<_, anyhow::Error>(())
    })?;

    results.sort_by(|left, right| left.profile.cmp(&right.profile));
    Ok(results)
}

fn probe_ping_target(target: PingTarget, options: &PingProbeOptions) -> PingResult {
    let started = Instant::now();
    let (status, detail) = match run_ping_command(&target, options) {
        Ok(output) => {
            let result = ping_result_from_output(
                &output.output,
                options.model.clone(),
                output
                    .first_stdout_match_latency
                    .map(|value| value.as_millis()),
                started,
            );
            return PingResult {
                profile: target.name,
                ..result
            };
        }
        Err(error) => classify_ping_process_error(&error),
    };
    PingResult {
        profile: target.name,
        model: options.model.clone(),
        effective_model: None,
        status,
        detail,
        first_response_latency_ms: None,
        latency_ms: Some(started.elapsed().as_millis()),
    }
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

fn ping_result_from_output(
    output: &Output,
    model: Option<String>,
    first_response_latency_ms: Option<u128>,
    started: Instant,
) -> PingResult {
    let no_stdout = output.stdout.iter().all(u8::is_ascii_whitespace);
    let (status, detail) = match validate_ping_output(output) {
        Ok(()) if output.status.success() => (
            PingStatus::Pass,
            "valid model response received".to_string(),
        ),
        Ok(()) => classify_failure(output),
        Err(failure) if no_stdout => {
            let classified = classify_failure(output);
            if classified.0 == PingStatus::ProcessFailed {
                if output.status.success() {
                    (failure.status, failure.detail)
                } else {
                    classified
                }
            } else {
                classified
            }
        }
        Err(failure) => (failure.status, failure.detail),
    };
    let detail = if status == PingStatus::Pass {
        detail
    } else {
        ping_output_failure_detail(&detail, output)
    };
    PingResult {
        profile: String::new(),
        model,
        effective_model: None,
        status,
        detail,
        first_response_latency_ms,
        latency_ms: Some(started.elapsed().as_millis()),
    }
}

#[cfg(feature = "mojo-core")]
fn ping_status_from_mojo(value: i64) -> PingStatus {
    use prodex_mojo_core::rich::*;
    match value {
        PING_STATUS_OK => PingStatus::Pass,
        PING_STATUS_PROTOCOL_FAILED => PingStatus::ProtocolFailed,
        PING_STATUS_UNEXPECTED_RESPONSE => PingStatus::UnexpectedResponse,
        PING_STATUS_TURN_FAILED => PingStatus::TurnFailed,
        PING_STATUS_AUTH_FAILED => PingStatus::AuthFailed,
        PING_STATUS_DNS_FAILED => PingStatus::DnsFailed,
        PING_STATUS_TLS_FAILED => PingStatus::TlsFailed,
        PING_STATUS_TIMEOUT => PingStatus::Timeout,
        PING_STATUS_RATE_LIMITED => PingStatus::RateLimited,
        PING_STATUS_QUOTA_EXHAUSTED => PingStatus::QuotaExhausted,
        PING_STATUS_UPSTREAM_OVERLOADED => PingStatus::UpstreamOverloaded,
        PING_STATUS_MODEL_UNAVAILABLE => PingStatus::ModelUnavailable,
        PING_STATUS_PROCESS_FAILED => PingStatus::ProcessFailed,
        PING_STATUS_SPAWN_FAILED => PingStatus::SpawnFailed,
        PING_STATUS_CANCELLED => PingStatus::Cancelled,
        _ => PingStatus::ProtocolFailed,
    }
}

#[cfg(feature = "mojo-core")]
fn ping_status_detail(status: PingStatus) -> &'static str {
    match status {
        PingStatus::Pass => "valid model response received",
        PingStatus::AuthFailed => "OpenAI authentication failed",
        PingStatus::DnsFailed => "OpenAI hostname resolution failed",
        PingStatus::TlsFailed => "OpenAI TLS connection failed",
        PingStatus::Timeout => "OpenAI diagnostic timed out",
        PingStatus::RateLimited => "OpenAI temporarily rate limited the profile",
        PingStatus::QuotaExhausted => "OpenAI reported quota exhaustion",
        PingStatus::UpstreamOverloaded => "OpenAI upstream is temporarily unavailable",
        PingStatus::ModelUnavailable => "selected OpenAI model is unavailable",
        PingStatus::ProtocolFailed => "Codex did not complete a structured turn",
        PingStatus::TurnFailed => "Codex turn failed",
        PingStatus::ProcessFailed => "Codex diagnostic process failed",
        PingStatus::SpawnFailed => "OpenAI application ping could not start",
        PingStatus::Cancelled => "OpenAI ping was cancelled",
        PingStatus::UnexpectedResponse => "completed turn did not return a model response",
    }
}

#[cfg(feature = "mojo-core")]
fn validate_ping_output(output: &Output) -> std::result::Result<(), PingValidationFailure> {
    let text = String::from_utf8_lossy(&output.stdout);
    let status = ping_status_from_mojo(
        prodex_mojo_core::rich::ping_validate_jsonl(&text)
            .expect("Mojo ping protocol validator returned invalid output"),
    );
    if status == PingStatus::Pass {
        return Ok(());
    }
    let detail = ping_structured_failure_detail(&text)
        .unwrap_or_else(|| ping_status_detail(status).to_string());
    Err(PingValidationFailure { status, detail })
}

#[cfg(not(feature = "mojo-core"))]
fn validate_ping_output(output: &Output) -> std::result::Result<(), PingValidationFailure> {
    validate_ping_output_rust(output)
}

#[cfg(feature = "mojo-core")]
fn ping_structured_failure_detail(text: &str) -> Option<String> {
    text.lines().find_map(|line| {
        let event: Value = serde_json::from_str(line).ok()?;
        let event_type = event.get("type").and_then(Value::as_str)?;
        matches!(event_type, "turn.failed" | "error").then(|| {
            ping_process::bounded_ping_detail(&ping_event_failure_text(
                &event,
                if event_type == "turn.failed" {
                    "turn failed"
                } else {
                    "Codex error"
                },
            ))
        })
    })
}

#[cfg(any(not(feature = "mojo-core"), test))]
#[derive(Default)]
struct PingValidationState {
    thread_started: bool,
    turn_started: bool,
    turn_completed: bool,
    agent_message_completed: bool,
    final_message: Option<String>,
}

#[cfg(any(not(feature = "mojo-core"), test))]
impl PingValidationState {
    fn finish(self) -> std::result::Result<(), PingValidationFailure> {
        if !self.thread_started || !self.turn_started || !self.turn_completed {
            return Err(PingValidationFailure {
                status: PingStatus::ProtocolFailed,
                detail: "Codex did not complete a structured turn".to_string(),
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
                detail: "completed turn did not return a model response".to_string(),
            });
        }
        Ok(())
    }
}

#[cfg(any(not(feature = "mojo-core"), test))]
fn validate_ping_output_rust(output: &Output) -> std::result::Result<(), PingValidationFailure> {
    let mut state = PingValidationState::default();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let event: Value = serde_json::from_str(line).map_err(|_| PingValidationFailure {
            status: PingStatus::ProtocolFailed,
            detail: "Codex JSONL output was malformed".to_string(),
        })?;
        validate_ping_event_rust(&event, &mut state)?;
    }
    state.finish()
}

#[cfg(any(not(feature = "mojo-core"), test))]
fn validate_ping_event_rust(
    event: &Value,
    state: &mut PingValidationState,
) -> std::result::Result<(), PingValidationFailure> {
    match event.get("type").and_then(Value::as_str) {
        Some("thread.started") => {
            if state.thread_started
                || state.turn_started
                || state.turn_completed
                || event
                    .get("thread_id")
                    .and_then(Value::as_str)
                    .is_none_or(|thread_id| thread_id.trim().is_empty())
            {
                return Err(PingValidationFailure {
                    status: PingStatus::ProtocolFailed,
                    detail: "Codex JSONL output contained an invalid thread start".to_string(),
                });
            }
            state.thread_started = true;
        }
        Some("turn.started") => {
            if !state.thread_started || state.turn_started || state.turn_completed {
                return Err(PingValidationFailure {
                    status: PingStatus::ProtocolFailed,
                    detail: "Codex JSONL output contained an invalid turn start".to_string(),
                });
            }
            state.turn_started = true;
        }
        Some("turn.completed") => {
            if !state.thread_started || !state.turn_started || state.turn_completed {
                return Err(PingValidationFailure {
                    status: PingStatus::ProtocolFailed,
                    detail: "Codex JSONL output contained an invalid turn completion".to_string(),
                });
            }
            state.turn_completed = true;
        }
        Some("turn.failed") => return ping_turn_failure_rust(event),
        Some("error") => return ping_event_failure_rust(event),
        Some(event_type) if is_ping_item_event_rust(event_type) => {
            if !state.thread_started || !state.turn_started || state.turn_completed {
                return Err(PingValidationFailure {
                    status: PingStatus::ProtocolFailed,
                    detail: "Codex JSONL output contained an item outside the active turn"
                        .to_string(),
                });
            }
            return validate_ping_item_rust(event_type, event, state);
        }
        Some(_) | None => {
            return Err(PingValidationFailure {
                status: PingStatus::ProtocolFailed,
                detail: "Codex JSONL output contained an unsupported event".to_string(),
            });
        }
    }
    Ok(())
}

#[cfg(any(not(feature = "mojo-core"), test))]
fn is_ping_item_event_rust(event_type: &str) -> bool {
    matches!(
        event_type,
        "item.started" | "item.updated" | "item.completed"
    )
}

#[cfg(any(not(feature = "mojo-core"), test))]
fn ping_turn_failure_rust(event: &Value) -> std::result::Result<(), PingValidationFailure> {
    let message = ping_event_failure_text(event, "turn failed");
    let status = classify_failure_text_rust(&message).0;
    Err(PingValidationFailure {
        status: if status == PingStatus::ProcessFailed {
            PingStatus::TurnFailed
        } else {
            status
        },
        detail: ping_process::bounded_ping_detail(&message),
    })
}

#[cfg(any(not(feature = "mojo-core"), test))]
fn ping_event_failure_rust(event: &Value) -> std::result::Result<(), PingValidationFailure> {
    let message = ping_event_failure_text(event, "Codex error");
    Err(PingValidationFailure {
        status: classify_failure_text_rust(&message).0,
        detail: ping_process::bounded_ping_detail(&message),
    })
}

fn ping_event_failure_text(event: &Value, fallback: &str) -> String {
    let values = [
        event.get("message").and_then(Value::as_str),
        event.get("code").and_then(Value::as_str),
        event.get("error").and_then(Value::as_str),
        event
            .get("error")
            .and_then(|error| error.get("message"))
            .and_then(Value::as_str),
        event
            .get("error")
            .and_then(|error| error.get("code"))
            .and_then(Value::as_str),
    ];
    let text = values
        .into_iter()
        .flatten()
        .filter(|value| !value.trim().is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    if text.is_empty() {
        fallback.to_string()
    } else {
        text
    }
}

#[cfg(any(not(feature = "mojo-core"), test))]
fn validate_ping_item_rust(
    event_type: &str,
    event: &Value,
    state: &mut PingValidationState,
) -> std::result::Result<(), PingValidationFailure> {
    let item = event.get("item").ok_or(PingValidationFailure {
        status: PingStatus::ProtocolFailed,
        detail: "Codex JSONL item event had no item".to_string(),
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
            detail: "diagnostic turn emitted tool or unsupported activity".to_string(),
        }),
    }
}

fn classify_failure(output: &Output) -> (PingStatus, String) {
    let text = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let (status, detail) = classify_failure_text(&text);
    (status, detail.to_string())
}

#[cfg(feature = "mojo-core")]
fn classify_failure_text(text: &str) -> (PingStatus, &'static str) {
    let status = ping_status_from_mojo(
        prodex_mojo_core::rich::ping_classify_failure(text)
            .expect("Mojo ping failure classifier returned invalid output"),
    );
    (status, ping_status_detail(status))
}

#[cfg(not(feature = "mojo-core"))]
fn classify_failure_text(text: &str) -> (PingStatus, &'static str) {
    classify_failure_text_rust(text)
}

#[cfg(any(not(feature = "mojo-core"), test))]
type PingFailureMatcher = fn(&str) -> bool;
#[cfg(any(not(feature = "mojo-core"), test))]
type PingFailureRule = (PingFailureMatcher, PingStatus, &'static str);

#[cfg(any(not(feature = "mojo-core"), test))]
const PING_FAILURE_RULES: &[PingFailureRule] = &[
    (
        is_overload_failure_rust,
        PingStatus::UpstreamOverloaded,
        "OpenAI upstream is temporarily unavailable",
    ),
    (
        is_quota_failure_rust,
        PingStatus::QuotaExhausted,
        "OpenAI reported quota exhaustion",
    ),
    (
        is_auth_failure_rust,
        PingStatus::AuthFailed,
        "OpenAI authentication failed",
    ),
    (
        is_rate_limit_failure_rust,
        PingStatus::RateLimited,
        "OpenAI temporarily rate limited the profile",
    ),
    (
        is_dns_failure_rust,
        PingStatus::DnsFailed,
        "OpenAI hostname resolution failed",
    ),
    (
        is_tls_failure_rust,
        PingStatus::TlsFailed,
        "OpenAI TLS connection failed",
    ),
    (
        is_model_failure_rust,
        PingStatus::ModelUnavailable,
        "selected OpenAI model is unavailable",
    ),
    (
        is_cancelled_failure_rust,
        PingStatus::Cancelled,
        "OpenAI ping was cancelled",
    ),
    (
        is_timeout_failure_rust,
        PingStatus::Timeout,
        "OpenAI diagnostic timed out",
    ),
];

#[cfg(any(not(feature = "mojo-core"), test))]
fn classify_failure_text_rust(text: &str) -> (PingStatus, &'static str) {
    let lower = text.to_ascii_lowercase();
    if contains_any_rust(
        &lower,
        &[
            "failed to start",
            "could not start",
            "failed to execute",
            "failed to resolve prodex executable",
        ],
    ) {
        return (
            PingStatus::SpawnFailed,
            "OpenAI application ping could not start",
        );
    }
    if contains_any_rust(
        &lower,
        &[
            "structured turn",
            "malformed jsonl",
            "unsupported event",
            "protocol failure",
        ],
    ) {
        return (
            PingStatus::ProtocolFailed,
            "Codex did not complete a structured turn",
        );
    }
    PING_FAILURE_RULES
        .iter()
        .find_map(|(matches, status, detail)| matches(&lower).then_some((*status, *detail)))
        .unwrap_or((PingStatus::ProcessFailed, "Codex diagnostic process failed"))
}

#[cfg(any(not(feature = "mojo-core"), test))]
fn contains_any_rust(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| text.contains(needle))
}

#[cfg(any(not(feature = "mojo-core"), test))]
fn is_quota_failure_rust(text: &str) -> bool {
    contains_any_rust(
        text,
        &[
            "insufficient_quota",
            "usage_limit_reached",
            "quota exceeded",
            "quota_exceeded",
            "quota exhausted",
            "insufficient quota",
        ],
    ) && (!text.contains("429")
        || contains_any_rust(
            text,
            &[
                "insufficient_quota",
                "usage_limit_reached",
                "quota_exceeded",
            ],
        ))
}

#[cfg(any(not(feature = "mojo-core"), test))]
fn is_auth_failure_rust(text: &str) -> bool {
    contains_any_rust(
        text,
        &["401", "unauthorized", "invalid api key", "authentication"],
    )
}

#[cfg(any(not(feature = "mojo-core"), test))]
fn is_rate_limit_failure_rust(text: &str) -> bool {
    contains_any_rust(text, &["429", "rate_limit", "rate limit"])
}

#[cfg(any(not(feature = "mojo-core"), test))]
fn is_overload_failure_rust(text: &str) -> bool {
    contains_any_rust(
        text,
        &["503", "502", "504", "overloaded", "temporarily unavailable"],
    )
}

#[cfg(any(not(feature = "mojo-core"), test))]
fn is_dns_failure_rust(text: &str) -> bool {
    contains_any_rust(
        text,
        &["dns", "resolve", "name or service not known", "getaddrinfo"],
    )
}

#[cfg(any(not(feature = "mojo-core"), test))]
fn is_tls_failure_rust(text: &str) -> bool {
    contains_any_rust(text, &["tls", "certificate", "handshake"])
}

#[cfg(any(not(feature = "mojo-core"), test))]
fn is_model_failure_rust(text: &str) -> bool {
    contains_any_rust(text, &["unsupported model", "model_not_found"])
}

#[cfg(any(not(feature = "mojo-core"), test))]
fn is_cancelled_failure_rust(text: &str) -> bool {
    text.contains("cancel")
}

#[cfg(any(not(feature = "mojo-core"), test))]
fn is_timeout_failure_rust(text: &str) -> bool {
    contains_any_rust(text, &["timeout", "timed out"])
}

#[cfg(test)]
#[path = "ping_tests.rs"]
mod tests;
