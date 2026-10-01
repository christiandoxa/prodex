use clap::{Args, ValueEnum};
use std::{error::Error, ffi::OsString, fmt};

use prodex_mojo_core::launch::{
    RuntimeFeatureClockSource, RuntimeFeatureConfigInput, RuntimeFeatureWebSearchMode,
    plan_runtime_feature_config,
};

#[derive(Args, Debug, Clone, Default)]
pub struct CodexRuntimeFeatureArgs {
    /// Override Codex hosted web search mode: disabled, cached, indexed, or live.
    #[arg(long, value_name = "MODE", value_enum)]
    pub web_search: Option<CodexWebSearchMode>,
    /// Enable Codex rollout token-budget reminders with this total token limit.
    #[arg(long, value_name = "TOKENS")]
    pub rollout_budget_tokens: Option<u64>,
    /// Remaining-token thresholds for rollout budget reminders. Defaults to 75%,50%,25% of the limit.
    #[arg(
        long,
        value_name = "TOKENS",
        value_delimiter = ',',
        requires = "rollout_budget_tokens"
    )]
    pub rollout_budget_reminders: Vec<u64>,
    /// Sampling token weight used by Codex rollout budget accounting.
    #[arg(long, value_name = "WEIGHT", requires = "rollout_budget_tokens")]
    pub rollout_budget_sampling_weight: Option<f64>,
    /// Prefill token weight used by Codex rollout budget accounting.
    #[arg(long, value_name = "WEIGHT", requires = "rollout_budget_tokens")]
    pub rollout_budget_prefill_weight: Option<f64>,
    /// Enable Codex current-time reminders.
    #[arg(long)]
    pub current_time_reminder: bool,
    /// Model-request interval for Codex current-time reminders.
    #[arg(long, value_name = "REQUESTS")]
    pub current_time_reminder_interval: Option<u64>,
    /// Clock source for Codex current-time reminders.
    #[arg(long, value_name = "SOURCE", value_enum)]
    pub current_time_clock_source: Option<CodexCurrentTimeClockSource>,

    /// Enable Codex auth clients to respect the OS system proxy when upstream Codex supports it.
    #[arg(long, conflicts_with = "no_respect_system_proxy")]
    pub respect_system_proxy: bool,

    /// Disable Codex auth system-proxy routing even when the profile config enables it.
    #[arg(long)]
    pub no_respect_system_proxy: bool,
}

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodexWebSearchMode {
    Disabled,
    Cached,
    Indexed,
    Live,
}

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodexCurrentTimeClockSource {
    System,
    External,
}

/// Failed closed when Mojo runtime-feature planning is unavailable or returns an invalid result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeFeaturePlanError;

impl fmt::Display for RuntimeFeaturePlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Codex runtime feature planning failed")
    }
}

impl Error for RuntimeFeaturePlanError {}

fn mojo_web_search_mode(mode: Option<CodexWebSearchMode>) -> Option<RuntimeFeatureWebSearchMode> {
    mode.map(|mode| match mode {
        CodexWebSearchMode::Disabled => RuntimeFeatureWebSearchMode::Disabled,
        CodexWebSearchMode::Cached => RuntimeFeatureWebSearchMode::Cached,
        CodexWebSearchMode::Indexed => RuntimeFeatureWebSearchMode::Indexed,
        CodexWebSearchMode::Live => RuntimeFeatureWebSearchMode::Live,
    })
}

fn mojo_clock_source(
    source: Option<CodexCurrentTimeClockSource>,
) -> Option<RuntimeFeatureClockSource> {
    source.map(|source| match source {
        CodexCurrentTimeClockSource::System => RuntimeFeatureClockSource::System,
        CodexCurrentTimeClockSource::External => RuntimeFeatureClockSource::External,
    })
}

fn runtime_web_search_config_value(mode: RuntimeFeatureWebSearchMode) -> &'static str {
    match mode {
        RuntimeFeatureWebSearchMode::Disabled => "disabled",
        RuntimeFeatureWebSearchMode::Cached => "cached",
        RuntimeFeatureWebSearchMode::Indexed => "indexed",
        RuntimeFeatureWebSearchMode::Live => "live",
    }
}

fn runtime_clock_source_config_value(source: RuntimeFeatureClockSource) -> &'static str {
    match source {
        RuntimeFeatureClockSource::System => "system",
        RuntimeFeatureClockSource::External => "external",
    }
}

impl CodexRuntimeFeatureArgs {
    pub fn to_codex_config_args(&self) -> Result<Vec<OsString>, RuntimeFeaturePlanError> {
        let plan = plan_runtime_feature_config(RuntimeFeatureConfigInput {
            web_search_mode: mojo_web_search_mode(self.web_search),
            rollout_budget_limit: self.rollout_budget_tokens,
            rollout_budget_reminders: &self.rollout_budget_reminders,
            rollout_budget_sampling_weight: self.rollout_budget_sampling_weight,
            rollout_budget_prefill_weight: self.rollout_budget_prefill_weight,
            current_time_reminder: self.current_time_reminder,
            current_time_reminder_interval: self.current_time_reminder_interval,
            current_time_clock_source: mojo_clock_source(self.current_time_clock_source),
            respect_system_proxy: self.respect_system_proxy,
            no_respect_system_proxy: self.no_respect_system_proxy,
        })
        .map_err(|_| RuntimeFeaturePlanError)?;

        let mut overrides = Vec::new();
        if let Some(mode) = plan.web_search_mode {
            overrides.push(format!(
                "web_search={}",
                toml_string_literal(runtime_web_search_config_value(mode))
            ));
        }
        if plan.rollout_budget_enabled {
            let limit = self.rollout_budget_tokens.ok_or(RuntimeFeaturePlanError)?;
            overrides.extend([
                "features.rollout_budget.enabled=true".to_string(),
                format!("features.rollout_budget.limit_tokens={limit}"),
                format!(
                    "features.rollout_budget.reminder_at_remaining_tokens=[{}]",
                    plan.rollout_budget_reminders
                        .iter()
                        .map(u64::to_string)
                        .collect::<Vec<_>>()
                        .join(",")
                ),
            ]);
            if plan.rollout_budget_sampling_weight {
                let weight = self
                    .rollout_budget_sampling_weight
                    .ok_or(RuntimeFeaturePlanError)?;
                overrides.push(format!(
                    "features.rollout_budget.sampling_token_weight={weight}"
                ));
            }
            if plan.rollout_budget_prefill_weight {
                let weight = self
                    .rollout_budget_prefill_weight
                    .ok_or(RuntimeFeaturePlanError)?;
                overrides.push(format!(
                    "features.rollout_budget.prefill_token_weight={weight}"
                ));
            }
        }

        if plan.current_time_reminder_enabled {
            overrides.push("features.current_time_reminder.enabled=true".to_string());
            if plan.current_time_reminder_interval {
                let interval = self
                    .current_time_reminder_interval
                    .ok_or(RuntimeFeaturePlanError)?;
                overrides.push(format!(
                    "features.current_time_reminder.reminder_interval_model_requests={interval}"
                ));
            }
            if let Some(source) = plan.current_time_clock_source {
                overrides.push(format!(
                    "features.current_time_reminder.clock_source={}",
                    toml_string_literal(runtime_clock_source_config_value(source))
                ));
            }
        }

        if let Some(respect_system_proxy) = plan.respect_system_proxy {
            overrides.push(format!(
                "features.respect_system_proxy={respect_system_proxy}"
            ));
        }

        let mut args = Vec::with_capacity(overrides.len() * 2);
        for override_entry in overrides {
            args.push(OsString::from("-c"));
            args.push(OsString::from(override_entry));
        }
        Ok(args)
    }
}

fn toml_string_literal(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}
