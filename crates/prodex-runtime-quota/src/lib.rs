use chrono::Local;
use prodex_quota::{
    RuntimeQuotaPressureBand, RuntimeQuotaSummary, RuntimeQuotaWindowStatus,
    RuntimeQuotaWindowSummary, UsageResponse, find_main_window, remaining_percent,
};
use prodex_runtime_state::{
    RuntimeProfileUsageSnapshot as RuntimeProfileUsageSnapshotGeneric, RuntimeRouteKind,
};
use prodex_shared_types::RuntimeQuotaSource;
use std::cmp::Reverse;

pub type RuntimeProfileUsageSnapshot = RuntimeProfileUsageSnapshotGeneric<RuntimeQuotaWindowStatus>;

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

pub fn runtime_route_kind_to_proxy(
    route_kind: RuntimeRouteKind,
) -> runtime_proxy::RuntimeRouteKind {
    match route_kind {
        RuntimeRouteKind::Responses => runtime_proxy::RuntimeRouteKind::Responses,
        RuntimeRouteKind::Compact => runtime_proxy::RuntimeRouteKind::Compact,
        RuntimeRouteKind::Websocket => runtime_proxy::RuntimeRouteKind::Websocket,
        RuntimeRouteKind::Standard => runtime_proxy::RuntimeRouteKind::Standard,
    }
}

pub fn runtime_quota_source_to_proxy(
    source: RuntimeQuotaSource,
) -> runtime_proxy::RuntimeSelectionQuotaSource {
    match source {
        RuntimeQuotaSource::LiveProbe => runtime_proxy::RuntimeSelectionQuotaSource::LiveProbe,
        RuntimeQuotaSource::PersistedSnapshot => {
            runtime_proxy::RuntimeSelectionQuotaSource::PersistedSnapshot
        }
    }
}

pub fn runtime_quota_source_option_to_proxy(
    source: Option<RuntimeQuotaSource>,
) -> Option<runtime_proxy::RuntimeSelectionQuotaSource> {
    source.map(runtime_quota_source_to_proxy)
}

pub fn runtime_quota_source_from_proxy(
    source: runtime_proxy::RuntimeSelectionQuotaSource,
) -> RuntimeQuotaSource {
    match source {
        runtime_proxy::RuntimeSelectionQuotaSource::LiveProbe => RuntimeQuotaSource::LiveProbe,
        runtime_proxy::RuntimeSelectionQuotaSource::PersistedSnapshot => {
            RuntimeQuotaSource::PersistedSnapshot
        }
    }
}

pub fn runtime_quota_window_status_to_proxy(
    status: RuntimeQuotaWindowStatus,
) -> runtime_proxy::RuntimeSelectionQuotaWindowStatus {
    match status {
        RuntimeQuotaWindowStatus::Ready => runtime_proxy::RuntimeSelectionQuotaWindowStatus::Ready,
        RuntimeQuotaWindowStatus::Thin => runtime_proxy::RuntimeSelectionQuotaWindowStatus::Thin,
        RuntimeQuotaWindowStatus::Critical => {
            runtime_proxy::RuntimeSelectionQuotaWindowStatus::Critical
        }
        RuntimeQuotaWindowStatus::Exhausted => {
            runtime_proxy::RuntimeSelectionQuotaWindowStatus::Exhausted
        }
        RuntimeQuotaWindowStatus::Unknown => {
            runtime_proxy::RuntimeSelectionQuotaWindowStatus::Unknown
        }
    }
}

pub fn runtime_quota_window_status_from_proxy(
    status: runtime_proxy::RuntimeSelectionQuotaWindowStatus,
) -> RuntimeQuotaWindowStatus {
    match status {
        runtime_proxy::RuntimeSelectionQuotaWindowStatus::Ready => RuntimeQuotaWindowStatus::Ready,
        runtime_proxy::RuntimeSelectionQuotaWindowStatus::Thin => RuntimeQuotaWindowStatus::Thin,
        runtime_proxy::RuntimeSelectionQuotaWindowStatus::Critical => {
            RuntimeQuotaWindowStatus::Critical
        }
        runtime_proxy::RuntimeSelectionQuotaWindowStatus::Exhausted => {
            RuntimeQuotaWindowStatus::Exhausted
        }
        runtime_proxy::RuntimeSelectionQuotaWindowStatus::Unknown => {
            RuntimeQuotaWindowStatus::Unknown
        }
    }
}

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

pub fn runtime_quota_window_summary_to_proxy(
    window: RuntimeQuotaWindowSummary,
) -> runtime_proxy::RuntimeProxyQuotaWindowSummary {
    runtime_proxy::RuntimeProxyQuotaWindowSummary {
        status: runtime_quota_window_status_to_proxy(window.status),
        remaining_percent: window.remaining_percent,
        reset_at: window.reset_at,
    }
}

pub fn runtime_quota_window_summary_from_proxy(
    window: runtime_proxy::RuntimeProxyQuotaWindowSummary,
) -> RuntimeQuotaWindowSummary {
    RuntimeQuotaWindowSummary {
        status: runtime_quota_window_status_from_proxy(window.status),
        remaining_percent: window.remaining_percent,
        reset_at: window.reset_at,
    }
}

pub fn runtime_quota_summary_to_proxy(
    summary: RuntimeQuotaSummary,
) -> runtime_proxy::RuntimeProxyQuotaSummary {
    runtime_proxy::RuntimeProxyQuotaSummary {
        five_hour: runtime_quota_window_summary_to_proxy(summary.five_hour),
        weekly: runtime_quota_window_summary_to_proxy(summary.weekly),
        route_band: runtime_quota_pressure_band_to_proxy(summary.route_band),
    }
}

pub fn runtime_quota_summary_from_proxy(
    summary: runtime_proxy::RuntimeProxyQuotaSummary,
) -> RuntimeQuotaSummary {
    RuntimeQuotaSummary {
        five_hour: runtime_quota_window_summary_from_proxy(summary.five_hour),
        weekly: runtime_quota_window_summary_from_proxy(summary.weekly),
        route_band: runtime_quota_pressure_band_from_proxy(summary.route_band),
    }
}

pub fn runtime_selection_quota_summary_to_proxy(
    summary: RuntimeQuotaSummary,
) -> runtime_proxy::RuntimeSelectionQuotaSummary {
    runtime_proxy::RuntimeSelectionQuotaSummary {
        five_hour: runtime_proxy::RuntimeSelectionQuotaWindowSummary {
            status: runtime_quota_window_status_to_proxy(summary.five_hour.status),
            remaining_percent: summary.five_hour.remaining_percent,
        },
        weekly: runtime_proxy::RuntimeSelectionQuotaWindowSummary {
            status: runtime_quota_window_status_to_proxy(summary.weekly.status),
            remaining_percent: summary.weekly.remaining_percent,
        },
        route_band: runtime_quota_pressure_band_to_proxy(summary.route_band),
    }
}

pub fn runtime_selection_quota_summary_from_proxy(
    summary: runtime_proxy::RuntimeSelectionQuotaSummary,
) -> RuntimeQuotaSummary {
    RuntimeQuotaSummary {
        five_hour: RuntimeQuotaWindowSummary {
            status: runtime_quota_window_status_from_proxy(summary.five_hour.status),
            remaining_percent: summary.five_hour.remaining_percent,
            reset_at: i64::MAX,
        },
        weekly: RuntimeQuotaWindowSummary {
            status: runtime_quota_window_status_from_proxy(summary.weekly.status),
            remaining_percent: summary.weekly.remaining_percent,
            reset_at: i64::MAX,
        },
        route_band: runtime_quota_pressure_band_from_proxy(summary.route_band),
    }
}

pub fn runtime_usage_snapshot_to_proxy(
    snapshot: &RuntimeProfileUsageSnapshot,
) -> runtime_proxy::RuntimeProxyUsageSnapshot {
    runtime_proxy::RuntimeProxyUsageSnapshot {
        checked_at: snapshot.checked_at,
        five_hour_status: runtime_quota_window_status_to_proxy(snapshot.five_hour_status),
        five_hour_remaining_percent: snapshot.five_hour_remaining_percent,
        five_hour_reset_at: snapshot.five_hour_reset_at,
        weekly_status: runtime_quota_window_status_to_proxy(snapshot.weekly_status),
        weekly_remaining_percent: snapshot.weekly_remaining_percent,
        weekly_reset_at: snapshot.weekly_reset_at,
    }
}

pub fn runtime_usage_snapshot_from_proxy(
    snapshot: runtime_proxy::RuntimeProxyUsageSnapshot,
) -> RuntimeProfileUsageSnapshot {
    RuntimeProfileUsageSnapshot {
        checked_at: snapshot.checked_at,
        five_hour_status: runtime_quota_window_status_from_proxy(snapshot.five_hour_status),
        five_hour_remaining_percent: snapshot.five_hour_remaining_percent,
        five_hour_reset_at: snapshot.five_hour_reset_at,
        weekly_status: runtime_quota_window_status_from_proxy(snapshot.weekly_status),
        weekly_remaining_percent: snapshot.weekly_remaining_percent,
        weekly_reset_at: snapshot.weekly_reset_at,
    }
}

pub fn runtime_quota_window_observation(
    usage: &UsageResponse,
    label: &str,
) -> Option<runtime_proxy::RuntimeProxyQuotaWindowObservation> {
    runtime_quota_window_observation_at(usage, label, Local::now().timestamp())
}

pub fn runtime_quota_window_observation_at(
    usage: &UsageResponse,
    label: &str,
    now: i64,
) -> Option<runtime_proxy::RuntimeProxyQuotaWindowObservation> {
    let window = find_main_window(usage.rate_limit.as_ref()?, label)?;
    let remaining_percent = remaining_percent(window.used_percent);
    let reset_at = window.reset_at.unwrap_or(i64::MAX);
    let seconds_until_reset = if reset_at == i64::MAX {
        i64::MAX
    } else {
        (reset_at - now).max(0)
    };
    let pressure_score = seconds_until_reset
        .saturating_mul(1_000)
        .checked_div(remaining_percent.max(1))
        .unwrap_or(i64::MAX);

    Some(runtime_proxy::RuntimeProxyQuotaWindowObservation {
        remaining_percent,
        reset_at,
        pressure_score,
    })
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

pub fn runtime_quota_window_status_reason(status: RuntimeQuotaWindowStatus) -> &'static str {
    runtime_proxy::runtime_proxy_quota_window_status_reason(runtime_quota_window_status_to_proxy(
        status,
    ))
}

pub fn runtime_quota_source_label(source: RuntimeQuotaSource) -> &'static str {
    runtime_proxy::runtime_selection_quota_source_label(runtime_quota_source_to_proxy(source))
}

pub fn runtime_quota_window_summary(
    observation: Option<runtime_proxy::RuntimeProxyQuotaWindowObservation>,
) -> RuntimeQuotaWindowSummary {
    runtime_quota_window_summary_from_proxy(runtime_proxy::runtime_proxy_quota_window_summary(
        observation,
    ))
}

pub fn runtime_quota_summary_for_route(
    usage: &UsageResponse,
    route_kind: RuntimeRouteKind,
) -> RuntimeQuotaSummary {
    runtime_quota_summary_from_proxy(runtime_proxy::runtime_proxy_quota_summary_for_route(
        runtime_quota_window_observation(usage, "5h"),
        runtime_quota_window_observation(usage, "weekly"),
        runtime_route_kind_to_proxy(route_kind),
    ))
}

pub fn runtime_quota_summary_blocking_reset_at(
    summary: RuntimeQuotaSummary,
    route_kind: RuntimeRouteKind,
    responses_critical_floor_percent: i64,
) -> Option<i64> {
    runtime_proxy::runtime_proxy_quota_summary_blocking_reset_at(
        runtime_quota_summary_to_proxy(summary),
        runtime_route_kind_to_proxy(route_kind),
        responses_critical_floor_percent,
    )
}

pub fn runtime_profile_usage_snapshot_from_usage(
    usage: &UsageResponse,
) -> RuntimeProfileUsageSnapshot {
    runtime_usage_snapshot_from_proxy(
        runtime_proxy::runtime_proxy_usage_snapshot_from_observations_at(
            runtime_quota_window_observation(usage, "5h"),
            runtime_quota_window_observation(usage, "weekly"),
            Local::now().timestamp(),
        ),
    )
}

pub fn runtime_quota_summary_from_usage_snapshot(
    snapshot: &RuntimeProfileUsageSnapshot,
    route_kind: RuntimeRouteKind,
) -> RuntimeQuotaSummary {
    runtime_quota_summary_from_usage_snapshot_at(snapshot, route_kind, Local::now().timestamp())
}

pub fn runtime_quota_summary_from_usage_snapshot_at(
    snapshot: &RuntimeProfileUsageSnapshot,
    route_kind: RuntimeRouteKind,
    now: i64,
) -> RuntimeQuotaSummary {
    runtime_quota_summary_from_proxy(
        runtime_proxy::runtime_proxy_quota_summary_from_usage_snapshot_at(
            runtime_usage_snapshot_to_proxy(snapshot),
            runtime_route_kind_to_proxy(route_kind),
            now,
        ),
    )
}

pub fn runtime_quota_window_summary_from_usage_snapshot_at(
    status: RuntimeQuotaWindowStatus,
    remaining_percent: i64,
    reset_at: i64,
    now: i64,
) -> RuntimeQuotaWindowSummary {
    runtime_quota_window_summary_from_proxy(
        runtime_proxy::runtime_proxy_quota_window_summary_from_usage_snapshot_at(
            runtime_quota_window_status_to_proxy(status),
            remaining_percent,
            reset_at,
            now,
        ),
    )
}

pub fn runtime_profile_usage_snapshot_hold_active(
    snapshot: &RuntimeProfileUsageSnapshot,
    now: i64,
) -> bool {
    runtime_proxy::runtime_proxy_usage_snapshot_hold_active(
        runtime_usage_snapshot_to_proxy(snapshot),
        now,
    )
}

pub fn runtime_profile_usage_snapshot_hold_expired(
    snapshot: &RuntimeProfileUsageSnapshot,
    now: i64,
) -> bool {
    runtime_proxy::runtime_proxy_usage_snapshot_hold_expired(
        runtime_usage_snapshot_to_proxy(snapshot),
        now,
    )
}

pub fn runtime_usage_snapshot_is_usable(
    snapshot: &RuntimeProfileUsageSnapshot,
    now: i64,
    stale_grace_seconds: i64,
) -> bool {
    runtime_proxy::runtime_proxy_usage_snapshot_is_usable(
        runtime_usage_snapshot_to_proxy(snapshot),
        now,
        stale_grace_seconds,
    )
}

pub fn runtime_quota_pressure_sort_key_for_route(
    usage: &UsageResponse,
    route_kind: RuntimeRouteKind,
) -> RuntimeQuotaPressureSortKey {
    runtime_quota_sort_key_from_proxy(
        runtime_proxy::runtime_proxy_quota_pressure_sort_key_for_route(
            runtime_quota_window_observation(usage, "5h"),
            runtime_quota_window_observation(usage, "weekly"),
            runtime_route_kind_to_proxy(route_kind),
        ),
    )
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

pub fn runtime_quota_summary_requires_precommit_live_probe(
    summary: RuntimeQuotaSummary,
    source: Option<RuntimeQuotaSource>,
    route_kind: RuntimeRouteKind,
) -> bool {
    runtime_proxy::runtime_proxy_quota_summary_requires_precommit_live_probe(
        runtime_quota_summary_to_proxy(summary),
        runtime_quota_source_option_to_proxy(source),
        runtime_route_kind_to_proxy(route_kind),
    )
}

pub fn runtime_quota_summary_requires_live_source_after_probe(
    summary: RuntimeQuotaSummary,
    source: Option<RuntimeQuotaSource>,
    route_kind: RuntimeRouteKind,
) -> bool {
    runtime_proxy::runtime_proxy_quota_summary_requires_live_source_after_probe(
        runtime_quota_summary_to_proxy(summary),
        runtime_quota_source_option_to_proxy(source),
        runtime_route_kind_to_proxy(route_kind),
    )
}

pub fn runtime_precommit_quota_block_reason(
    summary: RuntimeQuotaSummary,
    route_kind: RuntimeRouteKind,
    responses_critical_floor_percent: i64,
) -> Option<runtime_proxy::RuntimePrecommitQuotaBlockReason> {
    runtime_proxy::runtime_proxy_precommit_quota_block_reason(
        runtime_quota_summary_to_proxy(summary),
        runtime_route_kind_to_proxy(route_kind),
        responses_critical_floor_percent,
    )
}

pub fn runtime_precommit_quota_gate_initial_decision(
    summary: RuntimeQuotaSummary,
    source: Option<RuntimeQuotaSource>,
    route_kind: RuntimeRouteKind,
    has_continuation_context: bool,
    responses_critical_floor_percent: i64,
) -> runtime_proxy::RuntimeProxyPrecommitQuotaGateInitialDecision {
    runtime_proxy::runtime_proxy_precommit_quota_gate_initial_decision(
        runtime_proxy::RuntimeProxyPrecommitQuotaGateInitialInput {
            summary: runtime_quota_summary_to_proxy(summary),
            source: runtime_quota_source_option_to_proxy(source),
            route_kind: runtime_route_kind_to_proxy(route_kind),
            has_continuation_context,
            responses_critical_floor_percent,
        },
    )
}

pub fn runtime_precommit_quota_gate_final_decision(
    summary: RuntimeQuotaSummary,
    source: Option<RuntimeQuotaSource>,
    route_kind: RuntimeRouteKind,
    has_alternative_quota_profile: bool,
    responses_critical_floor_percent: i64,
) -> runtime_proxy::RuntimeProxyPrecommitQuotaGateFinalDecision {
    runtime_proxy::runtime_proxy_precommit_quota_gate_final_decision(
        runtime_proxy::RuntimeProxyPrecommitQuotaGateFinalInput {
            summary: runtime_quota_summary_to_proxy(summary),
            source: runtime_quota_source_option_to_proxy(source),
            route_kind: runtime_route_kind_to_proxy(route_kind),
            has_alternative_quota_profile,
            responses_critical_floor_percent,
        },
    )
}

pub fn runtime_quota_precommit_guard_reason(
    summary: RuntimeQuotaSummary,
    route_kind: RuntimeRouteKind,
    responses_critical_floor_percent: i64,
) -> Option<&'static str> {
    runtime_precommit_quota_block_reason(summary, route_kind, responses_critical_floor_percent)
        .map(runtime_proxy::RuntimePrecommitQuotaBlockReason::as_str)
}

pub fn runtime_quota_window_usable_for_auto_rotate(status: RuntimeQuotaWindowStatus) -> bool {
    matches!(
        status,
        RuntimeQuotaWindowStatus::Ready
            | RuntimeQuotaWindowStatus::Thin
            | RuntimeQuotaWindowStatus::Critical
    )
}

pub fn runtime_quota_summary_allows_soft_affinity(
    summary: RuntimeQuotaSummary,
    source: Option<RuntimeQuotaSource>,
    route_kind: RuntimeRouteKind,
    responses_critical_floor_percent: i64,
) -> bool {
    source.is_some()
        && runtime_quota_window_usable_for_auto_rotate(summary.five_hour.status)
        && runtime_quota_window_usable_for_auto_rotate(summary.weekly.status)
        && runtime_quota_precommit_guard_reason(
            summary,
            route_kind,
            responses_critical_floor_percent,
        )
        .is_none()
}

pub fn runtime_quota_soft_affinity_rejection_reason(
    summary: RuntimeQuotaSummary,
    source: Option<RuntimeQuotaSource>,
    route_kind: RuntimeRouteKind,
    responses_critical_floor_percent: i64,
) -> &'static str {
    if source.is_none()
        || matches!(summary.five_hour.status, RuntimeQuotaWindowStatus::Unknown)
        || matches!(summary.weekly.status, RuntimeQuotaWindowStatus::Unknown)
    {
        "quota_windows_unavailable"
    } else if let Some(reason) =
        runtime_quota_precommit_guard_reason(summary, route_kind, responses_critical_floor_percent)
    {
        reason
    } else if matches!(
        summary.five_hour.status,
        RuntimeQuotaWindowStatus::Exhausted
    ) || matches!(summary.weekly.status, RuntimeQuotaWindowStatus::Exhausted)
    {
        "quota_exhausted"
    } else {
        runtime_quota_pressure_band_reason(summary.route_band)
    }
}

pub fn runtime_snapshot_blocks_same_request_cold_start_probe(
    snapshot: &RuntimeProfileUsageSnapshot,
    route_kind: RuntimeRouteKind,
    now: i64,
    stale_grace_seconds: i64,
    responses_critical_floor_percent: i64,
) -> bool {
    matches!(
        route_kind,
        RuntimeRouteKind::Responses | RuntimeRouteKind::Websocket
    ) && runtime_usage_snapshot_is_usable(snapshot, now, stale_grace_seconds)
        && runtime_quota_precommit_guard_reason(
            runtime_quota_summary_from_usage_snapshot_at(snapshot, route_kind, now),
            route_kind,
            responses_critical_floor_percent,
        )
        .is_some()
}

#[cfg(test)]
#[path = "../tests/src/lib.rs"]
mod tests;
