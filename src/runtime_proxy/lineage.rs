use super::*;

pub(crate) fn schedule_runtime_binding_touch_save(
    shared: &RuntimeRotationProxyShared,
    runtime: &RuntimeRotationState,
    reason: &str,
) {
    schedule_runtime_state_save_from_runtime(shared, runtime, reason);
}

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
            &format!("continuation_stale:{previous_response_id}"),
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
            &format!("response_touch:{previous_response_id}"),
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
            &format!("continuation_stale:{turn_state}"),
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
            &format!("turn_state_touch:{turn_state}"),
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
            &format!("continuation_stale:{session_id}"),
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
            &format!("session_touch:{session_id}"),
        );
    }
    Ok(profile_name)
}

pub(crate) fn remember_runtime_turn_state(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    turn_state: Option<&str>,
    verified_route: RuntimeRouteKind,
) -> Result<()> {
    let Some(turn_state) = turn_state.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(());
    };

    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let bound_at = Local::now().timestamp();
    let mut changed = false;
    let should_refresh_binding = match runtime.turn_state_bindings.get_mut(turn_state) {
        Some(binding) if binding.profile_name == profile_name => {
            if binding.bound_at < bound_at {
                binding.bound_at = bound_at;
            }
            false
        }
        Some(binding) => {
            binding.profile_name = profile_name.to_string();
            binding.bound_at = bound_at;
            changed = true;
            true
        }
        None => {
            runtime.turn_state_bindings.insert(
                turn_state.to_string(),
                ResponseProfileBinding {
                    profile_name: profile_name.to_string(),
                    bound_at,
                },
            );
            changed = true;
            true
        }
    };
    if should_refresh_binding
        || runtime_continuation_status_should_refresh_verified(
            &runtime.continuation_statuses,
            RuntimeContinuationBindingKind::TurnState,
            turn_state,
            bound_at,
            Some(verified_route),
        )
    {
        changed = runtime_mark_continuation_status_verified(
            &mut runtime.continuation_statuses,
            RuntimeContinuationBindingKind::TurnState,
            turn_state,
            bound_at,
            Some(verified_route),
        ) || changed;
    }
    if changed {
        prune_profile_bindings(
            &mut runtime.turn_state_bindings,
            TURN_STATE_PROFILE_BINDING_LIMIT,
        );
        schedule_runtime_state_save_from_runtime(
            shared,
            &runtime,
            &format!("turn_state:{profile_name}"),
        );
        drop(runtime);
        runtime_proxy_log(
            shared,
            format!("binding turn_state profile={profile_name} value={turn_state}"),
        );
    } else {
        drop(runtime);
    }
    Ok(())
}

pub(crate) fn remember_runtime_session_id(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    session_id: Option<&str>,
    verified_route: RuntimeRouteKind,
) -> Result<()> {
    let Some(session_id) = session_id.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(());
    };

    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let bound_at = Local::now().timestamp();
    let mut changed = false;
    let mut should_refresh_binding = false;
    match runtime.session_id_bindings.get_mut(session_id) {
        Some(binding) if binding.profile_name == profile_name => {
            if binding.bound_at < bound_at {
                binding.bound_at = bound_at;
            }
        }
        Some(binding) => {
            binding.profile_name = profile_name.to_string();
            binding.bound_at = bound_at;
            changed = true;
            should_refresh_binding = true;
        }
        None => {
            runtime.session_id_bindings.insert(
                session_id.to_string(),
                ResponseProfileBinding {
                    profile_name: profile_name.to_string(),
                    bound_at,
                },
            );
            changed = true;
            should_refresh_binding = true;
        }
    }
    match runtime.state.session_profile_bindings.get_mut(session_id) {
        Some(binding) if binding.profile_name == profile_name => {
            if binding.bound_at < bound_at {
                binding.bound_at = bound_at;
            }
        }
        Some(binding) => {
            binding.profile_name = profile_name.to_string();
            binding.bound_at = bound_at;
            changed = true;
            should_refresh_binding = true;
        }
        None => {
            runtime.state.session_profile_bindings.insert(
                session_id.to_string(),
                ResponseProfileBinding {
                    profile_name: profile_name.to_string(),
                    bound_at,
                },
            );
            changed = true;
            should_refresh_binding = true;
        }
    }
    if should_refresh_binding
        || runtime_continuation_status_should_refresh_verified(
            &runtime.continuation_statuses,
            RuntimeContinuationBindingKind::SessionId,
            session_id,
            bound_at,
            Some(verified_route),
        )
    {
        changed = runtime_mark_continuation_status_verified(
            &mut runtime.continuation_statuses,
            RuntimeContinuationBindingKind::SessionId,
            session_id,
            bound_at,
            Some(verified_route),
        ) || changed;
    }
    if changed {
        prune_profile_bindings(
            &mut runtime.session_id_bindings,
            SESSION_ID_PROFILE_BINDING_LIMIT,
        );
        prune_profile_bindings(
            &mut runtime.state.session_profile_bindings,
            SESSION_ID_PROFILE_BINDING_LIMIT,
        );
        schedule_runtime_state_save_from_runtime(
            shared,
            &runtime,
            &format!("session_id:{profile_name}"),
        );
        drop(runtime);
        runtime_proxy_log(
            shared,
            format!("binding session_id profile={profile_name} value={session_id}"),
        );
    } else {
        drop(runtime);
    }
    Ok(())
}

pub(crate) fn remember_runtime_compact_lineage(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    session_id: Option<&str>,
    turn_state: Option<&str>,
    verified_route: RuntimeRouteKind,
) -> Result<()> {
    let session_id = session_id.map(str::trim).filter(|value| !value.is_empty());
    let turn_state = turn_state.map(str::trim).filter(|value| !value.is_empty());
    if session_id.is_none() && turn_state.is_none() {
        return Ok(());
    }

    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let bound_at = Local::now().timestamp();
    let mut changed = false;

    if let Some(session_id) = session_id {
        let key = runtime_compact_session_lineage_key(session_id);
        let should_refresh_binding = match runtime.session_id_bindings.get_mut(&key) {
            Some(binding) if binding.profile_name == profile_name => {
                if binding.bound_at < bound_at {
                    binding.bound_at = bound_at;
                }
                false
            }
            Some(binding) => {
                binding.profile_name = profile_name.to_string();
                binding.bound_at = bound_at;
                changed = true;
                true
            }
            None => {
                runtime.session_id_bindings.insert(
                    key.clone(),
                    ResponseProfileBinding {
                        profile_name: profile_name.to_string(),
                        bound_at,
                    },
                );
                changed = true;
                true
            }
        };
        if should_refresh_binding
            || runtime_continuation_status_should_refresh_verified(
                &runtime.continuation_statuses,
                RuntimeContinuationBindingKind::SessionId,
                &key,
                bound_at,
                Some(verified_route),
            )
        {
            changed = runtime_mark_continuation_status_verified(
                &mut runtime.continuation_statuses,
                RuntimeContinuationBindingKind::SessionId,
                &key,
                bound_at,
                Some(verified_route),
            ) || changed;
        }
    }

    if let Some(turn_state) = turn_state {
        let key = runtime_compact_turn_state_lineage_key(turn_state);
        let should_refresh_binding = match runtime.turn_state_bindings.get_mut(&key) {
            Some(binding) if binding.profile_name == profile_name => {
                if binding.bound_at < bound_at {
                    binding.bound_at = bound_at;
                }
                false
            }
            Some(binding) => {
                binding.profile_name = profile_name.to_string();
                binding.bound_at = bound_at;
                changed = true;
                true
            }
            None => {
                runtime.turn_state_bindings.insert(
                    key.clone(),
                    ResponseProfileBinding {
                        profile_name: profile_name.to_string(),
                        bound_at,
                    },
                );
                changed = true;
                true
            }
        };
        if should_refresh_binding
            || runtime_continuation_status_should_refresh_verified(
                &runtime.continuation_statuses,
                RuntimeContinuationBindingKind::TurnState,
                &key,
                bound_at,
                Some(verified_route),
            )
        {
            changed = runtime_mark_continuation_status_verified(
                &mut runtime.continuation_statuses,
                RuntimeContinuationBindingKind::TurnState,
                &key,
                bound_at,
                Some(verified_route),
            ) || changed;
        }
    }

    if changed {
        prune_profile_bindings(
            &mut runtime.turn_state_bindings,
            TURN_STATE_PROFILE_BINDING_LIMIT,
        );
        prune_profile_bindings(
            &mut runtime.session_id_bindings,
            SESSION_ID_PROFILE_BINDING_LIMIT,
        );
        schedule_runtime_state_save_from_runtime(
            shared,
            &runtime,
            &format!("compact_lineage:{profile_name}"),
        );
        drop(runtime);
    } else {
        drop(runtime);
    }
    Ok(())
}

pub(crate) fn release_runtime_compact_lineage(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    session_id: Option<&str>,
    turn_state: Option<&str>,
    reason: &str,
) -> Result<bool> {
    let session_id = session_id.map(str::trim).filter(|value| !value.is_empty());
    let turn_state = turn_state.map(str::trim).filter(|value| !value.is_empty());
    if session_id.is_none() && turn_state.is_none() {
        return Ok(false);
    }

    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let mut changed = false;
    let now = Local::now().timestamp();

    if let Some(session_id) = session_id {
        let key = runtime_compact_session_lineage_key(session_id);
        if runtime
            .session_id_bindings
            .get(&key)
            .is_some_and(|binding| binding.profile_name == profile_name)
        {
            runtime.session_id_bindings.remove(&key);
            let _ = runtime_mark_continuation_status_dead(
                &mut runtime.continuation_statuses,
                RuntimeContinuationBindingKind::SessionId,
                &key,
                now,
            );
            changed = true;
        }
    }

    if let Some(turn_state) = turn_state {
        let key = runtime_compact_turn_state_lineage_key(turn_state);
        if runtime
            .turn_state_bindings
            .get(&key)
            .is_some_and(|binding| binding.profile_name == profile_name)
        {
            runtime.turn_state_bindings.remove(&key);
            let _ = runtime_mark_continuation_status_dead(
                &mut runtime.continuation_statuses,
                RuntimeContinuationBindingKind::TurnState,
                &key,
                now,
            );
            changed = true;
        }
    }

    if changed {
        schedule_runtime_state_save_from_runtime(
            shared,
            &runtime,
            &format!("compact_lineage_release:{profile_name}"),
        );
        drop(runtime);
        runtime_proxy_log(
            shared,
            format!(
                "compact_lineage_released profile={profile_name} reason={reason} session={} turn_state={}",
                session_id.unwrap_or("-"),
                turn_state.unwrap_or("-"),
            ),
        );
    } else {
        drop(runtime);
    }
    Ok(changed)
}

#[cfg(test)]
pub(crate) fn remember_runtime_response_ids(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    response_ids: &[String],
    verified_route: RuntimeRouteKind,
) -> Result<()> {
    remember_runtime_response_ids_with_turn_state(
        shared,
        profile_name,
        response_ids,
        None,
        verified_route,
    )
}

pub(crate) fn remember_runtime_response_ids_with_turn_state(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    response_ids: &[String],
    turn_state: Option<&str>,
    verified_route: RuntimeRouteKind,
) -> Result<()> {
    if response_ids.is_empty() {
        return Ok(());
    }

    let turn_state = turn_state.map(str::trim).filter(|value| !value.is_empty());
    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let bound_at = Local::now().timestamp();
    let mut changed = false;
    let mut response_turn_state_changed = false;
    for response_id in response_ids {
        changed =
            clear_runtime_previous_response_negative_cache(&mut runtime, response_id, profile_name)
                || changed;
        let should_refresh_binding =
            match runtime.state.response_profile_bindings.get_mut(response_id) {
                Some(binding) if binding.profile_name == profile_name => {
                    if binding.bound_at < bound_at {
                        binding.bound_at = bound_at;
                    }
                    false
                }
                Some(binding) => {
                    binding.profile_name = profile_name.to_string();
                    binding.bound_at = bound_at;
                    changed = true;
                    true
                }
                None => {
                    runtime.state.response_profile_bindings.insert(
                        response_id.clone(),
                        ResponseProfileBinding {
                            profile_name: profile_name.to_string(),
                            bound_at,
                        },
                    );
                    changed = true;
                    true
                }
            };
        if should_refresh_binding
            || runtime_continuation_status_should_refresh_verified(
                &runtime.continuation_statuses,
                RuntimeContinuationBindingKind::Response,
                response_id,
                bound_at,
                Some(verified_route),
            )
        {
            changed = runtime_mark_continuation_status_verified(
                &mut runtime.continuation_statuses,
                RuntimeContinuationBindingKind::Response,
                response_id,
                bound_at,
                Some(verified_route),
            ) || changed;
        }
        if let Some(turn_state) = turn_state {
            let key = runtime_response_turn_state_lineage_key(response_id, turn_state);
            match runtime.state.response_profile_bindings.get_mut(&key) {
                Some(binding) if binding.profile_name == profile_name => {
                    if binding.bound_at < bound_at {
                        binding.bound_at = bound_at;
                    }
                }
                Some(binding) => {
                    binding.profile_name = profile_name.to_string();
                    binding.bound_at = bound_at;
                    changed = true;
                    response_turn_state_changed = true;
                }
                None => {
                    runtime.state.response_profile_bindings.insert(
                        key,
                        ResponseProfileBinding {
                            profile_name: profile_name.to_string(),
                            bound_at,
                        },
                    );
                    changed = true;
                    response_turn_state_changed = true;
                }
            }
        }
    }
    if changed {
        prune_profile_bindings(
            &mut runtime.state.response_profile_bindings,
            RESPONSE_PROFILE_BINDING_LIMIT,
        );
        schedule_runtime_state_save_from_runtime(
            shared,
            &runtime,
            &format!("response_ids:{profile_name}"),
        );
        drop(runtime);
        runtime_proxy_log(
            shared,
            format!(
                "binding response_ids profile={profile_name} count={} first={:?}",
                response_ids.len(),
                response_ids.first()
            ),
        );
        if response_turn_state_changed {
            runtime_proxy_log(
                shared,
                format!(
                    "binding response_turn_state profile={profile_name} count={} first={:?} turn_state={}",
                    response_ids.len(),
                    response_ids.first(),
                    turn_state.unwrap_or("-"),
                ),
            );
        } else if turn_state.is_none() {
            runtime_proxy_log(
                shared,
                format!(
                    "turn_state_coverage route={} profile={profile_name} status=missing response_ids={} first={:?}",
                    runtime_route_kind_label(verified_route),
                    response_ids.len(),
                    response_ids.first(),
                ),
            );
        }
    } else {
        drop(runtime);
    }
    Ok(())
}

pub(crate) fn runtime_previous_response_turn_state(
    shared: &RuntimeRotationProxyShared,
    previous_response_id: Option<&str>,
    bound_profile: Option<&str>,
) -> Result<Option<String>> {
    let Some(previous_response_id) = previous_response_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(None);
    };

    let prefix = format!(
        "{RUNTIME_RESPONSE_TURN_STATE_LINEAGE_PREFIX}{}:{previous_response_id}:",
        previous_response_id.len()
    );
    let runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let mut selected = None::<(i64, String)>;
    for (key, binding) in runtime
        .state
        .response_profile_bindings
        .range(prefix.clone()..)
    {
        if !key.starts_with(&prefix) {
            break;
        }
        if bound_profile.is_some_and(|profile_name| binding.profile_name != profile_name) {
            continue;
        }
        let Some((response_id, turn_state)) = runtime_response_turn_state_lineage_parts(key) else {
            continue;
        };
        if response_id != previous_response_id {
            continue;
        }
        let candidate = (binding.bound_at, turn_state.to_string());
        if selected
            .as_ref()
            .is_none_or(|(current_bound_at, _)| *current_bound_at <= candidate.0)
        {
            selected = Some(candidate);
        }
    }
    Ok(selected.map(|(_, turn_state)| turn_state))
}

pub(crate) fn clear_runtime_response_turn_state_lineage(
    bindings: &mut BTreeMap<String, ResponseProfileBinding>,
    previous_response_id: &str,
) -> bool {
    let prefix = format!(
        "{RUNTIME_RESPONSE_TURN_STATE_LINEAGE_PREFIX}{}:{previous_response_id}:",
        previous_response_id.len()
    );
    let keys = bindings
        .range(prefix.clone()..)
        .take_while(|(key, _)| key.starts_with(&prefix))
        .map(|(key, _)| key.clone())
        .collect::<Vec<_>>();
    let changed = !keys.is_empty();
    for key in keys {
        bindings.remove(&key);
    }
    changed
}

pub(crate) fn clear_runtime_dead_response_bindings(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    response_ids: &[String],
    reason: &str,
) -> Result<bool> {
    let response_ids = response_ids
        .iter()
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .collect::<BTreeSet<_>>();
    if response_ids.is_empty() {
        return Ok(false);
    }

    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let now = Local::now().timestamp();
    let mut changed = false;
    for response_id in &response_ids {
        if runtime
            .state
            .response_profile_bindings
            .get(*response_id)
            .is_some_and(|binding| binding.profile_name != profile_name)
        {
            continue;
        }
        changed = runtime
            .state
            .response_profile_bindings
            .remove(*response_id)
            .is_some()
            || changed;
        changed = clear_runtime_response_turn_state_lineage(
            &mut runtime.state.response_profile_bindings,
            response_id,
        ) || changed;
        changed = runtime_mark_continuation_status_dead(
            &mut runtime.continuation_statuses,
            RuntimeContinuationBindingKind::Response,
            response_id,
            now,
        ) || changed;
    }

    if changed {
        schedule_runtime_state_save_from_runtime(
            shared,
            &runtime,
            &format!("dead_response_binding_clear:{profile_name}"),
        );
        drop(runtime);
        runtime_proxy_log(
            shared,
            format!(
                "response_bindings_cleared profile={profile_name} count={} first={:?} reason={reason}",
                response_ids.len(),
                response_ids.first().copied(),
            ),
        );
    } else {
        drop(runtime);
    }

    Ok(changed)
}

pub(crate) fn remember_runtime_successful_previous_response_owner(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    previous_response_id: Option<&str>,
    verified_route: RuntimeRouteKind,
) -> Result<()> {
    let Some(previous_response_id) = previous_response_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(());
    };

    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let bound_at = Local::now().timestamp();
    let mut changed = clear_runtime_previous_response_negative_cache(
        &mut runtime,
        previous_response_id,
        profile_name,
    );
    let should_refresh_binding = match runtime
        .state
        .response_profile_bindings
        .get_mut(previous_response_id)
    {
        Some(binding) if binding.profile_name == profile_name => {
            if binding.bound_at < bound_at {
                binding.bound_at = bound_at;
            }
            false
        }
        Some(binding) => {
            binding.profile_name = profile_name.to_string();
            binding.bound_at = bound_at;
            changed = true;
            true
        }
        None => {
            runtime.state.response_profile_bindings.insert(
                previous_response_id.to_string(),
                ResponseProfileBinding {
                    profile_name: profile_name.to_string(),
                    bound_at,
                },
            );
            changed = true;
            true
        }
    };
    if should_refresh_binding
        || runtime_continuation_status_should_refresh_verified(
            &runtime.continuation_statuses,
            RuntimeContinuationBindingKind::Response,
            previous_response_id,
            bound_at,
            Some(verified_route),
        )
    {
        changed = runtime_mark_continuation_status_verified(
            &mut runtime.continuation_statuses,
            RuntimeContinuationBindingKind::Response,
            previous_response_id,
            bound_at,
            Some(verified_route),
        ) || changed;
    }
    if changed {
        prune_profile_bindings(
            &mut runtime.state.response_profile_bindings,
            RESPONSE_PROFILE_BINDING_LIMIT,
        );
        schedule_runtime_state_save_from_runtime(
            shared,
            &runtime,
            &format!("previous_response_owner:{profile_name}"),
        );
        drop(runtime);
        runtime_proxy_log(
            shared,
            format!(
                "binding previous_response_owner profile={profile_name} response_id={previous_response_id}"
            ),
        );
    } else {
        drop(runtime);
    }
    Ok(())
}

pub(crate) fn clear_runtime_stale_previous_response_binding(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    previous_response_id: Option<&str>,
) -> Result<bool> {
    let Some(previous_response_id) = previous_response_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(false);
    };

    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    if runtime
        .state
        .response_profile_bindings
        .get(previous_response_id)
        .is_none_or(|binding| binding.profile_name != profile_name)
    {
        drop(runtime);
        return Ok(false);
    }

    runtime
        .state
        .response_profile_bindings
        .remove(previous_response_id);
    let _ = clear_runtime_response_turn_state_lineage(
        &mut runtime.state.response_profile_bindings,
        previous_response_id,
    );
    let now = Local::now().timestamp();
    let _ = runtime_mark_continuation_status_dead(
        &mut runtime.continuation_statuses,
        RuntimeContinuationBindingKind::Response,
        previous_response_id,
        now,
    );
    schedule_runtime_state_save_from_runtime(
        shared,
        &runtime,
        &format!("previous_response_binding_clear:{profile_name}"),
    );
    drop(runtime);
    runtime_proxy_log(
        shared,
        format!(
            "previous_response_binding_cleared profile={profile_name} response_id={previous_response_id}"
        ),
    );
    Ok(true)
}

pub(crate) fn release_runtime_quota_blocked_affinity(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    previous_response_id: Option<&str>,
    turn_state: Option<&str>,
    session_id: Option<&str>,
) -> Result<bool> {
    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let mut changed = false;
    let now = Local::now().timestamp();
    let release_session_affinity = previous_response_id.is_none() && turn_state.is_none();

    if let Some(previous_response_id) = previous_response_id
        && runtime
            .state
            .response_profile_bindings
            .get(previous_response_id)
            .is_some_and(|binding| binding.profile_name == profile_name)
    {
        runtime
            .state
            .response_profile_bindings
            .remove(previous_response_id);
        let _ = clear_runtime_response_turn_state_lineage(
            &mut runtime.state.response_profile_bindings,
            previous_response_id,
        );
        let _ = runtime_mark_continuation_status_dead(
            &mut runtime.continuation_statuses,
            RuntimeContinuationBindingKind::Response,
            previous_response_id,
            now,
        );
        changed = true;
    }

    if let Some(turn_state) = turn_state
        && runtime
            .turn_state_bindings
            .get(turn_state)
            .is_some_and(|binding| binding.profile_name == profile_name)
    {
        runtime.turn_state_bindings.remove(turn_state);
        let _ = runtime_mark_continuation_status_dead(
            &mut runtime.continuation_statuses,
            RuntimeContinuationBindingKind::TurnState,
            turn_state,
            now,
        );
        changed = true;
    }

    // Dropping previous_response or turn_state affinity should not also erase an existing
    // session lineage. Fresh fallback may still need that session owner to preserve compact
    // context or to reapply soft session affinity on the next selection pass.
    if release_session_affinity
        && let Some(session_id) = session_id
        && runtime
            .session_id_bindings
            .get(session_id)
            .is_some_and(|binding| binding.profile_name == profile_name)
    {
        runtime.session_id_bindings.remove(session_id);
        runtime.state.session_profile_bindings.remove(session_id);
        let _ = runtime_mark_continuation_status_dead(
            &mut runtime.continuation_statuses,
            RuntimeContinuationBindingKind::SessionId,
            session_id,
            now,
        );
        changed = true;
    }

    if changed {
        schedule_runtime_state_save_from_runtime(
            shared,
            &runtime,
            &format!("quota_release:{profile_name}"),
        );
        drop(runtime);
        runtime_proxy_log(
            shared,
            format!(
                "quota_release_affinity profile={profile_name} previous_response_id={:?} turn_state={:?} session_id={:?}",
                previous_response_id, turn_state, session_id
            ),
        );
    } else {
        drop(runtime);
    }

    Ok(changed)
}

pub(crate) fn release_runtime_previous_response_affinity(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    previous_response_id: Option<&str>,
    turn_state: Option<&str>,
    session_id: Option<&str>,
    route_kind: RuntimeRouteKind,
) -> Result<bool> {
    let previous_response_failures = note_runtime_previous_response_not_found(
        shared,
        profile_name,
        previous_response_id,
        route_kind,
    )?;
    if previous_response_failures < RUNTIME_PREVIOUS_RESPONSE_NEGATIVE_CACHE_FAILURE_THRESHOLD {
        runtime_proxy_log(
            shared,
            format!(
                "previous_response_release_deferred profile={profile_name} route={} previous_response_id={:?} failures={previous_response_failures}",
                runtime_route_kind_label(route_kind),
                previous_response_id,
            ),
        );
        return Ok(false);
    }
    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let mut changed = false;
    let now = Local::now().timestamp();
    let release_session_affinity = previous_response_id.is_none() && turn_state.is_none();

    if let Some(previous_response_id) = previous_response_id
        && runtime
            .state
            .response_profile_bindings
            .get(previous_response_id)
            .is_some_and(|binding| binding.profile_name == profile_name)
    {
        runtime
            .state
            .response_profile_bindings
            .remove(previous_response_id);
        let _ = clear_runtime_response_turn_state_lineage(
            &mut runtime.state.response_profile_bindings,
            previous_response_id,
        );
        let _ = runtime_mark_continuation_status_dead(
            &mut runtime.continuation_statuses,
            RuntimeContinuationBindingKind::Response,
            previous_response_id,
            now,
        );
        changed = true;
    }

    if let Some(turn_state) = turn_state
        && runtime
            .turn_state_bindings
            .get(turn_state)
            .is_some_and(|binding| binding.profile_name == profile_name)
    {
        runtime.turn_state_bindings.remove(turn_state);
        let _ = runtime_mark_continuation_status_dead(
            &mut runtime.continuation_statuses,
            RuntimeContinuationBindingKind::TurnState,
            turn_state,
            now,
        );
        changed = true;
    }

    if release_session_affinity
        && let Some(session_id) = session_id
        && runtime
            .session_id_bindings
            .get(session_id)
            .is_some_and(|binding| binding.profile_name == profile_name)
    {
        runtime.session_id_bindings.remove(session_id);
        runtime.state.session_profile_bindings.remove(session_id);
        let _ = runtime_mark_continuation_status_dead(
            &mut runtime.continuation_statuses,
            RuntimeContinuationBindingKind::SessionId,
            session_id,
            now,
        );
        changed = true;
    }

    if changed {
        schedule_runtime_state_save_from_runtime(
            shared,
            &runtime,
            &format!("previous_response_release:{profile_name}"),
        );
        drop(runtime);
        runtime_proxy_log(
            shared,
            format!(
                "previous_response_release_affinity profile={profile_name} previous_response_id={:?} turn_state={:?} session_id={:?}",
                previous_response_id, turn_state, session_id
            ),
        );
    } else {
        drop(runtime);
    }

    Ok(changed)
}

pub(crate) fn prune_profile_bindings(
    bindings: &mut BTreeMap<String, ResponseProfileBinding>,
    max_entries: usize,
) {
    if bindings.len() <= max_entries {
        return;
    }

    let excess = bindings.len() - max_entries;
    let mut oldest = bindings
        .iter()
        .map(|(response_id, binding)| (response_id.clone(), binding.bound_at))
        .collect::<Vec<_>>();
    oldest.sort_by_key(|(_, bound_at)| *bound_at);

    for (response_id, _) in oldest.into_iter().take(excess) {
        bindings.remove(&response_id);
    }
}
