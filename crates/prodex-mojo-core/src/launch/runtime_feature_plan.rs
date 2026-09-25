use super::{boolean, status};
use crate::MojoError;

const ABI_VERSION: i64 = 1;
const OUTPUT_COUNT: usize = 9;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum RuntimeFeatureWebSearchMode {
    Disabled = 0,
    Cached = 1,
    Indexed = 2,
    Live = 3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeFeatureClockSource {
    System,
    External,
}

/// Normalized, non-secret inputs for Codex runtime feature planning.
pub struct RuntimeFeatureConfigInput<'a> {
    pub web_search_mode: Option<RuntimeFeatureWebSearchMode>,
    pub rollout_budget_limit: Option<u64>,
    pub rollout_budget_reminders: &'a [u64],
    pub rollout_budget_sampling_weight: Option<f64>,
    pub rollout_budget_prefill_weight: Option<f64>,
    pub current_time_reminder: bool,
    pub current_time_reminder_interval: Option<u64>,
    pub current_time_clock_source: Option<RuntimeFeatureClockSource>,
    pub respect_system_proxy: bool,
    pub no_respect_system_proxy: bool,
}

/// Validated choices returned by Mojo; original numeric settings remain Rust-owned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeFeatureConfigPlan {
    pub web_search_mode: Option<RuntimeFeatureWebSearchMode>,
    pub rollout_budget_enabled: bool,
    /// Descending, unique thresholds. Empty when rollout-budget settings are disabled.
    pub rollout_budget_reminders: Vec<u64>,
    pub rollout_budget_sampling_weight: bool,
    pub rollout_budget_prefill_weight: bool,
    pub current_time_reminder_enabled: bool,
    pub current_time_reminder_interval: bool,
    pub current_time_clock_source: Option<RuntimeFeatureClockSource>,
    pub respect_system_proxy: Option<bool>,
}

unsafe extern "C" {
    fn prodex_mojo_runtime_feature_plan_v1(
        abi_version: i64,
        fields: u64,
        values: u64,
        weights: u64,
        configured_reminders: u64,
        configured_reminders_count: i64,
        output: u64,
        output_reminders: u64,
        output_reminders_capacity: i64,
    ) -> i64;
}

fn decode_web_search_mode(tag: i64) -> Result<Option<RuntimeFeatureWebSearchMode>, MojoError> {
    match tag {
        -1 => Ok(None),
        0 => Ok(Some(RuntimeFeatureWebSearchMode::Disabled)),
        1 => Ok(Some(RuntimeFeatureWebSearchMode::Cached)),
        2 => Ok(Some(RuntimeFeatureWebSearchMode::Indexed)),
        3 => Ok(Some(RuntimeFeatureWebSearchMode::Live)),
        _ => Err(MojoError::InvalidOutput),
    }
}

fn decode_clock_source(tag: i64) -> Result<Option<RuntimeFeatureClockSource>, MojoError> {
    match tag {
        -1 => Ok(None),
        0 => Ok(Some(RuntimeFeatureClockSource::System)),
        1 => Ok(Some(RuntimeFeatureClockSource::External)),
        _ => Err(MojoError::InvalidOutput),
    }
}

fn decode_optional_bool(tag: i64) -> Result<Option<bool>, MojoError> {
    match tag {
        -1 => Ok(None),
        0 => Ok(Some(false)),
        1 => Ok(Some(true)),
        _ => Err(MojoError::InvalidOutput),
    }
}

fn expected_current_time_enabled(input: &RuntimeFeatureConfigInput<'_>) -> bool {
    input.current_time_reminder
        || input.current_time_reminder_interval.is_some()
        || input.current_time_clock_source.is_some()
}

fn expected_proxy_setting(input: &RuntimeFeatureConfigInput<'_>) -> Option<bool> {
    if input.respect_system_proxy {
        Some(true)
    } else if input.no_respect_system_proxy {
        Some(false)
    } else {
        None
    }
}

fn weight_selection_matches(enabled: bool, rollout_enabled: bool, configured: bool) -> bool {
    !enabled || (rollout_enabled && configured)
}

fn validate_plan_shape(
    input: &RuntimeFeatureConfigInput<'_>,
    plan: &RuntimeFeatureConfigPlan,
) -> Result<(), MojoError> {
    let checks = [
        plan.web_search_mode == input.web_search_mode,
        plan.rollout_budget_enabled == input.rollout_budget_limit.is_some_and(|limit| limit > 1),
        weight_selection_matches(
            plan.rollout_budget_sampling_weight,
            plan.rollout_budget_enabled,
            input.rollout_budget_sampling_weight.is_some(),
        ),
        weight_selection_matches(
            plan.rollout_budget_prefill_weight,
            plan.rollout_budget_enabled,
            input.rollout_budget_prefill_weight.is_some(),
        ),
        plan.current_time_reminder_enabled == expected_current_time_enabled(input),
        plan.current_time_reminder_interval
            == input
                .current_time_reminder_interval
                .is_some_and(|interval| interval > 0),
        plan.current_time_clock_source == input.current_time_clock_source,
        plan.respect_system_proxy == expected_proxy_setting(input),
        plan.rollout_budget_enabled == !plan.rollout_budget_reminders.is_empty(),
    ];
    checks
        .into_iter()
        .all(|matches| matches)
        .then_some(())
        .ok_or(MojoError::InvalidOutput)
}

fn validate_reminders(
    input: &RuntimeFeatureConfigInput<'_>,
    plan: &RuntimeFeatureConfigPlan,
) -> Result<(), MojoError> {
    let Some(limit) = input
        .rollout_budget_limit
        .filter(|_| plan.rollout_budget_enabled)
    else {
        return plan
            .rollout_budget_reminders
            .is_empty()
            .then_some(())
            .ok_or(MojoError::InvalidOutput);
    };

    let each_is_bounded = plan
        .rollout_budget_reminders
        .iter()
        .all(|reminder| *reminder > 0 && *reminder < limit);
    let strictly_descending = plan
        .rollout_budget_reminders
        .windows(2)
        .all(|pair| pair[0] > pair[1]);
    (each_is_bounded && strictly_descending)
        .then_some(())
        .ok_or(MojoError::InvalidOutput)
}

/// Plan Codex runtime-feature overrides using the versioned Mojo kernel.
pub fn plan_runtime_feature_config(
    input: RuntimeFeatureConfigInput<'_>,
) -> Result<RuntimeFeatureConfigPlan, MojoError> {
    let web_search_mode = input.web_search_mode.map_or(-1, |mode| mode as i64);
    let rollout_budget_limit = input.rollout_budget_limit.unwrap_or_default();
    let interval = input.current_time_reminder_interval.unwrap_or_default();
    let clock_source = input
        .current_time_clock_source
        .map_or(-1, |source| match source {
            RuntimeFeatureClockSource::System => 0,
            RuntimeFeatureClockSource::External => 1,
        });
    let fields = [
        web_search_mode,
        i64::from(input.rollout_budget_limit.is_some()),
        i64::from(input.rollout_budget_sampling_weight.is_some()),
        i64::from(input.rollout_budget_prefill_weight.is_some()),
        i64::from(input.current_time_reminder),
        i64::from(input.current_time_reminder_interval.is_some()),
        clock_source,
        i64::from(input.respect_system_proxy),
        i64::from(input.no_respect_system_proxy),
    ];
    let values = [rollout_budget_limit, interval];
    let weights = [
        input.rollout_budget_sampling_weight.unwrap_or_default(),
        input.rollout_budget_prefill_weight.unwrap_or_default(),
    ];
    let configured_count =
        i64::try_from(input.rollout_budget_reminders.len()).map_err(|_| MojoError::InvalidInput)?;
    let reminders_capacity = input.rollout_budget_reminders.len().max(3);
    let reminders_capacity_i64 =
        i64::try_from(reminders_capacity).map_err(|_| MojoError::InvalidInput)?;
    let mut reminders = vec![0_u64; reminders_capacity];
    let mut output = [0_i64; OUTPUT_COUNT];

    // SAFETY: input arrays and distinct initialized output buffers remain live
    // for this synchronous call. The Mojo result and every returned tag are checked.
    status(unsafe {
        prodex_mojo_runtime_feature_plan_v1(
            ABI_VERSION,
            fields.as_ptr() as u64,
            values.as_ptr() as u64,
            weights.as_ptr() as u64,
            input.rollout_budget_reminders.as_ptr() as u64,
            configured_count,
            output.as_mut_ptr() as u64,
            reminders.as_mut_ptr() as u64,
            reminders_capacity_i64,
        )
    })?;

    let reminder_count = usize::try_from(output[2]).map_err(|_| MojoError::InvalidOutput)?;
    if reminder_count > reminders.len() {
        return Err(MojoError::InvalidOutput);
    }
    reminders.truncate(reminder_count);

    let plan = RuntimeFeatureConfigPlan {
        web_search_mode: decode_web_search_mode(output[0])?,
        rollout_budget_enabled: boolean(output[1])?,
        rollout_budget_reminders: reminders,
        rollout_budget_sampling_weight: boolean(output[3])?,
        rollout_budget_prefill_weight: boolean(output[4])?,
        current_time_reminder_enabled: boolean(output[5])?,
        current_time_reminder_interval: boolean(output[6])?,
        current_time_clock_source: decode_clock_source(output[7])?,
        respect_system_proxy: decode_optional_bool(output[8])?,
    };
    validate_plan_shape(&input, &plan)?;
    validate_reminders(&input, &plan)?;
    Ok(plan)
}
