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

    fn mojo_code(self) -> i64 {
        match self {
            Self::Pass => prodex_mojo_core::rich::PING_STATUS_OK,
            Self::ProtocolFailed => prodex_mojo_core::rich::PING_STATUS_PROTOCOL_FAILED,
            Self::UnexpectedResponse => prodex_mojo_core::rich::PING_STATUS_UNEXPECTED_RESPONSE,
            Self::TurnFailed => prodex_mojo_core::rich::PING_STATUS_TURN_FAILED,
            Self::AuthFailed => prodex_mojo_core::rich::PING_STATUS_AUTH_FAILED,
            Self::DnsFailed => prodex_mojo_core::rich::PING_STATUS_DNS_FAILED,
            Self::TlsFailed => prodex_mojo_core::rich::PING_STATUS_TLS_FAILED,
            Self::Timeout => prodex_mojo_core::rich::PING_STATUS_TIMEOUT,
            Self::RateLimited => prodex_mojo_core::rich::PING_STATUS_RATE_LIMITED,
            Self::QuotaExhausted => prodex_mojo_core::rich::PING_STATUS_QUOTA_EXHAUSTED,
            Self::UpstreamOverloaded => prodex_mojo_core::rich::PING_STATUS_UPSTREAM_OVERLOADED,
            Self::ModelUnavailable => prodex_mojo_core::rich::PING_STATUS_MODEL_UNAVAILABLE,
            Self::ProcessFailed => prodex_mojo_core::rich::PING_STATUS_PROCESS_FAILED,
            Self::SpawnFailed => prodex_mojo_core::rich::PING_STATUS_SPAWN_FAILED,
            Self::Cancelled => prodex_mojo_core::rich::PING_STATUS_CANCELLED,
        }
    }
}

#[derive(Debug, Clone)]
struct PingResult {
    profile: String,
    model: Option<String>,
    effort: Option<String>,
    effective_model: Option<String>,
    status: PingStatus,
    detail: String,
    first_response_latency_ms: Option<u128>,
    latency_ms: Option<u128>,
}

#[derive(Clone)]
struct PingProbeOptions {
    model: Option<String>,
    effort: Option<String>,
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

fn ping_should_prompt_selection(json: bool, interactive: bool) -> bool {
    !json && interactive
}

fn handle_ping_openai(mut args: PingOpenaiArgs) -> Result<()> {
    validate_ping_args(&args)?;
    if ping_should_prompt_selection(
        args.json,
        super::super_prompt::super_prompt_is_interactive(),
    ) {
        let (model, effort) = super::super_main_prompt::prompt_openai_ping_model_and_effort(
            args.model.as_deref(),
            args.effort.as_deref(),
        )?;
        args.model = model;
        args.effort = effort;
    }
    validate_ping_args(&args)?;
    args.effort = normalize_ping_effort(args.model.as_deref(), args.effort.as_deref())?;
    let paths = AppPaths::discover()?;
    let targets = ping_targets(&paths, args.profile.as_deref())?;
    let started = Instant::now();
    if !args.json {
        print_stdout_line("OpenAI application ping")?;
    }
    let options = PingProbeOptions {
        model: args.model.clone(),
        effort: args.effort.clone(),
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

    Ok(ping_results_in_profile_order(results))
}

/// Parallel probe completion order is nondeterministic. Compiled ping
/// policy alone sorts the original result values by profile name.
fn ping_results_in_profile_order(results: Vec<PingResult>) -> Vec<PingResult> {
    let names = results
        .iter()
        .map(|result| result.profile.as_str())
        .collect::<Vec<_>>();
    let order = prodex_mojo_core::rich::ping_profile_order(&names)
        .expect("compiled Mojo ping result profile order returned invalid output");
    let mut original = results.into_iter().map(Some).collect::<Vec<_>>();
    order
        .into_iter()
        .map(|index| {
            original[index]
                .take()
                .expect("validated Mojo result indices are unique")
        })
        .collect()
}

fn probe_ping_target(target: PingTarget, options: &PingProbeOptions) -> PingResult {
    let started = Instant::now();
    let (status, detail) = match run_ping_command(&target, options) {
        Ok(output) => {
            let result = ping_result_from_output(
                &output.output,
                options.model.clone(),
                options.effort.clone(),
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
        effort: options.effort.clone(),
        effective_model: None,
        status,
        detail,
        first_response_latency_ms: None,
        latency_ms: Some(started.elapsed().as_millis()),
    }
}

fn normalize_ping_effort(model: Option<&str>, effort: Option<&str>) -> Result<Option<String>> {
    let Some(effort) = effort else {
        return Ok(None);
    };
    let resolution = prodex_provider_core::provider_model_reasoning_resolution(
        prodex_provider_core::ProviderId::OpenAi,
        model,
        Some(effort),
    )
    .map_err(anyhow::Error::new)?;
    resolution
        .selected_reasoning_effort
        .and_then(|effort| effort.label())
        .map(str::to_string)
        .ok_or_else(|| anyhow::anyhow!("reasoning effort is unsupported for the selected model"))
        .map(Some)
}

fn validate_ping_args(args: &PingOpenaiArgs) -> Result<()> {
    if let Some(base_url) = args.base_url.as_deref() {
        crate::validate_credential_free_http_url(base_url, "ping upstream base URL")?;
    }
    for (name, value) in [
        ("--model", args.model.as_deref()),
        ("--effort", args.effort.as_deref()),
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
    effort: Option<String>,
    first_response_latency_ms: Option<u128>,
    started: Instant,
) -> PingResult {
    let no_stdout = output.stdout.iter().all(u8::is_ascii_whitespace);
    let (status, detail) = match validate_ping_output(output) {
        Ok(()) if output.status.success() => (
            PingStatus::Pass,
            ping_status_detail(prodex_mojo_core::rich::PING_STATUS_OK),
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
        effort,
        effective_model: None,
        status,
        detail,
        first_response_latency_ms,
        latency_ms: Some(started.elapsed().as_millis()),
    }
}

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

fn ping_status_detail(status: i64) -> String {
    prodex_mojo_core::rich::ping_status_detail(status)
        .expect("Mojo ping status detail formatter returned invalid output")
}

fn validate_ping_output(output: &Output) -> std::result::Result<(), PingValidationFailure> {
    let text = String::from_utf8_lossy(&output.stdout);
    let status_value = prodex_mojo_core::rich::ping_validate_jsonl(&text)
        .expect("Mojo ping protocol validator returned invalid output");
    let status = ping_status_from_mojo(status_value);
    if status == PingStatus::Pass {
        return Ok(());
    }
    let detail =
        ping_structured_failure_detail(&text).unwrap_or_else(|| ping_status_detail(status_value));
    Err(PingValidationFailure { status, detail })
}

fn ping_structured_failure_detail(text: &str) -> Option<String> {
    // Serde retains JSONL syntax compatibility; Mojo owns event and field selection.
    let events = Value::Array(
        text.lines()
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect(),
    );
    let tree = crate::json_boundary::app_json_tree_without_wire(&events)
        .expect("ping diagnostic records exceeded the JSON ABI boundary");
    prodex_mojo_core::rich::ping_structured_failure_detail(&tree.nodes)
        .expect("Mojo ping structured failure projection returned invalid output")
        .map(|detail| ping_process::bounded_ping_detail(&detail))
}

fn classify_failure(output: &Output) -> (PingStatus, String) {
    let text = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let (status, detail) = classify_failure_text(&text);
    (status, detail)
}

fn classify_failure_text(text: &str) -> (PingStatus, String) {
    let status_value = prodex_mojo_core::rich::ping_classify_failure(text)
        .expect("Mojo ping failure classifier returned invalid output");
    (
        ping_status_from_mojo(status_value),
        ping_status_detail(status_value),
    )
}

#[cfg(test)]
#[path = "ping_tests.rs"]
mod tests;

#[cfg(test)]
mod compiled_ping_result_profile_order_tests {
    use super::*;

    fn sample(name: &str, detail: &str) -> PingResult {
        PingResult {
            profile: name.to_owned(),
            model: None,
            effort: None,
            effective_model: None,
            status: PingStatus::Pass,
            detail: detail.to_owned(),
            first_response_latency_ms: None,
            latency_ms: None,
        }
    }

    #[test]
    fn compiled_ping_result_order_preserves_profile_unicode_and_status_rows() {
        let input = vec![
            sample("雪", "first snow"),
            sample("z", "z"),
            sample("a", "first a"),
            sample("é", "accent"),
            sample("", "empty"),
            sample("a", "second a"),
            sample("雪", "second snow"),
        ];
        let output = ping_results_in_profile_order(input);
        assert_eq!(
            output
                .iter()
                .map(|row| row.profile.as_str())
                .collect::<Vec<_>>(),
            ["", "a", "a", "z", "é", "雪", "雪"],
        );
        assert_eq!(
            output
                .iter()
                .map(|row| row.detail.as_str())
                .collect::<Vec<_>>(),
            [
                "empty",
                "first a",
                "second a",
                "z",
                "accent",
                "first snow",
                "second snow"
            ],
        );
        assert_eq!(ping_results_in_profile_order(Vec::new()).len(), 0);
    }

    #[test]
    fn compiled_ping_result_order_handles_large_parallel_completion_permutations() {
        let input = (0..1025)
            .rev()
            .map(|index| {
                sample(
                    &format!("profile-{index:04}"),
                    &format!("completed-{index}"),
                )
            })
            .collect::<Vec<_>>();
        let output = ping_results_in_profile_order(input);
        assert_eq!(output.len(), 1025);
        assert_eq!(output[0].profile, "profile-0000");
        assert_eq!(output[1024].profile, "profile-1024");
        assert_eq!(output[0].detail, "completed-0");
    }
}
