use crate::AdditionalRateLimit;
#[cfg(feature = "mojo")]
use crate::{UsageResponse, WindowPair, find_main_window};

pub(crate) fn additional_rate_limit_is_spark(additional: &AdditionalRateLimit) -> bool {
    [
        additional.limit_name.as_deref(),
        additional.metered_feature.as_deref(),
    ]
    .into_iter()
    .flatten()
    .any(|value| {
        let normalized = value.to_ascii_lowercase();
        normalized.contains("spark")
    })
}

/// Checks explicit backend admission state plus the bucket's own windows.
pub fn additional_rate_limit_is_usable(additional: &AdditionalRateLimit) -> bool {
    #[cfg(feature = "mojo")]
    {
        classify_additional_rate_limit_is_usable(additional)
    }

    #[cfg(not(feature = "mojo"))]
    {
        additional.allowed != Some(false)
            && additional.limit_reached != Some(true)
            && crate::render::window_pair_has_ready_limit(&additional.rate_limit)
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
            route_kind,
            now,
        ));
    }
    for additional in &usage.additional_rate_limits {
        pairs.push(&additional.rate_limit);
        inputs.push(quota_capacity_input_for_pair(
            &additional.rate_limit,
            if additional_rate_limit_is_spark(additional) {
                prodex_mojo_core::quota::QUOTA_CAPACITY_LANE_SPARK
            } else {
                prodex_mojo_core::quota::QUOTA_CAPACITY_LANE_UNKNOWN_ADDITIONAL
            },
            additional.allowed,
            additional.limit_reached,
            scale_bps,
            route_kind,
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
        if additional_rate_limit_is_spark(additional) {
            prodex_mojo_core::quota::QUOTA_CAPACITY_LANE_SPARK
        } else {
            prodex_mojo_core::quota::QUOTA_CAPACITY_LANE_UNKNOWN_ADDITIONAL
        },
        additional.allowed,
        additional.limit_reached,
        10_000,
        RuntimeRouteKind::Standard,
        0,
    );
    crate::mojo::quota_capacity_batch(&[input], 3)
        .expect("Mojo quota capacity classification failed")
        .into_iter()
        .next()
        .is_some_and(|output| output.usable)
}

#[cfg(feature = "mojo")]
fn quota_capacity_input_for_pair(
    pair: &WindowPair,
    lane: i64,
    allowed: Option<bool>,
    limit_reached: Option<bool>,
    scale_bps: i64,
    route_kind: RuntimeRouteKind,
    now: i64,
) -> prodex_mojo_core::quota::QuotaCapacityInput {
    let (five_hour_used_percent, five_hour_has_value, five_hour_seconds_until_reset) =
        quota_capacity_window_input(pair, "5h", now);
    let (weekly_used_percent, weekly_has_value, weekly_seconds_until_reset) =
        quota_capacity_window_input(pair, "weekly", now);
    prodex_mojo_core::quota::QuotaCapacityInput {
        lane,
        allowed: admission_allowed_tag(pair.allowed, allowed),
        limit_reached: limit_reached_tag(pair.limit_reached, limit_reached),
        five_hour_used_percent,
        five_hour_has_value,
        five_hour_seconds_until_reset,
        weekly_used_percent,
        weekly_has_value,
        weekly_seconds_until_reset,
        scale_bps,
        weekly_weight: route_weekly_weight(route_kind),
    }
}

#[cfg(feature = "mojo")]
fn quota_capacity_window_input(pair: &WindowPair, label: &str, now: i64) -> (i64, bool, i64) {
    let window = find_main_window(pair, label);
    let used_percent = window.and_then(|window| window.used_percent);
    let reset_at = window.and_then(|window| window.reset_at);
    (
        used_percent.unwrap_or_default(),
        used_percent.is_some(),
        reset_at.map_or(i64::MAX, |reset_at| {
            if reset_at == i64::MAX {
                i64::MAX
            } else {
                reset_at.saturating_sub(now).max(0)
            }
        }),
    )
}

#[cfg(feature = "mojo")]
fn admission_allowed_tag(pair: Option<bool>, outer: Option<bool>) -> i64 {
    if pair == Some(false) || outer == Some(false) {
        2
    } else if pair == Some(true) || outer == Some(true) {
        1
    } else {
        0
    }
}

#[cfg(feature = "mojo")]
fn limit_reached_tag(pair: Option<bool>, outer: Option<bool>) -> i64 {
    if pair == Some(true) || outer == Some(true) {
        2
    } else if pair == Some(false) || outer == Some(false) {
        1
    } else {
        0
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

#[cfg(feature = "mojo")]
fn route_weekly_weight(route_kind: RuntimeRouteKind) -> i64 {
    match route_kind {
        RuntimeRouteKind::Responses | RuntimeRouteKind::Websocket => 10,
        RuntimeRouteKind::Compact | RuntimeRouteKind::Standard => 8,
    }
}
