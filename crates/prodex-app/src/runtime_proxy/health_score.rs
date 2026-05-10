use super::*;

fn runtime_route_kind_to_proxy(
    route_kind: RuntimeRouteKind,
) -> runtime_proxy_crate::RuntimeRouteKind {
    match route_kind {
        RuntimeRouteKind::Responses => runtime_proxy_crate::RuntimeRouteKind::Responses,
        RuntimeRouteKind::Compact => runtime_proxy_crate::RuntimeRouteKind::Compact,
        RuntimeRouteKind::Websocket => runtime_proxy_crate::RuntimeRouteKind::Websocket,
        RuntimeRouteKind::Standard => runtime_proxy_crate::RuntimeRouteKind::Standard,
    }
}

pub(crate) fn runtime_proxy_current_profile(shared: &RuntimeRotationProxyShared) -> Result<String> {
    Ok(shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?
        .current_profile
        .clone())
}

pub(crate) fn runtime_profile_in_retry_backoff(
    runtime: &RuntimeRotationState,
    profile_name: &str,
    now: i64,
) -> bool {
    runtime
        .profile_retry_backoff_until
        .get(profile_name)
        .copied()
        .is_some_and(|until| until > now)
}

pub(crate) fn runtime_profile_in_transport_backoff(
    runtime: &RuntimeRotationState,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
    now: i64,
) -> bool {
    runtime_profile_transport_backoff_until_from_map(
        &runtime.profile_transport_backoff_until,
        profile_name,
        route_kind,
        now,
    )
    .is_some()
}

pub(crate) fn runtime_profile_inflight_count(
    runtime: &RuntimeRotationState,
    profile_name: &str,
) -> usize {
    runtime
        .profile_inflight
        .get(profile_name)
        .copied()
        .unwrap_or(0)
}

pub(crate) fn runtime_profile_inflight_hard_limit_context(context: &str) -> usize {
    runtime_profile_inflight_weight(context)
}

pub(crate) fn runtime_profile_inflight_hard_limited_for_context(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    context: &str,
) -> Result<bool> {
    let hard_limit = runtime_proxy_profile_inflight_hard_limit();
    let runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    Ok(runtime_profile_inflight_count(&runtime, profile_name)
        .saturating_add(runtime_profile_inflight_hard_limit_context(context))
        > hard_limit)
}

pub(crate) fn runtime_profile_in_selection_backoff(
    runtime: &RuntimeRotationState,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
    now: i64,
) -> bool {
    runtime_profile_in_retry_backoff(runtime, profile_name, now)
        || runtime_profile_in_transport_backoff(runtime, profile_name, route_kind, now)
}

pub(crate) fn runtime_route_kind_label(route_kind: RuntimeRouteKind) -> &'static str {
    prodex_runtime_store::runtime_route_kind_label(route_kind)
}

pub(crate) fn runtime_profile_effective_health_score(
    entry: &RuntimeProfileHealth,
    now: i64,
) -> u32 {
    prodex_runtime_store::runtime_profile_effective_health_score(entry, now)
}

pub(crate) fn runtime_profile_effective_health_score_from_map(
    profile_health: &BTreeMap<String, RuntimeProfileHealth>,
    key: &str,
    now: i64,
) -> u32 {
    prodex_runtime_store::runtime_profile_effective_health_score_from_map(profile_health, key, now)
}

pub(crate) fn runtime_profile_effective_score_from_map(
    profile_health: &BTreeMap<String, RuntimeProfileHealth>,
    key: &str,
    now: i64,
    decay_seconds: i64,
) -> u32 {
    prodex_runtime_store::runtime_profile_effective_score_from_map(
        profile_health,
        key,
        now,
        decay_seconds,
    )
}

pub(crate) fn runtime_profile_route_health_key(
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) -> String {
    prodex_runtime_store::runtime_profile_route_health_key(profile_name, route_kind)
}

pub(crate) fn runtime_profile_route_bad_pairing_key(
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) -> String {
    prodex_runtime_store::runtime_profile_route_bad_pairing_key(profile_name, route_kind)
}

pub(crate) fn runtime_profile_route_success_streak_key(
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) -> String {
    prodex_runtime_store::runtime_profile_route_success_streak_key(profile_name, route_kind)
}

pub(crate) fn runtime_profile_route_performance_key(
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) -> String {
    prodex_runtime_store::runtime_profile_route_performance_key(profile_name, route_kind)
}

pub(crate) fn runtime_profile_route_health_score(
    runtime: &RuntimeRotationState,
    profile_name: &str,
    now: i64,
    route_kind: RuntimeRouteKind,
) -> u32 {
    prodex_runtime_store::runtime_profile_route_health_score(
        &runtime.profile_health,
        profile_name,
        now,
        route_kind,
    )
}

pub(crate) fn runtime_profile_route_performance_score(
    profile_health: &BTreeMap<String, RuntimeProfileHealth>,
    profile_name: &str,
    now: i64,
    route_kind: RuntimeRouteKind,
) -> u32 {
    prodex_runtime_store::runtime_profile_route_performance_score(
        profile_health,
        profile_name,
        now,
        route_kind,
    )
}

pub(crate) fn runtime_profile_health_score(
    runtime: &RuntimeRotationState,
    profile_name: &str,
    now: i64,
    route_kind: RuntimeRouteKind,
) -> u32 {
    prodex_runtime_store::runtime_profile_health_score(
        &runtime.profile_health,
        profile_name,
        now,
        route_kind,
    )
}

pub(crate) fn runtime_profile_selection_jitter(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) -> u64 {
    let mut hasher = DefaultHasher::new();
    shared
        .request_sequence
        .load(Ordering::Relaxed)
        .hash(&mut hasher);
    profile_name.hash(&mut hasher);
    runtime_route_kind_label(route_kind).hash(&mut hasher);
    hasher.finish()
}

pub(crate) fn runtime_profile_health_sort_key(
    profile_name: &str,
    profile_health: &BTreeMap<String, RuntimeProfileHealth>,
    now: i64,
    route_kind: RuntimeRouteKind,
) -> u32 {
    prodex_runtime_store::runtime_profile_health_sort_key(
        profile_name,
        profile_health,
        now,
        route_kind,
    )
}

pub(crate) fn runtime_profile_inflight_sort_key(
    profile_name: &str,
    profile_inflight: &BTreeMap<String, usize>,
) -> usize {
    runtime_proxy_crate::runtime_profile_inflight_sort_key(profile_name, profile_inflight)
}

pub(crate) fn runtime_profile_inflight_weight(context: &str) -> usize {
    runtime_proxy_crate::runtime_profile_inflight_weight(context)
}

pub(crate) fn runtime_route_kind_inflight_context(route_kind: RuntimeRouteKind) -> &'static str {
    runtime_proxy_crate::runtime_route_kind_inflight_context(runtime_route_kind_to_proxy(
        route_kind,
    ))
}

pub(crate) fn runtime_profile_inflight_soft_limit(
    route_kind: RuntimeRouteKind,
    pressure_mode: bool,
) -> usize {
    runtime_proxy_crate::runtime_profile_inflight_soft_limit(
        runtime_route_kind_to_proxy(route_kind),
        pressure_mode,
        runtime_proxy_profile_inflight_soft_limit(),
    )
}
