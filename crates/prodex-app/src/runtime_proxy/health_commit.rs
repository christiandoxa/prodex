use super::*;

pub(crate) fn commit_runtime_proxy_profile_selection(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) -> Result<bool> {
    commit_runtime_proxy_profile_selection_with_policy(shared, profile_name, route_kind, true)
}

pub(crate) fn commit_runtime_proxy_profile_selection_with_policy(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
    track_current_profile: bool,
) -> Result<bool> {
    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let switch_runtime_profile = track_current_profile && runtime.current_profile != profile_name;
    let switch_global_profile =
        track_current_profile && !matches!(route_kind, RuntimeRouteKind::Compact);
    let switched = switch_runtime_profile;
    let now = Local::now().timestamp();
    let cleared_retry_backoff = runtime
        .profile_retry_backoff_until
        .remove(profile_name)
        .is_some();
    let cleared_transport_backoff =
        clear_runtime_profile_transport_backoff_for_route(&mut runtime, profile_name, route_kind);
    let cleared_route_circuit =
        clear_runtime_profile_circuit_for_route(&mut runtime, profile_name, route_kind);
    let cleared_health =
        clear_runtime_profile_health_for_route(&mut runtime, profile_name, route_kind, now);
    if switch_runtime_profile {
        runtime.current_profile = profile_name.to_string();
    }
    let state_changed =
        switch_global_profile && runtime.state.active_profile.as_deref() != Some(profile_name);
    if switch_global_profile {
        runtime.state.active_profile = Some(profile_name.to_string());
        record_run_selection(&mut runtime.state, profile_name);
    }
    let should_persist = switched
        || state_changed
        || cleared_retry_backoff
        || cleared_transport_backoff
        || cleared_route_circuit
        || cleared_health;
    if should_persist {
        schedule_runtime_state_save_from_runtime(
            shared,
            &runtime,
            &format!("profile_commit:{profile_name}"),
        );
    }
    drop(runtime);
    if switch_runtime_profile {
        update_runtime_broker_current_profile(&shared.log_path, profile_name);
    }
    runtime_proxy_log(
        shared,
        format!(
            "profile_commit profile={profile_name} route={} switched={switched} persisted={should_persist} track_current_profile={track_current_profile} cleared_route_circuit={cleared_route_circuit}",
            runtime_route_kind_label(route_kind),
        ),
    );
    Ok(switched)
}

pub(crate) fn clear_runtime_profile_health_for_route(
    runtime: &mut RuntimeRotationState,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
    now: i64,
) -> bool {
    let mut changed = runtime.profile_health.remove(profile_name).is_some();
    let previous_route_score =
        runtime_profile_route_health_score(runtime, profile_name, now, route_kind);
    let previous_bad_pairing = runtime_profile_effective_score_from_map(
        &runtime.profile_health,
        &runtime_profile_route_bad_pairing_key(profile_name, route_kind),
        now,
        RUNTIME_PROFILE_BAD_PAIRING_DECAY_SECONDS,
    );
    let previous_circuit_reopen = runtime_profile_effective_score_from_map(
        &runtime.profile_health,
        &runtime_profile_route_circuit_reopen_key(profile_name, route_kind),
        now,
        RUNTIME_PROFILE_CIRCUIT_REOPEN_DECAY_SECONDS,
    );
    recover_runtime_profile_health_for_route(runtime, profile_name, route_kind, now);
    changed = changed || previous_route_score > 0;
    changed = changed || previous_bad_pairing > 0;
    changed = changed || previous_circuit_reopen > 0;
    changed = runtime
        .profile_health
        .remove(&runtime_profile_route_bad_pairing_key(
            profile_name,
            route_kind,
        ))
        .is_some()
        || changed;
    changed = runtime
        .profile_health
        .remove(&runtime_profile_route_circuit_reopen_key(
            profile_name,
            route_kind,
        ))
        .is_some()
        || changed;
    changed
}

pub(crate) fn commit_runtime_proxy_profile_selection_with_notice(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) -> Result<()> {
    let _ = commit_runtime_proxy_profile_selection(shared, profile_name, route_kind)?;
    Ok(())
}
