use super::*;

pub(crate) use prodex_runtime_store::{
    runtime_profile_backoff_sort_key, runtime_profile_name_in_selection_backoff,
};

pub(crate) fn prune_runtime_profile_retry_backoff(runtime: &mut RuntimeRotationState, now: i64) {
    runtime
        .profile_retry_backoff_until
        .retain(|_, until| *until > now);
}

pub(crate) fn prune_runtime_profile_transport_backoff(
    runtime: &mut RuntimeRotationState,
    now: i64,
) {
    runtime
        .profile_transport_backoff_until
        .retain(|_, until| *until > now);
}

pub(crate) fn prune_runtime_profile_route_circuits(runtime: &mut RuntimeRotationState, now: i64) {
    runtime
        .profile_route_circuit_open_until
        .retain(|key, until| {
            if *until > now {
                return true;
            }
            let health_key = runtime_profile_route_circuit_health_key(key);
            runtime_profile_effective_health_score_from_map(
                &runtime.profile_health,
                &health_key,
                now,
            ) > 0
        });
}

pub(crate) fn prune_runtime_profile_selection_backoff(
    runtime: &mut RuntimeRotationState,
    now: i64,
) {
    prune_runtime_profile_retry_backoff(runtime, now);
    prune_runtime_profile_transport_backoff(runtime, now);
    prune_runtime_profile_route_circuits(runtime, now);
}

pub(crate) fn runtime_profile_backoffs_snapshot(
    runtime: &RuntimeRotationState,
) -> RuntimeProfileBackoffs {
    RuntimeProfileBackoffs {
        retry_backoff_until: runtime.profile_retry_backoff_until.clone(),
        transport_backoff_until: runtime.profile_transport_backoff_until.clone(),
        route_circuit_open_until: runtime.profile_route_circuit_open_until.clone(),
    }
}

pub(crate) fn runtime_soften_persisted_backoffs_for_startup(
    backoffs: &mut RuntimeProfileBackoffs,
    profile_scores: &BTreeMap<String, RuntimeProfileHealth>,
    now: i64,
) -> bool {
    let mut changed = prodex_runtime_store::runtime_soften_persisted_backoff_map_for_startup(
        &mut backoffs.transport_backoff_until,
        now,
        RUNTIME_PROFILE_TRANSPORT_BACKOFF_SECONDS,
    );
    changed = runtime_soften_persisted_route_circuits_for_startup(
        &mut backoffs.route_circuit_open_until,
        profile_scores,
        now,
    ) || changed;
    changed
}

pub(crate) fn mark_runtime_profile_retry_backoff(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
) -> Result<()> {
    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let now = Local::now().timestamp();
    prune_runtime_profile_selection_backoff(&mut runtime, now);
    runtime.profile_probe_cache.remove(profile_name);
    let until = now.saturating_add(RUNTIME_PROFILE_RETRY_BACKOFF_SECONDS);
    runtime
        .profile_retry_backoff_until
        .insert(profile_name.to_string(), until);
    schedule_runtime_state_save_from_runtime(
        shared,
        &runtime,
        &format!("profile_retry_backoff:{profile_name}"),
    );
    drop(runtime);
    runtime_proxy_log(
        shared,
        runtime_proxy_structured_log_message(
            "profile_retry_backoff",
            [
                runtime_proxy_log_field("profile", profile_name),
                runtime_proxy_log_field("until", until.to_string()),
            ],
        ),
    );
    Ok(())
}

pub(crate) fn mark_runtime_profile_transport_backoff(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
    context: &str,
) -> Result<()> {
    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let now = Local::now().timestamp();
    prune_runtime_profile_selection_backoff(&mut runtime, now);
    runtime.profile_probe_cache.remove(profile_name);
    let route_key = runtime_profile_transport_backoff_key(profile_name, route_kind);
    let existing_remaining = runtime_profile_transport_backoff_until_from_map(
        &runtime.profile_transport_backoff_until,
        profile_name,
        route_kind,
        now,
    )
    .unwrap_or(now)
    .saturating_sub(now);
    let next_backoff_seconds = if existing_remaining > 0 {
        existing_remaining.saturating_mul(2).clamp(
            RUNTIME_PROFILE_TRANSPORT_BACKOFF_SECONDS,
            RUNTIME_PROFILE_TRANSPORT_BACKOFF_MAX_SECONDS,
        )
    } else {
        RUNTIME_PROFILE_TRANSPORT_BACKOFF_SECONDS
    };
    let until = now.saturating_add(next_backoff_seconds);
    runtime
        .profile_transport_backoff_until
        .entry(route_key)
        .and_modify(|current| *current = (*current).max(until))
        .or_insert(until);
    schedule_runtime_state_save_from_runtime(
        shared,
        &runtime,
        &format!(
            "profile_transport_backoff:{profile_name}:{}",
            runtime_route_kind_label(route_kind)
        ),
    );
    drop(runtime);
    runtime_proxy_log(
        shared,
        runtime_proxy_structured_log_message(
            "profile_transport_backoff",
            [
                runtime_proxy_log_field("profile", profile_name),
                runtime_proxy_log_field("route", runtime_route_kind_label(route_kind)),
                runtime_proxy_log_field("until", until.to_string()),
                runtime_proxy_log_field("seconds", next_backoff_seconds.to_string()),
                runtime_proxy_log_field("context", context),
            ],
        ),
    );
    Ok(())
}

pub(crate) fn clear_runtime_profile_transport_backoff_for_route(
    runtime: &mut RuntimeRotationState,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) -> bool {
    let mut changed = runtime
        .profile_transport_backoff_until
        .remove(&runtime_profile_transport_backoff_key(
            profile_name,
            route_kind,
        ))
        .is_some();
    changed = runtime
        .profile_transport_backoff_until
        .remove(profile_name)
        .is_some()
        || changed;
    changed
}
