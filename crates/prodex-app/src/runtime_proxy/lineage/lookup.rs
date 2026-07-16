use super::*;
pub(crate) fn runtime_response_bound_profile(
    shared: &RuntimeRotationProxyShared,
    previous_response_id: &str,
    route_kind: RuntimeRouteKind,
) -> Result<Option<String>> {
    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let now = Local::now().timestamp();
    let profile_name = runtime
        .state
        .response_profile_bindings
        .get(previous_response_id)
        .map(|binding| binding.profile_name.clone())
        .filter(|profile_name| runtime.state.profiles.contains_key(profile_name));
    if runtime_age_stale_verified_continuation_status(
        &mut runtime.continuation_statuses,
        RuntimeContinuationBindingKind::Response,
        previous_response_id,
        now,
    ) {
        runtime_proxy_log(
            shared,
            format!(
                "selection_keep_affinity route={} affinity=previous_response profile={} reason=continuation_stale response_id={previous_response_id}",
                runtime_route_kind_label(route_kind),
                profile_name.as_deref().unwrap_or("-"),
            ),
        );
        schedule_runtime_binding_touch_save(
            shared,
            &runtime,
            RuntimeStateMutation::ContinuationStale(previous_response_id.to_string()),
        );
    }
    let dead_shadowed_by_binding = runtime_dead_continuation_status_shadowed_by_live_binding(
        runtime_continuation_status_map(
            &runtime.continuation_statuses,
            RuntimeContinuationBindingKind::Response,
        )
        .get(previous_response_id),
        runtime
            .state
            .response_profile_bindings
            .get(previous_response_id)
            .filter(|binding| runtime.state.profiles.contains_key(&binding.profile_name)),
    );
    if runtime_continuation_status_map(
        &runtime.continuation_statuses,
        RuntimeContinuationBindingKind::Response,
    )
    .get(previous_response_id)
    .is_some_and(runtime_continuation_status_is_terminal)
        && !dead_shadowed_by_binding
    {
        runtime_proxy_log(
            shared,
            format!(
                "selection_skip_affinity route={} affinity=previous_response profile=- reason=continuation_dead response_id={previous_response_id}",
                runtime_route_kind_label(route_kind),
            ),
        );
        return Ok(None);
    }
    if runtime_continuation_status_recently_suspect(
        &runtime.continuation_statuses,
        RuntimeContinuationBindingKind::Response,
        previous_response_id,
        now,
    ) {
        runtime_proxy_log(
            shared,
            format!(
                "selection_skip_affinity route={} affinity=previous_response profile=- reason=continuation_suspect response_id={previous_response_id}",
                runtime_route_kind_label(route_kind),
            ),
        );
        return Ok(None);
    }
    if let Some(profile_name) = profile_name.as_deref()
        && runtime_previous_response_negative_cache_active(
            &runtime.profile_health,
            previous_response_id,
            profile_name,
            route_kind,
            now,
        )
    {
        runtime_proxy_log(
            shared,
            format!(
                "selection_skip_affinity route={} affinity=previous_response profile={} reason=negative_cache response_id={}",
                runtime_route_kind_label(route_kind),
                profile_name,
                previous_response_id,
            ),
        );
        return Ok(None);
    }
    let mut persist_touch = false;
    if let Some(profile_name) = profile_name.as_deref()
        && let Some(binding) = runtime
            .state
            .response_profile_bindings
            .get_mut(previous_response_id)
        && binding.profile_name == profile_name
    {
        if runtime_binding_touch_should_persist(binding.bound_at, now) {
            persist_touch = true;
        }
        if binding.bound_at < now {
            binding.bound_at = now;
        }
        persist_touch = runtime_continuation_status_should_persist_touch(
            &runtime.continuation_statuses,
            RuntimeContinuationBindingKind::Response,
            previous_response_id,
            now,
        ) || persist_touch;
        let _ = runtime_mark_continuation_status_touched(
            &mut runtime.continuation_statuses,
            RuntimeContinuationBindingKind::Response,
            previous_response_id,
            now,
        );
    }
    if persist_touch {
        schedule_runtime_binding_touch_save(
            shared,
            &runtime,
            RuntimeStateMutation::ResponseTouch(previous_response_id.to_string()),
        );
    }
    Ok(profile_name)
}

pub(crate) fn runtime_turn_state_bound_profile(
    shared: &RuntimeRotationProxyShared,
    turn_state: &str,
) -> Result<Option<String>> {
    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let now = Local::now().timestamp();
    let profile_name = runtime
        .turn_state_bindings
        .get(turn_state)
        .map(|binding| binding.profile_name.clone())
        .filter(|profile_name| runtime.state.profiles.contains_key(profile_name));
    if runtime_age_stale_verified_continuation_status(
        &mut runtime.continuation_statuses,
        RuntimeContinuationBindingKind::TurnState,
        turn_state,
        now,
    ) {
        runtime_proxy_log(
            shared,
            format!(
                "selection_skip_affinity route=responses affinity=turn_state profile={} reason=continuation_stale turn_state={turn_state}",
                profile_name.as_deref().unwrap_or("-"),
            ),
        );
        schedule_runtime_binding_touch_save(
            shared,
            &runtime,
            RuntimeStateMutation::ContinuationStale(turn_state.to_string()),
        );
        return Ok(None);
    }
    let dead_shadowed_by_binding = runtime_dead_continuation_status_shadowed_by_live_binding(
        runtime_continuation_status_map(
            &runtime.continuation_statuses,
            RuntimeContinuationBindingKind::TurnState,
        )
        .get(turn_state),
        runtime
            .turn_state_bindings
            .get(turn_state)
            .filter(|binding| runtime.state.profiles.contains_key(&binding.profile_name)),
    );
    if runtime_continuation_status_map(
        &runtime.continuation_statuses,
        RuntimeContinuationBindingKind::TurnState,
    )
    .get(turn_state)
    .is_some_and(runtime_continuation_status_is_terminal)
        && !dead_shadowed_by_binding
    {
        runtime_proxy_log(
            shared,
            format!(
                "selection_skip_affinity route=responses affinity=turn_state profile=- reason=continuation_dead turn_state={turn_state}",
            ),
        );
        return Ok(None);
    }
    if runtime_continuation_status_recently_suspect(
        &runtime.continuation_statuses,
        RuntimeContinuationBindingKind::TurnState,
        turn_state,
        now,
    ) {
        runtime_proxy_log(
            shared,
            format!(
                "selection_skip_affinity route=responses affinity=turn_state profile=- reason=continuation_suspect turn_state={turn_state}",
            ),
        );
        return Ok(None);
    }
    let mut persist_touch = false;
    if let Some(profile_name) = profile_name.as_deref()
        && let Some(binding) = runtime.turn_state_bindings.get_mut(turn_state)
        && binding.profile_name == profile_name
    {
        if runtime_binding_touch_should_persist(binding.bound_at, now) {
            persist_touch = true;
        }
        if binding.bound_at < now {
            binding.bound_at = now;
        }
        persist_touch = runtime_continuation_status_should_persist_touch(
            &runtime.continuation_statuses,
            RuntimeContinuationBindingKind::TurnState,
            turn_state,
            now,
        ) || persist_touch;
        let _ = runtime_mark_continuation_status_touched(
            &mut runtime.continuation_statuses,
            RuntimeContinuationBindingKind::TurnState,
            turn_state,
            now,
        );
    }
    if persist_touch {
        schedule_runtime_binding_touch_save(
            shared,
            &runtime,
            RuntimeStateMutation::TurnStateTouch(turn_state.to_string()),
        );
    }
    Ok(profile_name)
}

pub(crate) fn runtime_session_bound_profile(
    shared: &RuntimeRotationProxyShared,
    session_id: &str,
) -> Result<Option<String>> {
    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let now = Local::now().timestamp();
    let profile_name = runtime
        .session_id_bindings
        .get(session_id)
        .map(|binding| binding.profile_name.clone())
        .filter(|profile_name| runtime.state.profiles.contains_key(profile_name));
    if runtime_age_stale_verified_continuation_status(
        &mut runtime.continuation_statuses,
        RuntimeContinuationBindingKind::SessionId,
        session_id,
        now,
    ) {
        runtime_proxy_log(
            shared,
            format!(
                "selection_skip_affinity route=compact affinity=session_id profile={} reason=continuation_stale session_id={session_id}",
                profile_name.as_deref().unwrap_or("-"),
            ),
        );
        schedule_runtime_binding_touch_save(
            shared,
            &runtime,
            RuntimeStateMutation::ContinuationStale(session_id.to_string()),
        );
        return Ok(None);
    }
    let dead_shadowed_by_binding = runtime_dead_continuation_status_shadowed_by_live_binding(
        runtime_continuation_status_map(
            &runtime.continuation_statuses,
            RuntimeContinuationBindingKind::SessionId,
        )
        .get(session_id),
        runtime
            .session_id_bindings
            .get(session_id)
            .filter(|binding| runtime.state.profiles.contains_key(&binding.profile_name)),
    );
    if runtime_continuation_status_map(
        &runtime.continuation_statuses,
        RuntimeContinuationBindingKind::SessionId,
    )
    .get(session_id)
    .is_some_and(runtime_continuation_status_is_terminal)
        && !dead_shadowed_by_binding
    {
        runtime_proxy_log(
            shared,
            format!(
                "selection_skip_affinity route=compact affinity=session_id profile=- reason=continuation_dead session_id={session_id}",
            ),
        );
        return Ok(None);
    }
    if runtime_continuation_status_recently_suspect(
        &runtime.continuation_statuses,
        RuntimeContinuationBindingKind::SessionId,
        session_id,
        now,
    ) {
        runtime_proxy_log(
            shared,
            format!(
                "selection_skip_affinity route=compact affinity=session_id profile=- reason=continuation_suspect session_id={session_id}",
            ),
        );
        return Ok(None);
    }
    let mut persist_touch = false;
    if let Some(profile_name) = profile_name.as_deref() {
        if let Some(binding) = runtime.session_id_bindings.get_mut(session_id)
            && binding.profile_name == profile_name
        {
            if runtime_binding_touch_should_persist(binding.bound_at, now) {
                persist_touch = true;
            }
            if binding.bound_at < now {
                binding.bound_at = now;
            }
        }
        if let Some(binding) = runtime.state.session_profile_bindings.get_mut(session_id)
            && binding.profile_name == profile_name
        {
            if runtime_binding_touch_should_persist(binding.bound_at, now) {
                persist_touch = true;
            }
            if binding.bound_at < now {
                binding.bound_at = now;
            }
        }
        persist_touch = runtime_continuation_status_should_persist_touch(
            &runtime.continuation_statuses,
            RuntimeContinuationBindingKind::SessionId,
            session_id,
            now,
        ) || persist_touch;
        let _ = runtime_mark_continuation_status_touched(
            &mut runtime.continuation_statuses,
            RuntimeContinuationBindingKind::SessionId,
            session_id,
            now,
        );
    }
    if persist_touch {
        schedule_runtime_binding_touch_save(
            shared,
            &runtime,
            RuntimeStateMutation::SessionTouch(session_id.to_string()),
        );
    }
    Ok(profile_name)
}
