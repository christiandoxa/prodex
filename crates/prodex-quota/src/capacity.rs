use crate::AdditionalRateLimit;
#[cfg(feature = "mojo")]
use crate::{UsageResponse, WindowPair, find_main_window};

/// Checks explicit backend admission state plus the bucket's own windows.
pub fn additional_rate_limit_is_usable(additional: &AdditionalRateLimit) -> bool {
    #[cfg(feature = "mojo")]
    {
        classify_additional_rate_limit_is_usable(additional)
    }

    #[cfg(not(feature = "mojo"))]
    {
        let _ = additional;
        false
    }
}

#[cfg(feature = "mojo")]
use prodex_runtime_state::RuntimeRouteKind;

#[cfg(feature = "mojo")]
#[derive(Debug, Clone, Copy)]
pub(crate) struct QuotaCapacityCandidate<'a> {
    pub pair: &'a WindowPair,
    pub output: prodex_mojo_core::quota::QuotaCapacityOutput,
}

#[cfg(feature = "mojo")]
pub(crate) fn quota_capacity_candidates_for_usage_at(
    usage: &UsageResponse,
    route_kind: RuntimeRouteKind,
    now: i64,
) -> Result<Vec<QuotaCapacityCandidate<'_>>, prodex_mojo_core::MojoError> {
    let scale_bps = crate::usage_plan_capacity_pressure_scale_bps(usage);
    let mut pairs = Vec::with_capacity(usage.additional_rate_limits.len() + 1);
    let mut inputs = Vec::with_capacity(usage.additional_rate_limits.len() + 1);

    if let Some(pair) = usage.rate_limit.as_ref() {
        pairs.push(pair);
        inputs.push(quota_capacity_input_for_pair(
            pair,
            prodex_mojo_core::quota::QUOTA_CAPACITY_LANE_MAIN,
            None,
            None,
            scale_bps,
            now,
        ));
    }
    for additional in &usage.additional_rate_limits {
        pairs.push(&additional.rate_limit);
        inputs.push(quota_capacity_input_for_pair(
            &additional.rate_limit,
            prodex_mojo_core::quota::QUOTA_CAPACITY_LANE_UNKNOWN_ADDITIONAL,
            additional.allowed,
            additional.limit_reached,
            scale_bps,
            now,
        ));
    }

    let outputs = crate::mojo::quota_capacity_batch(&inputs, route_kind_code(route_kind))?;
    Ok(pairs
        .into_iter()
        .zip(outputs)
        .map(|(pair, output)| QuotaCapacityCandidate { pair, output })
        .collect())
}

#[cfg(feature = "mojo")]
pub(crate) fn classify_additional_rate_limit_is_usable(additional: &AdditionalRateLimit) -> bool {
    let input = quota_capacity_input_for_pair(
        &additional.rate_limit,
        prodex_mojo_core::quota::QUOTA_CAPACITY_LANE_UNKNOWN_ADDITIONAL,
        additional.allowed,
        additional.limit_reached,
        10_000,
        0,
    );
    quota_capacity_output(input, RuntimeRouteKind::Standard)
        .expect("Mojo quota capacity classification failed")
        .usable
}

#[cfg(feature = "mojo")]
pub(crate) fn quota_capacity_for_window_pair(
    pair: &WindowPair,
) -> Result<prodex_mojo_core::quota::QuotaCapacityOutput, prodex_mojo_core::MojoError> {
    let input = quota_capacity_input_for_pair(
        pair,
        prodex_mojo_core::quota::QUOTA_CAPACITY_LANE_MAIN,
        None,
        None,
        10_000,
        0,
    );
    quota_capacity_output(input, RuntimeRouteKind::Standard)
}

#[cfg(feature = "mojo")]
fn quota_capacity_output(
    input: prodex_mojo_core::quota::QuotaCapacityInput,
    route_kind: RuntimeRouteKind,
) -> Result<prodex_mojo_core::quota::QuotaCapacityOutput, prodex_mojo_core::MojoError> {
    crate::mojo::quota_capacity_batch(&[input], route_kind_code(route_kind))?
        .into_iter()
        .next()
        .ok_or(prodex_mojo_core::MojoError::InvalidOutput)
}

#[cfg(feature = "mojo")]
fn quota_capacity_input_for_pair(
    pair: &WindowPair,
    lane: i64,
    allowed: Option<bool>,
    limit_reached: Option<bool>,
    scale_bps: i64,
    now: i64,
) -> prodex_mojo_core::quota::QuotaCapacityInput {
    let (five_hour_used_percent, five_hour_has_value, five_hour_reset_at) =
        quota_capacity_window_input(pair, "5h");
    let (weekly_used_percent, weekly_has_value, weekly_reset_at) =
        quota_capacity_window_input(pair, "weekly");
    let primary_used_percent = pair
        .primary_window
        .as_ref()
        .and_then(|window| window.used_percent);
    let secondary_used_percent = pair
        .secondary_window
        .as_ref()
        .and_then(|window| window.used_percent);
    prodex_mojo_core::quota::QuotaCapacityInput {
        lane,
        pair_allowed: pair.allowed,
        outer_allowed: allowed,
        pair_limit_reached: pair.limit_reached,
        outer_limit_reached: limit_reached,
        rate_limit_reached_type: admission_value(&pair.extra, "rate_limit_reached_type"),
        camel_rate_limit_reached_type: admission_value(&pair.extra, "rateLimitReachedType"),
        spend_control_reached: admission_value(&pair.extra, "spend_control_reached"),
        camel_spend_control_reached: admission_value(&pair.extra, "spendControlReached"),
        ordinary_usage_allowed: admission_value(&pair.extra, "ordinaryUsageAllowed"),
        five_hour_used_percent,
        five_hour_has_value,
        five_hour_reset_at,
        weekly_used_percent,
        weekly_has_value,
        weekly_reset_at,
        primary_used_percent: primary_used_percent.unwrap_or_default(),
        primary_has_value: primary_used_percent.is_some(),
        secondary_used_percent: secondary_used_percent.unwrap_or_default(),
        secondary_has_value: secondary_used_percent.is_some(),
        scale_bps,
        now,
    }
}

#[cfg(feature = "mojo")]
fn quota_capacity_window_input(pair: &WindowPair, label: &str) -> (i64, bool, i64) {
    let window = find_main_window(pair, label);
    let used_percent = window.and_then(|window| window.used_percent);
    let reset_at = window.and_then(|window| window.reset_at);
    (
        used_percent.unwrap_or_default(),
        used_percent.is_some(),
        reset_at.unwrap_or(i64::MAX),
    )
}

#[cfg(feature = "mojo")]
fn admission_value(
    extra: &std::collections::BTreeMap<String, serde_json::Value>,
    key: &str,
) -> prodex_mojo_core::quota::QuotaAdmissionValue {
    match extra.get(key) {
        None => prodex_mojo_core::quota::QuotaAdmissionValue::Missing,
        Some(serde_json::Value::Null) => prodex_mojo_core::quota::QuotaAdmissionValue::Null,
        Some(serde_json::Value::Bool(true)) => prodex_mojo_core::quota::QuotaAdmissionValue::True,
        Some(serde_json::Value::Bool(false)) => prodex_mojo_core::quota::QuotaAdmissionValue::False,
        Some(_) => prodex_mojo_core::quota::QuotaAdmissionValue::Other,
    }
}

#[cfg(feature = "mojo")]
fn route_kind_code(route_kind: RuntimeRouteKind) -> i64 {
    match route_kind {
        RuntimeRouteKind::Responses => 0,
        RuntimeRouteKind::Compact => 1,
        RuntimeRouteKind::Websocket => 2,
        RuntimeRouteKind::Standard => 3,
    }
}
