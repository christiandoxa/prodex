use clap::{Args, ValueEnum};
use std::ffi::OsString;

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

impl CodexRuntimeFeatureArgs {
    pub fn to_codex_config_args(&self) -> Vec<OsString> {
        let mut overrides = Vec::new();
        if let Some(mode) = self.web_search {
            overrides.push(format!(
                "web_search={}",
                toml_string_literal(mode.config_value())
            ));
        }
        if let Some(limit) = self.rollout_budget_tokens.filter(|limit| *limit > 1) {
            overrides.push("features.rollout_budget.enabled=true".to_string());
            overrides.push(format!("features.rollout_budget.limit_tokens={limit}"));
            let reminders = rollout_budget_reminders(limit, &self.rollout_budget_reminders);
            overrides.push(format!(
                "features.rollout_budget.reminder_at_remaining_tokens=[{}]",
                reminders
                    .iter()
                    .map(u64::to_string)
                    .collect::<Vec<_>>()
                    .join(",")
            ));
            if let Some(weight) = self
                .rollout_budget_sampling_weight
                .filter(|weight| weight.is_finite() && *weight >= 0.0)
            {
                overrides.push(format!(
                    "features.rollout_budget.sampling_token_weight={weight}"
                ));
            }
            if let Some(weight) = self
                .rollout_budget_prefill_weight
                .filter(|weight| weight.is_finite() && *weight >= 0.0)
            {
                overrides.push(format!(
                    "features.rollout_budget.prefill_token_weight={weight}"
                ));
            }
        }
        if self.current_time_reminder
            || self.current_time_reminder_interval.is_some()
            || self.current_time_clock_source.is_some()
        {
            overrides.push("features.current_time_reminder.enabled=true".to_string());
            if let Some(interval) = self
                .current_time_reminder_interval
                .filter(|interval| *interval > 0)
            {
                overrides.push(format!(
                    "features.current_time_reminder.reminder_interval_model_requests={interval}"
                ));
            }
            if let Some(source) = self.current_time_clock_source {
                overrides.push(format!(
                    "features.current_time_reminder.clock_source={}",
                    toml_string_literal(source.config_value())
                ));
            }
        }

        let mut args = Vec::with_capacity(overrides.len() * 2);
        for override_entry in overrides {
            args.push(OsString::from("-c"));
            args.push(OsString::from(override_entry));
        }
        args
    }
}

impl CodexWebSearchMode {
    fn config_value(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::Cached => "cached",
            Self::Indexed => "indexed",
            Self::Live => "live",
        }
    }
}

impl CodexCurrentTimeClockSource {
    fn config_value(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::External => "external",
        }
    }
}

fn rollout_budget_reminders(limit: u64, configured: &[u64]) -> Vec<u64> {
    let mut reminders = configured
        .iter()
        .copied()
        .filter(|value| *value > 0 && *value < limit)
        .collect::<Vec<_>>();
    if reminders.is_empty() && limit > 1 {
        reminders = [75_u64, 50, 25]
            .iter()
            .filter_map(|percent| {
                let value = limit.saturating_mul(*percent) / 100;
                (value > 0 && value < limit).then_some(value)
            })
            .collect();
    }
    if reminders.is_empty() && limit > 1 {
        reminders.push(limit - 1);
    }
    reminders.sort_unstable();
    reminders.dedup();
    reminders.reverse();
    reminders
}

fn toml_string_literal(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}
