use anyhow::{Context, Result, bail};
use chrono::Local;
use codex_config::codex_config_value;
use prodex_cli::{PingCommands, PingOpenaiArgs};
use prodex_core::AppPaths;
use prodex_quota::{UsageResponse, usage_has_spark_limit};
use prodex_runtime_quota::{runtime_usage_snapshot_is_usable, usage_from_runtime_usage_snapshot};
use prodex_state::{AppState, ProfileProvider};
use serde_json::Value;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use terminal_ui::print_stdout_line;

use super::{
    ChildProcessPlan, codex_child_plan, collect_run_profile_reports, command_output_with_timeout,
    map_parallel, prepare_codex_launch_args, profile_openai_compatible_codex_args,
    remove_provider_secret_env, remove_upstream_proxy_env,
    runtime_launch_openai_spark_context_codex_args,
};
use crate::app_state::{AppStateIoExt, ProfileProviderExt, repair_missing_active_profile_and_save};
use crate::load_runtime_usage_snapshots;

const OPENAI_CODEX_SPARK_MODEL: &str = "gpt-5.3-codex-spark";
const OPENAI_CODEX_SPARK_REASONING_EFFORT: &str = "xhigh";
const PING_PROMPT: &str =
    "Reply with exactly: PONG. Do not use tools, inspect files, or modify anything.";
const PING_TIMEOUT: Duration = Duration::from_secs(45);
const PING_OUTPUT_MAX_BYTES: usize = 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PingStatus {
    Pass,
    SkippedQuota,
    AuthFailed,
    ConfigInvalid,
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
    fn as_str(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::SkippedQuota => "skipped_quota",
            Self::AuthFailed => "auth_failed",
            Self::ConfigInvalid => "config_invalid",
            Self::DnsFailed => "dns_failed",
            Self::TlsFailed => "tls_failed",
            Self::Timeout => "timeout",
            Self::RateLimited => "rate_limited",
            Self::QuotaExhausted => "quota_exhausted",
            Self::UpstreamOverloaded => "upstream_overloaded",
            Self::ModelUnavailable => "model_unavailable",
            Self::ProtocolFailed => "protocol_failed",
            Self::TurnFailed => "turn_failed",
            Self::ProcessFailed => "process_failed",
            Self::Cancelled => "cancelled",
            Self::UnexpectedResponse => "unexpected_response",
        }
    }

    fn is_failure(self) -> bool {
        !matches!(self, Self::Pass | Self::SkippedQuota)
    }
}

#[derive(Debug)]
struct PingResult {
    profile: String,
    model: Option<String>,
    status: PingStatus,
    detail: &'static str,
    latency_ms: Option<u128>,
    quota_preflight_degraded: bool,
}

#[derive(Debug)]
struct PingCandidate {
    name: String,
    codex_home: PathBuf,
    usage: Option<UsageResponse>,
    preflight_status: Option<PingStatus>,
    quota_preflight_degraded: bool,
    no_proxy: bool,
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
    let paths = AppPaths::discover()?;
    let mut state = AppState::load_and_repair(&paths)?;
    repair_missing_active_profile_and_save(&paths, &mut state)?;
    let candidates = collect_openai_ping_candidates(&paths, &state, &args);
    let mut results = map_parallel(candidates, ping_candidate)
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    results.sort_by(|left, right| {
        left.profile
            .cmp(&right.profile)
            .then_with(|| left.model.cmp(&right.model))
    });
    render_ping_results(&results, args.json)?;

    let attempted = results
        .iter()
        .filter(|result| !matches!(result.status, PingStatus::SkippedQuota))
        .count();
    let failures = results
        .iter()
        .filter(|result| result.status.is_failure())
        .map(|result| format!("{} ({})", result.profile, result.status.as_str()))
        .collect::<Vec<_>>();
    if attempted == 0 {
        bail!("no OpenAI profiles could be tested; configure authentication with `prodex login`");
    }
    if !failures.is_empty() {
        bail!("OpenAI ping failed for {}", failures.join(", "));
    }
    Ok(())
}

fn collect_openai_ping_candidates(
    paths: &AppPaths,
    state: &AppState,
    args: &PingOpenaiArgs,
) -> Vec<PingCandidate> {
    let profiles = state
        .profiles
        .iter()
        .filter(|(_, profile)| matches!(profile.provider, ProfileProvider::Openai))
        .map(|(name, profile)| (name.clone(), profile.codex_home.clone()))
        .collect::<Vec<_>>();
    let profile_names = profiles
        .iter()
        .map(|(name, _)| name.clone())
        .collect::<Vec<_>>();
    let reports = collect_run_profile_reports(
        state,
        profile_names,
        args.base_url.as_deref(),
        args.no_proxy,
    );
    let reports = reports
        .into_iter()
        .map(|report| (report.name.clone(), report))
        .collect::<std::collections::BTreeMap<_, _>>();
    let snapshots = load_runtime_usage_snapshots(paths, &state.profiles).unwrap_or_default();
    let now = Local::now().timestamp();

    profiles
        .into_iter()
        .map(|(name, codex_home)| {
            let Some(profile) = state.profiles.get(&name) else {
                return PingCandidate {
                    name,
                    codex_home,
                    usage: None,
                    preflight_status: Some(PingStatus::ConfigInvalid),
                    quota_preflight_degraded: false,
                    no_proxy: args.no_proxy,
                };
            };
            let auth = profile.provider.auth_summary(&profile.codex_home);
            if !auth.quota_compatible && !matches!(auth.label.as_str(), "api-key" | "chatgpt") {
                let status =
                    if auth.label.starts_with("model-provider:") || auth.label == "config-error" {
                        PingStatus::ConfigInvalid
                    } else {
                        PingStatus::AuthFailed
                    };
                return PingCandidate {
                    name,
                    codex_home,
                    usage: None,
                    preflight_status: Some(status),
                    quota_preflight_degraded: false,
                    no_proxy: args.no_proxy,
                };
            }

            let Some(report) = reports.get(&name) else {
                return PingCandidate {
                    name,
                    codex_home,
                    usage: None,
                    preflight_status: None,
                    quota_preflight_degraded: true,
                    no_proxy: args.no_proxy,
                };
            };
            let (usage, degraded) = match &report.result {
                Ok(usage) => (Some(usage.clone()), false),
                Err(_) => match snapshots.get(&name) {
                    Some(snapshot)
                        if runtime_usage_snapshot_is_usable(
                            snapshot,
                            now,
                            crate::RUNTIME_PROFILE_USAGE_CACHE_STALE_GRACE_SECONDS,
                        ) =>
                    {
                        (Some(usage_from_runtime_usage_snapshot(snapshot)), true)
                    }
                    _ => (None, true),
                },
            };
            let preflight_status = usage
                .as_ref()
                .filter(|usage| usage_main_quota_is_authoritatively_exhausted(usage))
                .map(|_| PingStatus::SkippedQuota);
            PingCandidate {
                name,
                codex_home,
                usage,
                preflight_status,
                quota_preflight_degraded: degraded,
                no_proxy: args.no_proxy,
            }
        })
        .collect()
}

fn usage_main_quota_is_authoritatively_exhausted(usage: &UsageResponse) -> bool {
    usage.rate_limit.as_ref().is_some_and(window_pair_exhausted)
        && (!usage_has_spark_limit(usage) || usage_spark_is_authoritatively_exhausted(usage))
}

fn window_pair_exhausted(pair: &prodex_quota::WindowPair) -> bool {
    let windows = [pair.primary_window.as_ref(), pair.secondary_window.as_ref()]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    !windows.is_empty()
        && windows
            .into_iter()
            .all(|window| window.used_percent.is_some_and(|used| used >= 100))
}

fn ping_candidate(candidate: PingCandidate) -> Vec<PingResult> {
    let PingCandidate {
        name,
        codex_home,
        usage,
        preflight_status,
        quota_preflight_degraded,
        no_proxy,
    } = candidate;
    if let Some(status) = preflight_status {
        return vec![PingResult {
            profile: name,
            model: None,
            status,
            detail: match status {
                PingStatus::SkippedQuota => "authoritative quota exhaustion",
                PingStatus::AuthFailed => "profile authentication is unavailable",
                PingStatus::ConfigInvalid => "profile configuration is not OpenAI-pingable",
                _ => "profile preflight failed",
            },
            latency_ms: None,
            quota_preflight_degraded,
        }];
    }

    let models = match ping_openai_models(&codex_home, usage.as_ref()) {
        Ok(models) => models,
        Err(_) => {
            return vec![PingResult {
                profile: name,
                model: None,
                status: PingStatus::ConfigInvalid,
                detail: "could not resolve the profile model",
                latency_ms: None,
                quota_preflight_degraded,
            }];
        }
    };
    models
        .into_iter()
        .map(|model| {
            run_ping_probe(
                &name,
                &codex_home,
                model,
                quota_preflight_degraded,
                no_proxy,
            )
        })
        .collect()
}

fn ping_openai_models(
    codex_home: &Path,
    usage: Option<&UsageResponse>,
) -> Result<Vec<Option<String>>> {
    let configured_model = codex_config_value(codex_home, "model")?
        .map(|model| model.trim().to_string())
        .filter(|model| !model.is_empty());
    let mut models = vec![configured_model.clone()];
    if usage.is_some_and(usage_has_spark_limit)
        && !usage.is_some_and(usage_spark_is_authoritatively_exhausted)
        && !configured_model
            .as_deref()
            .is_some_and(|model| model.eq_ignore_ascii_case(OPENAI_CODEX_SPARK_MODEL))
    {
        models.push(Some(OPENAI_CODEX_SPARK_MODEL.to_string()));
    }
    Ok(models)
}

fn usage_spark_is_authoritatively_exhausted(usage: &UsageResponse) -> bool {
    usage.additional_rate_limits.iter().any(|additional| {
        let is_spark = [
            additional.limit_name.as_deref(),
            additional.metered_feature.as_deref(),
        ]
        .into_iter()
        .flatten()
        .any(|value| {
            let value = value.to_ascii_lowercase();
            value.contains("spark") || value.contains("bengalfox")
        });
        is_spark
            && (additional.allowed == Some(false)
                || additional.limit_reached == Some(true)
                || window_pair_exhausted(&additional.rate_limit))
    })
}

fn run_ping_probe(
    profile_name: &str,
    codex_home: &Path,
    model: Option<String>,
    quota_preflight_degraded: bool,
    no_proxy: bool,
) -> PingResult {
    let started = Instant::now();
    let display_model = model.clone();
    let plan = match ping_openai_child_plan(codex_home.to_path_buf(), model.as_deref(), no_proxy) {
        Ok(plan) => plan,
        Err(_) => {
            return PingResult {
                profile: profile_name.to_string(),
                model: display_model,
                status: PingStatus::ConfigInvalid,
                detail: "could not construct a safe Codex diagnostic",
                latency_ms: Some(started.elapsed().as_millis()),
                quota_preflight_degraded,
            };
        }
    };
    let output = match run_ping_child(&plan) {
        Ok(output) => output,
        Err(error) => {
            let (status, detail) = if error.to_string().contains("timed out") {
                (PingStatus::Timeout, "Codex diagnostic timed out")
            } else {
                (PingStatus::ProcessFailed, "Codex diagnostic process failed")
            };
            return PingResult {
                profile: profile_name.to_string(),
                model: display_model,
                status,
                detail,
                latency_ms: Some(started.elapsed().as_millis()),
                quota_preflight_degraded,
            };
        }
    };
    let result = validate_ping_output(&output);
    let (status, detail) = match result {
        Ok(()) if output.status.success() => {
            (PingStatus::Pass, "authenticated Codex turn completed")
        }
        Ok(()) => classify_failure(&output),
        Err(failure) => (failure.status, failure.detail),
    };
    PingResult {
        profile: profile_name.to_string(),
        model: display_model,
        status,
        detail,
        latency_ms: Some(started.elapsed().as_millis()),
        quota_preflight_degraded,
    }
}

fn run_ping_child(plan: &ChildProcessPlan) -> Result<Output> {
    let cwd = create_ping_cwd()?;
    let result = {
        let mut command = Command::new(&plan.binary);
        command
            .args(&plan.args)
            .env("CODEX_HOME", &plan.codex_home)
            .current_dir(&cwd);
        for key in &plan.removed_env {
            command.env_remove(key);
        }
        for (key, value) in &plan.extra_env {
            command.env(key, value);
        }
        command_output_with_timeout(
            &mut command,
            PING_TIMEOUT,
            PING_OUTPUT_MAX_BYTES,
            "Codex ping",
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

fn ping_openai_child_plan(
    codex_home: PathBuf,
    model: Option<&str>,
    no_proxy: bool,
) -> Result<ChildProcessPlan> {
    let mut args = vec![
        OsString::from("--sandbox"),
        OsString::from("read-only"),
        OsString::from("--ephemeral"),
        OsString::from("--ignore-user-config"),
        OsString::from("--skip-git-repo-check"),
        OsString::from("--json"),
        OsString::from("--color"),
        OsString::from("never"),
    ];
    if let Some(model) = model {
        args.extend([OsString::from("--model"), OsString::from(model)]);
        if model.eq_ignore_ascii_case(OPENAI_CODEX_SPARK_MODEL) {
            args.extend([
                OsString::from("-c"),
                OsString::from(format!(
                    "model_reasoning_effort={OPENAI_CODEX_SPARK_REASONING_EFFORT}"
                )),
            ]);
        } else if let Some(effort) = diagnostic_reasoning_effort(model) {
            args.extend([
                OsString::from("-c"),
                OsString::from(format!("model_reasoning_effort={effort}")),
            ]);
        }
    }
    args.extend([OsString::from("exec"), OsString::from(PING_PROMPT)]);
    let (args, _) = prepare_codex_launch_args(&args, false);
    let args = runtime_launch_openai_spark_context_codex_args(&codex_home, &args)?;
    let args = profile_openai_compatible_codex_args(&codex_home, &args)?;
    let mut plan = codex_child_plan(codex_home, args);
    remove_provider_secret_env(&mut plan);
    if no_proxy {
        remove_upstream_proxy_env(&mut plan);
    }
    Ok(plan)
}

fn diagnostic_reasoning_effort(model: &str) -> Option<&'static str> {
    prodex_provider_core::provider_catalog_entry(prodex_provider_core::ProviderId::OpenAi, model)
        .and_then(|entry| entry.supported_reasoning_efforts.as_deref())
        .and_then(|efforts| {
            efforts.iter().find_map(|effort| match effort {
                prodex_provider_core::ProviderReasoningEffort::None => Some("none"),
                prodex_provider_core::ProviderReasoningEffort::Minimal => Some("minimal"),
                prodex_provider_core::ProviderReasoningEffort::Low => Some("low"),
                prodex_provider_core::ProviderReasoningEffort::Medium => Some("medium"),
                _ => None,
            })
        })
}

fn validate_ping_output(output: &Output) -> std::result::Result<(), PingValidationFailure> {
    let mut thread_started = false;
    let mut turn_started = false;
    let mut turn_completed = false;
    let mut agent_message_completed = false;
    let mut final_message = None;
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let event: Value = serde_json::from_str(line).map_err(|_| PingValidationFailure {
            status: PingStatus::ProtocolFailed,
            detail: "Codex JSONL output was malformed",
        })?;
        match event.get("type").and_then(Value::as_str) {
            Some("thread.started") => thread_started = true,
            Some("turn.started") => turn_started = true,
            Some("turn.completed") => turn_completed = true,
            Some("turn.failed") => {
                let status = classify_failure_text(
                    event
                        .get("error")
                        .and_then(|error| error.get("message"))
                        .and_then(Value::as_str)
                        .unwrap_or("turn failed"),
                )
                .0;
                return Err(PingValidationFailure {
                    status: if status == PingStatus::ProcessFailed {
                        PingStatus::TurnFailed
                    } else {
                        status
                    },
                    detail: "Codex turn failed",
                });
            }
            Some("error") => {
                return Err(PingValidationFailure {
                    status: classify_failure_text(
                        event
                            .get("message")
                            .and_then(Value::as_str)
                            .unwrap_or("Codex error"),
                    )
                    .0,
                    detail: "Codex reported an unrecoverable error",
                });
            }
            Some("item.started") | Some("item.updated") | Some("item.completed") => {
                let item = event.get("item").ok_or(PingValidationFailure {
                    status: PingStatus::ProtocolFailed,
                    detail: "Codex JSONL item event had no item",
                })?;
                match item.get("type").and_then(Value::as_str) {
                    Some("agent_message") => {
                        if event.get("type").and_then(Value::as_str) == Some("item.completed") {
                            agent_message_completed = true;
                            if let Some(text) = item.get("text").and_then(Value::as_str) {
                                final_message = Some(text.to_string());
                            }
                        }
                    }
                    Some("reasoning") => {}
                    Some(_) | None => {
                        return Err(PingValidationFailure {
                            status: PingStatus::ProtocolFailed,
                            detail: "diagnostic turn emitted tool or unsupported activity",
                        });
                    }
                }
            }
            Some(_) | None => {
                return Err(PingValidationFailure {
                    status: PingStatus::ProtocolFailed,
                    detail: "Codex JSONL output contained an unsupported event",
                });
            }
        }
    }
    if !thread_started || !turn_started || !turn_completed {
        return Err(PingValidationFailure {
            status: PingStatus::ProtocolFailed,
            detail: "Codex did not complete a structured turn",
        });
    }
    if !agent_message_completed
        || final_message
            .as_deref()
            .is_none_or(|message| message.trim_end() != "PONG")
    {
        return Err(PingValidationFailure {
            status: PingStatus::UnexpectedResponse,
            detail: "completed turn did not return exactly PONG",
        });
    }
    Ok(())
}

fn classify_failure(output: &Output) -> (PingStatus, &'static str) {
    let text = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    classify_failure_text(&text)
}

fn classify_failure_text(text: &str) -> (PingStatus, &'static str) {
    let lower = text.to_ascii_lowercase();
    if lower.contains("insufficient_quota")
        || lower.contains("usage_limit_reached")
        || lower.contains("quota exceeded")
    {
        return (
            PingStatus::QuotaExhausted,
            "OpenAI reported quota exhaustion",
        );
    }
    if lower.contains("401")
        || lower.contains("unauthorized")
        || lower.contains("invalid api key")
        || lower.contains("authentication")
    {
        return (PingStatus::AuthFailed, "OpenAI authentication failed");
    }
    if lower.contains("429") || lower.contains("rate_limit") || lower.contains("rate limit") {
        return (
            PingStatus::RateLimited,
            "OpenAI temporarily rate limited the profile",
        );
    }
    if lower.contains("503")
        || lower.contains("502")
        || lower.contains("504")
        || lower.contains("overloaded")
        || lower.contains("temporarily unavailable")
    {
        return (
            PingStatus::UpstreamOverloaded,
            "OpenAI upstream is temporarily unavailable",
        );
    }
    if lower.contains("dns")
        || lower.contains("resolve")
        || lower.contains("name or service not known")
        || lower.contains("getaddrinfo")
    {
        return (PingStatus::DnsFailed, "OpenAI hostname resolution failed");
    }
    if lower.contains("tls") || lower.contains("certificate") || lower.contains("handshake") {
        return (PingStatus::TlsFailed, "OpenAI TLS connection failed");
    }
    if lower.contains("unsupported model") || lower.contains("model_not_found") {
        return (
            PingStatus::ModelUnavailable,
            "selected OpenAI model is unavailable",
        );
    }
    if lower.contains("cancel") {
        return (PingStatus::Cancelled, "OpenAI ping was cancelled");
    }
    if lower.contains("timeout") || lower.contains("timed out") {
        return (PingStatus::Timeout, "OpenAI diagnostic timed out");
    }
    (PingStatus::ProcessFailed, "Codex diagnostic process failed")
}

fn render_ping_results(results: &[PingResult], json: bool) -> Result<()> {
    let passed = results
        .iter()
        .filter(|result| result.status == PingStatus::Pass)
        .count();
    let skipped = results
        .iter()
        .filter(|result| result.status == PingStatus::SkippedQuota)
        .count();
    let failed = results
        .iter()
        .filter(|result| result.status.is_failure())
        .count();
    if json {
        let value = serde_json::json!({
            "provider": "openai",
            "profiles": results.iter().map(|result| serde_json::json!({
                "profile": result.profile,
                "model": result.model,
                "status": result.status.as_str(),
                "detail": result.detail,
                "latency_ms": result.latency_ms,
                "quota_preflight": if result.quota_preflight_degraded { "degraded" } else { "checked" },
            })).collect::<Vec<_>>(),
            "summary": { "passed": passed, "skipped": skipped, "failed": failed },
        });
        print_stdout_line(&serde_json::to_string(&value)?)?;
        return Ok(());
    }
    print_stdout_line("OpenAI ping")?;
    print_stdout_line("Profile\tModel\tStatus\tLatency")?;
    for result in results {
        let model = result.model.as_deref().unwrap_or("catalog default");
        let latency = result
            .latency_ms
            .map_or_else(|| "-".to_string(), |value| format!("{value}ms"));
        let detail = if result.quota_preflight_degraded {
            format!("{}; quota preflight degraded", result.detail)
        } else {
            result.detail.to_string()
        };
        print_stdout_line(&format!(
            "{}\t{}\t{}\t{}\t{}",
            result.profile,
            model,
            result.status.as_str(),
            latency,
            detail
        ))?;
    }
    print_stdout_line(&format!(
        "{passed} passed · {skipped} skipped · {failed} failed"
    ))?;
    Ok(())
}

#[cfg(test)]
#[path = "ping_tests.rs"]
mod tests;
