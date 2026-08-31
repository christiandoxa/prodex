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
    let (summary, _) =
        runtime_profile_quota_summary_for_route_from_state(runtime, profile_name, route_kind, now);
    runtime_quota_summary_blocking_reset_at(summary, route_kind).filter(|reset_at| *reset_at > now)
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

pub(crate) fn runtime_profile_quota_summary_for_route_with_model(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
    requested_model: Option<&str>,
) -> Result<(RuntimeQuotaSummary, Option<RuntimeQuotaSource>)> {
    let runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let now = Local::now().timestamp();
    Ok(
        runtime_profile_quota_summary_for_route_from_state_with_model(
            &runtime,
            profile_name,
            route_kind,
            requested_model,
            now,
        ),
    )
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

pub(crate) fn runtime_profile_quota_summary_for_route_from_state_with_model(
    runtime: &RuntimeRotationState,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
    requested_model: Option<&str>,
    now: i64,
) -> (RuntimeQuotaSummary, Option<RuntimeQuotaSource>) {
    let live_probe_usage = runtime
        .profile_probe_cache
        .get(profile_name)
        .filter(|entry| runtime_profile_usage_cache_is_fresh(entry, now))
        .and_then(|entry| entry.result.as_ref().ok());
    runtime_quota_summary_from_cached_sources_for_model(
        live_probe_usage,
        runtime.profile_usage_snapshots.get(profile_name),
        route_kind,
        requested_model,
        now,
        RUNTIME_PROFILE_USAGE_CACHE_STALE_GRACE_SECONDS,
    )
}
