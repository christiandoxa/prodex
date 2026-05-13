use crate::pressure::{
    runtime_quota_pressure_band_from_proxy, runtime_quota_pressure_band_reason,
    runtime_quota_pressure_band_to_proxy,
};
use crate::route::runtime_route_kind_to_proxy;
use crate::snapshot::{
    RuntimeProfileUsageSnapshot, runtime_quota_summary_from_usage_snapshot_at,
    runtime_usage_snapshot_is_usable,
};
use crate::source::runtime_quota_source_option_to_proxy;
use crate::window::{
    runtime_quota_window_observation, runtime_quota_window_status_from_proxy,
    runtime_quota_window_status_to_proxy, runtime_quota_window_summary_from_proxy,
    runtime_quota_window_summary_to_proxy, runtime_quota_window_usable_for_auto_rotate,
};
use prodex_quota::{
    RuntimeQuotaPressureBand, RuntimeQuotaSummary, RuntimeQuotaWindowStatus,
    RuntimeQuotaWindowSummary, UsageResponse,
};
use prodex_runtime_state::RuntimeRouteKind;
use prodex_shared_types::RuntimeQuotaSource;

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

pub fn runtime_quota_summary_from_cached_sources(
    live_probe_usage: Option<&UsageResponse>,
    persisted_snapshot: Option<&RuntimeProfileUsageSnapshot>,
    route_kind: RuntimeRouteKind,
    now: i64,
    stale_grace_seconds: i64,
) -> (RuntimeQuotaSummary, Option<RuntimeQuotaSource>) {
    if let Some(usage) = live_probe_usage {
        return (
            runtime_quota_summary_for_route(usage, route_kind),
            Some(RuntimeQuotaSource::LiveProbe),
        );
    }

    if let Some(snapshot) = persisted_snapshot
        && runtime_usage_snapshot_is_usable(snapshot, now, stale_grace_seconds)
    {
        return (
            runtime_quota_summary_from_usage_snapshot_at(snapshot, route_kind, now),
            Some(RuntimeQuotaSource::PersistedSnapshot),
        );
    }

    (
        RuntimeQuotaSummary {
            five_hour: RuntimeQuotaWindowSummary {
                status: RuntimeQuotaWindowStatus::Unknown,
                remaining_percent: 0,
                reset_at: i64::MAX,
            },
            weekly: RuntimeQuotaWindowSummary {
                status: RuntimeQuotaWindowStatus::Unknown,
                remaining_percent: 0,
                reset_at: i64::MAX,
            },
            route_band: RuntimeQuotaPressureBand::Unknown,
        },
        None,
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
