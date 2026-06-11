use crate::route::runtime_route_kind_to_proxy;
use crate::summary::runtime_quota_summary_to_proxy;
use crate::window::runtime_quota_window_observation;
use prodex_quota::{
    RuntimeQuotaPressureBand, RuntimeQuotaSummary, UsageResponse, scale_quota_pressure_for_plan,
    usage_plan_capacity_pressure_scale_bps,
};
use prodex_runtime_state::RuntimeRouteKind;
use std::cmp::Reverse;

pub type RuntimeQuotaPressureSortKey = (
    RuntimeQuotaPressureBand,
    i64,
    i64,
    i64,
    Reverse<i64>,
    Reverse<i64>,
    Reverse<i64>,
    i64,
    i64,
);

pub fn runtime_quota_pressure_band_to_proxy(
    band: RuntimeQuotaPressureBand,
) -> runtime_proxy::RuntimeSelectionQuotaPressureBand {
    match band {
        RuntimeQuotaPressureBand::Healthy => {
            runtime_proxy::RuntimeSelectionQuotaPressureBand::Healthy
        }
        RuntimeQuotaPressureBand::Thin => runtime_proxy::RuntimeSelectionQuotaPressureBand::Thin,
        RuntimeQuotaPressureBand::Critical => {
            runtime_proxy::RuntimeSelectionQuotaPressureBand::Critical
        }
        RuntimeQuotaPressureBand::Exhausted => {
            runtime_proxy::RuntimeSelectionQuotaPressureBand::Exhausted
        }
        RuntimeQuotaPressureBand::Unknown => {
            runtime_proxy::RuntimeSelectionQuotaPressureBand::Unknown
        }
    }
}

pub fn runtime_quota_pressure_band_from_proxy(
    band: runtime_proxy::RuntimeSelectionQuotaPressureBand,
) -> RuntimeQuotaPressureBand {
    match band {
        runtime_proxy::RuntimeSelectionQuotaPressureBand::Healthy => {
            RuntimeQuotaPressureBand::Healthy
        }
        runtime_proxy::RuntimeSelectionQuotaPressureBand::Thin => RuntimeQuotaPressureBand::Thin,
        runtime_proxy::RuntimeSelectionQuotaPressureBand::Critical => {
            RuntimeQuotaPressureBand::Critical
        }
        runtime_proxy::RuntimeSelectionQuotaPressureBand::Exhausted => {
            RuntimeQuotaPressureBand::Exhausted
        }
        runtime_proxy::RuntimeSelectionQuotaPressureBand::Unknown => {
            RuntimeQuotaPressureBand::Unknown
        }
    }
}

pub fn runtime_quota_sort_key_from_proxy(
    sort_key: runtime_proxy::RuntimeProxyQuotaPressureSortKey,
) -> RuntimeQuotaPressureSortKey {
    let (
        band,
        total_pressure,
        weekly_pressure,
        five_hour_pressure,
        reserve_floor,
        weekly_remaining,
        five_hour_remaining,
        weekly_reset_at,
        five_hour_reset_at,
    ) = sort_key;
    (
        runtime_quota_pressure_band_from_proxy(band),
        total_pressure,
        weekly_pressure,
        five_hour_pressure,
        reserve_floor,
        weekly_remaining,
        five_hour_remaining,
        weekly_reset_at,
        five_hour_reset_at,
    )
}

pub fn runtime_quota_pressure_band_rank(band: RuntimeQuotaPressureBand) -> u8 {
    match band {
        RuntimeQuotaPressureBand::Healthy => 0,
        RuntimeQuotaPressureBand::Thin => 1,
        RuntimeQuotaPressureBand::Critical => 2,
        RuntimeQuotaPressureBand::Exhausted => 3,
        RuntimeQuotaPressureBand::Unknown => 4,
    }
}

pub fn runtime_response_quota_pressure_sort_key_to_proxy(
    sort_key: RuntimeQuotaPressureSortKey,
) -> runtime_proxy::RuntimeResponseQuotaPressureSortKey {
    let (
        band,
        total_pressure,
        weekly_pressure,
        five_hour_pressure,
        reserve_floor,
        weekly_remaining,
        five_hour_remaining,
        weekly_reset_at,
        five_hour_reset_at,
    ) = sort_key;
    (
        runtime_quota_pressure_band_rank(band),
        total_pressure,
        weekly_pressure,
        five_hour_pressure,
        reserve_floor,
        weekly_remaining,
        five_hour_remaining,
        weekly_reset_at,
        five_hour_reset_at,
    )
}

pub fn runtime_quota_pressure_band_reason(band: RuntimeQuotaPressureBand) -> &'static str {
    runtime_proxy::runtime_proxy_quota_pressure_band_reason(runtime_quota_pressure_band_to_proxy(
        band,
    ))
}

pub fn runtime_quota_pressure_sort_key_for_route(
    usage: &UsageResponse,
    route_kind: RuntimeRouteKind,
) -> RuntimeQuotaPressureSortKey {
    let mut sort_key = runtime_quota_sort_key_from_proxy(
        runtime_proxy::runtime_proxy_quota_pressure_sort_key_for_route(
            runtime_quota_window_observation(usage, "5h"),
            runtime_quota_window_observation(usage, "weekly"),
            runtime_route_kind_to_proxy(route_kind),
        ),
    );
    let scale_bps = usage_plan_capacity_pressure_scale_bps(usage);
    sort_key.1 = scale_quota_pressure_for_plan(sort_key.1, scale_bps);
    sort_key.2 = scale_quota_pressure_for_plan(sort_key.2, scale_bps);
    sort_key.3 = scale_quota_pressure_for_plan(sort_key.3, scale_bps);
    sort_key
}

pub fn runtime_quota_pressure_sort_key_for_route_from_summary(
    summary: RuntimeQuotaSummary,
) -> RuntimeQuotaPressureSortKey {
    runtime_quota_sort_key_from_proxy(
        runtime_proxy::runtime_proxy_quota_pressure_sort_key_for_route_from_summary(
            runtime_quota_summary_to_proxy(summary),
        ),
    )
}

pub fn runtime_quota_pressure_band_for_route(
    usage: &UsageResponse,
    route_kind: RuntimeRouteKind,
) -> RuntimeQuotaPressureBand {
    runtime_quota_pressure_band_from_proxy(
        runtime_proxy::runtime_proxy_quota_pressure_band_for_route(
            runtime_quota_window_observation(usage, "5h"),
            runtime_quota_window_observation(usage, "weekly"),
            runtime_route_kind_to_proxy(route_kind),
        ),
    )
}
