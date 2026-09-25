use super::{
    SuperArgs, SuperCliAgent, parse_runtime_base_url, parse_super_external_provider,
    parse_super_local_url,
};
use crate::{
    CodexCurrentTimeClockSource, CodexWebSearchMode, SubAgentMaxConcurrency,
    SubAgentReasoningEffort, parse_sub_agent_max_concurrency, parse_sub_agent_model,
    parse_sub_agent_provider, parse_sub_agent_reasoning_effort, parse_sub_agent_url,
};
use prodex_mojo_core::launch::{ScannedSuperOverride, SuperOverrideKind};
use prodex_optional_tools::OptionalToolId;

const API_KEY_OPTION: &str = "--api-key";

struct ScannedValue<'a> {
    value: Option<&'a str>,
    consumed_count: usize,
}

#[derive(Debug, PartialEq)]
enum SuperOverride {
    Provider(super::SuperExternalProvider),
    Cli(SuperCliAgent),
    ApiKey(String),
    LocalModel(String),
    Profile(String),
    AutoRotate(bool),
    AutoRedeem,
    SkipQuotaCheck,
    DryRun,
    NoProxy,
    Presidio(bool),
    SubAgent(bool),
    SubAgentProvider(prodex_provider_core::ProviderId),
    SubAgentModel(String),
    SubAgentReasoningEffort(SubAgentReasoningEffort),
    SubAgentUrl(String),
    SubAgentMaxConcurrency(SubAgentMaxConcurrency),
    BaseUrl(String),
    Url(String),
    LocalContextWindow(usize),
    LocalAutoCompactTokenLimit(usize),
    Tool(OptionalToolId),
    RequiredTool(OptionalToolId),
    FullAccess,
    WebSearch(CodexWebSearchMode),
    RolloutBudgetTokens(u64),
    RolloutBudgetReminders(Vec<u64>),
    RolloutBudgetSamplingWeight(f64),
    RolloutBudgetPrefillWeight(f64),
    CurrentTimeReminder,
    CurrentTimeReminderInterval(u64),
    CurrentTimeClockSource(CodexCurrentTimeClockSource),
    RespectSystemProxy(bool),
}

#[derive(Debug, PartialEq)]
enum ScanOutcome {
    Apply {
        value: SuperOverride,
        consumed_count: usize,
    },
    Unknown,
}

pub(super) fn extract_super_overrides_from_codex_args(args: &mut SuperArgs) -> Result<(), String> {
    extract_super_overrides_from_codex_args_inner(args, true)
}

pub(super) fn extract_super_overrides_from_codex_args_without_sub_agent_validation(
    args: &mut SuperArgs,
) -> Result<(), String> {
    extract_super_overrides_from_codex_args_inner(args, false)
}

fn extract_super_overrides_from_codex_args_inner(
    args: &mut SuperArgs,
    validate_sub_agent: bool,
) -> Result<(), String> {
    let codex_args = std::mem::take(&mut args.codex_args);
    let scan_result = {
        let views: Vec<_> = codex_args
            .iter()
            .map(|argument| argument.to_str())
            .collect();
        prodex_mojo_core::launch::scan_super_overrides(&views)
    };
    let override_plan = match scan_result {
        Ok(plan) => plan,
        Err(_) => {
            args.codex_args = codex_args;
            return Err("Mojo Super argument scan failed".to_string());
        }
    };
    let mut remaining = Vec::with_capacity(codex_args.len());
    let mut index = 0;
    while index < codex_args.len() {
        if codex_args[index] == "--" {
            remaining.extend(codex_args[index..].iter().cloned());
            break;
        }
        let outcome = scan_mojo_override(override_plan[index]);
        match outcome {
            Ok(ScanOutcome::Apply {
                value,
                consumed_count,
            }) => {
                apply_override(args, value);
                index += consumed_count;
            }
            Ok(ScanOutcome::Unknown) => {
                remaining.push(codex_args[index].clone());
                index += 1;
            }
            Err(err) => {
                remaining.extend(codex_args[index..].iter().cloned());
                args.codex_args = remaining;
                return Err(err);
            }
        }
    }
    args.codex_args = remaining;
    if validate_sub_agent {
        super::super_validation::validate_sub_agent_flags(args)?;
    }
    Ok(())
}

fn scan_mojo_override(directive: Option<ScannedSuperOverride<'_>>) -> Result<ScanOutcome, String> {
    let Some(directive) = directive else {
        return Ok(ScanOutcome::Unknown);
    };
    let scanned = ScannedValue {
        value: directive.value,
        consumed_count: directive.consumed_count,
    };
    use SuperOverrideKind as Kind;
    match directive.kind {
        Kind::Provider => parse_required(
            scanned,
            parse_super_external_provider,
            SuperOverride::Provider,
            "--provider",
        ),
        Kind::Cli => parse_required(
            scanned,
            |value| {
                match value {
                "agy" => Ok(SuperCliAgent::Agy),
                _ => Err("only --cli agy is retained; use --provider gemini|copilot|kiro for other providers".to_string()),
            }
            },
            SuperOverride::Cli,
            "--cli",
        ),
        Kind::ApiKey => parse_required_string(scanned, SuperOverride::ApiKey, API_KEY_OPTION),
        Kind::SubAgentProvider => parse_required(
            scanned,
            parse_sub_agent_provider,
            SuperOverride::SubAgentProvider,
            "--sub-agent-provider",
        ),
        Kind::SubAgentModel => parse_required(
            scanned,
            parse_sub_agent_model,
            SuperOverride::SubAgentModel,
            "--sub-agent-model",
        ),
        Kind::SubAgentReasoningEffort => parse_required(
            scanned,
            parse_sub_agent_reasoning_effort,
            SuperOverride::SubAgentReasoningEffort,
            "--sub-agent-model-reasoning-effort",
        ),
        Kind::SubAgentUrl => parse_required(
            scanned,
            parse_sub_agent_url,
            SuperOverride::SubAgentUrl,
            "--sub-agent-url",
        ),
        Kind::SubAgentMaxConcurrency => parse_required(
            scanned,
            parse_sub_agent_max_concurrency,
            SuperOverride::SubAgentMaxConcurrency,
            "--sub-agent-max-concurrency",
        ),
        Kind::LocalModel => parse_required_string(scanned, SuperOverride::LocalModel, "--model"),
        Kind::Profile => parse_required_string(scanned, SuperOverride::Profile, "--profile"),
        Kind::BaseUrl => parse_required(
            scanned,
            parse_runtime_base_url,
            SuperOverride::BaseUrl,
            "--base-url",
        ),
        Kind::Url => parse_required(scanned, parse_super_local_url, SuperOverride::Url, "--url"),
        Kind::LocalContextWindow => parse_required(
            scanned,
            str::parse::<usize>,
            SuperOverride::LocalContextWindow,
            "--context-window",
        ),
        Kind::LocalAutoCompactTokenLimit => parse_required(
            scanned,
            str::parse::<usize>,
            SuperOverride::LocalAutoCompactTokenLimit,
            "--auto-compact-token-limit",
        ),
        Kind::Tool => parse_required(
            scanned,
            str::parse::<OptionalToolId>,
            SuperOverride::Tool,
            "--tool",
        ),
        Kind::RequiredTool => parse_required(
            scanned,
            str::parse::<OptionalToolId>,
            SuperOverride::RequiredTool,
            "--require-tool",
        ),
        Kind::WebSearch => parse_required(
            scanned,
            parse_web_search_mode,
            SuperOverride::WebSearch,
            "--web-search",
        ),
        Kind::RolloutBudgetTokens => parse_required(
            scanned,
            str::parse::<u64>,
            SuperOverride::RolloutBudgetTokens,
            "--rollout-budget-tokens",
        ),
        Kind::RolloutBudgetReminders => parse_required(
            scanned,
            parse_rollout_budget_reminders,
            SuperOverride::RolloutBudgetReminders,
            "--rollout-budget-reminders",
        ),
        Kind::RolloutBudgetSamplingWeight => parse_required(
            scanned,
            str::parse::<f64>,
            SuperOverride::RolloutBudgetSamplingWeight,
            "--rollout-budget-sampling-weight",
        ),
        Kind::RolloutBudgetPrefillWeight => parse_required(
            scanned,
            str::parse::<f64>,
            SuperOverride::RolloutBudgetPrefillWeight,
            "--rollout-budget-prefill-weight",
        ),
        Kind::CurrentTimeReminderInterval => parse_required(
            scanned,
            str::parse::<u64>,
            SuperOverride::CurrentTimeReminderInterval,
            "--current-time-reminder-interval",
        ),
        Kind::CurrentTimeClockSource => parse_required(
            scanned,
            parse_current_time_clock_source,
            SuperOverride::CurrentTimeClockSource,
            "--current-time-clock-source",
        ),
        Kind::NoAutoRotate => Ok(apply(1, SuperOverride::AutoRotate(false))),
        Kind::AutoRotate => Ok(apply(1, SuperOverride::AutoRotate(true))),
        Kind::AutoRedeem => Ok(apply(1, SuperOverride::AutoRedeem)),
        Kind::SkipQuotaCheck => Ok(apply(1, SuperOverride::SkipQuotaCheck)),
        Kind::DryRun => Ok(apply(1, SuperOverride::DryRun)),
        Kind::NoProxy => Ok(apply(1, SuperOverride::NoProxy)),
        Kind::Presidio => Ok(apply(1, SuperOverride::Presidio(true))),
        Kind::NoPresidio => Ok(apply(1, SuperOverride::Presidio(false))),
        Kind::SubAgent => Ok(apply(1, SuperOverride::SubAgent(true))),
        Kind::NoSubAgent => Ok(apply(1, SuperOverride::SubAgent(false))),
        Kind::FullAccess => Ok(apply(1, SuperOverride::FullAccess)),
        Kind::CurrentTimeReminder => Ok(apply(1, SuperOverride::CurrentTimeReminder)),
        Kind::RespectSystemProxy => Ok(apply(1, SuperOverride::RespectSystemProxy(true))),
        Kind::NoRespectSystemProxy => Ok(apply(1, SuperOverride::RespectSystemProxy(false))),
    }
}

fn parse_required_string(
    scanned: ScannedValue<'_>,
    wrap: impl FnOnce(String) -> SuperOverride,
    option: &str,
) -> Result<ScanOutcome, String> {
    let value = scanned
        .value
        .ok_or_else(|| format!("{option} requires a value"))?;
    Ok(apply(scanned.consumed_count, wrap(value.to_string())))
}

fn parse_required<T, E: ToString>(
    scanned: ScannedValue<'_>,
    parse: impl FnOnce(&str) -> Result<T, E>,
    wrap: impl FnOnce(T) -> SuperOverride,
    option: &str,
) -> Result<ScanOutcome, String> {
    let value = scanned
        .value
        .ok_or_else(|| format!("{option} requires a value"))?;
    let value = parse(value).map_err(|err| err.to_string())?;
    Ok(apply(scanned.consumed_count, wrap(value)))
}

fn apply(consumed_count: usize, value: SuperOverride) -> ScanOutcome {
    ScanOutcome::Apply {
        value,
        consumed_count,
    }
}

fn apply_override(args: &mut SuperArgs, value: SuperOverride) {
    match value {
        SuperOverride::Provider(value) => args.provider = Some(value),
        SuperOverride::Cli(value) => args.cli = Some(value),
        SuperOverride::ApiKey(value) => args.api_key = Some(value),
        SuperOverride::LocalModel(value) => args.local_model = Some(value),
        SuperOverride::Profile(value) if args.profile.is_none() => args.profile = Some(value),
        SuperOverride::Profile(_) => {}
        SuperOverride::AutoRotate(true) => {
            args.auto_rotate = true;
            args.no_auto_rotate = false;
        }
        SuperOverride::AutoRotate(false) => {
            args.no_auto_rotate = true;
            args.auto_rotate = false;
        }
        SuperOverride::AutoRedeem => args.auto_redeem = true,
        SuperOverride::SkipQuotaCheck => args.skip_quota_check = true,
        SuperOverride::DryRun => args.dry_run = true,
        SuperOverride::NoProxy => args.no_proxy = true,
        SuperOverride::Presidio(true) => {
            args.presidio = true;
        }
        SuperOverride::Presidio(false) => {
            args.no_presidio = true;
        }
        SuperOverride::SubAgent(true) => args.sub_agent = true,
        SuperOverride::SubAgent(false) => args.no_sub_agent = true,
        SuperOverride::SubAgentProvider(value) => args.sub_agent_provider = Some(value),
        SuperOverride::SubAgentModel(value) => args.sub_agent_model = Some(value),
        SuperOverride::SubAgentReasoningEffort(value) => {
            args.sub_agent_model_reasoning_effort = Some(value)
        }
        SuperOverride::SubAgentUrl(value) => args.sub_agent_url = Some(value),
        SuperOverride::SubAgentMaxConcurrency(value) => {
            args.sub_agent_max_concurrency = Some(value)
        }
        SuperOverride::BaseUrl(value) => args.base_url = Some(value),
        SuperOverride::Url(value) => args.url = Some(value),
        SuperOverride::LocalContextWindow(value) => args.local_context_window = Some(value),
        SuperOverride::LocalAutoCompactTokenLimit(value) => {
            args.local_auto_compact_token_limit = Some(value);
        }
        SuperOverride::Tool(value) => {
            if !args.tools.contains(&value) {
                args.tools.push(value);
            }
        }
        SuperOverride::RequiredTool(value) => {
            if !args.required_tools.contains(&value) {
                args.required_tools.push(value);
            }
            if !args.tools.contains(&value) {
                args.tools.push(value);
            }
        }
        SuperOverride::FullAccess => args.full_access = true,
        SuperOverride::WebSearch(value) => args.codex_features.web_search = Some(value),
        SuperOverride::RolloutBudgetTokens(value) => {
            args.codex_features.rollout_budget_tokens = Some(value)
        }
        SuperOverride::RolloutBudgetReminders(value) => {
            args.codex_features.rollout_budget_reminders.extend(value)
        }
        SuperOverride::RolloutBudgetSamplingWeight(value) => {
            args.codex_features.rollout_budget_sampling_weight = Some(value)
        }
        SuperOverride::RolloutBudgetPrefillWeight(value) => {
            args.codex_features.rollout_budget_prefill_weight = Some(value)
        }
        SuperOverride::CurrentTimeReminder => args.codex_features.current_time_reminder = true,
        SuperOverride::CurrentTimeReminderInterval(value) => {
            args.codex_features.current_time_reminder_interval = Some(value)
        }
        SuperOverride::CurrentTimeClockSource(value) => {
            args.codex_features.current_time_clock_source = Some(value)
        }
        SuperOverride::RespectSystemProxy(true) => {
            args.codex_features.respect_system_proxy = true;
            args.codex_features.no_respect_system_proxy = false;
        }
        SuperOverride::RespectSystemProxy(false) => {
            args.codex_features.no_respect_system_proxy = true;
            args.codex_features.respect_system_proxy = false;
        }
    }
}

fn parse_web_search_mode(value: &str) -> Result<CodexWebSearchMode, String> {
    match value.to_ascii_lowercase().as_str() {
        "disabled" => Ok(CodexWebSearchMode::Disabled),
        "cached" => Ok(CodexWebSearchMode::Cached),
        "indexed" => Ok(CodexWebSearchMode::Indexed),
        "live" => Ok(CodexWebSearchMode::Live),
        _ => Err("expected disabled, cached, indexed, or live".to_string()),
    }
}

fn parse_current_time_clock_source(value: &str) -> Result<CodexCurrentTimeClockSource, String> {
    match value.to_ascii_lowercase().as_str() {
        "system" => Ok(CodexCurrentTimeClockSource::System),
        "external" => Ok(CodexCurrentTimeClockSource::External),
        _ => Err("expected system or external".to_string()),
    }
}

fn parse_rollout_budget_reminders(value: &str) -> Result<Vec<u64>, String> {
    value
        .split(',')
        .map(str::parse::<u64>)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| "expected comma-separated unsigned integers".to_string())
}
