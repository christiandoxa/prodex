use super::*;

pub(crate) fn runtime_quota_summary_blocking_reset_at(
    summary: RuntimeQuotaSummary,
    route_kind: RuntimeRouteKind,
) -> Option<i64> {
    prodex_runtime_quota::runtime_quota_summary_blocking_reset_at(
        summary,
        route_kind,
        runtime_proxy_responses_quota_critical_floor_percent(),
    )
}

pub(crate) fn runtime_profile_known_quota_reset_at(
    runtime: &RuntimeRotationState,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) -> Option<i64> {
    let now = Local::now().timestamp();
    runtime
        .profile_probe_cache
        .get(profile_name)
        .and_then(|entry| entry.result.as_ref().ok())
        .map(|usage| runtime_quota_summary_for_route(usage, route_kind))
        .and_then(|summary| runtime_quota_summary_blocking_reset_at(summary, route_kind))
        .filter(|reset_at| *reset_at > now)
        .or_else(|| {
            runtime
                .profile_usage_snapshots
                .get(profile_name)
                .and_then(|snapshot| {
                    runtime_quota_summary_blocking_reset_at(
                        runtime_quota_summary_from_usage_snapshot(snapshot, route_kind),
                        route_kind,
                    )
                })
                .filter(|reset_at| *reset_at > now)
        })
}

pub(crate) fn runtime_quota_summary_allows_soft_affinity(
    summary: RuntimeQuotaSummary,
    source: Option<RuntimeQuotaSource>,
    route_kind: RuntimeRouteKind,
) -> bool {
    prodex_runtime_quota::runtime_quota_summary_allows_soft_affinity(
        summary,
        source,
        route_kind,
        runtime_proxy_responses_quota_critical_floor_percent(),
    )
}

pub(crate) fn runtime_quota_soft_affinity_rejection_reason(
    summary: RuntimeQuotaSummary,
    source: Option<RuntimeQuotaSource>,
    route_kind: RuntimeRouteKind,
) -> &'static str {
    prodex_runtime_quota::runtime_quota_soft_affinity_rejection_reason(
        summary,
        source,
        route_kind,
        runtime_proxy_responses_quota_critical_floor_percent(),
    )
}

pub(crate) fn runtime_profile_quota_summary_for_route(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) -> Result<(RuntimeQuotaSummary, Option<RuntimeQuotaSource>)> {
    let runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let now = Local::now().timestamp();
    Ok(runtime_profile_quota_summary_for_route_from_state(
        &runtime,
        profile_name,
        route_kind,
        now,
    ))
}

pub(crate) fn runtime_profile_quota_summary_for_route_from_state(
    runtime: &RuntimeRotationState,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
    now: i64,
) -> (RuntimeQuotaSummary, Option<RuntimeQuotaSource>) {
    let live_probe_usage = runtime
        .profile_probe_cache
        .get(profile_name)
        .filter(|entry| runtime_profile_usage_cache_is_fresh(entry, now))
        .and_then(|entry| entry.result.as_ref().ok());
    runtime_quota_summary_from_cached_sources(
        live_probe_usage,
        runtime.profile_usage_snapshots.get(profile_name),
        route_kind,
        now,
        RUNTIME_PROFILE_USAGE_CACHE_STALE_GRACE_SECONDS,
    )
}
