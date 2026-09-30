use crate::pressure::{
    runtime_quota_pressure_band_from_proxy, runtime_quota_pressure_band_reason,
    runtime_quota_pressure_band_to_proxy,
};
use crate::snapshot::{
    RuntimeProfileUsageSnapshot, runtime_quota_summary_from_usage_snapshot_at,
    runtime_usage_snapshot_is_usable, usage_from_runtime_usage_snapshot,
};
use crate::source::runtime_quota_source_option_to_proxy;
use crate::window::{
    runtime_quota_window_observation, runtime_quota_window_observation_for_model_at,
    runtime_quota_window_status_from_proxy, runtime_quota_window_status_reason,
    runtime_quota_window_status_to_proxy, runtime_quota_window_summary_from_proxy,
    runtime_quota_window_summary_to_proxy,
};
use prodex_quota::{
    RuntimeQuotaPressureBand, RuntimeQuotaSummary, RuntimeQuotaWindowStatus,
    RuntimeQuotaWindowSummary, UsageResponse, WindowPair, find_main_window,
};
use prodex_runtime_state::RuntimeRouteKind;
use prodex_shared_types::RuntimeQuotaSource;
use runtime_proxy_crate as runtime_proxy;

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
    let summary =
        runtime_quota_summary_from_proxy(runtime_proxy::runtime_proxy_quota_summary_for_route(
            runtime_quota_window_observation(usage, "5h"),
            runtime_quota_window_observation(usage, "weekly"),
            route_kind,
        ));
    prodex_quota::openai_quota_runtime_window_pair(usage).map_or(summary, |pair| {
        preserve_unknown_window_status(summary, pair)
    })
}

pub fn runtime_quota_summary_for_route_with_model(
    usage: &UsageResponse,
    route_kind: RuntimeRouteKind,
    requested_model: Option<&str>,
) -> RuntimeQuotaSummary {
    runtime_quota_summary_for_route_with_model_at(
        usage,
        route_kind,
        requested_model,
        chrono::Local::now().timestamp(),
    )
}

pub fn runtime_quota_summary_for_route_with_model_at(
    usage: &UsageResponse,
    route_kind: RuntimeRouteKind,
    requested_model: Option<&str>,
    now: i64,
) -> RuntimeQuotaSummary {
    if prodex_quota::openai_model_is_retired_spark(requested_model) {
        return retired_model_quota_summary();
    }
    let summary =
        runtime_quota_summary_from_proxy(runtime_proxy::runtime_proxy_quota_summary_for_route(
            runtime_quota_window_observation_for_model_at(usage, "5h", requested_model, now),
            runtime_quota_window_observation_for_model_at(usage, "weekly", requested_model, now),
            route_kind,
        ));
    prodex_quota::openai_quota_runtime_window_pair_for_model(usage, requested_model)
        .map_or(summary, |pair| {
            preserve_unknown_window_status(summary, pair)
        })
}

fn preserve_unknown_window_status(
    mut summary: RuntimeQuotaSummary,
    pair: &WindowPair,
) -> RuntimeQuotaSummary {
    for (label, output) in [
        ("5h", &mut summary.five_hour),
        ("weekly", &mut summary.weekly),
    ] {
        if let Some(window) = find_main_window(pair, label)
            && let Some(reset_at) = prodex_mojo_core::runtime_state_quota::unknown_window_override(
                window.used_percent.is_some(),
                window.reset_at,
            )
            .expect("Mojo unknown-window override returned invalid output")
        {
            output.status = RuntimeQuotaWindowStatus::Unknown;
            output.remaining_percent = 0;
            output.reset_at = reset_at;
        }
    }
    summary
}

pub fn runtime_quota_summary_blocking_reset_at(
    summary: RuntimeQuotaSummary,
    route_kind: RuntimeRouteKind,
    responses_critical_floor_percent: i64,
) -> Option<i64> {
    runtime_proxy::runtime_proxy_quota_summary_blocking_reset_at(
        runtime_quota_summary_to_proxy(summary),
        route_kind,
        responses_critical_floor_percent,
    )
}

fn cached_source_from_mojo(
    source: prodex_mojo_core::runtime_state_quota::CachedQuotaSourceKind,
) -> Option<RuntimeQuotaSource> {
    use prodex_mojo_core::runtime_state_quota::CachedQuotaSourceKind;
    match source {
        CachedQuotaSourceKind::None => None,
        CachedQuotaSourceKind::Live => Some(RuntimeQuotaSource::LiveProbe),
        CachedQuotaSourceKind::Snapshot => Some(RuntimeQuotaSource::PersistedSnapshot),
    }
}

fn cached_model_kind(requested_model: Option<&str>) -> i64 {
    if prodex_quota::openai_model_is_retired_spark(requested_model) {
        prodex_mojo_core::runtime_state_quota::CACHED_MODEL_RETIRED
    } else if prodex_quota::openai_model_is_luna(requested_model) {
        prodex_mojo_core::runtime_state_quota::CACHED_MODEL_LUNA
    } else {
        prodex_mojo_core::runtime_state_quota::CACHED_MODEL_STANDARD
    }
}

fn cached_summary_from_plan(
    plan: prodex_mojo_core::runtime_state_quota::CachedQuotaSummaryPlan,
    live_probe_usage: Option<&UsageResponse>,
    persisted_snapshot: Option<&RuntimeProfileUsageSnapshot>,
    route_kind: RuntimeRouteKind,
    now: i64,
) -> (RuntimeQuotaSummary, Option<RuntimeQuotaSource>) {
    use prodex_mojo_core::runtime_state_quota::CachedQuotaSummaryKind;
    let summary = match plan.summary {
        CachedQuotaSummaryKind::Unknown => unknown_runtime_quota_summary(),
        CachedQuotaSummaryKind::Live => runtime_quota_summary_for_route(
            live_probe_usage.expect("Mojo cached-source plan selected missing live usage"),
            route_kind,
        ),
        CachedQuotaSummaryKind::Snapshot => runtime_quota_summary_from_usage_snapshot_at(
            persisted_snapshot.expect("Mojo cached-source plan selected missing snapshot"),
            route_kind,
            now,
        ),
        CachedQuotaSummaryKind::Retired => retired_model_quota_summary(),
    };
    (summary, cached_source_from_mojo(plan.source))
}

pub fn runtime_quota_summary_from_cached_sources(
    live_probe_usage: Option<&UsageResponse>,
    persisted_snapshot: Option<&RuntimeProfileUsageSnapshot>,
    route_kind: RuntimeRouteKind,
    now: i64,
    stale_grace_seconds: i64,
) -> (RuntimeQuotaSummary, Option<RuntimeQuotaSource>) {
    let snapshot_usable = persisted_snapshot.is_some_and(|snapshot| {
        runtime_usage_snapshot_is_usable(snapshot, now, stale_grace_seconds)
    });
    let plan = prodex_mojo_core::runtime_state_quota::cached_summary_source_plan(
        live_probe_usage.is_some(),
        persisted_snapshot.is_some(),
        snapshot_usable,
    )
    .expect("Mojo cached quota source planner returned invalid output");
    cached_summary_from_plan(plan, live_probe_usage, persisted_snapshot, route_kind, now)
}

pub fn runtime_quota_summary_from_cached_sources_for_model(
    live_probe_usage: Option<&UsageResponse>,
    persisted_snapshot: Option<&RuntimeProfileUsageSnapshot>,
    route_kind: RuntimeRouteKind,
    requested_model: Option<&str>,
    now: i64,
    stale_grace_seconds: i64,
) -> (RuntimeQuotaSummary, Option<RuntimeQuotaSource>) {
    let snapshot_usable = persisted_snapshot.is_some_and(|snapshot| {
        runtime_usage_snapshot_is_usable(snapshot, now, stale_grace_seconds)
    });
    let snapshot_model_pair_present = requested_model.is_none_or(|model| {
        persisted_snapshot.is_some_and(|snapshot| {
            prodex_quota::openai_quota_runtime_window_pair_for_model(
                &usage_from_runtime_usage_snapshot(snapshot),
                Some(model),
            )
            .is_some()
        })
    });
    let model_kind = cached_model_kind(requested_model);
    let plan = prodex_mojo_core::runtime_state_quota::cached_model_summary_source_plan(
        model_kind,
        live_probe_usage.is_some(),
        persisted_snapshot.is_some(),
        snapshot_usable,
        requested_model.is_some(),
        snapshot_model_pair_present,
    )
    .expect("Mojo cached model quota source planner returned invalid output");

    use prodex_mojo_core::runtime_state_quota::CachedQuotaSummaryKind;
    let source = cached_source_from_mojo(plan.source);
    let summary = match plan.summary {
        CachedQuotaSummaryKind::Unknown => unknown_runtime_quota_summary(),
        CachedQuotaSummaryKind::Retired => retired_model_quota_summary(),
        CachedQuotaSummaryKind::Live => runtime_quota_summary_for_route_with_model(
            live_probe_usage.expect("Mojo cached model plan selected missing live usage"),
            route_kind,
            requested_model,
        ),
        CachedQuotaSummaryKind::Snapshot => runtime_quota_summary_from_usage_snapshot_at(
            persisted_snapshot.expect("Mojo cached model plan selected missing snapshot"),
            route_kind,
            now,
        ),
    };
    let force_unknown = prodex_mojo_core::runtime_state_quota::cached_model_summary_force_unknown(
        model_kind,
        summary.route_band == RuntimeQuotaPressureBand::Exhausted,
    )
    .expect("Mojo cached model quota finalizer returned invalid output");
    (
        if force_unknown {
            unknown_runtime_quota_summary()
        } else {
            summary
        },
        source,
    )
}

fn unknown_runtime_quota_summary() -> RuntimeQuotaSummary {
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
    }
}

fn retired_model_quota_summary() -> RuntimeQuotaSummary {
    let exhausted = RuntimeQuotaWindowSummary {
        status: RuntimeQuotaWindowStatus::Exhausted,
        remaining_percent: 0,
        reset_at: i64::MAX,
    };
    RuntimeQuotaSummary {
        five_hour: exhausted,
        weekly: exhausted,
        route_band: RuntimeQuotaPressureBand::Exhausted,
    }
}

pub fn runtime_quota_summary_requires_precommit_live_probe(
    summary: RuntimeQuotaSummary,
    source: Option<RuntimeQuotaSource>,
    route_kind: RuntimeRouteKind,
) -> bool {
    runtime_proxy::runtime_proxy_quota_summary_requires_precommit_live_probe(
        runtime_quota_summary_to_proxy(summary),
        runtime_quota_source_option_to_proxy(source),
        route_kind,
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
        route_kind,
    )
}

pub fn runtime_precommit_quota_block_reason(
    summary: RuntimeQuotaSummary,
    route_kind: RuntimeRouteKind,
    responses_critical_floor_percent: i64,
) -> Option<runtime_proxy::RuntimePrecommitQuotaBlockReason> {
    runtime_proxy::runtime_proxy_precommit_quota_block_reason(
        runtime_quota_summary_to_proxy(summary),
        route_kind,
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
            route_kind,
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
            route_kind,
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
    runtime_proxy::runtime_quota_summary_allows_soft_affinity(
        runtime_selection_quota_summary_to_proxy(summary),
        runtime_quota_source_option_to_proxy(source),
        route_kind,
        responses_critical_floor_percent,
    )
}

pub fn runtime_quota_soft_affinity_rejection_reason(
    summary: RuntimeQuotaSummary,
    source: Option<RuntimeQuotaSource>,
    route_kind: RuntimeRouteKind,
    responses_critical_floor_percent: i64,
) -> &'static str {
    runtime_proxy::runtime_quota_soft_affinity_rejection_reason(
        runtime_selection_quota_summary_to_proxy(summary),
        runtime_quota_source_option_to_proxy(source),
        route_kind,
        responses_critical_floor_percent,
    )
}

pub fn runtime_quota_summary_log_fields(summary: RuntimeQuotaSummary) -> String {
    format!(
        "quota_band={} five_hour_status={} five_hour_remaining={} five_hour_reset_at={} weekly_status={} weekly_remaining={} weekly_reset_at={}",
        runtime_quota_pressure_band_reason(summary.route_band),
        runtime_quota_window_status_reason(summary.five_hour.status),
        summary.five_hour.remaining_percent,
        summary.five_hour.reset_at,
        runtime_quota_window_status_reason(summary.weekly.status),
        summary.weekly.remaining_percent,
        summary.weekly.reset_at,
    )
}
