use super::*;

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
    match route_kind {
        RuntimeRouteKind::Responses => "responses",
        RuntimeRouteKind::Compact => "compact",
        RuntimeRouteKind::Websocket => "websocket",
        RuntimeRouteKind::Standard => "standard",
    }
}

pub(crate) fn runtime_route_coupled_kinds(
    route_kind: RuntimeRouteKind,
) -> &'static [RuntimeRouteKind] {
    match route_kind {
        RuntimeRouteKind::Responses => &[RuntimeRouteKind::Websocket],
        RuntimeRouteKind::Websocket => &[RuntimeRouteKind::Responses],
        RuntimeRouteKind::Compact => &[RuntimeRouteKind::Standard],
        RuntimeRouteKind::Standard => &[RuntimeRouteKind::Compact],
    }
}

pub(crate) fn runtime_profile_effective_health_score(
    entry: &RuntimeProfileHealth,
    now: i64,
) -> u32 {
    runtime_profile_effective_score(entry, now, RUNTIME_PROFILE_HEALTH_DECAY_SECONDS)
}

pub(crate) fn runtime_profile_effective_score(
    entry: &RuntimeProfileHealth,
    now: i64,
    decay_seconds: i64,
) -> u32 {
    let decay = now
        .saturating_sub(entry.updated_at)
        .saturating_div(decay_seconds.max(1))
        .clamp(0, i64::from(u32::MAX)) as u32;
    entry.score.saturating_sub(decay)
}

pub(crate) fn runtime_profile_effective_health_score_from_map(
    profile_health: &BTreeMap<String, RuntimeProfileHealth>,
    key: &str,
    now: i64,
) -> u32 {
    profile_health
        .get(key)
        .map(|entry| runtime_profile_effective_health_score(entry, now))
        .unwrap_or(0)
}

pub(crate) fn runtime_profile_effective_score_from_map(
    profile_health: &BTreeMap<String, RuntimeProfileHealth>,
    key: &str,
    now: i64,
    decay_seconds: i64,
) -> u32 {
    profile_health
        .get(key)
        .map(|entry| runtime_profile_effective_score(entry, now, decay_seconds))
        .unwrap_or(0)
}

pub(crate) fn runtime_profile_global_health_score(
    runtime: &RuntimeRotationState,
    profile_name: &str,
    now: i64,
) -> u32 {
    runtime_profile_effective_health_score_from_map(&runtime.profile_health, profile_name, now)
}

pub(crate) fn runtime_profile_route_health_key(
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) -> String {
    format!(
        "__route_health__:{}:{profile_name}",
        runtime_route_kind_label(route_kind)
    )
}

pub(crate) fn runtime_profile_route_bad_pairing_key(
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) -> String {
    format!(
        "__route_bad_pairing__:{}:{profile_name}",
        runtime_route_kind_label(route_kind)
    )
}

pub(crate) fn runtime_profile_route_success_streak_key(
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) -> String {
    format!(
        "__route_success__:{}:{profile_name}",
        runtime_route_kind_label(route_kind)
    )
}

pub(crate) fn runtime_profile_route_performance_key(
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) -> String {
    format!(
        "__route_performance__:{}:{profile_name}",
        runtime_route_kind_label(route_kind)
    )
}

pub(crate) fn runtime_profile_route_health_score(
    runtime: &RuntimeRotationState,
    profile_name: &str,
    now: i64,
    route_kind: RuntimeRouteKind,
) -> u32 {
    runtime_profile_effective_health_score_from_map(
        &runtime.profile_health,
        &runtime_profile_route_health_key(profile_name, route_kind),
        now,
    )
}

pub(crate) fn runtime_profile_route_coupling_score(
    runtime: &RuntimeRotationState,
    profile_name: &str,
    now: i64,
    route_kind: RuntimeRouteKind,
) -> u32 {
    runtime_profile_route_coupling_score_from_map(
        &runtime.profile_health,
        profile_name,
        now,
        route_kind,
    )
}

pub(crate) fn runtime_profile_route_coupling_score_from_map(
    profile_health: &BTreeMap<String, RuntimeProfileHealth>,
    profile_name: &str,
    now: i64,
    route_kind: RuntimeRouteKind,
) -> u32 {
    runtime_route_coupled_kinds(route_kind)
        .iter()
        .copied()
        .map(|coupled_kind| {
            let route_score = runtime_profile_effective_health_score_from_map(
                profile_health,
                &runtime_profile_route_health_key(profile_name, coupled_kind),
                now,
            );
            let bad_pairing_score = runtime_profile_effective_score_from_map(
                profile_health,
                &runtime_profile_route_bad_pairing_key(profile_name, coupled_kind),
                now,
                RUNTIME_PROFILE_BAD_PAIRING_DECAY_SECONDS,
            );
            route_score
                .saturating_add(bad_pairing_score)
                .saturating_div(2)
        })
        .fold(0, u32::saturating_add)
}

pub(crate) fn runtime_profile_route_performance_score(
    profile_health: &BTreeMap<String, RuntimeProfileHealth>,
    profile_name: &str,
    now: i64,
    route_kind: RuntimeRouteKind,
) -> u32 {
    let route_score = runtime_profile_effective_score_from_map(
        profile_health,
        &runtime_profile_route_performance_key(profile_name, route_kind),
        now,
        RUNTIME_PROFILE_PERFORMANCE_DECAY_SECONDS,
    );
    let coupled_score = runtime_route_coupled_kinds(route_kind)
        .iter()
        .copied()
        .map(|coupled_kind| {
            runtime_profile_effective_score_from_map(
                profile_health,
                &runtime_profile_route_performance_key(profile_name, coupled_kind),
                now,
                RUNTIME_PROFILE_PERFORMANCE_DECAY_SECONDS,
            )
            .saturating_div(2)
        })
        .fold(0, u32::saturating_add);
    route_score.saturating_add(coupled_score)
}

pub(crate) fn runtime_profile_health_score(
    runtime: &RuntimeRotationState,
    profile_name: &str,
    now: i64,
    route_kind: RuntimeRouteKind,
) -> u32 {
    runtime_profile_global_health_score(runtime, profile_name, now)
        .saturating_add(runtime_profile_route_health_score(
            runtime,
            profile_name,
            now,
            route_kind,
        ))
        .saturating_add(runtime_profile_route_coupling_score(
            runtime,
            profile_name,
            now,
            route_kind,
        ))
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
    runtime_profile_effective_health_score_from_map(profile_health, profile_name, now)
        .saturating_add(runtime_profile_effective_health_score_from_map(
            profile_health,
            &runtime_profile_route_health_key(profile_name, route_kind),
            now,
        ))
        .saturating_add(runtime_profile_effective_score_from_map(
            profile_health,
            &runtime_profile_route_bad_pairing_key(profile_name, route_kind),
            now,
            RUNTIME_PROFILE_BAD_PAIRING_DECAY_SECONDS,
        ))
        .saturating_add(runtime_profile_route_coupling_score_from_map(
            profile_health,
            profile_name,
            now,
            route_kind,
        ))
        .saturating_add(runtime_profile_route_performance_score(
            profile_health,
            profile_name,
            now,
            route_kind,
        ))
}

pub(crate) fn runtime_profile_inflight_sort_key(
    profile_name: &str,
    profile_inflight: &BTreeMap<String, usize>,
) -> usize {
    profile_inflight.get(profile_name).copied().unwrap_or(0)
}

pub(crate) fn runtime_profile_inflight_weight(context: &str) -> usize {
    match context {
        "websocket_session" | "responses_http" => 2,
        _ => 1,
    }
}

pub(crate) fn runtime_route_kind_inflight_context(route_kind: RuntimeRouteKind) -> &'static str {
    match route_kind {
        RuntimeRouteKind::Responses => "responses_http",
        RuntimeRouteKind::Compact => "compact_http",
        RuntimeRouteKind::Websocket => "websocket_session",
        RuntimeRouteKind::Standard => "standard_http",
    }
}

pub(crate) fn runtime_profile_inflight_soft_limit(
    route_kind: RuntimeRouteKind,
    pressure_mode: bool,
) -> usize {
    let base = runtime_proxy_profile_inflight_soft_limit().max(1);
    if !pressure_mode {
        return base;
    }
    match route_kind {
        RuntimeRouteKind::Responses | RuntimeRouteKind::Websocket => base.saturating_sub(1).max(1),
        RuntimeRouteKind::Compact | RuntimeRouteKind::Standard => base.saturating_sub(2).max(1),
    }
}
