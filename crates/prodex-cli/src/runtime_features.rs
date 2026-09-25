use clap::{Args, ValueEnum};
use std::{error::Error, ffi::OsString, fmt};

#[cfg(feature = "mojo-core")]
use prodex_mojo_core::launch::{
    RuntimeFeatureClockSource, RuntimeFeatureConfigInput, RuntimeFeatureConfigPlan,
    RuntimeFeatureWebSearchMode, plan_runtime_feature_config,
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

/// Failed closed when the Mojo runtime-feature planner returns an invalid result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeFeaturePlanError;

impl fmt::Display for RuntimeFeaturePlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Codex runtime feature planning failed")
    }
}

impl Error for RuntimeFeaturePlanError {}

#[derive(Debug, PartialEq)]
struct FeaturePlan {
    web_search: Option<CodexWebSearchMode>,
    rollout_budget: Option<RolloutBudgetPlan>,
    current_time_reminder_enabled: bool,
    current_time_reminder_interval: Option<u64>,
    current_time_clock_source: Option<CodexCurrentTimeClockSource>,
    respect_system_proxy: Option<bool>,
}

#[derive(Debug, PartialEq)]
struct RolloutBudgetPlan {
    limit: u64,
    reminders: Vec<u64>,
    sampling_weight: Option<f64>,
    prefill_weight: Option<f64>,
}

#[cfg(feature = "mojo-core")]
fn mojo_web_search_mode(mode: Option<CodexWebSearchMode>) -> Option<RuntimeFeatureWebSearchMode> {
    mode.map(|mode| match mode {
        CodexWebSearchMode::Disabled => RuntimeFeatureWebSearchMode::Disabled,
        CodexWebSearchMode::Cached => RuntimeFeatureWebSearchMode::Cached,
        CodexWebSearchMode::Indexed => RuntimeFeatureWebSearchMode::Indexed,
        CodexWebSearchMode::Live => RuntimeFeatureWebSearchMode::Live,
    })
}

#[cfg(feature = "mojo-core")]
fn codex_web_search_mode(mode: Option<RuntimeFeatureWebSearchMode>) -> Option<CodexWebSearchMode> {
    mode.map(|mode| match mode {
        RuntimeFeatureWebSearchMode::Disabled => CodexWebSearchMode::Disabled,
        RuntimeFeatureWebSearchMode::Cached => CodexWebSearchMode::Cached,
        RuntimeFeatureWebSearchMode::Indexed => CodexWebSearchMode::Indexed,
        RuntimeFeatureWebSearchMode::Live => CodexWebSearchMode::Live,
    })
}

#[cfg(feature = "mojo-core")]
fn mojo_clock_source(
    source: Option<CodexCurrentTimeClockSource>,
) -> Option<RuntimeFeatureClockSource> {
    source.map(|source| match source {
        CodexCurrentTimeClockSource::System => RuntimeFeatureClockSource::System,
        CodexCurrentTimeClockSource::External => RuntimeFeatureClockSource::External,
    })
}

#[cfg(feature = "mojo-core")]
fn codex_clock_source(
    source: Option<RuntimeFeatureClockSource>,
) -> Option<CodexCurrentTimeClockSource> {
    source.map(|source| match source {
        RuntimeFeatureClockSource::System => CodexCurrentTimeClockSource::System,
        RuntimeFeatureClockSource::External => CodexCurrentTimeClockSource::External,
    })
}

#[cfg(feature = "mojo-core")]
fn enabled_weight(
    enabled: bool,
    value: Option<f64>,
) -> Result<Option<f64>, RuntimeFeaturePlanError> {
    if !enabled {
        return Ok(None);
    }
    value.map(Some).ok_or(RuntimeFeaturePlanError)
}

#[cfg(feature = "mojo-core")]
fn rollout_budget_from_mojo(
    args: &CodexRuntimeFeatureArgs,
    plan: &RuntimeFeatureConfigPlan,
) -> Result<Option<RolloutBudgetPlan>, RuntimeFeaturePlanError> {
    if !plan.rollout_budget_enabled {
        let disabled_outputs_are_empty = plan.rollout_budget_reminders.is_empty()
            && !plan.rollout_budget_sampling_weight
            && !plan.rollout_budget_prefill_weight;
        return disabled_outputs_are_empty
            .then_some(None)
            .ok_or(RuntimeFeaturePlanError);
    }

    Ok(Some(RolloutBudgetPlan {
        limit: args.rollout_budget_tokens.ok_or(RuntimeFeaturePlanError)?,
        reminders: plan.rollout_budget_reminders.clone(),
        sampling_weight: enabled_weight(
            plan.rollout_budget_sampling_weight,
            args.rollout_budget_sampling_weight,
        )?,
        prefill_weight: enabled_weight(
            plan.rollout_budget_prefill_weight,
            args.rollout_budget_prefill_weight,
        )?,
    }))
}

#[cfg(feature = "mojo-core")]
fn current_time_interval_from_mojo(
    args: &CodexRuntimeFeatureArgs,
    plan: &RuntimeFeatureConfigPlan,
) -> Result<Option<u64>, RuntimeFeaturePlanError> {
    if !plan.current_time_reminder_interval {
        return Ok(None);
    }
    args.current_time_reminder_interval
        .map(Some)
        .ok_or(RuntimeFeaturePlanError)
}

impl CodexRuntimeFeatureArgs {
    pub fn to_codex_config_args(&self) -> Result<Vec<OsString>, RuntimeFeaturePlanError> {
        #[cfg(feature = "mojo-core")]
        let plan = self.mojo_plan()?;
        #[cfg(not(feature = "mojo-core"))]
        let plan = self.rust_plan();

        Ok(render_plan(plan))
    }

    #[cfg(feature = "mojo-core")]
    fn mojo_plan(&self) -> Result<FeaturePlan, RuntimeFeaturePlanError> {
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

        Ok(FeaturePlan {
            web_search: codex_web_search_mode(plan.web_search_mode),
            rollout_budget: rollout_budget_from_mojo(self, &plan)?,
            current_time_reminder_enabled: plan.current_time_reminder_enabled,
            current_time_reminder_interval: current_time_interval_from_mojo(self, &plan)?,
            current_time_clock_source: codex_clock_source(plan.current_time_clock_source),
            respect_system_proxy: plan.respect_system_proxy,
        })
    }

    #[cfg(any(not(feature = "mojo-core"), test))]
    fn rust_plan(&self) -> FeaturePlan {
        let rollout_budget = self
            .rollout_budget_tokens
            .filter(|limit| *limit > 1)
            .map(|limit| {
                let reminders = rollout_budget_reminders(limit, &self.rollout_budget_reminders);
                let sampling_weight = self
                    .rollout_budget_sampling_weight
                    .filter(|weight| weight.is_finite() && *weight >= 0.0);
                let prefill_weight = self
                    .rollout_budget_prefill_weight
                    .filter(|weight| weight.is_finite() && *weight >= 0.0);
                RolloutBudgetPlan {
                    limit,
                    reminders,
                    sampling_weight,
                    prefill_weight,
                }
            });
        let current_time_reminder_enabled = self.current_time_reminder
            || self.current_time_reminder_interval.is_some()
            || self.current_time_clock_source.is_some();
        FeaturePlan {
            web_search: self.web_search,
            rollout_budget,
            current_time_reminder_enabled,
            current_time_reminder_interval: self
                .current_time_reminder_interval
                .filter(|interval| *interval > 0),
            current_time_clock_source: self.current_time_clock_source,
            respect_system_proxy: if self.respect_system_proxy {
                Some(true)
            } else if self.no_respect_system_proxy {
                Some(false)
            } else {
                None
            },
        }
    }

    #[cfg(all(test, feature = "mojo-core"))]
    fn to_codex_config_args_rust(&self) -> Vec<OsString> {
        render_plan(self.rust_plan())
    }
}

fn render_plan(plan: FeaturePlan) -> Vec<OsString> {
    let mut overrides = Vec::new();
    if let Some(mode) = plan.web_search {
        overrides.push(format!(
            "web_search={}",
            toml_string_literal(mode.config_value())
        ));
    }
    if let Some(budget) = plan.rollout_budget {
        overrides.extend([
            "features.rollout_budget.enabled=true".to_string(),
            format!("features.rollout_budget.limit_tokens={}", budget.limit),
            format!(
                "features.rollout_budget.reminder_at_remaining_tokens=[{}]",
                budget
                    .reminders
                    .iter()
                    .map(u64::to_string)
                    .collect::<Vec<_>>()
                    .join(",")
            ),
        ]);
        if let Some(weight) = budget.sampling_weight {
            overrides.push(format!(
                "features.rollout_budget.sampling_token_weight={weight}"
            ));
        }
        if let Some(weight) = budget.prefill_weight {
            overrides.push(format!(
                "features.rollout_budget.prefill_token_weight={weight}"
            ));
        }
    }

    if plan.current_time_reminder_enabled {
        overrides.push("features.current_time_reminder.enabled=true".to_string());
        if let Some(interval) = plan.current_time_reminder_interval {
            overrides.push(format!(
                "features.current_time_reminder.reminder_interval_model_requests={interval}"
            ));
        }
        if let Some(source) = plan.current_time_clock_source {
            overrides.push(format!(
                "features.current_time_reminder.clock_source={}",
                toml_string_literal(source.config_value())
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
    args
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

#[cfg(any(not(feature = "mojo-core"), test))]
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

#[cfg(all(test, feature = "mojo-core"))]
mod tests {
    use super::*;

    fn next(state: &mut u64) -> u64 {
        *state ^= *state << 13;
        *state ^= *state >> 7;
        *state ^= *state << 17;
        *state
    }

    #[cfg(feature = "mojo-core")]
    #[test]
    fn mojo_feature_plan_matches_rust_oracle_for_seeded_inputs() {
        let weights = [
            -1.0,
            0.0,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NAN,
            f64::MAX,
        ];
        let mut state = 0x8a5c_3e21_74d9_b60f;
        for case in 0..2_000 {
            let limit = match case % 7 {
                0 => None,
                1 => Some(0),
                2 => Some(1),
                3 => Some(2),
                4 => Some(u64::MAX),
                _ => Some(next(&mut state) | 1),
            };
            let reminder_count = (next(&mut state) % 12) as usize;
            let reminders = (0..reminder_count)
                .map(|_| match next(&mut state) % 5 {
                    0 => 0,
                    1 => limit.unwrap_or_default(),
                    2 => u64::MAX,
                    _ => next(&mut state),
                })
                .collect();
            let args = CodexRuntimeFeatureArgs {
                web_search: match case % 5 {
                    0 => None,
                    1 => Some(CodexWebSearchMode::Disabled),
                    2 => Some(CodexWebSearchMode::Cached),
                    3 => Some(CodexWebSearchMode::Indexed),
                    _ => Some(CodexWebSearchMode::Live),
                },
                rollout_budget_tokens: limit,
                rollout_budget_reminders: reminders,
                rollout_budget_sampling_weight: (case % 3 != 0)
                    .then(|| weights[(next(&mut state) % weights.len() as u64) as usize]),
                rollout_budget_prefill_weight: (case % 4 != 0)
                    .then(|| weights[(next(&mut state) % weights.len() as u64) as usize]),
                current_time_reminder: case % 2 == 0,
                current_time_reminder_interval: match case % 4 {
                    0 => None,
                    1 => Some(0),
                    2 => Some(1),
                    _ => Some(next(&mut state)),
                },
                current_time_clock_source: match case % 3 {
                    0 => None,
                    1 => Some(CodexCurrentTimeClockSource::System),
                    _ => Some(CodexCurrentTimeClockSource::External),
                },
                respect_system_proxy: case % 5 == 0,
                no_respect_system_proxy: case % 7 == 0,
            };
            assert_eq!(
                args.to_codex_config_args()
                    .expect("Mojo plan should validate"),
                args.to_codex_config_args_rust(),
                "generated feature plan {case}"
            );
        }
    }
}
