use super::*;

mod admission;
mod affinity;
mod classification;
mod continuation;
mod dispatch;
mod lifecycle;
mod standard;
mod websocket;

pub(crate) use self::admission::*;
pub(crate) use self::affinity::*;
pub(crate) use self::classification::*;
pub(super) use self::continuation::*;
pub(crate) use self::dispatch::*;
pub(crate) use self::lifecycle::*;
pub(crate) use self::standard::*;
pub(crate) use self::websocket::*;

pub(super) fn schedule_runtime_binding_touch_save(
    shared: &RuntimeRotationProxyShared,
    runtime: &RuntimeRotationState,
    reason: &str,
) {
    schedule_runtime_state_save_from_runtime(shared, runtime, reason);
}

pub(super) fn runtime_response_bound_profile(
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

pub(super) fn runtime_turn_state_bound_profile(
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

pub(super) fn runtime_session_bound_profile(
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

pub(super) fn remember_runtime_turn_state(
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

pub(super) fn remember_runtime_session_id(
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

pub(super) fn remember_runtime_compact_lineage(
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

pub(super) fn release_runtime_compact_lineage(
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
pub(super) fn remember_runtime_response_ids(
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

pub(super) fn remember_runtime_response_ids_with_turn_state(
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
        }
    } else {
        drop(runtime);
    }
    Ok(())
}

pub(super) fn runtime_previous_response_turn_state(
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

pub(super) fn clear_runtime_response_turn_state_lineage(
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

pub(super) fn remember_runtime_successful_previous_response_owner(
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

pub(super) fn clear_runtime_stale_previous_response_binding(
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

pub(super) fn release_runtime_quota_blocked_affinity(
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

pub(super) fn release_runtime_previous_response_affinity(
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

pub(super) fn prune_profile_bindings(
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

pub(super) fn runtime_previous_response_retry_delay(retry_index: usize) -> Option<Duration> {
    RUNTIME_PREVIOUS_RESPONSE_RETRY_DELAYS_MS
        .get(retry_index)
        .copied()
        .map(Duration::from_millis)
}

pub(super) fn runtime_proxy_precommit_budget_exhausted(
    started_at: Instant,
    attempts: usize,
    continuation: bool,
    pressure_mode: bool,
) -> bool {
    let (attempt_limit, budget) = runtime_proxy_precommit_budget(continuation, pressure_mode);

    attempts >= attempt_limit || started_at.elapsed() >= budget
}

pub(super) fn runtime_proxy_final_retryable_http_failure_response(
    last_failure: Option<(tiny_http::ResponseBox, bool)>,
    saw_inflight_saturation: bool,
    json_errors: bool,
) -> Option<tiny_http::ResponseBox> {
    let service_unavailable = |message: &str| {
        if json_errors {
            build_runtime_proxy_json_error_response(503, "service_unavailable", message)
        } else {
            build_runtime_proxy_text_response(503, message)
        }
    };
    match last_failure {
        Some((response, false)) => Some(response),
        Some((_response, true)) if saw_inflight_saturation => Some(service_unavailable(
            "All runtime auto-rotate candidates are temporarily saturated. Retry the request.",
        )),
        Some((_response, true)) => Some(service_unavailable(
            runtime_proxy_local_selection_failure_message(),
        )),
        None if saw_inflight_saturation => Some(service_unavailable(
            "All runtime auto-rotate candidates are temporarily saturated. Retry the request.",
        )),
        None => None,
    }
}

pub(super) fn runtime_proxy_final_responses_failure_reply(
    last_failure: Option<(RuntimeUpstreamFailureResponse, bool)>,
    saw_inflight_saturation: bool,
) -> RuntimeResponsesReply {
    match last_failure {
        Some((failure, false)) => match failure {
            RuntimeUpstreamFailureResponse::Http(response) => response,
            RuntimeUpstreamFailureResponse::Websocket(_) => {
                RuntimeResponsesReply::Buffered(build_runtime_proxy_json_error_parts(
                    503,
                    "service_unavailable",
                    runtime_proxy_local_selection_failure_message(),
                ))
            }
        },
        _ if saw_inflight_saturation => {
            RuntimeResponsesReply::Buffered(build_runtime_proxy_json_error_parts(
                503,
                "service_unavailable",
                "All runtime auto-rotate candidates are temporarily saturated. Retry the request.",
            ))
        }
        _ => RuntimeResponsesReply::Buffered(build_runtime_proxy_json_error_parts(
            503,
            "service_unavailable",
            runtime_proxy_local_selection_failure_message(),
        )),
    }
}

pub(super) fn send_runtime_proxy_final_websocket_failure(
    local_socket: &mut RuntimeLocalWebSocket,
    last_failure: Option<(RuntimeUpstreamFailureResponse, bool)>,
    saw_inflight_saturation: bool,
) -> Result<()> {
    match last_failure {
        Some((failure, false)) => match failure {
            RuntimeUpstreamFailureResponse::Websocket(payload) => {
                forward_runtime_proxy_websocket_error(local_socket, &payload)
            }
            RuntimeUpstreamFailureResponse::Http(_) => send_runtime_proxy_websocket_error(
                local_socket,
                503,
                "service_unavailable",
                runtime_proxy_local_selection_failure_message(),
            ),
        },
        _ if saw_inflight_saturation => send_runtime_proxy_websocket_error(
            local_socket,
            503,
            "service_unavailable",
            "All runtime auto-rotate candidates are temporarily saturated. Retry the request.",
        ),
        _ => send_runtime_proxy_websocket_error(
            local_socket,
            503,
            "service_unavailable",
            runtime_proxy_local_selection_failure_message(),
        ),
    }
}

pub(super) fn runtime_proxy_precommit_budget(
    continuation: bool,
    pressure_mode: bool,
) -> (usize, Duration) {
    if continuation {
        (
            RUNTIME_PROXY_PRECOMMIT_CONTINUATION_ATTEMPT_LIMIT,
            Duration::from_millis(RUNTIME_PROXY_PRECOMMIT_CONTINUATION_BUDGET_MS),
        )
    } else if pressure_mode {
        (
            RUNTIME_PROXY_PRESSURE_PRECOMMIT_ATTEMPT_LIMIT,
            Duration::from_millis(RUNTIME_PROXY_PRESSURE_PRECOMMIT_BUDGET_MS),
        )
    } else {
        (
            RUNTIME_PROXY_PRECOMMIT_ATTEMPT_LIMIT,
            Duration::from_millis(RUNTIME_PROXY_PRECOMMIT_BUDGET_MS),
        )
    }
}

pub(super) fn runtime_proxy_has_continuation_priority(
    previous_response_id: Option<&str>,
    pinned_profile: Option<&str>,
    request_turn_state: Option<&str>,
    turn_state_profile: Option<&str>,
    session_profile: Option<&str>,
) -> bool {
    previous_response_id.is_some()
        || pinned_profile.is_some()
        || request_turn_state.is_some()
        || turn_state_profile.is_some()
        || session_profile.is_some()
}

pub(super) fn runtime_wait_affinity_owner<'a>(
    strict_affinity_profile: Option<&'a str>,
    pinned_profile: Option<&'a str>,
    turn_state_profile: Option<&'a str>,
    session_profile: Option<&'a str>,
    trusted_previous_response_affinity: bool,
) -> Option<&'a str> {
    strict_affinity_profile
        .or(turn_state_profile)
        .or_else(|| {
            trusted_previous_response_affinity
                .then_some(pinned_profile)
                .flatten()
        })
        .or(session_profile)
}

pub(super) fn runtime_noncompact_session_priority_profile<'a>(
    session_profile: Option<&'a str>,
    compact_session_profile: Option<&str>,
) -> Option<&'a str> {
    if compact_session_profile.is_some_and(|profile_name| session_profile == Some(profile_name)) {
        None
    } else {
        session_profile
    }
}

pub(super) fn runtime_proxy_allows_direct_current_profile_fallback(
    previous_response_id: Option<&str>,
    pinned_profile: Option<&str>,
    request_turn_state: Option<&str>,
    turn_state_profile: Option<&str>,
    session_profile: Option<&str>,
    saw_inflight_saturation: bool,
    saw_upstream_failure: bool,
) -> bool {
    previous_response_id.is_none()
        && pinned_profile.is_none()
        && request_turn_state.is_none()
        && turn_state_profile.is_none()
        && session_profile.is_none()
        && !saw_inflight_saturation
        && !saw_upstream_failure
}

pub(super) fn runtime_profile_codex_home(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
) -> Result<Option<PathBuf>> {
    let runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    Ok(runtime
        .state
        .profiles
        .get(profile_name)
        .map(|profile| profile.codex_home.clone()))
}

#[cfg(test)]
pub(super) fn runtime_has_alternative_quota_compatible_profile(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
) -> Result<bool> {
    let allow_disk_fallback = !runtime_proxy_sync_probe_pressure_mode_active_for_route(
        shared,
        RuntimeRouteKind::Responses,
    );
    let fallback_profiles = {
        let runtime = shared
            .runtime
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
        let mut fallback_profiles = Vec::new();
        for (name, profile) in &runtime.state.profiles {
            if name == profile_name {
                continue;
            }
            if runtime_profile_cached_auth_summary_for_selection(
                runtime.profile_usage_auth.get(name).cloned(),
                runtime.profile_probe_cache.get(name).cloned(),
            )
            .is_some_and(|summary| summary.quota_compatible)
            {
                return Ok(true);
            }
            fallback_profiles.push((profile.codex_home.clone(), profile.provider.clone()));
        }
        fallback_profiles
    };
    if !allow_disk_fallback {
        return Ok(false);
    }
    Ok(fallback_profiles
        .into_iter()
        .any(|(codex_home, provider)| provider.auth_summary(&codex_home).quota_compatible))
}

pub(super) fn runtime_has_route_eligible_quota_fallback(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    excluded_profiles: &BTreeSet<String>,
    route_kind: RuntimeRouteKind,
) -> Result<bool> {
    let now = Local::now().timestamp();
    let pressure_mode = runtime_proxy_pressure_mode_active_for_route(shared, route_kind);
    let allow_disk_auth_fallback =
        !runtime_proxy_sync_probe_pressure_mode_active_for_route(shared, route_kind);
    let inflight_soft_limit = runtime_profile_inflight_soft_limit(route_kind, pressure_mode);
    let (
        state,
        retry_backoff_until,
        transport_backoff_until,
        route_circuit_open_until,
        profile_inflight,
        profile_health,
        profile_usage_auth,
        profile_probe_cache,
    ) = {
        let mut runtime = shared
            .runtime
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
        prune_runtime_profile_selection_backoff(&mut runtime, now);
        (
            runtime.state.clone(),
            runtime.profile_retry_backoff_until.clone(),
            runtime.profile_transport_backoff_until.clone(),
            runtime.profile_route_circuit_open_until.clone(),
            runtime.profile_inflight.clone(),
            runtime.profile_health.clone(),
            runtime.profile_usage_auth.clone(),
            runtime.profile_probe_cache.clone(),
        )
    };
    for (name, profile) in &state.profiles {
        if name == profile_name || excluded_profiles.contains(name) {
            continue;
        }
        if !runtime_profile_auth_summary_for_selection_with_policy(
            name,
            &profile.codex_home,
            &profile_usage_auth,
            &profile_probe_cache,
            allow_disk_auth_fallback,
        )
        .is_some_and(|summary| summary.quota_compatible)
        {
            continue;
        }
        if runtime_profile_auth_failure_active_with_auth_cache(
            &profile_health,
            &profile_usage_auth,
            name,
            now,
        ) {
            continue;
        }
        if runtime_profile_name_in_selection_backoff(
            name,
            &retry_backoff_until,
            &transport_backoff_until,
            &route_circuit_open_until,
            route_kind,
            now,
        ) {
            continue;
        }
        if runtime_profile_inflight_sort_key(name, &profile_inflight) >= inflight_soft_limit {
            continue;
        }
        return Ok(true);
    }
    Ok(false)
}

pub(super) fn refresh_runtime_profile_quota_inline(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    context: &str,
) -> Result<()> {
    let Some(codex_home) = runtime_profile_codex_home(shared, profile_name)? else {
        return Ok(());
    };
    runtime_proxy_log(shared, format!("{context}_start profile={profile_name}"));
    run_runtime_probe_jobs_inline(
        shared,
        vec![(profile_name.to_string(), codex_home)],
        context,
    );
    Ok(())
}

pub(super) fn runtime_quota_summary_requires_precommit_live_probe(
    summary: RuntimeQuotaSummary,
    source: Option<RuntimeQuotaSource>,
    route_kind: RuntimeRouteKind,
) -> bool {
    matches!(
        route_kind,
        RuntimeRouteKind::Responses | RuntimeRouteKind::Websocket
    ) && !matches!(source, Some(RuntimeQuotaSource::LiveProbe))
        && (matches!(summary.five_hour.status, RuntimeQuotaWindowStatus::Critical)
            || matches!(summary.weekly.status, RuntimeQuotaWindowStatus::Critical)
            || matches!(summary.five_hour.status, RuntimeQuotaWindowStatus::Unknown)
            || matches!(summary.weekly.status, RuntimeQuotaWindowStatus::Unknown))
}

pub(super) fn runtime_quota_summary_requires_live_source_after_probe(
    summary: RuntimeQuotaSummary,
    source: Option<RuntimeQuotaSource>,
    route_kind: RuntimeRouteKind,
) -> bool {
    matches!(
        route_kind,
        RuntimeRouteKind::Responses | RuntimeRouteKind::Websocket
    ) && !matches!(source, Some(RuntimeQuotaSource::LiveProbe))
        && (matches!(summary.five_hour.status, RuntimeQuotaWindowStatus::Unknown)
            || matches!(summary.weekly.status, RuntimeQuotaWindowStatus::Unknown))
}

pub(super) fn ensure_runtime_profile_precommit_quota_ready(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
    context: &str,
) -> Result<(RuntimeQuotaSummary, Option<RuntimeQuotaSource>)> {
    let (mut quota_summary, mut quota_source) =
        runtime_profile_quota_summary_for_route(shared, profile_name, route_kind)?;
    if runtime_quota_summary_requires_precommit_live_probe(quota_summary, quota_source, route_kind)
    {
        refresh_runtime_profile_quota_inline(shared, profile_name, context)?;
        (quota_summary, quota_source) =
            runtime_profile_quota_summary_for_route(shared, profile_name, route_kind)?;
    }
    Ok((quota_summary, quota_source))
}

pub(super) fn runtime_proxy_direct_current_fallback_profile(
    shared: &RuntimeRotationProxyShared,
    excluded_profiles: &BTreeSet<String>,
    route_kind: RuntimeRouteKind,
) -> Result<Option<String>> {
    let (profile_name, codex_home, auth_failure_active, cached_usage_auth_entry, probe_cache_entry) = {
        let runtime = shared
            .runtime
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
        let profile_name = runtime.current_profile.clone();
        let Some(profile) = runtime.state.profiles.get(&profile_name) else {
            return Ok(None);
        };
        let now = Local::now().timestamp();
        (
            profile_name.clone(),
            profile.codex_home.clone(),
            runtime_profile_auth_failure_active(&runtime, &profile_name, now),
            runtime.profile_usage_auth.get(&profile_name).cloned(),
            runtime.profile_probe_cache.get(&profile_name).cloned(),
        )
    };
    if excluded_profiles.contains(&profile_name) {
        return Ok(None);
    }
    if auth_failure_active {
        runtime_proxy_log(
            shared,
            format!(
                "selection_skip_current route={} profile={} reason=auth_failure_backoff",
                runtime_route_kind_label(route_kind),
                profile_name,
            ),
        );
        return Ok(None);
    }
    let allow_disk_auth_fallback =
        !runtime_proxy_sync_probe_pressure_mode_active_for_route(shared, route_kind);
    if !runtime_profile_cached_auth_summary_for_selection(
        cached_usage_auth_entry,
        probe_cache_entry,
    )
    .unwrap_or_else(|| {
        if allow_disk_auth_fallback {
            read_auth_summary(&codex_home)
        } else {
            AuthSummary {
                label: "uncached-auth".to_string(),
                quota_compatible: false,
            }
        }
    })
    .quota_compatible
    {
        return Ok(None);
    }
    if runtime_profile_inflight_hard_limited_for_context(
        shared,
        &profile_name,
        runtime_route_kind_inflight_context(route_kind),
    )? {
        return Ok(None);
    }
    if matches!(
        route_kind,
        RuntimeRouteKind::Responses | RuntimeRouteKind::Websocket
    ) {
        let (quota_summary, quota_source) =
            runtime_profile_quota_summary_for_route(shared, &profile_name, route_kind)?;
        if quota_source.is_none()
            || runtime_quota_summary_requires_live_source_after_probe(
                quota_summary,
                quota_source,
                route_kind,
            )
            || runtime_quota_precommit_guard_reason(quota_summary, route_kind).is_some()
        {
            runtime_proxy_log(
                shared,
                format!(
                    "selection_skip_current route={} profile={} reason={} quota_source={} {}",
                    runtime_route_kind_label(route_kind),
                    profile_name,
                    runtime_quota_precommit_guard_reason(quota_summary, route_kind).unwrap_or_else(
                        || {
                            runtime_quota_soft_affinity_rejection_reason(
                                quota_summary,
                                quota_source,
                                route_kind,
                            )
                        }
                    ),
                    quota_source
                        .map(runtime_quota_source_label)
                        .unwrap_or("unknown"),
                    runtime_quota_summary_log_fields(quota_summary),
                ),
            );
            return Ok(None);
        }
    }
    Ok(Some(profile_name))
}

pub(super) fn runtime_proxy_local_selection_failure_message() -> &'static str {
    "Runtime proxy could not secure a healthy upstream profile before the pre-commit retry budget was exhausted. Retry the request."
}

pub(super) fn runtime_quota_window_usable_for_auto_rotate(
    status: RuntimeQuotaWindowStatus,
) -> bool {
    matches!(
        status,
        RuntimeQuotaWindowStatus::Ready
            | RuntimeQuotaWindowStatus::Thin
            | RuntimeQuotaWindowStatus::Critical
    )
}

pub(super) fn runtime_quota_summary_allows_soft_affinity(
    summary: RuntimeQuotaSummary,
    source: Option<RuntimeQuotaSource>,
    route_kind: RuntimeRouteKind,
) -> bool {
    source.is_some()
        && runtime_quota_window_usable_for_auto_rotate(summary.five_hour.status)
        && runtime_quota_window_usable_for_auto_rotate(summary.weekly.status)
        && runtime_quota_precommit_guard_reason(summary, route_kind).is_none()
}

pub(super) fn runtime_quota_soft_affinity_rejection_reason(
    summary: RuntimeQuotaSummary,
    source: Option<RuntimeQuotaSource>,
    route_kind: RuntimeRouteKind,
) -> &'static str {
    if source.is_none()
        || matches!(summary.five_hour.status, RuntimeQuotaWindowStatus::Unknown)
        || matches!(summary.weekly.status, RuntimeQuotaWindowStatus::Unknown)
    {
        "quota_windows_unavailable"
    } else if let Some(reason) = runtime_quota_precommit_guard_reason(summary, route_kind) {
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

pub(super) fn runtime_quota_source_sort_key(
    route_kind: RuntimeRouteKind,
    source: RuntimeQuotaSource,
) -> usize {
    match (route_kind, source) {
        (
            RuntimeRouteKind::Responses | RuntimeRouteKind::Websocket,
            RuntimeQuotaSource::LiveProbe,
        ) => 0,
        (
            RuntimeRouteKind::Responses | RuntimeRouteKind::Websocket,
            RuntimeQuotaSource::PersistedSnapshot,
        ) => 1,
        _ => 0,
    }
}

pub(super) fn runtime_profile_quota_summary_for_route(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) -> Result<(RuntimeQuotaSummary, Option<RuntimeQuotaSource>)> {
    let runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let now = Local::now().timestamp();
    Ok(runtime
        .profile_probe_cache
        .get(profile_name)
        .filter(|entry| runtime_profile_usage_cache_is_fresh(entry, now))
        .and_then(|entry| entry.result.as_ref().ok())
        .map(|usage| {
            (
                runtime_quota_summary_for_route(usage, route_kind),
                Some(RuntimeQuotaSource::LiveProbe),
            )
        })
        .or_else(|| {
            runtime
                .profile_usage_snapshots
                .get(profile_name)
                .filter(|snapshot| runtime_usage_snapshot_is_usable(snapshot, now))
                .map(|snapshot| {
                    (
                        runtime_quota_summary_from_usage_snapshot_at(snapshot, route_kind, now),
                        Some(RuntimeQuotaSource::PersistedSnapshot),
                    )
                })
        })
        .unwrap_or((
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
        )))
}

pub(super) fn runtime_profile_cached_auth_summary_for_selection(
    usage_auth_entry: Option<RuntimeProfileUsageAuthCacheEntry>,
    probe_entry: Option<RuntimeProfileProbeCacheEntry>,
) -> Option<AuthSummary> {
    if let Some(entry) = usage_auth_entry {
        match runtime_profile_usage_auth_cache_entry_freshness(&entry) {
            RuntimeProfileUsageAuthCacheFreshness::Fresh => {
                return Some(AuthSummary {
                    label: "chatgpt".to_string(),
                    quota_compatible: true,
                });
            }
            RuntimeProfileUsageAuthCacheFreshness::Stale
            | RuntimeProfileUsageAuthCacheFreshness::Unknown => {}
        }
    }
    probe_entry.map(|entry| entry.auth)
}

pub(super) fn runtime_profile_cached_auth_summary_from_maps_for_selection(
    profile_name: &str,
    profile_usage_auth: &BTreeMap<String, RuntimeProfileUsageAuthCacheEntry>,
    profile_probe_cache: &BTreeMap<String, RuntimeProfileProbeCacheEntry>,
) -> Option<AuthSummary> {
    runtime_profile_cached_auth_summary_for_selection(
        profile_usage_auth.get(profile_name).cloned(),
        profile_probe_cache.get(profile_name).cloned(),
    )
}

pub(super) fn runtime_snapshot_blocks_same_request_cold_start_probe(
    snapshot: &RuntimeProfileUsageSnapshot,
    route_kind: RuntimeRouteKind,
    now: i64,
) -> bool {
    matches!(
        route_kind,
        RuntimeRouteKind::Responses | RuntimeRouteKind::Websocket
    ) && runtime_usage_snapshot_is_usable(snapshot, now)
        && runtime_quota_precommit_guard_reason(
            runtime_quota_summary_from_usage_snapshot_at(snapshot, route_kind, now),
            route_kind,
        )
        .is_some()
}

#[cfg(test)]
pub(super) fn runtime_profile_auth_summary_for_selection(
    profile_name: &str,
    codex_home: &Path,
    profile_usage_auth: &BTreeMap<String, RuntimeProfileUsageAuthCacheEntry>,
    profile_probe_cache: &BTreeMap<String, RuntimeProfileProbeCacheEntry>,
) -> AuthSummary {
    runtime_profile_auth_summary_for_selection_with_policy(
        profile_name,
        codex_home,
        profile_usage_auth,
        profile_probe_cache,
        true,
    )
    .unwrap_or_else(runtime_profile_uncached_auth_summary_for_selection)
}

pub(super) fn runtime_profile_uncached_auth_summary_for_selection() -> AuthSummary {
    AuthSummary {
        label: "uncached-auth".to_string(),
        quota_compatible: false,
    }
}

pub(super) fn runtime_profile_auth_summary_for_selection_with_policy(
    profile_name: &str,
    codex_home: &Path,
    profile_usage_auth: &BTreeMap<String, RuntimeProfileUsageAuthCacheEntry>,
    profile_probe_cache: &BTreeMap<String, RuntimeProfileProbeCacheEntry>,
    allow_disk_fallback: bool,
) -> Option<AuthSummary> {
    runtime_profile_cached_auth_summary_from_maps_for_selection(
        profile_name,
        profile_usage_auth,
        profile_probe_cache,
    )
    .or_else(|| allow_disk_fallback.then(|| read_auth_summary(codex_home)))
}

pub(super) fn runtime_proxy_sync_probe_pressure_pause(
    shared: &RuntimeRotationProxyShared,
    route_kind: RuntimeRouteKind,
) {
    if !runtime_proxy_sync_probe_pressure_mode_active_for_route(shared, route_kind) {
        return;
    }
    let pause_ms = RUNTIME_PROXY_SYNC_PROBE_PRESSURE_PAUSE_MS;
    let observed_revision = runtime_probe_refresh_revision();
    let started_at = Instant::now();
    let wait_outcome = runtime_probe_refresh_wait_outcome_since(
        Duration::from_millis(pause_ms),
        observed_revision,
    );
    runtime_proxy_log(
        shared,
        format!(
            "runtime_proxy_sync_probe_pressure_pause route={} pause_ms={} waited_ms={} outcome={}",
            runtime_route_kind_label(route_kind),
            pause_ms,
            started_at.elapsed().as_millis(),
            runtime_profile_wait_outcome_label(wait_outcome),
        ),
    );
}

pub(super) fn runtime_previous_response_affinity_is_trusted(
    shared: &RuntimeRotationProxyShared,
    previous_response_id: Option<&str>,
    bound_profile: Option<&str>,
) -> Result<bool> {
    let Some(previous_response_id) = previous_response_id else {
        return Ok(false);
    };
    let Some(bound_profile) = bound_profile else {
        return Ok(false);
    };

    let runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let Some(binding) = runtime
        .state
        .response_profile_bindings
        .get(previous_response_id)
    else {
        return Ok(false);
    };
    if binding.profile_name != bound_profile {
        return Ok(false);
    }
    Ok(runtime_continuation_status_map(
        &runtime.continuation_statuses,
        RuntimeContinuationBindingKind::Response,
    )
    .get(previous_response_id)
    .is_none_or(|status| {
        status.state == RuntimeContinuationBindingLifecycle::Verified
            || (status.state == RuntimeContinuationBindingLifecycle::Warm
                && status.last_verified_at.is_some())
    }))
}

pub(super) fn runtime_previous_response_affinity_is_bound(
    shared: &RuntimeRotationProxyShared,
    previous_response_id: Option<&str>,
    bound_profile: Option<&str>,
) -> Result<bool> {
    let Some(previous_response_id) = previous_response_id else {
        return Ok(false);
    };
    let Some(bound_profile) = bound_profile else {
        return Ok(false);
    };

    let runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    Ok(runtime
        .state
        .response_profile_bindings
        .get(previous_response_id)
        .is_some_and(|binding| binding.profile_name == bound_profile))
}

#[derive(Clone, Copy, Debug)]
pub(super) struct RuntimeCandidateAffinity<'a> {
    route_kind: RuntimeRouteKind,
    candidate_name: &'a str,
    strict_affinity_profile: Option<&'a str>,
    pinned_profile: Option<&'a str>,
    turn_state_profile: Option<&'a str>,
    session_profile: Option<&'a str>,
    trusted_previous_response_affinity: bool,
}

impl<'a> RuntimeCandidateAffinity<'a> {
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn new(
        route_kind: RuntimeRouteKind,
        candidate_name: &'a str,
        strict_affinity_profile: Option<&'a str>,
        pinned_profile: Option<&'a str>,
        turn_state_profile: Option<&'a str>,
        session_profile: Option<&'a str>,
        trusted_previous_response_affinity: bool,
    ) -> Self {
        Self {
            route_kind,
            candidate_name,
            strict_affinity_profile,
            pinned_profile,
            turn_state_profile,
            session_profile,
            trusted_previous_response_affinity,
        }
    }
}

pub(super) fn runtime_candidate_has_hard_affinity(affinity: RuntimeCandidateAffinity<'_>) -> bool {
    affinity
        .strict_affinity_profile
        .is_some_and(|profile_name| profile_name == affinity.candidate_name)
        || affinity
            .turn_state_profile
            .is_some_and(|profile_name| profile_name == affinity.candidate_name)
        || (affinity.trusted_previous_response_affinity
            && affinity
                .pinned_profile
                .is_some_and(|profile_name| profile_name == affinity.candidate_name))
        || (affinity.route_kind == RuntimeRouteKind::Compact
            && affinity
                .session_profile
                .is_some_and(|profile_name| profile_name == affinity.candidate_name))
}

pub(super) fn runtime_quota_blocked_affinity_is_releasable(
    affinity: RuntimeCandidateAffinity<'_>,
    request_requires_previous_response_affinity: bool,
) -> bool {
    if request_requires_previous_response_affinity {
        // Some continuations cannot be replayed safely on another account/session. Tool outputs
        // carry chain-scoped call ids, and websocket previous_response continuations without turn
        // state would silently degrade into fresh requests. These requests must fail in place.
        return false;
    }

    if affinity
        .strict_affinity_profile
        .is_some_and(|profile_name| profile_name == affinity.candidate_name)
        || affinity
            .turn_state_profile
            .is_some_and(|profile_name| profile_name == affinity.candidate_name)
        || (affinity.route_kind == RuntimeRouteKind::Compact
            && affinity
                .session_profile
                .is_some_and(|profile_name| profile_name == affinity.candidate_name))
    {
        return false;
    }

    if affinity.trusted_previous_response_affinity
        && affinity
            .pinned_profile
            .is_some_and(|profile_name| profile_name == affinity.candidate_name)
    {
        // Pre-commit quota or overload means the owning response chain is temporarily unavailable,
        // so a fresh fallback may safely drop previous_response affinity even for *_call_output.
        return true;
    }

    true
}

pub(super) fn runtime_quota_blocked_previous_response_fresh_fallback_allowed(
    previous_response_id: Option<&str>,
    trusted_previous_response_affinity: bool,
    previous_response_fresh_fallback_used: bool,
) -> bool {
    previous_response_id.is_some()
        && trusted_previous_response_affinity
        && !previous_response_fresh_fallback_used
}

pub(super) fn runtime_quota_precommit_floor_percent(route_kind: RuntimeRouteKind) -> i64 {
    match route_kind {
        RuntimeRouteKind::Responses | RuntimeRouteKind::Websocket => {
            runtime_proxy_responses_quota_critical_floor_percent()
        }
        RuntimeRouteKind::Compact | RuntimeRouteKind::Standard => 1,
    }
}

pub(super) fn runtime_quota_window_precommit_guard(
    window: RuntimeQuotaWindowSummary,
    floor_percent: i64,
) -> bool {
    matches!(
        window.status,
        RuntimeQuotaWindowStatus::Critical | RuntimeQuotaWindowStatus::Exhausted
    ) && window.remaining_percent <= floor_percent
}

pub(super) fn runtime_quota_precommit_guard_reason(
    summary: RuntimeQuotaSummary,
    route_kind: RuntimeRouteKind,
) -> Option<&'static str> {
    let floor_percent = runtime_quota_precommit_floor_percent(route_kind);
    if summary.route_band == RuntimeQuotaPressureBand::Exhausted {
        return Some("quota_exhausted_before_send");
    }

    if matches!(
        route_kind,
        RuntimeRouteKind::Responses | RuntimeRouteKind::Websocket
    ) && (runtime_quota_window_precommit_guard(summary.five_hour, floor_percent)
        || runtime_quota_window_precommit_guard(summary.weekly, floor_percent))
    {
        return Some("quota_critical_floor_before_send");
    }

    None
}

#[derive(Clone, Copy, Debug)]
pub(super) struct RuntimeResponseCandidateSelection<'a> {
    excluded_profiles: &'a BTreeSet<String>,
    strict_affinity_profile: Option<&'a str>,
    pinned_profile: Option<&'a str>,
    turn_state_profile: Option<&'a str>,
    session_profile: Option<&'a str>,
    discover_previous_response_owner: bool,
    previous_response_id: Option<&'a str>,
    route_kind: RuntimeRouteKind,
}

#[cfg_attr(not(test), allow(dead_code))]
#[allow(clippy::too_many_arguments)]
pub(super) fn select_runtime_response_candidate_for_route(
    shared: &RuntimeRotationProxyShared,
    excluded_profiles: &BTreeSet<String>,
    strict_affinity_profile: Option<&str>,
    pinned_profile: Option<&str>,
    turn_state_profile: Option<&str>,
    session_profile: Option<&str>,
    discover_previous_response_owner: bool,
    previous_response_id: Option<&str>,
    route_kind: RuntimeRouteKind,
) -> Result<Option<String>> {
    select_runtime_response_candidate_for_route_with_selection(
        shared,
        RuntimeResponseCandidateSelection {
            excluded_profiles,
            strict_affinity_profile,
            pinned_profile,
            turn_state_profile,
            session_profile,
            discover_previous_response_owner,
            previous_response_id,
            route_kind,
        },
    )
}

pub(super) fn select_runtime_response_candidate_for_route_with_selection(
    shared: &RuntimeRotationProxyShared,
    selection: RuntimeResponseCandidateSelection<'_>,
) -> Result<Option<String>> {
    let RuntimeResponseCandidateSelection {
        excluded_profiles,
        strict_affinity_profile,
        pinned_profile,
        turn_state_profile,
        session_profile,
        discover_previous_response_owner,
        previous_response_id,
        route_kind,
    } = selection;

    if let Some(profile_name) = strict_affinity_profile {
        if excluded_profiles.contains(profile_name) {
            return Ok(None);
        }
        if runtime_candidate_has_hard_affinity(RuntimeCandidateAffinity {
            route_kind,
            candidate_name: profile_name,
            strict_affinity_profile,
            pinned_profile,
            turn_state_profile,
            session_profile,
            trusted_previous_response_affinity: false,
        }) {
            return Ok(Some(profile_name.to_string()));
        }
        let (quota_summary, quota_source) =
            runtime_profile_quota_summary_for_route(shared, profile_name, route_kind)?;
        let compact_followup_owner_without_probe = matches!(
            route_kind,
            RuntimeRouteKind::Responses | RuntimeRouteKind::Websocket
        ) && quota_source.is_none();
        if runtime_quota_summary_allows_soft_affinity(quota_summary, quota_source, route_kind)
            || compact_followup_owner_without_probe
        {
            return Ok(Some(profile_name.to_string()));
        }
        runtime_proxy_log(
            shared,
            format!(
                "selection_skip_affinity route={} affinity=compact_followup profile={} reason={} quota_source={} {}",
                runtime_route_kind_label(route_kind),
                profile_name,
                runtime_quota_soft_affinity_rejection_reason(
                    quota_summary,
                    quota_source,
                    route_kind,
                ),
                quota_source
                    .map(runtime_quota_source_label)
                    .unwrap_or("unknown"),
                runtime_quota_summary_log_fields(quota_summary),
            ),
        );
        return Ok(None);
    }

    if let Some(profile_name) = pinned_profile.filter(|name| !excluded_profiles.contains(*name)) {
        if runtime_previous_response_affinity_is_bound(
            shared,
            previous_response_id,
            pinned_profile,
        )? {
            return Ok(Some(profile_name.to_string()));
        }
        if runtime_candidate_has_hard_affinity(RuntimeCandidateAffinity {
            route_kind,
            candidate_name: profile_name,
            strict_affinity_profile,
            pinned_profile,
            turn_state_profile,
            session_profile,
            trusted_previous_response_affinity: runtime_previous_response_affinity_is_trusted(
                shared,
                previous_response_id,
                pinned_profile,
            )?,
        }) {
            return Ok(Some(profile_name.to_string()));
        }
        let (quota_summary, quota_source) =
            runtime_profile_quota_summary_for_route(shared, profile_name, route_kind)?;
        if quota_summary.route_band <= RuntimeQuotaPressureBand::Critical
            && runtime_quota_precommit_guard_reason(quota_summary, route_kind).is_none()
        {
            return Ok(Some(profile_name.to_string()));
        }
        runtime_proxy_log(
            shared,
            format!(
                "selection_skip_affinity route={} affinity=pinned profile={} reason={} quota_source={} {}",
                runtime_route_kind_label(route_kind),
                profile_name,
                runtime_quota_pressure_band_reason(quota_summary.route_band),
                quota_source
                    .map(runtime_quota_source_label)
                    .unwrap_or("unknown"),
                runtime_quota_summary_log_fields(quota_summary),
            ),
        );
    }

    if let Some(profile_name) = turn_state_profile.filter(|name| !excluded_profiles.contains(*name))
    {
        if runtime_candidate_has_hard_affinity(RuntimeCandidateAffinity {
            route_kind,
            candidate_name: profile_name,
            strict_affinity_profile,
            pinned_profile,
            turn_state_profile,
            session_profile,
            trusted_previous_response_affinity: false,
        }) {
            return Ok(Some(profile_name.to_string()));
        }
        let (quota_summary, quota_source) =
            runtime_profile_quota_summary_for_route(shared, profile_name, route_kind)?;
        if quota_summary.route_band <= RuntimeQuotaPressureBand::Critical
            && runtime_quota_precommit_guard_reason(quota_summary, route_kind).is_none()
        {
            return Ok(Some(profile_name.to_string()));
        }
        runtime_proxy_log(
            shared,
            format!(
                "selection_skip_affinity route={} affinity=turn_state profile={} reason={} quota_source={} {}",
                runtime_route_kind_label(route_kind),
                profile_name,
                runtime_quota_pressure_band_reason(quota_summary.route_band),
                quota_source
                    .map(runtime_quota_source_label)
                    .unwrap_or("unknown"),
                runtime_quota_summary_log_fields(quota_summary),
            ),
        );
    }

    if discover_previous_response_owner {
        return next_runtime_previous_response_candidate(
            shared,
            excluded_profiles,
            previous_response_id,
            route_kind,
        );
    }

    if let Some(profile_name) = session_profile.filter(|name| !excluded_profiles.contains(*name)) {
        if runtime_candidate_has_hard_affinity(RuntimeCandidateAffinity {
            route_kind,
            candidate_name: profile_name,
            strict_affinity_profile,
            pinned_profile,
            turn_state_profile,
            session_profile,
            trusted_previous_response_affinity: false,
        }) {
            return Ok(Some(profile_name.to_string()));
        }
        let (quota_summary, quota_source) =
            runtime_profile_quota_summary_for_route(shared, profile_name, route_kind)?;
        let compact_session_owner_without_probe =
            route_kind == RuntimeRouteKind::Compact && quota_source.is_none();
        let websocket_unknown_current_profile_without_pool_fallback = route_kind
            == RuntimeRouteKind::Websocket
            && quota_source.is_none()
            && runtime_proxy_current_profile(shared)? == profile_name
            && !runtime_has_route_eligible_quota_fallback(
                shared,
                profile_name,
                excluded_profiles,
                route_kind,
            )?;
        if runtime_quota_summary_allows_soft_affinity(quota_summary, quota_source, route_kind)
            || compact_session_owner_without_probe
            || websocket_unknown_current_profile_without_pool_fallback
        {
            return Ok(Some(profile_name.to_string()));
        }
        runtime_proxy_log(
            shared,
            format!(
                "selection_skip_affinity route={} affinity=session profile={} reason={} quota_source={} {}",
                runtime_route_kind_label(route_kind),
                profile_name,
                runtime_quota_soft_affinity_rejection_reason(
                    quota_summary,
                    quota_source,
                    route_kind,
                ),
                quota_source
                    .map(runtime_quota_source_label)
                    .unwrap_or("unknown"),
                runtime_quota_summary_log_fields(quota_summary),
            ),
        );
    }

    if let Some(profile_name) =
        runtime_proxy_optimistic_current_candidate_for_route(shared, excluded_profiles, route_kind)?
    {
        return Ok(Some(profile_name));
    }

    next_runtime_response_candidate_for_route(shared, excluded_profiles, route_kind)
}

pub(super) fn next_runtime_previous_response_candidate(
    shared: &RuntimeRotationProxyShared,
    excluded_profiles: &BTreeSet<String>,
    previous_response_id: Option<&str>,
    route_kind: RuntimeRouteKind,
) -> Result<Option<String>> {
    let allow_disk_auth_fallback =
        !runtime_proxy_sync_probe_pressure_mode_active_for_route(shared, route_kind);
    let (state, current_profile, profile_health, profile_usage_auth, profile_probe_cache) = {
        let runtime = shared
            .runtime
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
        (
            runtime.state.clone(),
            runtime.current_profile.clone(),
            runtime.profile_health.clone(),
            runtime.profile_usage_auth.clone(),
            runtime.profile_probe_cache.clone(),
        )
    };
    let now = Local::now().timestamp();
    if let Some(previous_response_id) = previous_response_id
        && let Some(binding) = state.response_profile_bindings.get(previous_response_id)
    {
        let owner = binding.profile_name.as_str();
        if !excluded_profiles.contains(owner)
            && state.profiles.contains_key(owner)
            && !runtime_previous_response_negative_cache_active(
                &profile_health,
                previous_response_id,
                owner,
                route_kind,
                now,
            )
        {
            return Ok(Some(owner.to_string()));
        }
    }

    for name in active_profile_selection_order(&state, &current_profile) {
        if excluded_profiles.contains(&name) {
            continue;
        }
        if let Some(previous_response_id) = previous_response_id
            && runtime_previous_response_negative_cache_active(
                &profile_health,
                previous_response_id,
                &name,
                route_kind,
                now,
            )
        {
            runtime_proxy_log(
                shared,
                format!(
                    "selection_skip_affinity route={} affinity=previous_response_discovery profile={} reason=negative_cache response_id={}",
                    runtime_route_kind_label(route_kind),
                    name,
                    previous_response_id,
                ),
            );
            continue;
        }
        let Some(profile) = state.profiles.get(&name) else {
            continue;
        };
        if !runtime_profile_auth_summary_for_selection_with_policy(
            &name,
            &profile.codex_home,
            &profile_usage_auth,
            &profile_probe_cache,
            allow_disk_auth_fallback,
        )
        .is_some_and(|summary| summary.quota_compatible)
        {
            continue;
        }
        if runtime_profile_auth_failure_active_with_auth_cache(
            &profile_health,
            &profile_usage_auth,
            &name,
            now,
        ) {
            runtime_proxy_log(
                shared,
                format!(
                    "selection_skip_affinity route={} affinity=previous_response_discovery profile={} reason=auth_failure_backoff",
                    runtime_route_kind_label(route_kind),
                    name,
                ),
            );
            continue;
        }
        let (quota_summary, quota_source) =
            runtime_profile_quota_summary_for_route(shared, &name, route_kind)?;
        if quota_summary.route_band == RuntimeQuotaPressureBand::Exhausted
            || runtime_quota_precommit_guard_reason(quota_summary, route_kind).is_some()
        {
            runtime_proxy_log(
                shared,
                format!(
                    "selection_skip_affinity route={} affinity=previous_response_discovery profile={} reason={} quota_source={} {}",
                    runtime_route_kind_label(route_kind),
                    name,
                    runtime_quota_precommit_guard_reason(quota_summary, route_kind).unwrap_or_else(
                        || runtime_quota_pressure_band_reason(quota_summary.route_band)
                    ),
                    quota_source
                        .map(runtime_quota_source_label)
                        .unwrap_or("unknown"),
                    runtime_quota_summary_log_fields(quota_summary),
                ),
            );
            continue;
        }
        return Ok(Some(name));
    }
    Ok(None)
}

pub(super) fn runtime_proxy_optimistic_current_candidate_for_route(
    shared: &RuntimeRotationProxyShared,
    excluded_profiles: &BTreeSet<String>,
    route_kind: RuntimeRouteKind,
) -> Result<Option<String>> {
    let pressure_mode = runtime_proxy_pressure_mode_active(shared);
    let (
        current_profile,
        codex_home,
        in_selection_backoff,
        circuit_open_until,
        inflight_count,
        health_score,
        performance_score,
        auth_failure_active,
        cached_usage_auth_entry,
        probe_cache_entry,
    ) = {
        let mut runtime = shared
            .runtime
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
        let now = Local::now().timestamp();
        prune_runtime_profile_selection_backoff(&mut runtime, now);

        if excluded_profiles.contains(&runtime.current_profile) {
            return Ok(None);
        }

        let Some(profile) = runtime.state.profiles.get(&runtime.current_profile) else {
            return Ok(None);
        };
        (
            runtime.current_profile.clone(),
            profile.codex_home.clone(),
            runtime_profile_in_selection_backoff(
                &runtime,
                &runtime.current_profile,
                route_kind,
                now,
            ),
            runtime_profile_route_circuit_open_until(
                &runtime,
                &runtime.current_profile,
                route_kind,
                now,
            ),
            runtime_profile_inflight_count(&runtime, &runtime.current_profile),
            runtime_profile_health_score(&runtime, &runtime.current_profile, now, route_kind),
            runtime_profile_route_performance_score(
                &runtime.profile_health,
                &runtime.current_profile,
                now,
                route_kind,
            ),
            runtime_profile_auth_failure_active_with_auth_cache(
                &runtime.profile_health,
                &runtime.profile_usage_auth,
                &runtime.current_profile,
                now,
            ),
            runtime
                .profile_usage_auth
                .get(&runtime.current_profile)
                .cloned(),
            runtime
                .profile_probe_cache
                .get(&runtime.current_profile)
                .cloned(),
        )
    };
    let has_alternative_quota_compatible_profile = runtime_has_route_eligible_quota_fallback(
        shared,
        &current_profile,
        excluded_profiles,
        route_kind,
    )?;
    let allow_disk_auth_fallback =
        !runtime_proxy_sync_probe_pressure_mode_active_for_route(shared, route_kind);
    let current_profile_quota_compatible = runtime_profile_cached_auth_summary_for_selection(
        cached_usage_auth_entry,
        probe_cache_entry,
    )
    .unwrap_or_else(|| {
        if allow_disk_auth_fallback {
            read_auth_summary(&codex_home)
        } else {
            AuthSummary {
                label: "uncached-auth".to_string(),
                quota_compatible: false,
            }
        }
    })
    .quota_compatible;
    let (quota_summary, quota_source) =
        runtime_profile_quota_summary_for_route(shared, &current_profile, route_kind)?;
    let inflight_soft_limit = runtime_profile_inflight_soft_limit(route_kind, pressure_mode);
    let quota_evidence_required =
        has_alternative_quota_compatible_profile && quota_source.is_none();
    let live_quota_probe_required = has_alternative_quota_compatible_profile
        && matches!(
            route_kind,
            RuntimeRouteKind::Responses | RuntimeRouteKind::Websocket
        )
        && !matches!(quota_source, Some(RuntimeQuotaSource::LiveProbe));
    let unknown_quota_allowed = quota_summary.route_band == RuntimeQuotaPressureBand::Unknown
        && !has_alternative_quota_compatible_profile;
    let quota_band_blocks_current =
        quota_summary.route_band > RuntimeQuotaPressureBand::Healthy && !unknown_quota_allowed;

    if auth_failure_active
        || in_selection_backoff
        || circuit_open_until.is_some()
        || health_score > 0
        || performance_score > 0
        || quota_evidence_required
        || live_quota_probe_required
        || inflight_count >= inflight_soft_limit
        || quota_band_blocks_current
    {
        let reason = if auth_failure_active {
            "auth_failure_backoff"
        } else if in_selection_backoff {
            "selection_backoff"
        } else if circuit_open_until.is_some() {
            "route_circuit_open"
        } else if health_score > 0 {
            "profile_health"
        } else if performance_score > 0 {
            "profile_performance"
        } else if quota_evidence_required {
            "quota_probe_unavailable"
        } else if live_quota_probe_required {
            if matches!(quota_source, Some(RuntimeQuotaSource::PersistedSnapshot)) {
                "stale_persisted_quota"
            } else {
                "quota_probe_unavailable"
            }
        } else if quota_band_blocks_current {
            runtime_quota_pressure_band_reason(quota_summary.route_band)
        } else {
            "profile_inflight_soft_limit"
        };
        runtime_proxy_log(
            shared,
            format!(
                "selection_skip_current route={} profile={} reason={} inflight={} health={} performance={} soft_limit={} circuit_until={} quota_source={} {}",
                runtime_route_kind_label(route_kind),
                current_profile,
                reason,
                inflight_count,
                health_score,
                performance_score,
                inflight_soft_limit,
                circuit_open_until.unwrap_or_default(),
                quota_source
                    .map(runtime_quota_source_label)
                    .unwrap_or("unknown"),
                runtime_quota_summary_log_fields(quota_summary),
            ),
        );
        return Ok(None);
    }
    if !current_profile_quota_compatible {
        runtime_proxy_log(
            shared,
            format!(
                "selection_skip_current route={} profile={} reason=auth_not_quota_compatible",
                runtime_route_kind_label(route_kind),
                current_profile
            ),
        );
        return Ok(None);
    }

    runtime_proxy_log(
        shared,
        format!(
            "selection_keep_current route={} profile={} inflight={} health={} performance={} quota_source={} {}",
            runtime_route_kind_label(route_kind),
            current_profile,
            inflight_count,
            health_score,
            performance_score,
            quota_source
                .map(runtime_quota_source_label)
                .unwrap_or("unknown"),
            runtime_quota_summary_log_fields(quota_summary),
        ),
    );
    if !reserve_runtime_profile_route_circuit_half_open_probe(shared, &current_profile, route_kind)?
    {
        runtime_proxy_log(
            shared,
            format!(
                "selection_skip_current route={} profile={} reason=route_circuit_half_open_probe_wait inflight={} health={} performance={} quota_source={} {}",
                runtime_route_kind_label(route_kind),
                current_profile,
                inflight_count,
                health_score,
                performance_score,
                quota_source
                    .map(runtime_quota_source_label)
                    .unwrap_or("unknown"),
                runtime_quota_summary_log_fields(quota_summary),
            ),
        );
        return Ok(None);
    }
    Ok(Some(current_profile))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn proxy_runtime_websocket_text_message(
    session_id: u64,
    request_id: u64,
    local_socket: &mut RuntimeLocalWebSocket,
    handshake_request: &RuntimeProxyRequest,
    request_text: &str,
    request_metadata: &RuntimeWebsocketRequestMetadata,
    shared: &RuntimeRotationProxyShared,
    websocket_session: &mut RuntimeWebsocketSessionState,
) -> Result<()> {
    let mut handshake_request = handshake_request.clone();
    let mut request_text = request_text.to_string();
    let request_requires_previous_response_affinity =
        request_metadata.requires_previous_response_affinity;
    let mut previous_response_id = request_metadata.previous_response_id.clone();
    let mut request_turn_state = runtime_request_turn_state(&handshake_request);
    let explicit_request_session_id = runtime_request_explicit_session_id(&handshake_request);
    let request_session_id_header_present = explicit_request_session_id.is_some();
    let request_session_id = explicit_request_session_id
        .as_ref()
        .map(|session_id| session_id.as_str().to_string())
        .or_else(|| request_metadata.session_id.clone());
    let mut bound_profile = previous_response_id
        .as_deref()
        .map(|response_id| {
            runtime_response_bound_profile(shared, response_id, RuntimeRouteKind::Websocket)
        })
        .transpose()?
        .flatten();
    let mut trusted_previous_response_affinity = runtime_previous_response_affinity_is_trusted(
        shared,
        previous_response_id.as_deref(),
        bound_profile.as_deref(),
    )?;
    if request_turn_state.is_none()
        && let Some(turn_state) = runtime_previous_response_turn_state(
            shared,
            previous_response_id.as_deref(),
            bound_profile.as_deref(),
        )?
    {
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=websocket route=responses previous_response_turn_state_rehydrated response_id={} profile={} turn_state={turn_state}",
                previous_response_id.as_deref().unwrap_or("-"),
                bound_profile.as_deref().unwrap_or("-"),
            ),
        );
        request_turn_state = Some(turn_state);
    }
    let mut turn_state_profile = request_turn_state
        .as_deref()
        .map(|value| runtime_turn_state_bound_profile(shared, value))
        .transpose()?
        .flatten();
    let mut bound_session_profile = None;
    let mut compact_followup_profile = None;
    let mut compact_session_profile = None;
    let mut session_profile = None;
    let mut pinned_profile = None;
    macro_rules! recompute_route_affinity {
        ($reason:expr) => {
            refresh_and_log_runtime_response_route_affinity(
                shared,
                request_id,
                Some(session_id),
                $reason,
                previous_response_id.as_deref(),
                bound_profile.as_deref(),
                turn_state_profile.as_deref(),
                request_turn_state.as_deref(),
                request_session_id.as_deref(),
                explicit_request_session_id.as_ref(),
                websocket_session.profile_name.as_deref(),
                &mut bound_session_profile,
                &mut compact_followup_profile,
                &mut compact_session_profile,
                &mut session_profile,
                &mut pinned_profile,
            )
        };
    }
    recompute_route_affinity!("initial")?;
    let mut excluded_profiles = BTreeSet::new();
    let mut last_failure: Option<(RuntimeUpstreamFailureResponse, bool)> = None;
    let mut previous_response_retry_candidate: Option<String> = None;
    let mut previous_response_retry_index = 0usize;
    let mut candidate_turn_state_retry_profile: Option<String> = None;
    let mut candidate_turn_state_retry_value: Option<String> = None;
    let mut saw_inflight_saturation = false;
    let mut selection_started_at = Instant::now();
    let mut selection_attempts = 0usize;
    let mut previous_response_fresh_fallback_used = false;
    let mut saw_previous_response_not_found = false;
    let mut websocket_reuse_fresh_retry_profiles = BTreeSet::new();
    macro_rules! runtime_candidate_affinity {
        ($route_kind:expr, $candidate_name:expr, $strict_affinity_profile:expr $(,)?) => {
            RuntimeCandidateAffinity {
                route_kind: $route_kind,
                candidate_name: $candidate_name,
                strict_affinity_profile: $strict_affinity_profile,
                pinned_profile: pinned_profile.as_deref(),
                turn_state_profile: turn_state_profile.as_deref(),
                session_profile: session_profile.as_deref(),
                trusted_previous_response_affinity,
            }
        };
    }
    macro_rules! runtime_websocket_fresh_fallback_target {
        () => {
            RuntimeWebsocketFreshFallbackTarget {
                request_text: &mut request_text,
                handshake_request: &mut handshake_request,
                websocket_reuse_fresh_retry_profiles: &mut websocket_reuse_fresh_retry_profiles,
            }
        };
    }
    macro_rules! runtime_previous_response_fresh_fallback_state {
        () => {
            RuntimePreviousResponseFreshFallbackState {
                previous_response_id: &mut previous_response_id,
                request_turn_state: &mut request_turn_state,
                previous_response_fresh_fallback_used: &mut previous_response_fresh_fallback_used,
                saw_previous_response_not_found: &mut saw_previous_response_not_found,
                previous_response_retry_candidate: &mut previous_response_retry_candidate,
                previous_response_retry_index: &mut previous_response_retry_index,
                candidate_turn_state_retry_profile: &mut candidate_turn_state_retry_profile,
                candidate_turn_state_retry_value: &mut candidate_turn_state_retry_value,
                trusted_previous_response_affinity: &mut trusted_previous_response_affinity,
                bound_profile: &mut bound_profile,
                pinned_profile: &mut pinned_profile,
                turn_state_profile: &mut turn_state_profile,
                session_profile: &mut session_profile,
                excluded_profiles: &mut excluded_profiles,
                last_failure: &mut last_failure,
                selection_started_at: &mut selection_started_at,
                selection_attempts: &mut selection_attempts,
            }
        };
    }

    loop {
        let pressure_mode =
            runtime_proxy_pressure_mode_active_for_route(shared, RuntimeRouteKind::Websocket);
        if runtime_proxy_precommit_budget_exhausted(
            selection_started_at,
            selection_attempts,
            runtime_proxy_has_continuation_priority(
                previous_response_id.as_deref(),
                pinned_profile.as_deref(),
                request_turn_state.as_deref(),
                turn_state_profile.as_deref(),
                runtime_noncompact_session_priority_profile(
                    session_profile.as_deref(),
                    compact_session_profile.as_deref(),
                ),
            ),
            pressure_mode,
        ) {
            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} websocket_session={session_id} precommit_budget_exhausted attempts={selection_attempts} elapsed_ms={} pressure_mode={pressure_mode}",
                    selection_started_at.elapsed().as_millis()
                ),
            );
            if previous_response_id.is_some()
                && saw_previous_response_not_found
                && !previous_response_fresh_fallback_used
                && !runtime_websocket_request_requires_locked_previous_response_affinity(
                    request_requires_previous_response_affinity,
                    trusted_previous_response_affinity,
                    previous_response_id.as_deref(),
                    request_turn_state.as_deref(),
                )
                && let Some(fresh_request_text) =
                    runtime_request_text_without_previous_response_id(&request_text)
            {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} websocket_session={session_id} previous_response_fresh_fallback reason=precommit_budget_exhausted"
                    ),
                );
                apply_runtime_websocket_previous_response_fresh_fallback(
                    fresh_request_text,
                    runtime_websocket_fresh_fallback_target!(),
                    runtime_previous_response_fresh_fallback_state!(),
                );
                recompute_route_affinity!("previous_response_fresh_fallback")?;
                continue;
            }
            if let Some((profile_name, source)) = compact_followup_profile.as_ref() {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} websocket_session={session_id} compact_fresh_fallback_blocked profile={profile_name} source={source} reason=precommit_budget_exhausted"
                    ),
                );
                send_runtime_proxy_final_websocket_failure(
                    local_socket,
                    last_failure,
                    saw_inflight_saturation,
                )?;
                return Ok(());
            }
            if runtime_proxy_allows_direct_current_profile_fallback(
                previous_response_id.as_deref(),
                pinned_profile.as_deref(),
                request_turn_state.as_deref(),
                turn_state_profile.as_deref(),
                runtime_noncompact_session_priority_profile(
                    session_profile.as_deref(),
                    compact_session_profile.as_deref(),
                ),
                saw_inflight_saturation,
                last_failure.is_some(),
            ) && let Some(current_profile) = runtime_proxy_direct_current_fallback_profile(
                shared,
                &excluded_profiles,
                RuntimeRouteKind::Websocket,
            )? {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} websocket_session={session_id} direct_current_profile_fallback profile={current_profile} reason=precommit_budget_exhausted"
                    ),
                );
                match attempt_runtime_websocket_request(RuntimeWebsocketAttemptRequest {
                    request_id,
                    local_socket,
                    handshake_request: &handshake_request,
                    request_text: &request_text,
                    request_previous_response_id: previous_response_id.as_deref(),
                    request_session_id: request_session_id.as_deref(),
                    request_turn_state: request_turn_state.as_deref(),
                    shared,
                    websocket_session,
                    profile_name: &current_profile,
                    turn_state_override: request_turn_state.as_deref(),
                    promote_committed_profile: previous_response_id.is_none()
                        && bound_profile.is_none()
                        && request_turn_state.is_none()
                        && turn_state_profile.is_none()
                        && compact_followup_profile.is_none()
                        && !(request_session_id_header_present || bound_session_profile.is_some()),
                })? {
                    RuntimeWebsocketAttempt::Delivered => return Ok(()),
                    RuntimeWebsocketAttempt::QuotaBlocked {
                        profile_name,
                        payload,
                    } => {
                        mark_runtime_profile_retry_backoff(shared, &profile_name)?;
                        if !runtime_quota_blocked_affinity_is_releasable(
                            runtime_candidate_affinity!(
                                RuntimeRouteKind::Websocket,
                                &profile_name,
                                compact_followup_profile
                                    .as_ref()
                                    .map(|(profile_name, _)| profile_name.as_str()),
                            ),
                            request_requires_previous_response_affinity,
                        ) {
                            forward_runtime_proxy_websocket_error(local_socket, &payload)?;
                            return Ok(());
                        }
                        let released_affinity = release_runtime_quota_blocked_affinity(
                            shared,
                            &profile_name,
                            previous_response_id.as_deref(),
                            request_turn_state.as_deref(),
                            request_session_id.as_deref(),
                        )?;
                        clear_runtime_response_profile_affinity(
                            &profile_name,
                            &mut bound_profile,
                            &mut session_profile,
                            &mut candidate_turn_state_retry_profile,
                            &mut candidate_turn_state_retry_value,
                            &mut pinned_profile,
                            &mut previous_response_retry_index,
                            true,
                            &mut turn_state_profile,
                            None,
                        );
                        if released_affinity {
                            runtime_proxy_log(
                                shared,
                                format!(
                                    "request={request_id} websocket_session={session_id} quota_blocked_affinity_released profile={profile_name} via=direct_current_profile_fallback"
                                ),
                            );
                        }
                        if runtime_quota_blocked_previous_response_fresh_fallback_allowed(
                            previous_response_id.as_deref(),
                            trusted_previous_response_affinity,
                            previous_response_fresh_fallback_used,
                        ) && let Some(fresh_request_text) =
                            runtime_request_text_without_previous_response_id(&request_text)
                        {
                            runtime_proxy_log(
                                shared,
                                format!(
                                    "request={request_id} websocket_session={session_id} previous_response_fresh_fallback reason=quota_blocked via=direct_current_profile_fallback"
                                ),
                            );
                            apply_runtime_websocket_previous_response_fresh_fallback(
                                fresh_request_text,
                                runtime_websocket_fresh_fallback_target!(),
                                runtime_previous_response_fresh_fallback_state!(),
                            );
                            recompute_route_affinity!("previous_response_fresh_fallback")?;
                            continue;
                        }
                        if !runtime_has_route_eligible_quota_fallback(
                            shared,
                            &profile_name,
                            &BTreeSet::new(),
                            RuntimeRouteKind::Websocket,
                        )? {
                            forward_runtime_proxy_websocket_error(local_socket, &payload)?;
                            return Ok(());
                        }
                        excluded_profiles.insert(profile_name);
                        last_failure =
                            Some((RuntimeUpstreamFailureResponse::Websocket(payload), true));
                        continue;
                    }
                    RuntimeWebsocketAttempt::Overloaded {
                        profile_name,
                        payload,
                    } => {
                        let overload_message =
                            extract_runtime_proxy_overload_message_from_websocket_payload(&payload);
                        runtime_proxy_log(
                            shared,
                            format!(
                                "request={request_id} websocket_session={session_id} upstream_overloaded route=websocket profile={profile_name} via=direct_current_profile_fallback message={}",
                                overload_message.as_deref().unwrap_or("-"),
                            ),
                        );
                        mark_runtime_profile_retry_backoff(shared, &profile_name)?;
                        let _ = bump_runtime_profile_health_score(
                            shared,
                            &profile_name,
                            RuntimeRouteKind::Websocket,
                            RUNTIME_PROFILE_OVERLOAD_HEALTH_PENALTY,
                            "websocket_overload",
                        );
                        let _ = bump_runtime_profile_bad_pairing_score(
                            shared,
                            &profile_name,
                            RuntimeRouteKind::Websocket,
                            RUNTIME_PROFILE_BAD_PAIRING_PENALTY,
                            "websocket_overload",
                        );
                        if !runtime_quota_blocked_affinity_is_releasable(
                            runtime_candidate_affinity!(
                                RuntimeRouteKind::Websocket,
                                &profile_name,
                                compact_followup_profile
                                    .as_ref()
                                    .map(|(profile_name, _)| profile_name.as_str()),
                            ),
                            runtime_websocket_request_requires_locked_previous_response_affinity(
                                request_requires_previous_response_affinity,
                                trusted_previous_response_affinity,
                                previous_response_id.as_deref(),
                                request_turn_state.as_deref(),
                            ),
                        ) {
                            runtime_proxy_log(
                                shared,
                                format!(
                                    "request={request_id} websocket_session={session_id} upstream_overload_passthrough route=websocket profile={profile_name} reason=hard_affinity via=direct_current_profile_fallback"
                                ),
                            );
                            forward_runtime_proxy_websocket_error(local_socket, &payload)?;
                            return Ok(());
                        }
                        if runtime_quota_blocked_previous_response_fresh_fallback_allowed(
                            previous_response_id.as_deref(),
                            trusted_previous_response_affinity,
                            previous_response_fresh_fallback_used,
                        ) && let Some(fresh_request_text) =
                            runtime_request_text_without_previous_response_id(&request_text)
                        {
                            runtime_proxy_log(
                                shared,
                                format!(
                                    "request={request_id} websocket_session={session_id} previous_response_fresh_fallback reason=upstream_overloaded via=direct_current_profile_fallback"
                                ),
                            );
                            apply_runtime_websocket_previous_response_fresh_fallback(
                                fresh_request_text,
                                runtime_websocket_fresh_fallback_target!(),
                                runtime_previous_response_fresh_fallback_state!(),
                            );
                            recompute_route_affinity!("previous_response_fresh_fallback")?;
                            continue;
                        }
                        excluded_profiles.insert(profile_name);
                        last_failure =
                            Some((RuntimeUpstreamFailureResponse::Websocket(payload), false));
                        continue;
                    }
                    RuntimeWebsocketAttempt::PreviousResponseNotFound {
                        profile_name,
                        payload,
                        turn_state,
                    } => {
                        runtime_proxy_log(
                            shared,
                            format!(
                                "request={request_id} transport=websocket route=websocket websocket_session={session_id} previous_response_not_found profile={profile_name} retry_index={previous_response_retry_index} replay_turn_state={:?} via=direct_current_profile_fallback",
                                turn_state
                            ),
                        );
                        saw_previous_response_not_found = true;
                        if previous_response_retry_candidate.as_deref()
                            != Some(profile_name.as_str())
                        {
                            previous_response_retry_candidate = Some(profile_name.clone());
                            previous_response_retry_index = 0;
                        }
                        let has_turn_state_retry = turn_state.is_some();
                        if has_turn_state_retry {
                            candidate_turn_state_retry_profile = Some(profile_name.clone());
                            candidate_turn_state_retry_value = turn_state;
                        }
                        if has_turn_state_retry
                            && let Some(delay) =
                                runtime_previous_response_retry_delay(previous_response_retry_index)
                        {
                            previous_response_retry_index += 1;
                            last_failure =
                                Some((RuntimeUpstreamFailureResponse::Websocket(payload), false));
                            runtime_proxy_log(
                                shared,
                                format!(
                                    "request={request_id} websocket_session={session_id} previous_response_retry_immediate profile={profile_name} delay_ms={} reason=non_blocking_retry via=direct_current_profile_fallback",
                                    delay.as_millis()
                                ),
                            );
                            continue;
                        }
                        previous_response_retry_candidate = None;
                        previous_response_retry_index = 0;
                        if !has_turn_state_retry
                            && !runtime_websocket_request_requires_locked_previous_response_affinity(
                                request_requires_previous_response_affinity,
                                trusted_previous_response_affinity,
                                previous_response_id.as_deref(),
                                request_turn_state.as_deref(),
                            )
                        {
                            let _ = clear_runtime_stale_previous_response_binding(
                                shared,
                                &profile_name,
                                previous_response_id.as_deref(),
                            )?;
                        }
                        let released_affinity = release_runtime_previous_response_affinity(
                            shared,
                            &profile_name,
                            previous_response_id.as_deref(),
                            request_turn_state.as_deref(),
                            request_session_id.as_deref(),
                            RuntimeRouteKind::Websocket,
                        )?;
                        if released_affinity {
                            runtime_proxy_log(
                                shared,
                                format!(
                                    "request={request_id} websocket_session={session_id} previous_response_affinity_released profile={profile_name} via=direct_current_profile_fallback"
                                ),
                            );
                        }
                        clear_runtime_response_profile_affinity(
                            &profile_name,
                            &mut bound_profile,
                            &mut session_profile,
                            &mut candidate_turn_state_retry_profile,
                            &mut candidate_turn_state_retry_value,
                            &mut pinned_profile,
                            &mut previous_response_retry_index,
                            true,
                            &mut turn_state_profile,
                            Some(&mut compact_followup_profile),
                        );
                        excluded_profiles.insert(profile_name);
                        last_failure =
                            Some((RuntimeUpstreamFailureResponse::Websocket(payload), false));
                        continue;
                    }
                    RuntimeWebsocketAttempt::ReuseWatchdogTripped { profile_name, .. } => {
                        excluded_profiles.insert(profile_name);
                        continue;
                    }
                    RuntimeWebsocketAttempt::LocalSelectionBlocked {
                        profile_name,
                        reason,
                    } => {
                        mark_runtime_profile_retry_backoff(shared, &profile_name)?;
                        if !runtime_quota_blocked_affinity_is_releasable(
                            runtime_candidate_affinity!(
                                RuntimeRouteKind::Websocket,
                                &profile_name,
                                compact_followup_profile
                                    .as_ref()
                                    .map(|(profile_name, _)| profile_name.as_str()),
                            ),
                            runtime_websocket_request_requires_locked_previous_response_affinity(
                                request_requires_previous_response_affinity,
                                trusted_previous_response_affinity,
                                previous_response_id.as_deref(),
                                request_turn_state.as_deref(),
                            ),
                        ) {
                            send_runtime_proxy_websocket_error(
                                local_socket,
                                503,
                                "service_unavailable",
                                runtime_proxy_local_selection_failure_message(),
                            )?;
                            return Ok(());
                        }
                        let released_affinity = release_runtime_quota_blocked_affinity(
                            shared,
                            &profile_name,
                            previous_response_id.as_deref(),
                            request_turn_state.as_deref(),
                            request_session_id.as_deref(),
                        )?;
                        clear_runtime_response_profile_affinity(
                            &profile_name,
                            &mut bound_profile,
                            &mut session_profile,
                            &mut candidate_turn_state_retry_profile,
                            &mut candidate_turn_state_retry_value,
                            &mut pinned_profile,
                            &mut previous_response_retry_index,
                            true,
                            &mut turn_state_profile,
                            None,
                        );
                        if released_affinity {
                            runtime_proxy_log(
                                shared,
                                format!(
                                    "request={request_id} websocket_session={session_id} quota_blocked_affinity_released profile={profile_name} reason={reason} via=direct_current_profile_fallback"
                                ),
                            );
                        }
                        if runtime_quota_blocked_previous_response_fresh_fallback_allowed(
                            previous_response_id.as_deref(),
                            trusted_previous_response_affinity,
                            previous_response_fresh_fallback_used,
                        ) && let Some(fresh_request_text) =
                            runtime_request_text_without_previous_response_id(&request_text)
                        {
                            runtime_proxy_log(
                                shared,
                                format!(
                                    "request={request_id} websocket_session={session_id} previous_response_fresh_fallback reason={reason} via=direct_current_profile_fallback"
                                ),
                            );
                            apply_runtime_websocket_previous_response_fresh_fallback(
                                fresh_request_text,
                                runtime_websocket_fresh_fallback_target!(),
                                runtime_previous_response_fresh_fallback_state!(),
                            );
                            recompute_route_affinity!("previous_response_fresh_fallback")?;
                            continue;
                        }
                        excluded_profiles.insert(profile_name);
                        continue;
                    }
                }
            }
            send_runtime_proxy_final_websocket_failure(
                local_socket,
                last_failure,
                saw_inflight_saturation,
            )?;
            return Ok(());
        }

        let Some(candidate_name) = select_runtime_response_candidate_for_route_with_selection(
            shared,
            RuntimeResponseCandidateSelection {
                excluded_profiles: &excluded_profiles,
                strict_affinity_profile: compact_followup_profile
                    .as_ref()
                    .map(|(profile_name, _)| profile_name.as_str()),
                pinned_profile: pinned_profile.as_deref(),
                turn_state_profile: turn_state_profile.as_deref(),
                session_profile: session_profile.as_deref(),
                discover_previous_response_owner: previous_response_id.is_some(),
                previous_response_id: previous_response_id.as_deref(),
                route_kind: RuntimeRouteKind::Websocket,
            },
        )?
        else {
            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} websocket_session={session_id} candidate_exhausted last_failure={}",
                    match &last_failure {
                        Some((RuntimeUpstreamFailureResponse::Websocket(_), _)) => "websocket",
                        Some((RuntimeUpstreamFailureResponse::Http(_), _)) => "http",
                        None => "none",
                    }
                ),
            );
            if previous_response_id.is_some()
                && saw_previous_response_not_found
                && !previous_response_fresh_fallback_used
                && !runtime_websocket_request_requires_locked_previous_response_affinity(
                    request_requires_previous_response_affinity,
                    trusted_previous_response_affinity,
                    previous_response_id.as_deref(),
                    request_turn_state.as_deref(),
                )
                && let Some(fresh_request_text) =
                    runtime_request_text_without_previous_response_id(&request_text)
            {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} websocket_session={session_id} previous_response_fresh_fallback reason=candidate_exhausted"
                    ),
                );
                apply_runtime_websocket_previous_response_fresh_fallback(
                    fresh_request_text,
                    runtime_websocket_fresh_fallback_target!(),
                    runtime_previous_response_fresh_fallback_state!(),
                );
                recompute_route_affinity!("previous_response_fresh_fallback")?;
                continue;
            }
            if let Some((profile_name, source)) = compact_followup_profile.as_ref() {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} websocket_session={session_id} compact_fresh_fallback_blocked profile={profile_name} source={source} reason=candidate_exhausted"
                    ),
                );
                send_runtime_proxy_final_websocket_failure(
                    local_socket,
                    last_failure,
                    saw_inflight_saturation,
                )?;
                return Ok(());
            }
            let remaining_cold_start_profiles =
                runtime_remaining_sync_probe_cold_start_profiles_for_route(
                    shared,
                    &excluded_profiles,
                    RuntimeRouteKind::Websocket,
                )?;
            if remaining_cold_start_profiles > 0 {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} websocket_session={session_id} candidate_exhausted_continue route=websocket remaining_cold_start_profiles={remaining_cold_start_profiles}"
                    ),
                );
                runtime_proxy_sync_probe_pressure_pause(shared, RuntimeRouteKind::Websocket);
                continue;
            }
            if runtime_proxy_allows_direct_current_profile_fallback(
                previous_response_id.as_deref(),
                pinned_profile.as_deref(),
                request_turn_state.as_deref(),
                turn_state_profile.as_deref(),
                runtime_noncompact_session_priority_profile(
                    session_profile.as_deref(),
                    compact_session_profile.as_deref(),
                ),
                saw_inflight_saturation,
                last_failure.is_some(),
            ) && let Some(current_profile) = runtime_proxy_direct_current_fallback_profile(
                shared,
                &excluded_profiles,
                RuntimeRouteKind::Websocket,
            )? {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} websocket_session={session_id} direct_current_profile_fallback profile={current_profile} reason=candidate_exhausted"
                    ),
                );
                match attempt_runtime_websocket_request(RuntimeWebsocketAttemptRequest {
                    request_id,
                    local_socket,
                    handshake_request: &handshake_request,
                    request_text: &request_text,
                    request_previous_response_id: previous_response_id.as_deref(),
                    request_session_id: request_session_id.as_deref(),
                    request_turn_state: request_turn_state.as_deref(),
                    shared,
                    websocket_session,
                    profile_name: &current_profile,
                    turn_state_override: request_turn_state.as_deref(),
                    promote_committed_profile: previous_response_id.is_none()
                        && bound_profile.is_none()
                        && request_turn_state.is_none()
                        && turn_state_profile.is_none()
                        && compact_followup_profile.is_none()
                        && !(request_session_id_header_present || bound_session_profile.is_some()),
                })? {
                    RuntimeWebsocketAttempt::Delivered => return Ok(()),
                    RuntimeWebsocketAttempt::QuotaBlocked {
                        profile_name,
                        payload,
                    } => {
                        mark_runtime_profile_retry_backoff(shared, &profile_name)?;
                        if !runtime_quota_blocked_affinity_is_releasable(
                            runtime_candidate_affinity!(
                                RuntimeRouteKind::Websocket,
                                &profile_name,
                                compact_followup_profile
                                    .as_ref()
                                    .map(|(profile_name, _)| profile_name.as_str()),
                            ),
                            request_requires_previous_response_affinity,
                        ) {
                            forward_runtime_proxy_websocket_error(local_socket, &payload)?;
                            return Ok(());
                        }
                        let released_affinity = release_runtime_quota_blocked_affinity(
                            shared,
                            &profile_name,
                            previous_response_id.as_deref(),
                            request_turn_state.as_deref(),
                            request_session_id.as_deref(),
                        )?;
                        clear_runtime_response_profile_affinity(
                            &profile_name,
                            &mut bound_profile,
                            &mut session_profile,
                            &mut candidate_turn_state_retry_profile,
                            &mut candidate_turn_state_retry_value,
                            &mut pinned_profile,
                            &mut previous_response_retry_index,
                            true,
                            &mut turn_state_profile,
                            None,
                        );
                        if released_affinity {
                            runtime_proxy_log(
                                shared,
                                format!(
                                    "request={request_id} websocket_session={session_id} quota_blocked_affinity_released profile={profile_name} via=direct_current_profile_fallback"
                                ),
                            );
                        }
                        if runtime_quota_blocked_previous_response_fresh_fallback_allowed(
                            previous_response_id.as_deref(),
                            trusted_previous_response_affinity,
                            previous_response_fresh_fallback_used,
                        ) && let Some(fresh_request_text) =
                            runtime_request_text_without_previous_response_id(&request_text)
                        {
                            runtime_proxy_log(
                                shared,
                                format!(
                                    "request={request_id} websocket_session={session_id} previous_response_fresh_fallback reason=quota_blocked via=direct_current_profile_fallback"
                                ),
                            );
                            apply_runtime_websocket_previous_response_fresh_fallback(
                                fresh_request_text,
                                runtime_websocket_fresh_fallback_target!(),
                                runtime_previous_response_fresh_fallback_state!(),
                            );
                            recompute_route_affinity!("previous_response_fresh_fallback")?;
                            continue;
                        }
                        if !runtime_has_route_eligible_quota_fallback(
                            shared,
                            &profile_name,
                            &BTreeSet::new(),
                            RuntimeRouteKind::Websocket,
                        )? {
                            forward_runtime_proxy_websocket_error(local_socket, &payload)?;
                            return Ok(());
                        }
                        excluded_profiles.insert(profile_name);
                        last_failure =
                            Some((RuntimeUpstreamFailureResponse::Websocket(payload), true));
                        continue;
                    }
                    RuntimeWebsocketAttempt::Overloaded {
                        profile_name,
                        payload,
                    } => {
                        let overload_message =
                            extract_runtime_proxy_overload_message_from_websocket_payload(&payload);
                        runtime_proxy_log(
                            shared,
                            format!(
                                "request={request_id} websocket_session={session_id} upstream_overloaded route=websocket profile={profile_name} via=direct_current_profile_fallback message={}",
                                overload_message.as_deref().unwrap_or("-"),
                            ),
                        );
                        mark_runtime_profile_retry_backoff(shared, &profile_name)?;
                        let _ = bump_runtime_profile_health_score(
                            shared,
                            &profile_name,
                            RuntimeRouteKind::Websocket,
                            RUNTIME_PROFILE_OVERLOAD_HEALTH_PENALTY,
                            "websocket_overload",
                        );
                        let _ = bump_runtime_profile_bad_pairing_score(
                            shared,
                            &profile_name,
                            RuntimeRouteKind::Websocket,
                            RUNTIME_PROFILE_BAD_PAIRING_PENALTY,
                            "websocket_overload",
                        );
                        if !runtime_quota_blocked_affinity_is_releasable(
                            runtime_candidate_affinity!(
                                RuntimeRouteKind::Websocket,
                                &profile_name,
                                compact_followup_profile
                                    .as_ref()
                                    .map(|(profile_name, _)| profile_name.as_str()),
                            ),
                            runtime_websocket_request_requires_locked_previous_response_affinity(
                                request_requires_previous_response_affinity,
                                trusted_previous_response_affinity,
                                previous_response_id.as_deref(),
                                request_turn_state.as_deref(),
                            ),
                        ) {
                            runtime_proxy_log(
                                shared,
                                format!(
                                    "request={request_id} websocket_session={session_id} upstream_overload_passthrough route=websocket profile={profile_name} reason=hard_affinity via=direct_current_profile_fallback"
                                ),
                            );
                            forward_runtime_proxy_websocket_error(local_socket, &payload)?;
                            return Ok(());
                        }
                        excluded_profiles.insert(profile_name);
                        last_failure =
                            Some((RuntimeUpstreamFailureResponse::Websocket(payload), false));
                        continue;
                    }
                    RuntimeWebsocketAttempt::PreviousResponseNotFound {
                        profile_name,
                        payload,
                        turn_state,
                    } => {
                        runtime_proxy_log(
                            shared,
                            format!(
                                "request={request_id} transport=websocket route=websocket websocket_session={session_id} previous_response_not_found profile={profile_name} retry_index={previous_response_retry_index} replay_turn_state={:?} via=direct_current_profile_fallback",
                                turn_state
                            ),
                        );
                        saw_previous_response_not_found = true;
                        if previous_response_retry_candidate.as_deref()
                            != Some(profile_name.as_str())
                        {
                            previous_response_retry_candidate = Some(profile_name.clone());
                            previous_response_retry_index = 0;
                        }
                        let has_turn_state_retry = turn_state.is_some();
                        if has_turn_state_retry {
                            candidate_turn_state_retry_profile = Some(profile_name.clone());
                            candidate_turn_state_retry_value = turn_state;
                        }
                        if has_turn_state_retry
                            && let Some(delay) =
                                runtime_previous_response_retry_delay(previous_response_retry_index)
                        {
                            previous_response_retry_index += 1;
                            last_failure =
                                Some((RuntimeUpstreamFailureResponse::Websocket(payload), false));
                            runtime_proxy_log(
                                shared,
                                format!(
                                    "request={request_id} websocket_session={session_id} previous_response_retry_immediate profile={profile_name} delay_ms={} reason=non_blocking_retry via=direct_current_profile_fallback",
                                    delay.as_millis()
                                ),
                            );
                            continue;
                        }
                        previous_response_retry_candidate = None;
                        previous_response_retry_index = 0;
                        if !has_turn_state_retry
                            && !runtime_websocket_request_requires_locked_previous_response_affinity(
                                request_requires_previous_response_affinity,
                                trusted_previous_response_affinity,
                                previous_response_id.as_deref(),
                                request_turn_state.as_deref(),
                            )
                        {
                            let _ = clear_runtime_stale_previous_response_binding(
                                shared,
                                &profile_name,
                                previous_response_id.as_deref(),
                            )?;
                        }
                        let released_affinity = release_runtime_previous_response_affinity(
                            shared,
                            &profile_name,
                            previous_response_id.as_deref(),
                            request_turn_state.as_deref(),
                            request_session_id.as_deref(),
                            RuntimeRouteKind::Websocket,
                        )?;
                        if released_affinity {
                            runtime_proxy_log(
                                shared,
                                format!(
                                    "request={request_id} websocket_session={session_id} previous_response_affinity_released profile={profile_name} via=direct_current_profile_fallback"
                                ),
                            );
                        }
                        clear_runtime_response_profile_affinity(
                            &profile_name,
                            &mut bound_profile,
                            &mut session_profile,
                            &mut candidate_turn_state_retry_profile,
                            &mut candidate_turn_state_retry_value,
                            &mut pinned_profile,
                            &mut previous_response_retry_index,
                            false,
                            &mut turn_state_profile,
                            Some(&mut compact_followup_profile),
                        );
                        excluded_profiles.insert(profile_name);
                        last_failure =
                            Some((RuntimeUpstreamFailureResponse::Websocket(payload), false));
                        continue;
                    }
                    RuntimeWebsocketAttempt::ReuseWatchdogTripped { profile_name, .. } => {
                        excluded_profiles.insert(profile_name);
                        continue;
                    }
                    RuntimeWebsocketAttempt::LocalSelectionBlocked {
                        profile_name,
                        reason,
                    } => {
                        mark_runtime_profile_retry_backoff(shared, &profile_name)?;
                        if !runtime_quota_blocked_affinity_is_releasable(
                            runtime_candidate_affinity!(
                                RuntimeRouteKind::Websocket,
                                &profile_name,
                                compact_followup_profile
                                    .as_ref()
                                    .map(|(profile_name, _)| profile_name.as_str()),
                            ),
                            runtime_websocket_request_requires_locked_previous_response_affinity(
                                request_requires_previous_response_affinity,
                                trusted_previous_response_affinity,
                                previous_response_id.as_deref(),
                                request_turn_state.as_deref(),
                            ),
                        ) {
                            send_runtime_proxy_websocket_error(
                                local_socket,
                                503,
                                "service_unavailable",
                                runtime_proxy_local_selection_failure_message(),
                            )?;
                            return Ok(());
                        }
                        let released_affinity = release_runtime_quota_blocked_affinity(
                            shared,
                            &profile_name,
                            previous_response_id.as_deref(),
                            request_turn_state.as_deref(),
                            request_session_id.as_deref(),
                        )?;
                        clear_runtime_response_profile_affinity(
                            &profile_name,
                            &mut bound_profile,
                            &mut session_profile,
                            &mut candidate_turn_state_retry_profile,
                            &mut candidate_turn_state_retry_value,
                            &mut pinned_profile,
                            &mut previous_response_retry_index,
                            false,
                            &mut turn_state_profile,
                            None,
                        );
                        if released_affinity {
                            runtime_proxy_log(
                                shared,
                                format!(
                                    "request={request_id} websocket_session={session_id} quota_blocked_affinity_released profile={profile_name} reason={reason} via=direct_current_profile_fallback"
                                ),
                            );
                        }
                        if runtime_quota_blocked_previous_response_fresh_fallback_allowed(
                            previous_response_id.as_deref(),
                            trusted_previous_response_affinity,
                            previous_response_fresh_fallback_used,
                        ) && let Some(fresh_request_text) =
                            runtime_request_text_without_previous_response_id(&request_text)
                        {
                            runtime_proxy_log(
                                shared,
                                format!(
                                    "request={request_id} websocket_session={session_id} previous_response_fresh_fallback reason={reason} via=direct_current_profile_fallback"
                                ),
                            );
                            apply_runtime_websocket_previous_response_fresh_fallback(
                                fresh_request_text,
                                runtime_websocket_fresh_fallback_target!(),
                                runtime_previous_response_fresh_fallback_state!(),
                            );
                            recompute_route_affinity!("previous_response_fresh_fallback")?;
                            continue;
                        }
                        excluded_profiles.insert(profile_name);
                        continue;
                    }
                }
            }
            send_runtime_proxy_final_websocket_failure(
                local_socket,
                last_failure,
                saw_inflight_saturation,
            )?;
            return Ok(());
        };
        selection_attempts = selection_attempts.saturating_add(1);
        let turn_state_override =
            if candidate_turn_state_retry_profile.as_deref() == Some(candidate_name.as_str()) {
                candidate_turn_state_retry_value.as_deref()
            } else {
                request_turn_state.as_deref()
            };
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} websocket_session={session_id} candidate={} pinned={:?} turn_state_profile={:?} turn_state_override={:?} excluded_count={}",
                candidate_name,
                pinned_profile,
                turn_state_profile,
                turn_state_override,
                excluded_profiles.len()
            ),
        );
        let session_affinity_candidate =
            session_profile.as_deref() == Some(candidate_name.as_str());
        if previous_response_id.is_none()
            && pinned_profile.is_none()
            && turn_state_profile.is_none()
            && !session_affinity_candidate
            && runtime_profile_inflight_hard_limited_for_context(
                shared,
                &candidate_name,
                "websocket_session",
            )?
        {
            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} websocket_session={session_id} profile_inflight_saturated profile={candidate_name} hard_limit={}",
                    runtime_proxy_profile_inflight_hard_limit(),
                ),
            );
            excluded_profiles.insert(candidate_name);
            saw_inflight_saturation = true;
            continue;
        }

        match attempt_runtime_websocket_request(RuntimeWebsocketAttemptRequest {
            request_id,
            local_socket,
            handshake_request: &handshake_request,
            request_text: &request_text,
            request_previous_response_id: previous_response_id.as_deref(),
            request_session_id: request_session_id.as_deref(),
            request_turn_state: request_turn_state.as_deref(),
            shared,
            websocket_session,
            profile_name: &candidate_name,
            turn_state_override,
            promote_committed_profile: previous_response_id.is_none()
                && bound_profile.is_none()
                && request_turn_state.is_none()
                && turn_state_profile.is_none()
                && compact_followup_profile.is_none()
                && !(request_session_id_header_present || bound_session_profile.is_some()),
        })? {
            RuntimeWebsocketAttempt::Delivered => return Ok(()),
            RuntimeWebsocketAttempt::QuotaBlocked {
                profile_name,
                payload,
            } => {
                let quota_message =
                    extract_runtime_proxy_quota_message_from_websocket_payload(&payload);
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} websocket_session={session_id} quota_blocked profile={profile_name}"
                    ),
                );
                mark_runtime_profile_quota_quarantine(
                    shared,
                    &profile_name,
                    RuntimeRouteKind::Websocket,
                    quota_message.as_deref(),
                )?;
                if !runtime_quota_blocked_affinity_is_releasable(
                    runtime_candidate_affinity!(
                        RuntimeRouteKind::Websocket,
                        &profile_name,
                        compact_followup_profile
                            .as_ref()
                            .map(|(profile_name, _)| profile_name.as_str()),
                    ),
                    request_requires_previous_response_affinity,
                ) {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} websocket_session={session_id} upstream_usage_limit_passthrough route=websocket profile={profile_name} reason=hard_affinity"
                        ),
                    );
                    forward_runtime_proxy_websocket_error(local_socket, &payload)?;
                    return Ok(());
                }
                let released_affinity = release_runtime_quota_blocked_affinity(
                    shared,
                    &profile_name,
                    previous_response_id.as_deref(),
                    request_turn_state.as_deref(),
                    request_session_id.as_deref(),
                )?;
                clear_runtime_response_profile_affinity(
                    &profile_name,
                    &mut bound_profile,
                    &mut session_profile,
                    &mut candidate_turn_state_retry_profile,
                    &mut candidate_turn_state_retry_value,
                    &mut pinned_profile,
                    &mut previous_response_retry_index,
                    true,
                    &mut turn_state_profile,
                    None,
                );
                if released_affinity {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} websocket_session={session_id} quota_blocked_affinity_released profile={profile_name}"
                        ),
                    );
                }
                if runtime_quota_blocked_previous_response_fresh_fallback_allowed(
                    previous_response_id.as_deref(),
                    trusted_previous_response_affinity,
                    previous_response_fresh_fallback_used,
                ) && let Some(fresh_request_text) =
                    runtime_request_text_without_previous_response_id(&request_text)
                {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} websocket_session={session_id} previous_response_fresh_fallback reason=quota_blocked"
                        ),
                    );
                    apply_runtime_websocket_previous_response_fresh_fallback(
                        fresh_request_text,
                        runtime_websocket_fresh_fallback_target!(),
                        runtime_previous_response_fresh_fallback_state!(),
                    );
                    recompute_route_affinity!("previous_response_fresh_fallback")?;
                    continue;
                }
                if !runtime_has_route_eligible_quota_fallback(
                    shared,
                    &profile_name,
                    &BTreeSet::new(),
                    RuntimeRouteKind::Websocket,
                )? {
                    forward_runtime_proxy_websocket_error(local_socket, &payload)?;
                    return Ok(());
                }
                excluded_profiles.insert(profile_name);
                last_failure = Some((RuntimeUpstreamFailureResponse::Websocket(payload), true));
            }
            RuntimeWebsocketAttempt::Overloaded {
                profile_name,
                payload,
            } => {
                let overload_message =
                    extract_runtime_proxy_overload_message_from_websocket_payload(&payload);
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} websocket_session={session_id} upstream_overloaded route=websocket profile={profile_name} message={}",
                        overload_message.as_deref().unwrap_or("-"),
                    ),
                );
                mark_runtime_profile_retry_backoff(shared, &profile_name)?;
                let _ = bump_runtime_profile_health_score(
                    shared,
                    &profile_name,
                    RuntimeRouteKind::Websocket,
                    RUNTIME_PROFILE_OVERLOAD_HEALTH_PENALTY,
                    "websocket_overload",
                );
                let _ = bump_runtime_profile_bad_pairing_score(
                    shared,
                    &profile_name,
                    RuntimeRouteKind::Websocket,
                    RUNTIME_PROFILE_BAD_PAIRING_PENALTY,
                    "websocket_overload",
                );
                if !runtime_quota_blocked_affinity_is_releasable(
                    runtime_candidate_affinity!(
                        RuntimeRouteKind::Websocket,
                        &profile_name,
                        compact_followup_profile
                            .as_ref()
                            .map(|(profile_name, _)| profile_name.as_str()),
                    ),
                    runtime_websocket_request_requires_locked_previous_response_affinity(
                        request_requires_previous_response_affinity,
                        trusted_previous_response_affinity,
                        previous_response_id.as_deref(),
                        request_turn_state.as_deref(),
                    ),
                ) {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} websocket_session={session_id} upstream_overload_passthrough route=websocket profile={profile_name} reason=hard_affinity"
                        ),
                    );
                    forward_runtime_proxy_websocket_error(local_socket, &payload)?;
                    return Ok(());
                }
                if runtime_quota_blocked_previous_response_fresh_fallback_allowed(
                    previous_response_id.as_deref(),
                    trusted_previous_response_affinity,
                    previous_response_fresh_fallback_used,
                ) && let Some(fresh_request_text) =
                    runtime_request_text_without_previous_response_id(&request_text)
                {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} websocket_session={session_id} previous_response_fresh_fallback reason=upstream_overloaded"
                        ),
                    );
                    apply_runtime_websocket_previous_response_fresh_fallback(
                        fresh_request_text,
                        runtime_websocket_fresh_fallback_target!(),
                        runtime_previous_response_fresh_fallback_state!(),
                    );
                    recompute_route_affinity!("previous_response_fresh_fallback")?;
                    continue;
                }
                excluded_profiles.insert(profile_name);
                last_failure = Some((RuntimeUpstreamFailureResponse::Websocket(payload), false));
            }
            RuntimeWebsocketAttempt::LocalSelectionBlocked {
                profile_name,
                reason,
            } => {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} websocket_session={session_id} local_selection_blocked profile={profile_name} reason={reason}"
                    ),
                );
                mark_runtime_profile_retry_backoff(shared, &profile_name)?;
                if !runtime_quota_blocked_affinity_is_releasable(
                    runtime_candidate_affinity!(
                        RuntimeRouteKind::Websocket,
                        &profile_name,
                        compact_followup_profile
                            .as_ref()
                            .map(|(profile_name, _)| profile_name.as_str()),
                    ),
                    runtime_websocket_request_requires_locked_previous_response_affinity(
                        request_requires_previous_response_affinity,
                        trusted_previous_response_affinity,
                        previous_response_id.as_deref(),
                        request_turn_state.as_deref(),
                    ),
                ) {
                    send_runtime_proxy_websocket_error(
                        local_socket,
                        503,
                        "service_unavailable",
                        runtime_proxy_local_selection_failure_message(),
                    )?;
                    return Ok(());
                }
                let released_affinity = release_runtime_quota_blocked_affinity(
                    shared,
                    &profile_name,
                    previous_response_id.as_deref(),
                    request_turn_state.as_deref(),
                    request_session_id.as_deref(),
                )?;
                clear_runtime_response_profile_affinity(
                    &profile_name,
                    &mut bound_profile,
                    &mut session_profile,
                    &mut candidate_turn_state_retry_profile,
                    &mut candidate_turn_state_retry_value,
                    &mut pinned_profile,
                    &mut previous_response_retry_index,
                    true,
                    &mut turn_state_profile,
                    None,
                );
                if released_affinity {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} websocket_session={session_id} quota_blocked_affinity_released profile={profile_name} reason={reason}"
                        ),
                    );
                }
                if runtime_quota_blocked_previous_response_fresh_fallback_allowed(
                    previous_response_id.as_deref(),
                    trusted_previous_response_affinity,
                    previous_response_fresh_fallback_used,
                ) && let Some(fresh_request_text) =
                    runtime_request_text_without_previous_response_id(&request_text)
                {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} websocket_session={session_id} previous_response_fresh_fallback reason={reason}"
                        ),
                    );
                    apply_runtime_websocket_previous_response_fresh_fallback(
                        fresh_request_text,
                        runtime_websocket_fresh_fallback_target!(),
                        runtime_previous_response_fresh_fallback_state!(),
                    );
                    recompute_route_affinity!("previous_response_fresh_fallback")?;
                    continue;
                }
                excluded_profiles.insert(profile_name);
            }
            RuntimeWebsocketAttempt::ReuseWatchdogTripped {
                profile_name,
                event,
            } => {
                let reuse_terminal_idle = websocket_session.last_terminal_elapsed();
                let retry_same_profile_with_fresh_connect = !websocket_reuse_fresh_retry_profiles
                    .contains(&profile_name)
                    && (bound_profile.as_deref() == Some(profile_name.as_str())
                        || turn_state_profile.as_deref() == Some(profile_name.as_str())
                        || compact_followup_profile
                            .as_ref()
                            .is_some_and(|(owner, _)| owner == &profile_name)
                        || (request_session_id.is_some()
                            && session_profile.as_deref() == Some(profile_name.as_str())));
                let reuse_failed_bound_previous_response = previous_response_id.is_some()
                    && !previous_response_fresh_fallback_used
                    && (bound_profile.as_deref() == Some(profile_name.as_str())
                        || pinned_profile.as_deref() == Some(profile_name.as_str()));
                let nonreplayable_previous_response_reuse = previous_response_id.is_some()
                    && !previous_response_fresh_fallback_used
                    && turn_state_override.is_none();
                let stale_previous_response_reuse = nonreplayable_previous_response_reuse
                    && turn_state_override.is_none()
                    && reuse_terminal_idle.is_some_and(|elapsed| {
                        elapsed
                            >= Duration::from_millis(
                                runtime_proxy_websocket_previous_response_reuse_stale_ms(),
                            )
                    });
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} websocket_session={session_id} websocket_reuse_watchdog_timeout profile={profile_name} event={event}"
                    ),
                );
                if nonreplayable_previous_response_reuse {
                    if stale_previous_response_reuse {
                        runtime_proxy_log(
                            shared,
                            format!(
                                "request={request_id} websocket_session={session_id} websocket_reuse_stale_previous_response_blocked profile={profile_name} event={event} elapsed_ms={} threshold_ms={}",
                                reuse_terminal_idle
                                    .map(|elapsed| elapsed.as_millis())
                                    .unwrap_or(0),
                                runtime_proxy_websocket_previous_response_reuse_stale_ms(),
                            ),
                        );
                    } else {
                        runtime_proxy_log(
                            shared,
                            format!(
                                "request={request_id} websocket_session={session_id} websocket_reuse_previous_response_blocked profile={profile_name} event={event} reason=missing_turn_state elapsed_ms={}",
                                reuse_terminal_idle
                                    .map(|elapsed| elapsed.as_millis())
                                    .unwrap_or(0),
                            ),
                        );
                    }
                    return Err(anyhow::anyhow!(
                        "runtime websocket upstream closed before response.completed for previous_response_id continuation without replayable turn_state: profile={profile_name} event={event}"
                    ));
                }
                if retry_same_profile_with_fresh_connect {
                    websocket_reuse_fresh_retry_profiles.insert(profile_name.clone());
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} websocket_session={session_id} websocket_reuse_owner_fresh_retry profile={profile_name} event={event}"
                        ),
                    );
                    continue;
                }
                if reuse_failed_bound_previous_response
                    && !runtime_websocket_request_requires_locked_previous_response_affinity(
                        request_requires_previous_response_affinity,
                        trusted_previous_response_affinity,
                        previous_response_id.as_deref(),
                        request_turn_state.as_deref(),
                    )
                    && let Some(fresh_request_text) =
                        runtime_request_text_without_previous_response_id(&request_text)
                {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} websocket_session={session_id} previous_response_fresh_fallback reason=websocket_reuse_watchdog"
                        ),
                    );
                    apply_runtime_websocket_previous_response_fresh_fallback(
                        fresh_request_text,
                        runtime_websocket_fresh_fallback_target!(),
                        runtime_previous_response_fresh_fallback_state!(),
                    );
                    recompute_route_affinity!("previous_response_fresh_fallback")?;
                    continue;
                }
                clear_runtime_response_profile_affinity(
                    &profile_name,
                    &mut bound_profile,
                    &mut session_profile,
                    &mut candidate_turn_state_retry_profile,
                    &mut candidate_turn_state_retry_value,
                    &mut pinned_profile,
                    &mut previous_response_retry_index,
                    true,
                    &mut turn_state_profile,
                    None,
                );
                excluded_profiles.insert(profile_name);
            }
            RuntimeWebsocketAttempt::PreviousResponseNotFound {
                profile_name,
                payload,
                turn_state,
            } => {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=websocket route=websocket websocket_session={session_id} previous_response_not_found profile={profile_name} retry_index={previous_response_retry_index} replay_turn_state={:?}",
                        turn_state
                    ),
                );
                saw_previous_response_not_found = true;
                if previous_response_retry_candidate.as_deref() != Some(profile_name.as_str()) {
                    previous_response_retry_candidate = Some(profile_name.clone());
                    previous_response_retry_index = 0;
                }
                let has_turn_state_retry = turn_state.is_some();
                if has_turn_state_retry {
                    candidate_turn_state_retry_profile = Some(profile_name.clone());
                    candidate_turn_state_retry_value = turn_state;
                }
                if has_turn_state_retry
                    && let Some(delay) =
                        runtime_previous_response_retry_delay(previous_response_retry_index)
                {
                    previous_response_retry_index += 1;
                    last_failure =
                        Some((RuntimeUpstreamFailureResponse::Websocket(payload), false));
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} websocket_session={session_id} previous_response_retry_immediate profile={profile_name} delay_ms={} reason=non_blocking_retry",
                            delay.as_millis()
                        ),
                    );
                    continue;
                }
                previous_response_retry_candidate = None;
                previous_response_retry_index = 0;
                if !has_turn_state_retry
                    && !runtime_websocket_request_requires_locked_previous_response_affinity(
                        request_requires_previous_response_affinity,
                        trusted_previous_response_affinity,
                        previous_response_id.as_deref(),
                        request_turn_state.as_deref(),
                    )
                {
                    let _ = clear_runtime_stale_previous_response_binding(
                        shared,
                        &profile_name,
                        previous_response_id.as_deref(),
                    )?;
                }
                let released_affinity = release_runtime_previous_response_affinity(
                    shared,
                    &profile_name,
                    previous_response_id.as_deref(),
                    request_turn_state.as_deref(),
                    request_session_id.as_deref(),
                    RuntimeRouteKind::Websocket,
                )?;
                if released_affinity {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} websocket_session={session_id} previous_response_affinity_released profile={profile_name}"
                        ),
                    );
                }
                clear_runtime_response_profile_affinity(
                    &profile_name,
                    &mut bound_profile,
                    &mut session_profile,
                    &mut candidate_turn_state_retry_profile,
                    &mut candidate_turn_state_retry_value,
                    &mut pinned_profile,
                    &mut previous_response_retry_index,
                    false,
                    &mut turn_state_profile,
                    Some(&mut compact_followup_profile),
                );
                trusted_previous_response_affinity = false;
                excluded_profiles.insert(profile_name);
                last_failure = Some((RuntimeUpstreamFailureResponse::Websocket(payload), false));
            }
        }
    }
}

pub(super) struct RuntimeWebsocketAttemptRequest<'a> {
    request_id: u64,
    local_socket: &'a mut RuntimeLocalWebSocket,
    handshake_request: &'a RuntimeProxyRequest,
    request_text: &'a str,
    request_previous_response_id: Option<&'a str>,
    request_session_id: Option<&'a str>,
    request_turn_state: Option<&'a str>,
    shared: &'a RuntimeRotationProxyShared,
    websocket_session: &'a mut RuntimeWebsocketSessionState,
    profile_name: &'a str,
    turn_state_override: Option<&'a str>,
    promote_committed_profile: bool,
}

pub(super) fn attempt_runtime_websocket_request(
    attempt: RuntimeWebsocketAttemptRequest<'_>,
) -> Result<RuntimeWebsocketAttempt> {
    let RuntimeWebsocketAttemptRequest {
        request_id,
        local_socket,
        handshake_request,
        request_text,
        request_previous_response_id,
        request_session_id,
        request_turn_state,
        shared,
        websocket_session,
        profile_name,
        turn_state_override,
        promote_committed_profile,
    } = attempt;

    let realtime_websocket = is_runtime_realtime_websocket_path(&handshake_request.path_and_query);
    let (initial_quota_summary, initial_quota_source) =
        runtime_profile_quota_summary_for_route(shared, profile_name, RuntimeRouteKind::Websocket)?;
    if (request_previous_response_id.is_some()
        || request_session_id.is_some()
        || request_turn_state.is_some())
        && matches!(
            initial_quota_source,
            Some(RuntimeQuotaSource::PersistedSnapshot)
        )
        && let Some(reason) =
            runtime_quota_precommit_guard_reason(initial_quota_summary, RuntimeRouteKind::Websocket)
    {
        websocket_session.close();
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=websocket websocket_pre_send_skip profile={profile_name} reason={reason} quota_source={} {}",
                initial_quota_source
                    .map(runtime_quota_source_label)
                    .unwrap_or("unknown"),
                runtime_quota_summary_log_fields(initial_quota_summary),
            ),
        );
        return Ok(RuntimeWebsocketAttempt::LocalSelectionBlocked {
            profile_name: profile_name.to_string(),
            reason,
        });
    }
    let has_alternative_quota_profile = runtime_has_route_eligible_quota_fallback(
        shared,
        profile_name,
        &BTreeSet::new(),
        RuntimeRouteKind::Websocket,
    )?;
    let (quota_summary, quota_source) = ensure_runtime_profile_precommit_quota_ready(
        shared,
        profile_name,
        RuntimeRouteKind::Websocket,
        "websocket_precommit_reprobe",
    )?;
    if runtime_quota_summary_requires_live_source_after_probe(
        quota_summary,
        quota_source,
        RuntimeRouteKind::Websocket,
    ) && has_alternative_quota_profile
    {
        websocket_session.close();
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=websocket websocket_pre_send_skip profile={profile_name} reason=quota_windows_unavailable_after_reprobe quota_source={} {}",
                quota_source
                    .map(runtime_quota_source_label)
                    .unwrap_or("unknown"),
                runtime_quota_summary_log_fields(quota_summary),
            ),
        );
        return Ok(RuntimeWebsocketAttempt::LocalSelectionBlocked {
            profile_name: profile_name.to_string(),
            reason: "quota_windows_unavailable_after_reprobe",
        });
    }
    if let Some(reason) =
        runtime_quota_precommit_guard_reason(quota_summary, RuntimeRouteKind::Websocket)
    {
        websocket_session.close();
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=websocket websocket_pre_send_skip profile={profile_name} reason={reason} quota_source={} {}",
                quota_source
                    .map(runtime_quota_source_label)
                    .unwrap_or("unknown"),
                runtime_quota_summary_log_fields(quota_summary),
            ),
        );
        return Ok(RuntimeWebsocketAttempt::LocalSelectionBlocked {
            profile_name: profile_name.to_string(),
            reason,
        });
    }

    let reuse_existing_session = websocket_session.can_reuse(profile_name, turn_state_override);
    let reuse_started_at = reuse_existing_session.then(Instant::now);
    let precommit_started_at = Instant::now();
    let (mut upstream_socket, mut upstream_turn_state, mut inflight_guard) =
        if reuse_existing_session {
            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} transport=websocket websocket_reuse_start profile={profile_name} turn_state_override={:?}",
                    turn_state_override
                ),
            );
            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} transport=websocket upstream_session=reuse profile={profile_name} turn_state_override={:?}",
                    turn_state_override
                ),
            );
            (
                websocket_session
                    .take_socket()
                    .expect("runtime websocket session should keep its upstream socket"),
                websocket_session.turn_state.clone(),
                None,
            )
        } else {
            websocket_session.close();
            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} transport=websocket upstream_session=connect profile={profile_name} turn_state_override={:?}",
                    turn_state_override
                ),
            );
            match connect_runtime_proxy_upstream_websocket(
                request_id,
                handshake_request,
                shared,
                profile_name,
                turn_state_override,
            )? {
                RuntimeWebsocketConnectResult::Connected { socket, turn_state } => (
                    socket,
                    turn_state,
                    Some(acquire_runtime_profile_inflight_guard(
                        shared,
                        profile_name,
                        "websocket_session",
                    )?),
                ),
                RuntimeWebsocketConnectResult::QuotaBlocked(payload) => {
                    return Ok(RuntimeWebsocketAttempt::QuotaBlocked {
                        profile_name: profile_name.to_string(),
                        payload,
                    });
                }
                RuntimeWebsocketConnectResult::Overloaded(payload) => {
                    return Ok(RuntimeWebsocketAttempt::Overloaded {
                        profile_name: profile_name.to_string(),
                        payload,
                    });
                }
            }
        };
    runtime_set_upstream_websocket_io_timeout(
        &mut upstream_socket,
        Some(Duration::from_millis(
            runtime_proxy_websocket_precommit_progress_timeout_ms(),
        )),
    )
    .context("failed to configure runtime websocket pre-commit timeout")?;

    if let Err(err) = upstream_socket.send(WsMessage::Text(request_text.to_string().into())) {
        let _ = upstream_socket.close(None);
        websocket_session.reset();
        let transport_error =
            anyhow::anyhow!("failed to send runtime websocket request upstream: {err}");
        note_runtime_profile_transport_failure(
            shared,
            profile_name,
            RuntimeRouteKind::Websocket,
            "websocket_upstream_send",
            &transport_error,
        );
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=websocket upstream_send_error profile={profile_name} error={err}"
            ),
        );
        if reuse_existing_session {
            return Ok(RuntimeWebsocketAttempt::ReuseWatchdogTripped {
                profile_name: profile_name.to_string(),
                event: "upstream_send_error",
            });
        }
        return Err(transport_error);
    }

    let mut committed = false;
    let mut first_upstream_frame_seen = false;
    let mut buffered_precommit_text_frames = Vec::new();
    let mut previous_response_owner_recorded = false;
    let mut precommit_hold_count = 0usize;
    loop {
        match upstream_socket.read() {
            Ok(WsMessage::Text(text)) => {
                let text = text.to_string();
                if !first_upstream_frame_seen {
                    first_upstream_frame_seen = true;
                    runtime_set_upstream_websocket_io_timeout(
                        &mut upstream_socket,
                        Some(Duration::from_millis(if reuse_existing_session {
                            runtime_proxy_websocket_precommit_progress_timeout_ms()
                        } else {
                            runtime_proxy_stream_idle_timeout_ms()
                        })),
                    )
                    .context("failed to restore runtime websocket upstream timeout")?;
                }

                let mut inspected = inspect_runtime_websocket_text_frame(text.as_str());
                if realtime_websocket
                    && inspected
                        .event_type
                        .as_deref()
                        .is_some_and(runtime_realtime_websocket_terminal_event_kind)
                {
                    inspected.terminal_event = true;
                }
                if let Some(turn_state) = inspected.turn_state.as_deref() {
                    remember_runtime_turn_state(
                        shared,
                        profile_name,
                        Some(turn_state),
                        RuntimeRouteKind::Websocket,
                    )?;
                    upstream_turn_state = Some(turn_state.to_string());
                }

                if !committed {
                    match inspected.retry_kind {
                        Some(RuntimeWebsocketRetryInspectionKind::QuotaBlocked) => {
                            let _ = upstream_socket.close(None);
                            websocket_session.reset();
                            return Ok(RuntimeWebsocketAttempt::QuotaBlocked {
                                profile_name: profile_name.to_string(),
                                payload: RuntimeWebsocketErrorPayload::Text(text),
                            });
                        }
                        Some(RuntimeWebsocketRetryInspectionKind::Overloaded) => {
                            let _ = upstream_socket.close(None);
                            websocket_session.reset();
                            return Ok(RuntimeWebsocketAttempt::Overloaded {
                                profile_name: profile_name.to_string(),
                                payload: RuntimeWebsocketErrorPayload::Text(text),
                            });
                        }
                        Some(RuntimeWebsocketRetryInspectionKind::PreviousResponseNotFound) => {
                            let _ = upstream_socket.close(None);
                            websocket_session.reset();
                            return Ok(RuntimeWebsocketAttempt::PreviousResponseNotFound {
                                profile_name: profile_name.to_string(),
                                payload: RuntimeWebsocketErrorPayload::Text(text),
                                turn_state: upstream_turn_state.clone(),
                            });
                        }
                        None => {}
                    }
                }

                if reuse_existing_session && !committed && inspected.precommit_hold {
                    if precommit_hold_count == 0 {
                        runtime_proxy_log(
                            shared,
                            format!(
                                "request={request_id} transport=websocket precommit_hold profile={profile_name} event_type={}",
                                inspected.event_type.as_deref().unwrap_or("-")
                            ),
                        );
                    }
                    precommit_hold_count = precommit_hold_count.saturating_add(1);
                    buffered_precommit_text_frames.push(RuntimeBufferedWebsocketTextFrame {
                        text,
                        response_ids: inspected.response_ids,
                    });
                    let elapsed_ms = precommit_started_at.elapsed().as_millis();
                    let timeout_ms = runtime_proxy_websocket_precommit_progress_timeout_ms();
                    if elapsed_ms >= u128::from(timeout_ms) {
                        let _ = upstream_socket.close(None);
                        websocket_session.reset();
                        runtime_proxy_log(
                            shared,
                            format!(
                                "websocket_precommit_hold_timeout profile={profile_name} elapsed_ms={elapsed_ms} threshold_ms={timeout_ms} reuse={reuse_existing_session} hold_count={precommit_hold_count}"
                            ),
                        );
                        let transport_error = anyhow::anyhow!(
                            "runtime websocket upstream remained in pre-commit hold beyond the progress deadline after {elapsed_ms} ms"
                        );
                        note_runtime_profile_transport_failure(
                            shared,
                            profile_name,
                            RuntimeRouteKind::Websocket,
                            "websocket_precommit_hold_timeout",
                            &transport_error,
                        );
                        return Ok(RuntimeWebsocketAttempt::ReuseWatchdogTripped {
                            profile_name: profile_name.to_string(),
                            event: "precommit_hold_timeout",
                        });
                    }
                    continue;
                }

                if !committed {
                    runtime_set_upstream_websocket_io_timeout(
                        &mut upstream_socket,
                        Some(Duration::from_millis(runtime_proxy_stream_idle_timeout_ms())),
                    )
                    .context("failed to restore runtime websocket idle timeout")?;
                    remember_runtime_session_id(
                        shared,
                        profile_name,
                        request_session_id,
                        RuntimeRouteKind::Websocket,
                    )?;
                    remember_runtime_turn_state(
                        shared,
                        profile_name,
                        upstream_turn_state.as_deref(),
                        RuntimeRouteKind::Websocket,
                    )?;
                    let _ = commit_runtime_proxy_profile_selection_with_policy(
                        shared,
                        profile_name,
                        RuntimeRouteKind::Websocket,
                        promote_committed_profile,
                    )?;
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} transport=websocket committed profile={profile_name}"
                        ),
                    );
                    committed = true;
                    forward_runtime_proxy_buffered_websocket_text_frames(
                        local_socket,
                        &mut buffered_precommit_text_frames,
                        RuntimeWebsocketResponseBindingContext {
                            shared,
                            profile_name,
                            request_previous_response_id,
                            request_session_id,
                            request_turn_state,
                            response_turn_state: upstream_turn_state.as_deref(),
                        },
                        &mut previous_response_owner_recorded,
                    )?;
                }

                remember_runtime_websocket_response_ids(
                    RuntimeWebsocketResponseBindingContext {
                        shared,
                        profile_name,
                        request_previous_response_id,
                        request_session_id,
                        request_turn_state,
                        response_turn_state: upstream_turn_state.as_deref(),
                    },
                    &inspected.response_ids,
                    &mut previous_response_owner_recorded,
                )?;
                local_socket
                    .send(WsMessage::Text(text.into()))
                    .with_context(|| {
                        websocket_session.reset();
                        "failed to forward runtime websocket text frame"
                    })?;
                if inspected.terminal_event {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} transport=websocket terminal_event profile={profile_name} event_type={} precommit_hold_count={precommit_hold_count}",
                            inspected.event_type.as_deref().unwrap_or("-"),
                        ),
                    );
                    websocket_session.store(
                        upstream_socket,
                        profile_name,
                        upstream_turn_state,
                        inflight_guard.take(),
                    );
                    return Ok(RuntimeWebsocketAttempt::Delivered);
                }
            }
            Ok(WsMessage::Binary(payload)) => {
                if !first_upstream_frame_seen {
                    first_upstream_frame_seen = true;
                    runtime_set_upstream_websocket_io_timeout(
                        &mut upstream_socket,
                        Some(Duration::from_millis(if reuse_existing_session {
                            runtime_proxy_websocket_precommit_progress_timeout_ms()
                        } else {
                            runtime_proxy_stream_idle_timeout_ms()
                        })),
                    )
                    .context("failed to restore runtime websocket upstream timeout")?;
                }
                if !committed {
                    runtime_set_upstream_websocket_io_timeout(
                        &mut upstream_socket,
                        Some(Duration::from_millis(runtime_proxy_stream_idle_timeout_ms())),
                    )
                    .context("failed to restore runtime websocket idle timeout")?;
                    remember_runtime_session_id(
                        shared,
                        profile_name,
                        request_session_id,
                        RuntimeRouteKind::Websocket,
                    )?;
                    remember_runtime_turn_state(
                        shared,
                        profile_name,
                        upstream_turn_state.as_deref(),
                        RuntimeRouteKind::Websocket,
                    )?;
                    let _ = commit_runtime_proxy_profile_selection_with_policy(
                        shared,
                        profile_name,
                        RuntimeRouteKind::Websocket,
                        promote_committed_profile,
                    )?;
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} transport=websocket committed_binary profile={profile_name}"
                        ),
                    );
                    committed = true;
                    forward_runtime_proxy_buffered_websocket_text_frames(
                        local_socket,
                        &mut buffered_precommit_text_frames,
                        RuntimeWebsocketResponseBindingContext {
                            shared,
                            profile_name,
                            request_previous_response_id,
                            request_session_id,
                            request_turn_state,
                            response_turn_state: upstream_turn_state.as_deref(),
                        },
                        &mut previous_response_owner_recorded,
                    )?;
                }
                local_socket
                    .send(WsMessage::Binary(payload))
                    .with_context(|| {
                        websocket_session.reset();
                        "failed to forward runtime websocket binary frame"
                    })?;
            }
            Ok(WsMessage::Ping(payload)) => {
                if !first_upstream_frame_seen {
                    first_upstream_frame_seen = true;
                    runtime_set_upstream_websocket_io_timeout(
                        &mut upstream_socket,
                        Some(Duration::from_millis(if reuse_existing_session {
                            runtime_proxy_websocket_precommit_progress_timeout_ms()
                        } else {
                            runtime_proxy_stream_idle_timeout_ms()
                        })),
                    )
                    .context("failed to restore runtime websocket upstream timeout")?;
                }
                upstream_socket
                    .send(WsMessage::Pong(payload))
                    .context("failed to respond to upstream websocket ping")?;
            }
            Ok(WsMessage::Pong(_)) | Ok(WsMessage::Frame(_)) => {
                if !first_upstream_frame_seen {
                    first_upstream_frame_seen = true;
                    runtime_set_upstream_websocket_io_timeout(
                        &mut upstream_socket,
                        Some(Duration::from_millis(if reuse_existing_session {
                            runtime_proxy_websocket_precommit_progress_timeout_ms()
                        } else {
                            runtime_proxy_stream_idle_timeout_ms()
                        })),
                    )
                    .context("failed to restore runtime websocket upstream timeout")?;
                }
            }
            Ok(WsMessage::Close(frame)) => {
                websocket_session.reset();
                if let Some(started_at) = reuse_started_at {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "websocket_reuse_watchdog profile={profile_name} event=upstream_close_before_terminal elapsed_ms={} committed={committed}",
                            started_at.elapsed().as_millis()
                        ),
                    );
                }
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=websocket upstream_close_before_completed profile={profile_name}"
                    ),
                );
                let _ = frame;
                let transport_error =
                    anyhow::anyhow!("runtime websocket upstream closed before response.completed");
                note_runtime_profile_transport_failure(
                    shared,
                    profile_name,
                    RuntimeRouteKind::Websocket,
                    "websocket_upstream_close",
                    &transport_error,
                );
                if reuse_existing_session && !committed {
                    return Ok(RuntimeWebsocketAttempt::ReuseWatchdogTripped {
                        profile_name: profile_name.to_string(),
                        event: "upstream_close_before_commit",
                    });
                }
                return Err(transport_error);
            }
            Err(WsError::ConnectionClosed) | Err(WsError::AlreadyClosed) => {
                websocket_session.reset();
                if let Some(started_at) = reuse_started_at {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "websocket_reuse_watchdog profile={profile_name} event=connection_closed elapsed_ms={} committed={committed}",
                            started_at.elapsed().as_millis()
                        ),
                    );
                }
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=websocket upstream_connection_closed profile={profile_name}"
                    ),
                );
                let transport_error =
                    anyhow::anyhow!("runtime websocket upstream closed before response.completed");
                note_runtime_profile_transport_failure(
                    shared,
                    profile_name,
                    RuntimeRouteKind::Websocket,
                    "websocket_upstream_connection_closed",
                    &transport_error,
                );
                if reuse_existing_session && !committed {
                    return Ok(RuntimeWebsocketAttempt::ReuseWatchdogTripped {
                        profile_name: profile_name.to_string(),
                        event: "connection_closed_before_commit",
                    });
                }
                return Err(transport_error);
            }
            Err(err) => {
                websocket_session.reset();
                if !committed
                    && reuse_existing_session
                    && precommit_hold_count > 0
                    && runtime_websocket_timeout_error(&err)
                {
                    let elapsed_ms = precommit_started_at.elapsed().as_millis();
                    let timeout_ms = runtime_proxy_websocket_precommit_progress_timeout_ms();
                    runtime_proxy_log(
                        shared,
                        format!(
                            "websocket_precommit_hold_timeout profile={profile_name} elapsed_ms={elapsed_ms} threshold_ms={timeout_ms} reuse={reuse_existing_session} hold_count={precommit_hold_count}"
                        ),
                    );
                    let transport_error = anyhow::anyhow!(
                        "runtime websocket upstream remained in pre-commit hold beyond the progress deadline after {elapsed_ms} ms: {err}"
                    );
                    note_runtime_profile_transport_failure(
                        shared,
                        profile_name,
                        RuntimeRouteKind::Websocket,
                        "websocket_precommit_hold_timeout",
                        &transport_error,
                    );
                    if let Some(started_at) = reuse_started_at {
                        runtime_proxy_log(
                            shared,
                            format!(
                                "websocket_reuse_watchdog profile={profile_name} event=precommit_hold_timeout elapsed_ms={} committed={committed}",
                                started_at.elapsed().as_millis()
                            ),
                        );
                    }
                    return Ok(RuntimeWebsocketAttempt::ReuseWatchdogTripped {
                        profile_name: profile_name.to_string(),
                        event: "precommit_hold_timeout",
                    });
                }
                if !committed && !first_upstream_frame_seen && runtime_websocket_timeout_error(&err)
                {
                    let elapsed_ms = precommit_started_at.elapsed().as_millis();
                    runtime_proxy_log(
                        shared,
                        format!(
                            "websocket_precommit_frame_timeout profile={profile_name} event=no_first_upstream_frame_before_deadline elapsed_ms={elapsed_ms} reuse={reuse_existing_session}"
                        ),
                    );
                    let transport_error = anyhow::anyhow!(
                        "runtime websocket upstream produced no first frame before the pre-commit deadline: {err}"
                    );
                    note_runtime_profile_transport_failure(
                        shared,
                        profile_name,
                        RuntimeRouteKind::Websocket,
                        "websocket_first_frame_timeout",
                        &transport_error,
                    );
                    if reuse_existing_session {
                        runtime_proxy_log(
                            shared,
                            format!(
                                "websocket_reuse_watchdog profile={profile_name} event=no_first_upstream_frame_before_deadline elapsed_ms={elapsed_ms} committed={committed}"
                            ),
                        );
                        return Ok(RuntimeWebsocketAttempt::ReuseWatchdogTripped {
                            profile_name: profile_name.to_string(),
                            event: "no_first_upstream_frame_before_deadline",
                        });
                    }
                    return Err(transport_error);
                }
                if let Some(started_at) = reuse_started_at {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "websocket_reuse_watchdog profile={profile_name} event=read_error elapsed_ms={} committed={committed}",
                            started_at.elapsed().as_millis()
                        ),
                    );
                }
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=websocket upstream_read_error profile={profile_name} error={err}"
                    ),
                );
                let transport_error = anyhow::anyhow!(
                    "runtime websocket upstream failed before response.completed: {err}"
                );
                note_runtime_profile_transport_failure(
                    shared,
                    profile_name,
                    RuntimeRouteKind::Websocket,
                    "websocket_upstream_read",
                    &transport_error,
                );
                if reuse_existing_session && !committed {
                    return Ok(RuntimeWebsocketAttempt::ReuseWatchdogTripped {
                        profile_name: profile_name.to_string(),
                        event: "upstream_read_error",
                    });
                }
                return Err(transport_error);
            }
        }
    }
}

pub(super) fn proxy_runtime_responses_request(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
) -> Result<RuntimeResponsesReply> {
    let mut request = request.clone();
    let request_requires_previous_response_affinity =
        runtime_request_requires_previous_response_affinity(&request);
    let mut previous_response_id = runtime_request_previous_response_id(&request);
    let mut request_turn_state = runtime_request_turn_state(&request);
    let explicit_request_session_id = runtime_request_explicit_session_id(&request);
    let request_session_id = runtime_request_session_id(&request);
    let mut bound_profile = previous_response_id
        .as_deref()
        .map(|response_id| {
            runtime_response_bound_profile(shared, response_id, RuntimeRouteKind::Responses)
        })
        .transpose()?
        .flatten();
    let mut trusted_previous_response_affinity = runtime_previous_response_affinity_is_trusted(
        shared,
        previous_response_id.as_deref(),
        bound_profile.as_deref(),
    )?;
    if request_turn_state.is_none()
        && let Some(turn_state) = runtime_previous_response_turn_state(
            shared,
            previous_response_id.as_deref(),
            bound_profile.as_deref(),
        )?
    {
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=http route=responses previous_response_turn_state_rehydrated response_id={} profile={} turn_state={turn_state}",
                previous_response_id.as_deref().unwrap_or("-"),
                bound_profile.as_deref().unwrap_or("-"),
            ),
        );
        request_turn_state = Some(turn_state);
    }
    let mut turn_state_profile = request_turn_state
        .as_deref()
        .map(|value| runtime_turn_state_bound_profile(shared, value))
        .transpose()?
        .flatten();
    let mut bound_session_profile = None;
    let mut compact_followup_profile = None;
    let mut compact_session_profile = None;
    let mut session_profile = None;
    let mut pinned_profile = None;
    macro_rules! recompute_route_affinity {
        ($reason:expr) => {
            refresh_and_log_runtime_response_route_affinity(
                shared,
                request_id,
                None,
                $reason,
                previous_response_id.as_deref(),
                bound_profile.as_deref(),
                turn_state_profile.as_deref(),
                request_turn_state.as_deref(),
                request_session_id.as_deref(),
                explicit_request_session_id.as_ref(),
                None,
                &mut bound_session_profile,
                &mut compact_followup_profile,
                &mut compact_session_profile,
                &mut session_profile,
                &mut pinned_profile,
            )
        };
    }
    recompute_route_affinity!("initial")?;
    let mut excluded_profiles = BTreeSet::new();
    let mut last_failure: Option<(RuntimeUpstreamFailureResponse, bool)> = None;
    let mut previous_response_retry_candidate: Option<String> = None;
    let mut previous_response_retry_index = 0usize;
    let mut candidate_turn_state_retry_profile: Option<String> = None;
    let mut candidate_turn_state_retry_value: Option<String> = None;
    let mut saw_inflight_saturation = false;
    let mut selection_started_at = Instant::now();
    let mut selection_attempts = 0usize;
    let mut previous_response_fresh_fallback_used = false;
    let mut saw_previous_response_not_found = false;
    macro_rules! runtime_candidate_affinity {
        ($route_kind:expr, $candidate_name:expr, $strict_affinity_profile:expr $(,)?) => {
            RuntimeCandidateAffinity {
                route_kind: $route_kind,
                candidate_name: $candidate_name,
                strict_affinity_profile: $strict_affinity_profile,
                pinned_profile: pinned_profile.as_deref(),
                turn_state_profile: turn_state_profile.as_deref(),
                session_profile: session_profile.as_deref(),
                trusted_previous_response_affinity,
            }
        };
    }
    macro_rules! runtime_previous_response_fresh_fallback_state {
        () => {
            RuntimePreviousResponseFreshFallbackState {
                previous_response_id: &mut previous_response_id,
                request_turn_state: &mut request_turn_state,
                previous_response_fresh_fallback_used: &mut previous_response_fresh_fallback_used,
                saw_previous_response_not_found: &mut saw_previous_response_not_found,
                previous_response_retry_candidate: &mut previous_response_retry_candidate,
                previous_response_retry_index: &mut previous_response_retry_index,
                candidate_turn_state_retry_profile: &mut candidate_turn_state_retry_profile,
                candidate_turn_state_retry_value: &mut candidate_turn_state_retry_value,
                trusted_previous_response_affinity: &mut trusted_previous_response_affinity,
                bound_profile: &mut bound_profile,
                pinned_profile: &mut pinned_profile,
                turn_state_profile: &mut turn_state_profile,
                session_profile: &mut session_profile,
                excluded_profiles: &mut excluded_profiles,
                last_failure: &mut last_failure,
                selection_started_at: &mut selection_started_at,
                selection_attempts: &mut selection_attempts,
            }
        };
    }

    loop {
        let pressure_mode =
            runtime_proxy_pressure_mode_active_for_route(shared, RuntimeRouteKind::Responses);
        if runtime_proxy_precommit_budget_exhausted(
            selection_started_at,
            selection_attempts,
            runtime_proxy_has_continuation_priority(
                previous_response_id.as_deref(),
                pinned_profile.as_deref(),
                request_turn_state.as_deref(),
                turn_state_profile.as_deref(),
                runtime_noncompact_session_priority_profile(
                    session_profile.as_deref(),
                    compact_session_profile.as_deref(),
                ),
            ),
            pressure_mode,
        ) {
            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} transport=http precommit_budget_exhausted attempts={selection_attempts} elapsed_ms={} pressure_mode={pressure_mode}",
                    selection_started_at.elapsed().as_millis()
                ),
            );
            if previous_response_id.is_some()
                && saw_previous_response_not_found
                && !previous_response_fresh_fallback_used
                && !runtime_request_requires_previous_response_affinity(&request)
                && let Some(fresh_request) =
                    runtime_request_without_previous_response_affinity(&request)
            {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=http previous_response_fresh_fallback reason=precommit_budget_exhausted"
                    ),
                );
                apply_runtime_responses_previous_response_fresh_fallback(
                    &mut request,
                    fresh_request,
                    runtime_previous_response_fresh_fallback_state!(),
                );
                recompute_route_affinity!("previous_response_fresh_fallback")?;
                continue;
            }
            if let Some((profile_name, source)) = compact_followup_profile.as_ref() {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=http compact_fresh_fallback_blocked profile={profile_name} source={source} reason=precommit_budget_exhausted"
                    ),
                );
                return Ok(runtime_proxy_final_responses_failure_reply(
                    last_failure,
                    saw_inflight_saturation,
                ));
            }
            if runtime_proxy_allows_direct_current_profile_fallback(
                previous_response_id.as_deref(),
                pinned_profile.as_deref(),
                request_turn_state.as_deref(),
                turn_state_profile.as_deref(),
                runtime_noncompact_session_priority_profile(
                    session_profile.as_deref(),
                    compact_session_profile.as_deref(),
                ),
                saw_inflight_saturation,
                last_failure.is_some(),
            ) && let Some(current_profile) = runtime_proxy_direct_current_fallback_profile(
                shared,
                &excluded_profiles,
                RuntimeRouteKind::Responses,
            )? {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=http direct_current_profile_fallback profile={current_profile} reason=precommit_budget_exhausted"
                    ),
                );
                match attempt_runtime_responses_request(
                    request_id,
                    &request,
                    shared,
                    &current_profile,
                    request_turn_state.as_deref(),
                )? {
                    RuntimeResponsesAttempt::Success {
                        profile_name,
                        response,
                    } => {
                        if saw_previous_response_not_found {
                            remember_runtime_successful_previous_response_owner(
                                shared,
                                &profile_name,
                                previous_response_id.as_deref(),
                                RuntimeRouteKind::Responses,
                            )?;
                        }
                        commit_runtime_proxy_profile_selection_with_notice(
                            shared,
                            &profile_name,
                            RuntimeRouteKind::Responses,
                        )?;
                        let _ = release_runtime_compact_lineage(
                            shared,
                            &profile_name,
                            request_session_id.as_deref(),
                            request_turn_state.as_deref(),
                            "response_committed_post_commit",
                        );
                        runtime_proxy_log(
                            shared,
                            format!(
                                "request={request_id} transport=http committed profile={profile_name} via=direct_current_profile_fallback"
                            ),
                        );
                        return Ok(response);
                    }
                    RuntimeResponsesAttempt::QuotaBlocked {
                        profile_name,
                        response,
                    } => {
                        mark_runtime_profile_retry_backoff(shared, &profile_name)?;
                        if !runtime_quota_blocked_affinity_is_releasable(
                            runtime_candidate_affinity!(
                                RuntimeRouteKind::Responses,
                                &profile_name,
                                compact_followup_profile
                                    .as_ref()
                                    .map(|(profile_name, _)| profile_name.as_str()),
                            ),
                            request_requires_previous_response_affinity,
                        ) {
                            return Ok(response);
                        }
                        let released_affinity = release_runtime_quota_blocked_affinity(
                            shared,
                            &profile_name,
                            previous_response_id.as_deref(),
                            request_turn_state.as_deref(),
                            request_session_id.as_deref(),
                        )?;
                        clear_runtime_response_profile_affinity(
                            &profile_name,
                            &mut bound_profile,
                            &mut session_profile,
                            &mut candidate_turn_state_retry_profile,
                            &mut candidate_turn_state_retry_value,
                            &mut pinned_profile,
                            &mut previous_response_retry_index,
                            true,
                            &mut turn_state_profile,
                            None,
                        );
                        if released_affinity {
                            runtime_proxy_log(
                                shared,
                                format!(
                                    "request={request_id} transport=http quota_blocked_affinity_released profile={profile_name} via=direct_current_profile_fallback"
                                ),
                            );
                        }
                        if runtime_quota_blocked_previous_response_fresh_fallback_allowed(
                            previous_response_id.as_deref(),
                            trusted_previous_response_affinity,
                            previous_response_fresh_fallback_used,
                        ) && let Some(fresh_request) =
                            runtime_request_without_previous_response_affinity(&request)
                        {
                            runtime_proxy_log(
                                shared,
                                format!(
                                    "request={request_id} transport=http previous_response_fresh_fallback reason=quota_blocked via=direct_current_profile_fallback"
                                ),
                            );
                            apply_runtime_responses_previous_response_fresh_fallback(
                                &mut request,
                                fresh_request,
                                runtime_previous_response_fresh_fallback_state!(),
                            );
                            recompute_route_affinity!("previous_response_fresh_fallback")?;
                            continue;
                        }
                        if !runtime_has_route_eligible_quota_fallback(
                            shared,
                            &profile_name,
                            &BTreeSet::new(),
                            RuntimeRouteKind::Responses,
                        )? {
                            return Ok(response);
                        }
                        excluded_profiles.insert(profile_name);
                        last_failure = Some((RuntimeUpstreamFailureResponse::Http(response), true));
                        continue;
                    }
                    RuntimeResponsesAttempt::PreviousResponseNotFound {
                        profile_name,
                        response,
                        turn_state,
                    } => {
                        runtime_proxy_log(
                            shared,
                            format!(
                                "request={request_id} transport=http route=responses previous_response_not_found profile={profile_name} retry_index={previous_response_retry_index} replay_turn_state={:?} via=direct_current_profile_fallback",
                                turn_state
                            ),
                        );
                        saw_previous_response_not_found = true;
                        if previous_response_retry_candidate.as_deref()
                            != Some(profile_name.as_str())
                        {
                            previous_response_retry_candidate = Some(profile_name.clone());
                            previous_response_retry_index = 0;
                        }
                        let has_turn_state_retry = turn_state.is_some();
                        if has_turn_state_retry {
                            candidate_turn_state_retry_profile = Some(profile_name.clone());
                            candidate_turn_state_retry_value = turn_state;
                        }
                        if has_turn_state_retry
                            && let Some(delay) =
                                runtime_previous_response_retry_delay(previous_response_retry_index)
                        {
                            previous_response_retry_index += 1;
                            last_failure =
                                Some((RuntimeUpstreamFailureResponse::Http(response), false));
                            runtime_proxy_log(
                                shared,
                                format!(
                                    "request={request_id} transport=http previous_response_retry_immediate profile={profile_name} delay_ms={} reason=non_blocking_retry via=direct_current_profile_fallback",
                                    delay.as_millis()
                                ),
                            );
                            continue;
                        }
                        previous_response_retry_candidate = None;
                        previous_response_retry_index = 0;
                        if !has_turn_state_retry && !request_requires_previous_response_affinity {
                            let _ = clear_runtime_stale_previous_response_binding(
                                shared,
                                &profile_name,
                                previous_response_id.as_deref(),
                            )?;
                        }
                        let released_affinity = release_runtime_previous_response_affinity(
                            shared,
                            &profile_name,
                            previous_response_id.as_deref(),
                            request_turn_state.as_deref(),
                            request_session_id.as_deref(),
                            RuntimeRouteKind::Responses,
                        )?;
                        if released_affinity {
                            runtime_proxy_log(
                                shared,
                                format!(
                                    "request={request_id} transport=http previous_response_affinity_released profile={profile_name} via=direct_current_profile_fallback"
                                ),
                            );
                        }
                        clear_runtime_response_profile_affinity(
                            &profile_name,
                            &mut bound_profile,
                            &mut session_profile,
                            &mut candidate_turn_state_retry_profile,
                            &mut candidate_turn_state_retry_value,
                            &mut pinned_profile,
                            &mut previous_response_retry_index,
                            false,
                            &mut turn_state_profile,
                            Some(&mut compact_followup_profile),
                        );
                        excluded_profiles.insert(profile_name);
                        last_failure =
                            Some((RuntimeUpstreamFailureResponse::Http(response), false));
                        continue;
                    }
                    RuntimeResponsesAttempt::LocalSelectionBlocked {
                        profile_name,
                        reason,
                    } => {
                        mark_runtime_profile_retry_backoff(shared, &profile_name)?;
                        if !runtime_quota_blocked_affinity_is_releasable(
                            runtime_candidate_affinity!(
                                RuntimeRouteKind::Responses,
                                &profile_name,
                                compact_followup_profile
                                    .as_ref()
                                    .map(|(profile_name, _)| profile_name.as_str()),
                            ),
                            request_requires_previous_response_affinity,
                        ) {
                            return Ok(RuntimeResponsesReply::Buffered(
                                build_runtime_proxy_json_error_parts(
                                    503,
                                    "service_unavailable",
                                    runtime_proxy_local_selection_failure_message(),
                                ),
                            ));
                        }
                        let released_affinity = release_runtime_quota_blocked_affinity(
                            shared,
                            &profile_name,
                            previous_response_id.as_deref(),
                            request_turn_state.as_deref(),
                            request_session_id.as_deref(),
                        )?;
                        clear_runtime_response_profile_affinity(
                            &profile_name,
                            &mut bound_profile,
                            &mut session_profile,
                            &mut candidate_turn_state_retry_profile,
                            &mut candidate_turn_state_retry_value,
                            &mut pinned_profile,
                            &mut previous_response_retry_index,
                            true,
                            &mut turn_state_profile,
                            None,
                        );
                        if released_affinity {
                            runtime_proxy_log(
                                shared,
                                format!(
                                    "request={request_id} transport=http quota_blocked_affinity_released profile={profile_name} reason={reason} via=direct_current_profile_fallback"
                                ),
                            );
                        }
                        if runtime_quota_blocked_previous_response_fresh_fallback_allowed(
                            previous_response_id.as_deref(),
                            trusted_previous_response_affinity,
                            previous_response_fresh_fallback_used,
                        ) && let Some(fresh_request) =
                            runtime_request_without_previous_response_affinity(&request)
                        {
                            runtime_proxy_log(
                                shared,
                                format!(
                                    "request={request_id} transport=http previous_response_fresh_fallback reason={reason} via=direct_current_profile_fallback"
                                ),
                            );
                            apply_runtime_responses_previous_response_fresh_fallback(
                                &mut request,
                                fresh_request,
                                runtime_previous_response_fresh_fallback_state!(),
                            );
                            recompute_route_affinity!("previous_response_fresh_fallback")?;
                            continue;
                        }
                        excluded_profiles.insert(profile_name);
                        continue;
                    }
                }
            }
            return Ok(runtime_proxy_final_responses_failure_reply(
                last_failure,
                saw_inflight_saturation,
            ));
        }

        let Some(candidate_name) = select_runtime_response_candidate_for_route_with_selection(
            shared,
            RuntimeResponseCandidateSelection {
                excluded_profiles: &excluded_profiles,
                strict_affinity_profile: compact_followup_profile
                    .as_ref()
                    .map(|(profile_name, _)| profile_name.as_str()),
                pinned_profile: pinned_profile.as_deref(),
                turn_state_profile: turn_state_profile.as_deref(),
                session_profile: session_profile.as_deref(),
                discover_previous_response_owner: previous_response_id.is_some(),
                previous_response_id: previous_response_id.as_deref(),
                route_kind: RuntimeRouteKind::Responses,
            },
        )?
        else {
            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} transport=http candidate_exhausted last_failure={}",
                    match &last_failure {
                        Some((RuntimeUpstreamFailureResponse::Http(_), _)) => "http",
                        Some((RuntimeUpstreamFailureResponse::Websocket(_), _)) => "websocket",
                        None => "none",
                    }
                ),
            );
            if runtime_proxy_maybe_wait_for_interactive_inflight_relief(
                RuntimeInflightReliefWait {
                    request_id,
                    request: &request,
                    shared,
                    excluded_profiles: &excluded_profiles,
                    route_kind: RuntimeRouteKind::Responses,
                    selection_started_at,
                    continuation: runtime_proxy_has_continuation_priority(
                        previous_response_id.as_deref(),
                        pinned_profile.as_deref(),
                        request_turn_state.as_deref(),
                        turn_state_profile.as_deref(),
                        runtime_noncompact_session_priority_profile(
                            session_profile.as_deref(),
                            compact_session_profile.as_deref(),
                        ),
                    ),
                    wait_affinity_owner: runtime_wait_affinity_owner(
                        compact_followup_profile
                            .as_ref()
                            .map(|(profile_name, _)| profile_name.as_str()),
                        pinned_profile.as_deref(),
                        turn_state_profile.as_deref(),
                        runtime_noncompact_session_priority_profile(
                            session_profile.as_deref(),
                            compact_session_profile.as_deref(),
                        ),
                        trusted_previous_response_affinity,
                    ),
                },
            )? {
                continue;
            }
            if previous_response_id.is_some()
                && saw_previous_response_not_found
                && !previous_response_fresh_fallback_used
                && !runtime_request_requires_previous_response_affinity(&request)
                && let Some(fresh_request) =
                    runtime_request_without_previous_response_affinity(&request)
            {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=http previous_response_fresh_fallback reason=candidate_exhausted"
                    ),
                );
                apply_runtime_responses_previous_response_fresh_fallback(
                    &mut request,
                    fresh_request,
                    runtime_previous_response_fresh_fallback_state!(),
                );
                recompute_route_affinity!("previous_response_fresh_fallback")?;
                continue;
            }
            if let Some((profile_name, source)) = compact_followup_profile.as_ref() {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=http compact_fresh_fallback_blocked profile={profile_name} source={source} reason=candidate_exhausted"
                    ),
                );
                return Ok(runtime_proxy_final_responses_failure_reply(
                    last_failure,
                    saw_inflight_saturation,
                ));
            }
            let remaining_cold_start_profiles =
                runtime_remaining_sync_probe_cold_start_profiles_for_route(
                    shared,
                    &excluded_profiles,
                    RuntimeRouteKind::Responses,
                )?;
            if remaining_cold_start_profiles > 0 {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=http candidate_exhausted_continue route=responses remaining_cold_start_profiles={remaining_cold_start_profiles}"
                    ),
                );
                runtime_proxy_sync_probe_pressure_pause(shared, RuntimeRouteKind::Responses);
                continue;
            }
            if runtime_proxy_allows_direct_current_profile_fallback(
                previous_response_id.as_deref(),
                pinned_profile.as_deref(),
                request_turn_state.as_deref(),
                turn_state_profile.as_deref(),
                runtime_noncompact_session_priority_profile(
                    session_profile.as_deref(),
                    compact_session_profile.as_deref(),
                ),
                saw_inflight_saturation,
                last_failure.is_some(),
            ) && let Some(current_profile) = runtime_proxy_direct_current_fallback_profile(
                shared,
                &excluded_profiles,
                RuntimeRouteKind::Responses,
            )? {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=http direct_current_profile_fallback profile={current_profile} reason=candidate_exhausted"
                    ),
                );
                match attempt_runtime_responses_request(
                    request_id,
                    &request,
                    shared,
                    &current_profile,
                    request_turn_state.as_deref(),
                )? {
                    RuntimeResponsesAttempt::Success {
                        profile_name,
                        response,
                    } => {
                        if saw_previous_response_not_found {
                            remember_runtime_successful_previous_response_owner(
                                shared,
                                &profile_name,
                                previous_response_id.as_deref(),
                                RuntimeRouteKind::Responses,
                            )?;
                        }
                        commit_runtime_proxy_profile_selection_with_notice(
                            shared,
                            &profile_name,
                            RuntimeRouteKind::Responses,
                        )?;
                        let _ = release_runtime_compact_lineage(
                            shared,
                            &profile_name,
                            request_session_id.as_deref(),
                            request_turn_state.as_deref(),
                            "response_committed_post_commit",
                        );
                        runtime_proxy_log(
                            shared,
                            format!(
                                "request={request_id} transport=http committed profile={profile_name} via=direct_current_profile_fallback"
                            ),
                        );
                        return Ok(response);
                    }
                    RuntimeResponsesAttempt::QuotaBlocked {
                        profile_name,
                        response,
                    } => {
                        mark_runtime_profile_retry_backoff(shared, &profile_name)?;
                        if !runtime_quota_blocked_affinity_is_releasable(
                            runtime_candidate_affinity!(
                                RuntimeRouteKind::Responses,
                                &profile_name,
                                compact_followup_profile
                                    .as_ref()
                                    .map(|(profile_name, _)| profile_name.as_str()),
                            ),
                            request_requires_previous_response_affinity,
                        ) {
                            return Ok(response);
                        }
                        let released_affinity = release_runtime_quota_blocked_affinity(
                            shared,
                            &profile_name,
                            previous_response_id.as_deref(),
                            request_turn_state.as_deref(),
                            request_session_id.as_deref(),
                        )?;
                        clear_runtime_response_profile_affinity(
                            &profile_name,
                            &mut bound_profile,
                            &mut session_profile,
                            &mut candidate_turn_state_retry_profile,
                            &mut candidate_turn_state_retry_value,
                            &mut pinned_profile,
                            &mut previous_response_retry_index,
                            true,
                            &mut turn_state_profile,
                            None,
                        );
                        if released_affinity {
                            runtime_proxy_log(
                                shared,
                                format!(
                                    "request={request_id} transport=http quota_blocked_affinity_released profile={profile_name} via=direct_current_profile_fallback"
                                ),
                            );
                        }
                        if runtime_quota_blocked_previous_response_fresh_fallback_allowed(
                            previous_response_id.as_deref(),
                            trusted_previous_response_affinity,
                            previous_response_fresh_fallback_used,
                        ) && let Some(fresh_request) =
                            runtime_request_without_previous_response_affinity(&request)
                        {
                            runtime_proxy_log(
                                shared,
                                format!(
                                    "request={request_id} transport=http previous_response_fresh_fallback reason=quota_blocked via=direct_current_profile_fallback"
                                ),
                            );
                            apply_runtime_responses_previous_response_fresh_fallback(
                                &mut request,
                                fresh_request,
                                runtime_previous_response_fresh_fallback_state!(),
                            );
                            recompute_route_affinity!("previous_response_fresh_fallback")?;
                            continue;
                        }
                        if !runtime_has_route_eligible_quota_fallback(
                            shared,
                            &profile_name,
                            &BTreeSet::new(),
                            RuntimeRouteKind::Responses,
                        )? {
                            return Ok(response);
                        }
                        excluded_profiles.insert(profile_name);
                        last_failure = Some((RuntimeUpstreamFailureResponse::Http(response), true));
                        continue;
                    }
                    RuntimeResponsesAttempt::PreviousResponseNotFound {
                        profile_name,
                        response,
                        turn_state,
                    } => {
                        runtime_proxy_log(
                            shared,
                            format!(
                                "request={request_id} transport=http route=responses previous_response_not_found profile={profile_name} retry_index={previous_response_retry_index} replay_turn_state={:?} via=direct_current_profile_fallback",
                                turn_state
                            ),
                        );
                        saw_previous_response_not_found = true;
                        if previous_response_retry_candidate.as_deref()
                            != Some(profile_name.as_str())
                        {
                            previous_response_retry_candidate = Some(profile_name.clone());
                            previous_response_retry_index = 0;
                        }
                        let has_turn_state_retry = turn_state.is_some();
                        if has_turn_state_retry {
                            candidate_turn_state_retry_profile = Some(profile_name.clone());
                            candidate_turn_state_retry_value = turn_state;
                        }
                        if has_turn_state_retry
                            && let Some(delay) =
                                runtime_previous_response_retry_delay(previous_response_retry_index)
                        {
                            previous_response_retry_index += 1;
                            last_failure =
                                Some((RuntimeUpstreamFailureResponse::Http(response), false));
                            runtime_proxy_log(
                                shared,
                                format!(
                                    "request={request_id} transport=http previous_response_retry_immediate profile={profile_name} delay_ms={} reason=non_blocking_retry via=direct_current_profile_fallback",
                                    delay.as_millis()
                                ),
                            );
                            continue;
                        }
                        previous_response_retry_candidate = None;
                        previous_response_retry_index = 0;
                        if !has_turn_state_retry && !request_requires_previous_response_affinity {
                            let _ = clear_runtime_stale_previous_response_binding(
                                shared,
                                &profile_name,
                                previous_response_id.as_deref(),
                            )?;
                        }
                        let released_affinity = release_runtime_previous_response_affinity(
                            shared,
                            &profile_name,
                            previous_response_id.as_deref(),
                            request_turn_state.as_deref(),
                            request_session_id.as_deref(),
                            RuntimeRouteKind::Responses,
                        )?;
                        if released_affinity {
                            runtime_proxy_log(
                                shared,
                                format!(
                                    "request={request_id} transport=http previous_response_affinity_released profile={profile_name} via=direct_current_profile_fallback"
                                ),
                            );
                        }
                        clear_runtime_response_profile_affinity(
                            &profile_name,
                            &mut bound_profile,
                            &mut session_profile,
                            &mut candidate_turn_state_retry_profile,
                            &mut candidate_turn_state_retry_value,
                            &mut pinned_profile,
                            &mut previous_response_retry_index,
                            false,
                            &mut turn_state_profile,
                            Some(&mut compact_followup_profile),
                        );
                        excluded_profiles.insert(profile_name);
                        last_failure =
                            Some((RuntimeUpstreamFailureResponse::Http(response), false));
                        continue;
                    }
                    RuntimeResponsesAttempt::LocalSelectionBlocked {
                        profile_name,
                        reason,
                    } => {
                        mark_runtime_profile_retry_backoff(shared, &profile_name)?;
                        if !runtime_quota_blocked_affinity_is_releasable(
                            runtime_candidate_affinity!(
                                RuntimeRouteKind::Responses,
                                &profile_name,
                                compact_followup_profile
                                    .as_ref()
                                    .map(|(profile_name, _)| profile_name.as_str()),
                            ),
                            request_requires_previous_response_affinity,
                        ) {
                            return Ok(RuntimeResponsesReply::Buffered(
                                build_runtime_proxy_json_error_parts(
                                    503,
                                    "service_unavailable",
                                    runtime_proxy_local_selection_failure_message(),
                                ),
                            ));
                        }
                        let released_affinity = release_runtime_quota_blocked_affinity(
                            shared,
                            &profile_name,
                            previous_response_id.as_deref(),
                            request_turn_state.as_deref(),
                            request_session_id.as_deref(),
                        )?;
                        clear_runtime_response_profile_affinity(
                            &profile_name,
                            &mut bound_profile,
                            &mut session_profile,
                            &mut candidate_turn_state_retry_profile,
                            &mut candidate_turn_state_retry_value,
                            &mut pinned_profile,
                            &mut previous_response_retry_index,
                            true,
                            &mut turn_state_profile,
                            None,
                        );
                        if released_affinity {
                            runtime_proxy_log(
                                shared,
                                format!(
                                    "request={request_id} transport=http quota_blocked_affinity_released profile={profile_name} reason={reason} via=direct_current_profile_fallback"
                                ),
                            );
                        }
                        if runtime_quota_blocked_previous_response_fresh_fallback_allowed(
                            previous_response_id.as_deref(),
                            trusted_previous_response_affinity,
                            previous_response_fresh_fallback_used,
                        ) && let Some(fresh_request) =
                            runtime_request_without_previous_response_affinity(&request)
                        {
                            runtime_proxy_log(
                                shared,
                                format!(
                                    "request={request_id} transport=http previous_response_fresh_fallback reason={reason} via=direct_current_profile_fallback"
                                ),
                            );
                            apply_runtime_responses_previous_response_fresh_fallback(
                                &mut request,
                                fresh_request,
                                runtime_previous_response_fresh_fallback_state!(),
                            );
                            recompute_route_affinity!("previous_response_fresh_fallback")?;
                            continue;
                        }
                        excluded_profiles.insert(profile_name);
                        continue;
                    }
                }
            }
            return Ok(runtime_proxy_final_responses_failure_reply(
                last_failure,
                saw_inflight_saturation,
            ));
        };
        selection_attempts = selection_attempts.saturating_add(1);
        let turn_state_override =
            if candidate_turn_state_retry_profile.as_deref() == Some(candidate_name.as_str()) {
                candidate_turn_state_retry_value.as_deref()
            } else {
                request_turn_state.as_deref()
            };
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=http candidate={} pinned={:?} turn_state_profile={:?} turn_state_override={:?} excluded_count={}",
                candidate_name,
                pinned_profile,
                turn_state_profile,
                turn_state_override,
                excluded_profiles.len()
            ),
        );
        if previous_response_id.is_none()
            && pinned_profile.is_none()
            && turn_state_profile.is_none()
            && runtime_profile_inflight_hard_limited_for_context(
                shared,
                &candidate_name,
                "responses_http",
            )?
        {
            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} transport=http profile_inflight_saturated profile={candidate_name} hard_limit={}",
                    runtime_proxy_profile_inflight_hard_limit(),
                ),
            );
            saw_inflight_saturation = true;
            if runtime_proxy_maybe_wait_for_interactive_inflight_relief(
                RuntimeInflightReliefWait {
                    request_id,
                    request: &request,
                    shared,
                    excluded_profiles: &excluded_profiles,
                    route_kind: RuntimeRouteKind::Responses,
                    selection_started_at,
                    continuation: runtime_proxy_has_continuation_priority(
                        previous_response_id.as_deref(),
                        pinned_profile.as_deref(),
                        request_turn_state.as_deref(),
                        turn_state_profile.as_deref(),
                        runtime_noncompact_session_priority_profile(
                            session_profile.as_deref(),
                            compact_session_profile.as_deref(),
                        ),
                    ),
                    wait_affinity_owner: runtime_wait_affinity_owner(
                        compact_followup_profile
                            .as_ref()
                            .map(|(profile_name, _)| profile_name.as_str()),
                        pinned_profile.as_deref(),
                        turn_state_profile.as_deref(),
                        runtime_noncompact_session_priority_profile(
                            session_profile.as_deref(),
                            compact_session_profile.as_deref(),
                        ),
                        trusted_previous_response_affinity,
                    ),
                },
            )? {
                continue;
            }
            excluded_profiles.insert(candidate_name);
            continue;
        }

        match attempt_runtime_responses_request(
            request_id,
            &request,
            shared,
            &candidate_name,
            turn_state_override,
        )? {
            RuntimeResponsesAttempt::Success {
                profile_name,
                response,
            } => {
                if saw_previous_response_not_found {
                    remember_runtime_successful_previous_response_owner(
                        shared,
                        &profile_name,
                        previous_response_id.as_deref(),
                        RuntimeRouteKind::Responses,
                    )?;
                }
                commit_runtime_proxy_profile_selection_with_notice(
                    shared,
                    &profile_name,
                    RuntimeRouteKind::Responses,
                )?;
                let _ = release_runtime_compact_lineage(
                    shared,
                    &profile_name,
                    request_session_id.as_deref(),
                    request_turn_state.as_deref(),
                    "response_committed_post_commit",
                );
                runtime_proxy_log(
                    shared,
                    format!("request={request_id} transport=http committed profile={profile_name}"),
                );
                return Ok(response);
            }
            RuntimeResponsesAttempt::QuotaBlocked {
                profile_name,
                response,
            } => {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=http quota_blocked profile={profile_name}"
                    ),
                );
                let quota_message =
                    extract_runtime_proxy_quota_message_from_response_reply(&response);
                mark_runtime_profile_quota_quarantine(
                    shared,
                    &profile_name,
                    RuntimeRouteKind::Responses,
                    quota_message.as_deref(),
                )?;
                if !runtime_quota_blocked_affinity_is_releasable(
                    runtime_candidate_affinity!(
                        RuntimeRouteKind::Responses,
                        &profile_name,
                        compact_followup_profile
                            .as_ref()
                            .map(|(profile_name, _)| profile_name.as_str()),
                    ),
                    request_requires_previous_response_affinity,
                ) {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} transport=http upstream_usage_limit_passthrough route=responses profile={profile_name} reason=hard_affinity"
                        ),
                    );
                    return Ok(response);
                }
                let released_affinity = release_runtime_quota_blocked_affinity(
                    shared,
                    &profile_name,
                    previous_response_id.as_deref(),
                    request_turn_state.as_deref(),
                    request_session_id.as_deref(),
                )?;
                clear_runtime_response_profile_affinity(
                    &profile_name,
                    &mut bound_profile,
                    &mut session_profile,
                    &mut candidate_turn_state_retry_profile,
                    &mut candidate_turn_state_retry_value,
                    &mut pinned_profile,
                    &mut previous_response_retry_index,
                    true,
                    &mut turn_state_profile,
                    None,
                );
                if released_affinity {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} transport=http quota_blocked_affinity_released profile={profile_name}"
                        ),
                    );
                }
                if runtime_quota_blocked_previous_response_fresh_fallback_allowed(
                    previous_response_id.as_deref(),
                    trusted_previous_response_affinity,
                    previous_response_fresh_fallback_used,
                ) && let Some(fresh_request) =
                    runtime_request_without_previous_response_affinity(&request)
                {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} transport=http previous_response_fresh_fallback reason=quota_blocked"
                        ),
                    );
                    apply_runtime_responses_previous_response_fresh_fallback(
                        &mut request,
                        fresh_request,
                        runtime_previous_response_fresh_fallback_state!(),
                    );
                    recompute_route_affinity!("previous_response_fresh_fallback")?;
                    continue;
                }
                if !runtime_has_route_eligible_quota_fallback(
                    shared,
                    &profile_name,
                    &BTreeSet::new(),
                    RuntimeRouteKind::Responses,
                )? {
                    return Ok(response);
                }
                excluded_profiles.insert(profile_name);
                last_failure = Some((RuntimeUpstreamFailureResponse::Http(response), true));
            }
            RuntimeResponsesAttempt::LocalSelectionBlocked {
                profile_name,
                reason,
            } => {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=http local_selection_blocked profile={profile_name} route=responses reason={reason}"
                    ),
                );
                mark_runtime_profile_retry_backoff(shared, &profile_name)?;
                if !runtime_quota_blocked_affinity_is_releasable(
                    runtime_candidate_affinity!(
                        RuntimeRouteKind::Responses,
                        &profile_name,
                        compact_followup_profile
                            .as_ref()
                            .map(|(profile_name, _)| profile_name.as_str()),
                    ),
                    request_requires_previous_response_affinity,
                ) {
                    return Ok(RuntimeResponsesReply::Buffered(
                        build_runtime_proxy_json_error_parts(
                            503,
                            "service_unavailable",
                            runtime_proxy_local_selection_failure_message(),
                        ),
                    ));
                }
                let released_affinity = release_runtime_quota_blocked_affinity(
                    shared,
                    &profile_name,
                    previous_response_id.as_deref(),
                    request_turn_state.as_deref(),
                    request_session_id.as_deref(),
                )?;
                clear_runtime_response_profile_affinity(
                    &profile_name,
                    &mut bound_profile,
                    &mut session_profile,
                    &mut candidate_turn_state_retry_profile,
                    &mut candidate_turn_state_retry_value,
                    &mut pinned_profile,
                    &mut previous_response_retry_index,
                    true,
                    &mut turn_state_profile,
                    None,
                );
                if released_affinity {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} transport=http quota_blocked_affinity_released profile={profile_name} reason={reason}"
                        ),
                    );
                }
                if runtime_quota_blocked_previous_response_fresh_fallback_allowed(
                    previous_response_id.as_deref(),
                    trusted_previous_response_affinity,
                    previous_response_fresh_fallback_used,
                ) && let Some(fresh_request) =
                    runtime_request_without_previous_response_affinity(&request)
                {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} transport=http previous_response_fresh_fallback reason={reason}"
                        ),
                    );
                    apply_runtime_responses_previous_response_fresh_fallback(
                        &mut request,
                        fresh_request,
                        runtime_previous_response_fresh_fallback_state!(),
                    );
                    recompute_route_affinity!("previous_response_fresh_fallback")?;
                    continue;
                }
                excluded_profiles.insert(profile_name);
            }
            RuntimeResponsesAttempt::PreviousResponseNotFound {
                profile_name,
                response,
                turn_state,
            } => {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=http route=responses previous_response_not_found profile={profile_name} retry_index={previous_response_retry_index} replay_turn_state={:?}",
                        turn_state
                    ),
                );
                saw_previous_response_not_found = true;
                if previous_response_retry_candidate.as_deref() != Some(profile_name.as_str()) {
                    previous_response_retry_candidate = Some(profile_name.clone());
                    previous_response_retry_index = 0;
                }
                let has_turn_state_retry = turn_state.is_some();
                if has_turn_state_retry {
                    candidate_turn_state_retry_profile = Some(profile_name.clone());
                    candidate_turn_state_retry_value = turn_state;
                }
                if has_turn_state_retry
                    && let Some(delay) =
                        runtime_previous_response_retry_delay(previous_response_retry_index)
                {
                    previous_response_retry_index += 1;
                    last_failure = Some((RuntimeUpstreamFailureResponse::Http(response), false));
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} transport=http previous_response_retry_immediate profile={profile_name} delay_ms={} reason=non_blocking_retry",
                            delay.as_millis()
                        ),
                    );
                    continue;
                }
                previous_response_retry_candidate = None;
                previous_response_retry_index = 0;
                if !has_turn_state_retry && !request_requires_previous_response_affinity {
                    let _ = clear_runtime_stale_previous_response_binding(
                        shared,
                        &profile_name,
                        previous_response_id.as_deref(),
                    )?;
                }
                let released_affinity = release_runtime_previous_response_affinity(
                    shared,
                    &profile_name,
                    previous_response_id.as_deref(),
                    request_turn_state.as_deref(),
                    request_session_id.as_deref(),
                    RuntimeRouteKind::Responses,
                )?;
                if released_affinity {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} transport=http previous_response_affinity_released profile={profile_name}"
                        ),
                    );
                }
                clear_runtime_response_profile_affinity(
                    &profile_name,
                    &mut bound_profile,
                    &mut session_profile,
                    &mut candidate_turn_state_retry_profile,
                    &mut candidate_turn_state_retry_value,
                    &mut pinned_profile,
                    &mut previous_response_retry_index,
                    false,
                    &mut turn_state_profile,
                    Some(&mut compact_followup_profile),
                );
                trusted_previous_response_affinity = false;
                excluded_profiles.insert(profile_name);
                last_failure = Some((RuntimeUpstreamFailureResponse::Http(response), false));
            }
        }
    }
}

pub(super) fn attempt_runtime_responses_request(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    turn_state_override: Option<&str>,
) -> Result<RuntimeResponsesAttempt> {
    let request_session_id = runtime_request_session_id(request);
    let request_previous_response_id = runtime_request_previous_response_id(request);
    let request_turn_state = runtime_request_turn_state(request);
    let (initial_quota_summary, initial_quota_source) =
        runtime_profile_quota_summary_for_route(shared, profile_name, RuntimeRouteKind::Responses)?;
    if (request_previous_response_id.is_some()
        || request_session_id.is_some()
        || request_turn_state.is_some())
        && matches!(
            initial_quota_source,
            Some(RuntimeQuotaSource::PersistedSnapshot)
        )
        && let Some(reason) =
            runtime_quota_precommit_guard_reason(initial_quota_summary, RuntimeRouteKind::Responses)
    {
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=http responses_pre_send_skip profile={profile_name} route=responses reason={reason} quota_source={} {}",
                initial_quota_source
                    .map(runtime_quota_source_label)
                    .unwrap_or("unknown"),
                runtime_quota_summary_log_fields(initial_quota_summary),
            ),
        );
        return Ok(RuntimeResponsesAttempt::LocalSelectionBlocked {
            profile_name: profile_name.to_string(),
            reason,
        });
    }
    let has_alternative_quota_profile = runtime_has_route_eligible_quota_fallback(
        shared,
        profile_name,
        &BTreeSet::new(),
        RuntimeRouteKind::Responses,
    )?;
    let (quota_summary, quota_source) = ensure_runtime_profile_precommit_quota_ready(
        shared,
        profile_name,
        RuntimeRouteKind::Responses,
        "responses_precommit_reprobe",
    )?;
    if runtime_quota_summary_requires_live_source_after_probe(
        quota_summary,
        quota_source,
        RuntimeRouteKind::Responses,
    ) && has_alternative_quota_profile
    {
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=http responses_pre_send_skip profile={profile_name} route=responses reason=quota_windows_unavailable_after_reprobe quota_source={} {}",
                quota_source
                    .map(runtime_quota_source_label)
                    .unwrap_or("unknown"),
                runtime_quota_summary_log_fields(quota_summary),
            ),
        );
        return Ok(RuntimeResponsesAttempt::LocalSelectionBlocked {
            profile_name: profile_name.to_string(),
            reason: "quota_windows_unavailable_after_reprobe",
        });
    }
    if let Some(reason) =
        runtime_quota_precommit_guard_reason(quota_summary, RuntimeRouteKind::Responses)
    {
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=http responses_pre_send_skip profile={profile_name} route=responses reason={reason} quota_source={} {}",
                quota_source
                    .map(runtime_quota_source_label)
                    .unwrap_or("unknown"),
                runtime_quota_summary_log_fields(quota_summary),
            ),
        );
        return Ok(RuntimeResponsesAttempt::LocalSelectionBlocked {
            profile_name: profile_name.to_string(),
            reason,
        });
    }
    let inflight_guard =
        acquire_runtime_profile_inflight_guard(shared, profile_name, "responses_http")?;
    let mut inflight_guard = Some(inflight_guard);
    let mut recovery_steps = [
        RuntimeProfileUnauthorizedRecoveryStep::Reload,
        RuntimeProfileUnauthorizedRecoveryStep::Refresh,
    ]
    .into_iter();
    loop {
        let response = send_runtime_proxy_upstream_responses_request(
            request_id,
            request,
            shared,
            profile_name,
            turn_state_override,
        )
        .inspect_err(|err| {
            note_runtime_profile_transport_failure(
                shared,
                profile_name,
                RuntimeRouteKind::Responses,
                "responses_upstream_request",
                err,
            );
        })?;
        let response_turn_state =
            runtime_proxy_header_value(response.headers(), "x-codex-turn-state");
        if !response.status().is_success() {
            let status = response.status().as_u16();
            let parts = buffer_runtime_proxy_async_response_parts(shared, response, Vec::new())
                .inspect_err(|err| {
                    note_runtime_profile_transport_failure(
                        shared,
                        profile_name,
                        RuntimeRouteKind::Responses,
                        "responses_buffer_response",
                        err,
                    );
                })?;
            if status == 401
                && runtime_try_recover_profile_auth_from_unauthorized_steps(
                    request_id,
                    shared,
                    profile_name,
                    RuntimeRouteKind::Responses,
                    &mut recovery_steps,
                )
            {
                continue;
            }
            let retryable_quota = matches!(status, 403 | 429)
                && extract_runtime_proxy_quota_message(&parts.body).is_some();
            let retryable_previous = status == 400
                && extract_runtime_proxy_previous_response_message(&parts.body).is_some();
            let response = RuntimeResponsesReply::Buffered(parts);

            if retryable_quota {
                return Ok(RuntimeResponsesAttempt::QuotaBlocked {
                    profile_name: profile_name.to_string(),
                    response,
                });
            }
            if retryable_previous {
                return Ok(RuntimeResponsesAttempt::PreviousResponseNotFound {
                    profile_name: profile_name.to_string(),
                    response,
                    turn_state: response_turn_state,
                });
            }
            if matches!(status, 401 | 403) {
                note_runtime_profile_auth_failure(
                    shared,
                    profile_name,
                    RuntimeRouteKind::Responses,
                    status,
                );
            }

            return Ok(RuntimeResponsesAttempt::Success {
                profile_name: profile_name.to_string(),
                response,
            });
        }
        return prepare_runtime_proxy_responses_success(
            RuntimeResponsesSuccessContext {
                request_id,
                request_previous_response_id: runtime_request_previous_response_id(request)
                    .as_deref(),
                request_session_id: request_session_id.as_deref(),
                request_turn_state: runtime_request_turn_state(request).as_deref(),
                turn_state_override,
                shared,
                profile_name,
                inflight_guard: inflight_guard
                    .take()
                    .expect("responses inflight guard should be present"),
            },
            response,
        )
        .inspect_err(|err| {
            note_runtime_profile_transport_failure(
                shared,
                profile_name,
                RuntimeRouteKind::Responses,
                "responses_prepare_success",
                err,
            );
        });
    }
}

pub(super) fn next_runtime_response_candidate_for_route(
    shared: &RuntimeRotationProxyShared,
    excluded_profiles: &BTreeSet<String>,
    route_kind: RuntimeRouteKind,
) -> Result<Option<String>> {
    let now = Local::now().timestamp();
    let pressure_mode = runtime_proxy_pressure_mode_active_for_route(shared, route_kind);
    let sync_probe_pressure_mode =
        runtime_proxy_sync_probe_pressure_mode_active_for_route(shared, route_kind);
    let allow_disk_auth_fallback = !sync_probe_pressure_mode;
    let inflight_soft_limit = runtime_profile_inflight_soft_limit(route_kind, pressure_mode);
    let (
        state,
        current_profile,
        include_code_review,
        upstream_base_url,
        cached_reports,
        mut cached_usage_snapshots,
        profile_usage_auth,
        mut retry_backoff_until,
        mut transport_backoff_until,
        mut route_circuit_open_until,
        profile_inflight,
        profile_health,
    ) = {
        let mut runtime = shared
            .runtime
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
        prune_runtime_profile_selection_backoff(&mut runtime, now);
        (
            runtime.state.clone(),
            runtime.current_profile.clone(),
            runtime.include_code_review,
            runtime.upstream_base_url.clone(),
            runtime.profile_probe_cache.clone(),
            runtime.profile_usage_snapshots.clone(),
            runtime.profile_usage_auth.clone(),
            runtime.profile_retry_backoff_until.clone(),
            runtime.profile_transport_backoff_until.clone(),
            runtime.profile_route_circuit_open_until.clone(),
            runtime.profile_inflight.clone(),
            runtime.profile_health.clone(),
        )
    };

    let mut reports = Vec::new();
    let mut cold_start_probe_jobs = Vec::new();
    for (order_index, name) in active_profile_selection_order(&state, &current_profile)
        .into_iter()
        .enumerate()
    {
        if excluded_profiles.contains(&name) {
            continue;
        }
        let Some(profile) = state.profiles.get(&name) else {
            continue;
        };
        if let Some(entry) = cached_reports.get(&name) {
            reports.push(RunProfileProbeReport {
                name: name.clone(),
                order_index,
                auth: entry.auth.clone(),
                result: entry.result.clone(),
            });
            if runtime_profile_probe_cache_freshness(entry, now)
                != RuntimeProbeCacheFreshness::Fresh
            {
                if profile.provider.supports_codex_runtime() {
                    schedule_runtime_probe_refresh(shared, &name, &profile.codex_home);
                }
            }
        } else {
            let auth = runtime_profile_auth_summary_for_selection_with_policy(
                &name,
                &profile.codex_home,
                &profile_usage_auth,
                &cached_reports,
                allow_disk_auth_fallback,
            )
            .unwrap_or_else(runtime_profile_uncached_auth_summary_for_selection);
            reports.push(RunProfileProbeReport {
                name: name.clone(),
                order_index,
                auth,
                result: Err("runtime quota snapshot unavailable".to_string()),
            });
            cold_start_probe_jobs.push(RunProfileProbeJob {
                name,
                order_index,
                provider: profile.provider.clone(),
                codex_home: profile.codex_home.clone(),
            });
        }
    }

    cold_start_probe_jobs.sort_by_key(|job| {
        let quota_summary = cached_usage_snapshots
            .get(&job.name)
            .filter(|snapshot| runtime_usage_snapshot_is_usable(snapshot, now))
            .map(|snapshot| runtime_quota_summary_from_usage_snapshot_at(snapshot, route_kind, now))
            .unwrap_or(RuntimeQuotaSummary {
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
            });
        (
            runtime_quota_pressure_sort_key_for_route_from_summary(quota_summary),
            job.order_index,
        )
    });
    let request_probe_jobs = cold_start_probe_jobs
        .iter()
        .filter(|job| {
            !cached_usage_snapshots
                .get(&job.name)
                .is_some_and(|snapshot| {
                    runtime_snapshot_blocks_same_request_cold_start_probe(snapshot, route_kind, now)
                })
        })
        .cloned()
        .collect::<Vec<_>>();

    reports.sort_by_key(|report| report.order_index);
    let mut candidates = ready_profile_candidates(
        &reports,
        include_code_review,
        Some(current_profile.as_str()),
        &state,
        Some(&cached_usage_snapshots),
    );

    let best_candidate_order_index = candidates
        .iter()
        .filter(|candidate| !excluded_profiles.contains(&candidate.name))
        .filter(|candidate| {
            !runtime_profile_name_in_selection_backoff(
                &candidate.name,
                &retry_backoff_until,
                &transport_backoff_until,
                &route_circuit_open_until,
                route_kind,
                now,
            )
        })
        .filter(|candidate| {
            !runtime_profile_auth_failure_active_with_auth_cache(
                &profile_health,
                &profile_usage_auth,
                &candidate.name,
                now,
            )
        })
        .filter(|candidate| {
            runtime_profile_inflight_sort_key(&candidate.name, &profile_inflight)
                < inflight_soft_limit
        })
        .map(|candidate| candidate.order_index)
        .min();
    let should_sync_probe_cold_start = !sync_probe_pressure_mode
        && !request_probe_jobs.is_empty()
        && (candidates.is_empty()
            || best_candidate_order_index.is_none()
            || best_candidate_order_index.is_some_and(|best_order_index| {
                request_probe_jobs
                    .iter()
                    .any(|job| job.order_index < best_order_index)
            }));
    if sync_probe_pressure_mode && !request_probe_jobs.is_empty() {
        runtime_proxy_log(
            shared,
            format!(
                "selection_skip_sync_probe route={} reason=pressure_mode cold_start_jobs={}",
                runtime_route_kind_label(route_kind),
                request_probe_jobs.len(),
            ),
        );
    }

    if should_sync_probe_cold_start {
        let base_url = Some(upstream_base_url.clone());
        let sync_jobs = request_probe_jobs
            .iter()
            .filter(|job| {
                candidates.is_empty()
                    || best_candidate_order_index.is_none()
                    || best_candidate_order_index
                        .is_some_and(|best_order_index| job.order_index < best_order_index)
            })
            .take(RUNTIME_PROFILE_SYNC_PROBE_FALLBACK_LIMIT)
            .map(|job| RunProfileProbeJob {
                name: job.name.clone(),
                order_index: job.order_index,
                provider: job.provider.clone(),
                codex_home: job.codex_home.clone(),
            })
            .collect::<Vec<_>>();
        let probed_names = sync_jobs
            .iter()
            .map(|job| job.name.clone())
            .collect::<BTreeSet<_>>();
        let fresh_reports = map_parallel(sync_jobs, |job| {
            let auth = job.provider.auth_summary(&job.codex_home);
            let result = if auth.quota_compatible {
                fetch_usage(&job.codex_home, base_url.as_deref()).map_err(|err| err.to_string())
            } else {
                Err("auth mode is not quota-compatible".to_string())
            };

            RunProfileProbeReport {
                name: job.name,
                order_index: job.order_index,
                auth,
                result,
            }
        });

        for report in &fresh_reports {
            apply_runtime_profile_probe_result(
                shared,
                &report.name,
                report.auth.clone(),
                report.result.clone(),
            )?;
        }
        {
            let runtime = shared
                .runtime
                .lock()
                .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
            cached_usage_snapshots = runtime.profile_usage_snapshots.clone();
            retry_backoff_until = runtime.profile_retry_backoff_until.clone();
            transport_backoff_until = runtime.profile_transport_backoff_until.clone();
            route_circuit_open_until = runtime.profile_route_circuit_open_until.clone();
        }

        for fresh_report in fresh_reports {
            if let Some(existing) = reports
                .iter_mut()
                .find(|report| report.name == fresh_report.name)
            {
                *existing = fresh_report;
            }
        }
        reports.sort_by_key(|report| report.order_index);
        candidates = ready_profile_candidates(
            &reports,
            include_code_review,
            Some(current_profile.as_str()),
            &state,
            Some(&cached_usage_snapshots),
        );
        for job in cold_start_probe_jobs
            .into_iter()
            .filter(|job| !probed_names.contains(&job.name))
        {
            schedule_runtime_probe_refresh(shared, &job.name, &job.codex_home);
        }
    } else {
        if sync_probe_pressure_mode && !cold_start_probe_jobs.is_empty() {
            runtime_proxy_log(
                shared,
                format!(
                    "selection_skip_sync_probe route={} reason=pressure_mode cold_start_profiles={}",
                    runtime_route_kind_label(route_kind),
                    cold_start_probe_jobs.len()
                ),
            );
        }
        for job in cold_start_probe_jobs {
            schedule_runtime_probe_refresh(shared, &job.name, &job.codex_home);
        }
    }
    let available_candidates = candidates
        .into_iter()
        .enumerate()
        .filter(|(_, candidate)| !excluded_profiles.contains(&candidate.name))
        .collect::<Vec<_>>();

    let mut ready_candidates = available_candidates
        .iter()
        .filter(|(_, candidate)| {
            !runtime_profile_name_in_selection_backoff(
                &candidate.name,
                &retry_backoff_until,
                &transport_backoff_until,
                &route_circuit_open_until,
                route_kind,
                now,
            )
        })
        .collect::<Vec<_>>();
    ready_candidates.sort_by_key(|(index, candidate)| {
        (
            candidate.provider_priority,
            runtime_quota_pressure_sort_key_for_route(&candidate.usage, route_kind),
            runtime_quota_source_sort_key(route_kind, candidate.quota_source),
            runtime_profile_inflight_sort_key(&candidate.name, &profile_inflight),
            runtime_profile_health_sort_key(&candidate.name, &profile_health, now, route_kind),
            *index,
            runtime_profile_selection_jitter(shared, &candidate.name, route_kind),
        )
    });
    for (index, candidate) in ready_candidates {
        let inflight = runtime_profile_inflight_sort_key(&candidate.name, &profile_inflight);
        let quota_summary = runtime_quota_summary_for_route(&candidate.usage, route_kind);
        if runtime_profile_auth_failure_active_with_auth_cache(
            &profile_health,
            &profile_usage_auth,
            &candidate.name,
            now,
        ) {
            runtime_proxy_log(
                shared,
                format!(
                    "selection_skip_current route={} profile={} reason=auth_failure_backoff inflight={} health={} quota_source={} {}",
                    runtime_route_kind_label(route_kind),
                    candidate.name,
                    inflight,
                    runtime_profile_health_sort_key(
                        &candidate.name,
                        &profile_health,
                        now,
                        route_kind
                    ),
                    runtime_quota_source_label(candidate.quota_source),
                    runtime_quota_summary_log_fields(runtime_quota_summary_for_route(
                        &candidate.usage,
                        route_kind
                    )),
                ),
            );
            continue;
        }
        if matches!(
            route_kind,
            RuntimeRouteKind::Responses | RuntimeRouteKind::Websocket
        ) && let Some(reason) = runtime_quota_precommit_guard_reason(quota_summary, route_kind)
        {
            runtime_proxy_log(
                shared,
                format!(
                    "selection_skip_current route={} profile={} reason={} inflight={} health={} quota_source={} {}",
                    runtime_route_kind_label(route_kind),
                    candidate.name,
                    reason,
                    inflight,
                    runtime_profile_health_sort_key(
                        &candidate.name,
                        &profile_health,
                        now,
                        route_kind
                    ),
                    runtime_quota_source_label(candidate.quota_source),
                    runtime_quota_summary_log_fields(quota_summary),
                ),
            );
            continue;
        }
        if inflight >= inflight_soft_limit {
            runtime_proxy_log(
                shared,
                format!(
                    "selection_skip_current route={} profile={} reason=profile_inflight_soft_limit inflight={} soft_limit={} health={} quota_source={} {}",
                    runtime_route_kind_label(route_kind),
                    candidate.name,
                    inflight,
                    inflight_soft_limit,
                    runtime_profile_health_sort_key(
                        &candidate.name,
                        &profile_health,
                        now,
                        route_kind
                    ),
                    runtime_quota_source_label(candidate.quota_source),
                    runtime_quota_summary_log_fields(quota_summary),
                ),
            );
            continue;
        }
        if !reserve_runtime_profile_route_circuit_half_open_probe(
            shared,
            &candidate.name,
            route_kind,
        )? {
            runtime_proxy_log(
                shared,
                format!(
                    "selection_skip_current route={} profile={} reason=route_circuit_half_open_probe_wait inflight={} health={} quota_source={} {}",
                    runtime_route_kind_label(route_kind),
                    candidate.name,
                    inflight,
                    runtime_profile_health_sort_key(
                        &candidate.name,
                        &profile_health,
                        now,
                        route_kind
                    ),
                    runtime_quota_source_label(candidate.quota_source),
                    runtime_quota_summary_log_fields(quota_summary),
                ),
            );
            continue;
        }
        runtime_proxy_log(
            shared,
            format!(
                "selection_pick route={} profile={} mode=ready inflight={} health={} order={} {}",
                runtime_route_kind_label(route_kind),
                candidate.name,
                inflight,
                runtime_profile_health_sort_key(&candidate.name, &profile_health, now, route_kind),
                index,
                format_args!(
                    "quota_source={} {}",
                    runtime_quota_source_label(candidate.quota_source),
                    runtime_quota_summary_log_fields(quota_summary)
                ),
            ),
        );
        return Ok(Some(candidate.name.clone()));
    }

    let mut fallback_candidates = available_candidates.into_iter().collect::<Vec<_>>();
    fallback_candidates.sort_by_key(|(index, candidate)| {
        (
            runtime_profile_backoff_sort_key(
                &candidate.name,
                &retry_backoff_until,
                &transport_backoff_until,
                &route_circuit_open_until,
                route_kind,
                now,
            ),
            candidate.provider_priority,
            runtime_quota_pressure_sort_key_for_route(&candidate.usage, route_kind),
            runtime_quota_source_sort_key(route_kind, candidate.quota_source),
            runtime_profile_inflight_sort_key(&candidate.name, &profile_inflight),
            runtime_profile_health_sort_key(&candidate.name, &profile_health, now, route_kind),
            *index,
            runtime_profile_selection_jitter(shared, &candidate.name, route_kind),
        )
    });
    let mut fallback = None;
    for (index, candidate) in fallback_candidates {
        if runtime_profile_auth_failure_active_with_auth_cache(
            &profile_health,
            &profile_usage_auth,
            &candidate.name,
            now,
        ) {
            runtime_proxy_log(
                shared,
                format!(
                    "selection_skip_current route={} profile={} reason=auth_failure_backoff inflight={} health={} quota_source={} {}",
                    runtime_route_kind_label(route_kind),
                    candidate.name,
                    runtime_profile_inflight_sort_key(&candidate.name, &profile_inflight),
                    runtime_profile_health_sort_key(
                        &candidate.name,
                        &profile_health,
                        now,
                        route_kind
                    ),
                    runtime_quota_source_label(candidate.quota_source),
                    runtime_quota_summary_log_fields(runtime_quota_summary_for_route(
                        &candidate.usage,
                        route_kind
                    )),
                ),
            );
            continue;
        }
        let quota_summary = runtime_quota_summary_for_route(&candidate.usage, route_kind);
        if matches!(
            route_kind,
            RuntimeRouteKind::Responses | RuntimeRouteKind::Websocket
        ) && let Some(reason) = runtime_quota_precommit_guard_reason(quota_summary, route_kind)
        {
            runtime_proxy_log(
                shared,
                format!(
                    "selection_skip_current route={} profile={} reason={} inflight={} health={} quota_source={} {}",
                    runtime_route_kind_label(route_kind),
                    candidate.name,
                    reason,
                    runtime_profile_inflight_sort_key(&candidate.name, &profile_inflight),
                    runtime_profile_health_sort_key(
                        &candidate.name,
                        &profile_health,
                        now,
                        route_kind
                    ),
                    runtime_quota_source_label(candidate.quota_source),
                    runtime_quota_summary_log_fields(quota_summary),
                ),
            );
            continue;
        }
        if !reserve_runtime_profile_route_circuit_half_open_probe(
            shared,
            &candidate.name,
            route_kind,
        )? {
            runtime_proxy_log(
                shared,
                format!(
                    "selection_skip_current route={} profile={} reason=route_circuit_half_open_probe_wait inflight={} health={} quota_source={} {}",
                    runtime_route_kind_label(route_kind),
                    candidate.name,
                    runtime_profile_inflight_sort_key(&candidate.name, &profile_inflight),
                    runtime_profile_health_sort_key(
                        &candidate.name,
                        &profile_health,
                        now,
                        route_kind
                    ),
                    runtime_quota_source_label(candidate.quota_source),
                    runtime_quota_summary_log_fields(quota_summary),
                ),
            );
            continue;
        }
        runtime_proxy_log(
            shared,
            format!(
                "selection_pick route={} profile={} mode=backoff inflight={} health={} backoff={:?} order={} quota_source={} {}",
                runtime_route_kind_label(route_kind),
                candidate.name,
                runtime_profile_inflight_sort_key(&candidate.name, &profile_inflight),
                runtime_profile_health_sort_key(&candidate.name, &profile_health, now, route_kind),
                runtime_profile_backoff_sort_key(
                    &candidate.name,
                    &retry_backoff_until,
                    &transport_backoff_until,
                    &route_circuit_open_until,
                    route_kind,
                    now,
                ),
                index,
                runtime_quota_source_label(candidate.quota_source),
                runtime_quota_summary_log_fields(quota_summary),
            ),
        );
        fallback = Some(candidate.name);
        break;
    }

    if fallback.is_none() {
        runtime_proxy_log(
            shared,
            format!(
                "selection_pick route={} profile=none mode=exhausted excluded_count={}",
                runtime_route_kind_label(route_kind),
                excluded_profiles.len()
            ),
        );
    }

    Ok(fallback)
}

pub(super) fn runtime_remaining_sync_probe_cold_start_profiles_for_route(
    shared: &RuntimeRotationProxyShared,
    excluded_profiles: &BTreeSet<String>,
    route_kind: RuntimeRouteKind,
) -> Result<usize> {
    let allow_disk_auth_fallback =
        !runtime_proxy_sync_probe_pressure_mode_active_for_route(shared, route_kind);
    let (state, current_profile, cached_reports, cached_usage_snapshots, profile_usage_auth) = {
        let runtime = shared
            .runtime
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
        (
            runtime.state.clone(),
            runtime.current_profile.clone(),
            runtime.profile_probe_cache.clone(),
            runtime.profile_usage_snapshots.clone(),
            runtime.profile_usage_auth.clone(),
        )
    };
    let now = Local::now().timestamp();

    Ok(active_profile_selection_order(&state, &current_profile)
        .into_iter()
        .filter(|name| !excluded_profiles.contains(name))
        .filter(|name| {
            state.profiles.get(name).is_some_and(|profile| {
                runtime_profile_auth_summary_for_selection_with_policy(
                    name,
                    &profile.codex_home,
                    &profile_usage_auth,
                    &cached_reports,
                    allow_disk_auth_fallback,
                )
                .is_some_and(|summary| summary.quota_compatible)
            })
        })
        .filter(|name| !cached_reports.contains_key(name))
        .filter(|name| {
            !cached_usage_snapshots.get(name).is_some_and(|snapshot| {
                runtime_snapshot_blocks_same_request_cold_start_probe(snapshot, route_kind, now)
            })
        })
        .count())
}

pub(super) fn runtime_waitable_inflight_candidates_for_route(
    shared: &RuntimeRotationProxyShared,
    excluded_profiles: &BTreeSet<String>,
    route_kind: RuntimeRouteKind,
    wait_affinity_owner: Option<&str>,
) -> Result<BTreeSet<String>> {
    let now = Local::now().timestamp();
    let pressure_mode = runtime_proxy_pressure_mode_active_for_route(shared, route_kind);
    let inflight_soft_limit = runtime_profile_inflight_soft_limit(route_kind, pressure_mode);
    let (
        state,
        current_profile,
        include_code_review,
        cached_reports,
        cached_usage_snapshots,
        profile_usage_auth,
        retry_backoff_until,
        transport_backoff_until,
        route_circuit_open_until,
        profile_inflight,
        profile_health,
    ) = {
        let mut runtime = shared
            .runtime
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
        prune_runtime_profile_selection_backoff(&mut runtime, now);
        (
            runtime.state.clone(),
            runtime.current_profile.clone(),
            runtime.include_code_review,
            runtime.profile_probe_cache.clone(),
            runtime.profile_usage_snapshots.clone(),
            runtime.profile_usage_auth.clone(),
            runtime.profile_retry_backoff_until.clone(),
            runtime.profile_transport_backoff_until.clone(),
            runtime.profile_route_circuit_open_until.clone(),
            runtime.profile_inflight.clone(),
            runtime.profile_health.clone(),
        )
    };

    let mut waitable_profiles = BTreeSet::new();
    let mut reports = Vec::new();
    for (order_index, name) in active_profile_selection_order(&state, &current_profile)
        .into_iter()
        .enumerate()
    {
        if excluded_profiles.contains(&name) {
            continue;
        }
        if wait_affinity_owner.is_some_and(|owner| owner != name) {
            continue;
        }
        let Some(entry) = cached_reports.get(&name) else {
            continue;
        };
        reports.push(RunProfileProbeReport {
            name,
            order_index,
            auth: entry.auth.clone(),
            result: entry.result.clone(),
        });
    }

    for candidate in ready_profile_candidates(
        &reports,
        include_code_review,
        Some(current_profile.as_str()),
        &state,
        Some(&cached_usage_snapshots),
    ) {
        if excluded_profiles.contains(&candidate.name) {
            continue;
        }
        if runtime_profile_name_in_selection_backoff(
            &candidate.name,
            &retry_backoff_until,
            &transport_backoff_until,
            &route_circuit_open_until,
            route_kind,
            now,
        ) {
            continue;
        }
        if runtime_profile_auth_failure_active_with_auth_cache(
            &profile_health,
            &profile_usage_auth,
            &candidate.name,
            now,
        ) {
            continue;
        }
        if runtime_quota_precommit_guard_reason(
            runtime_quota_summary_for_route(&candidate.usage, route_kind),
            route_kind,
        )
        .is_some()
        {
            continue;
        }
        if runtime_profile_inflight_sort_key(&candidate.name, &profile_inflight)
            >= inflight_soft_limit
        {
            waitable_profiles.insert(candidate.name.clone());
        }
    }

    Ok(waitable_profiles)
}

pub(super) fn runtime_any_waited_candidate_relieved(
    shared: &RuntimeRotationProxyShared,
    waited_profiles: &BTreeSet<String>,
    route_kind: RuntimeRouteKind,
) -> Result<bool> {
    if waited_profiles.is_empty() {
        return Ok(false);
    }
    let now = Local::now().timestamp();
    let pressure_mode = runtime_proxy_pressure_mode_active_for_route(shared, route_kind);
    let inflight_soft_limit = runtime_profile_inflight_soft_limit(route_kind, pressure_mode);
    let (
        state,
        current_profile,
        include_code_review,
        cached_reports,
        cached_usage_snapshots,
        profile_usage_auth,
        retry_backoff_until,
        transport_backoff_until,
        route_circuit_open_until,
        profile_inflight,
        profile_health,
    ) = {
        let mut runtime = shared
            .runtime
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
        prune_runtime_profile_selection_backoff(&mut runtime, now);
        (
            runtime.state.clone(),
            runtime.current_profile.clone(),
            runtime.include_code_review,
            runtime.profile_probe_cache.clone(),
            runtime.profile_usage_snapshots.clone(),
            runtime.profile_usage_auth.clone(),
            runtime.profile_retry_backoff_until.clone(),
            runtime.profile_transport_backoff_until.clone(),
            runtime.profile_route_circuit_open_until.clone(),
            runtime.profile_inflight.clone(),
            runtime.profile_health.clone(),
        )
    };

    let mut reports = Vec::new();
    for (order_index, name) in active_profile_selection_order(&state, &current_profile)
        .into_iter()
        .enumerate()
    {
        if !waited_profiles.contains(&name) {
            continue;
        }
        let Some(entry) = cached_reports.get(&name) else {
            continue;
        };
        reports.push(RunProfileProbeReport {
            name,
            order_index,
            auth: entry.auth.clone(),
            result: entry.result.clone(),
        });
    }

    for candidate in ready_profile_candidates(
        &reports,
        include_code_review,
        Some(current_profile.as_str()),
        &state,
        Some(&cached_usage_snapshots),
    ) {
        if !waited_profiles.contains(&candidate.name) {
            continue;
        }
        if runtime_profile_name_in_selection_backoff(
            &candidate.name,
            &retry_backoff_until,
            &transport_backoff_until,
            &route_circuit_open_until,
            route_kind,
            now,
        ) {
            continue;
        }
        if runtime_profile_auth_failure_active_with_auth_cache(
            &profile_health,
            &profile_usage_auth,
            &candidate.name,
            now,
        ) {
            continue;
        }
        if runtime_quota_precommit_guard_reason(
            runtime_quota_summary_for_route(&candidate.usage, route_kind),
            route_kind,
        )
        .is_some()
        {
            continue;
        }
        if runtime_profile_inflight_sort_key(&candidate.name, &profile_inflight)
            < inflight_soft_limit
        {
            return Ok(true);
        }
    }

    Ok(false)
}

pub(super) struct RuntimeInflightReliefWait<'a> {
    request_id: u64,
    request: &'a RuntimeProxyRequest,
    shared: &'a RuntimeRotationProxyShared,
    excluded_profiles: &'a BTreeSet<String>,
    route_kind: RuntimeRouteKind,
    selection_started_at: Instant,
    continuation: bool,
    wait_affinity_owner: Option<&'a str>,
}

impl<'a> RuntimeInflightReliefWait<'a> {
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn new(
        request_id: u64,
        request: &'a RuntimeProxyRequest,
        shared: &'a RuntimeRotationProxyShared,
        excluded_profiles: &'a BTreeSet<String>,
        route_kind: RuntimeRouteKind,
        selection_started_at: Instant,
        continuation: bool,
        wait_affinity_owner: Option<&'a str>,
    ) -> Self {
        Self {
            request_id,
            request,
            shared,
            excluded_profiles,
            route_kind,
            selection_started_at,
            continuation,
            wait_affinity_owner,
        }
    }
}

pub(super) fn runtime_proxy_maybe_wait_for_interactive_inflight_relief(
    wait: RuntimeInflightReliefWait<'_>,
) -> Result<bool> {
    let RuntimeInflightReliefWait {
        request_id,
        request,
        shared,
        excluded_profiles,
        route_kind,
        selection_started_at,
        continuation,
        wait_affinity_owner,
    } = wait;

    let pressure_mode = runtime_proxy_pressure_mode_active_for_route(shared, route_kind);
    let wait_budget = runtime_proxy_request_inflight_wait_budget(request, pressure_mode);
    if wait_budget.is_zero() {
        return Ok(false);
    }
    let waited_profiles = runtime_waitable_inflight_candidates_for_route(
        shared,
        excluded_profiles,
        route_kind,
        wait_affinity_owner,
    )?;
    if waited_profiles.is_empty() {
        return Ok(false);
    }

    let (_, precommit_budget) = runtime_proxy_precommit_budget(continuation, pressure_mode);
    let remaining_budget = precommit_budget.saturating_sub(selection_started_at.elapsed());
    let total_wait_budget = wait_budget.min(remaining_budget);
    if total_wait_budget.is_zero() {
        return Ok(false);
    }
    let wait_deadline = Instant::now() + total_wait_budget;

    runtime_proxy_log(
        shared,
        format!(
            "request={request_id} transport=http inflight_wait_started route={} wait_ms={}",
            runtime_route_kind_label(route_kind),
            total_wait_budget.as_millis()
        ),
    );
    let started_at = Instant::now();
    let mut observed_revision = runtime_profile_inflight_release_revision(shared);
    let mut signaled = false;
    let mut useful_relief = false;
    let mut wake_source = RuntimeProfileInFlightWaitOutcome::Timeout;
    loop {
        let remaining_wait = wait_deadline.saturating_duration_since(Instant::now());
        if remaining_wait.is_zero() {
            break;
        }
        match runtime_profile_inflight_wait_outcome_since(shared, remaining_wait, observed_revision)
        {
            RuntimeProfileInFlightWaitOutcome::InflightRelease => {
                signaled = true;
                wake_source = RuntimeProfileInFlightWaitOutcome::InflightRelease;
                observed_revision = runtime_profile_inflight_release_revision(shared);
                useful_relief =
                    runtime_any_waited_candidate_relieved(shared, &waited_profiles, route_kind)?;
                if useful_relief {
                    break;
                }
            }
            RuntimeProfileInFlightWaitOutcome::OtherNotify => {
                signaled = true;
                wake_source = RuntimeProfileInFlightWaitOutcome::OtherNotify;
                observed_revision = runtime_profile_inflight_release_revision(shared);
            }
            RuntimeProfileInFlightWaitOutcome::Timeout => {
                if !signaled {
                    wake_source = RuntimeProfileInFlightWaitOutcome::Timeout;
                }
                break;
            }
        }
    }
    runtime_proxy_log(
        shared,
        format!(
            "request={request_id} transport=http inflight_wait_finished route={} waited_ms={} signaled={signaled} useful={} wake_source={}",
            runtime_route_kind_label(route_kind),
            started_at.elapsed().as_millis(),
            useful_relief,
            runtime_profile_inflight_wait_outcome_label(wake_source),
        ),
    );
    Ok(useful_relief)
}

pub(super) fn runtime_profile_usage_cache_is_fresh(
    entry: &RuntimeProfileProbeCacheEntry,
    now: i64,
) -> bool {
    now.saturating_sub(entry.checked_at) <= RUNTIME_PROFILE_USAGE_CACHE_FRESH_SECONDS
}

pub(super) fn runtime_profile_probe_cache_freshness(
    entry: &RuntimeProfileProbeCacheEntry,
    now: i64,
) -> RuntimeProbeCacheFreshness {
    let age = now.saturating_sub(entry.checked_at);
    if age <= RUNTIME_PROFILE_USAGE_CACHE_FRESH_SECONDS {
        RuntimeProbeCacheFreshness::Fresh
    } else if age <= RUNTIME_PROFILE_USAGE_CACHE_STALE_GRACE_SECONDS {
        RuntimeProbeCacheFreshness::StaleUsable
    } else {
        RuntimeProbeCacheFreshness::Expired
    }
}

pub(super) fn update_runtime_profile_probe_cache_with_usage(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    usage: UsageResponse,
) -> Result<()> {
    let auth = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?
        .state
        .profiles
        .get(profile_name)
        .map(|profile| profile.provider.auth_summary(&profile.codex_home))
        .unwrap_or(AuthSummary {
            label: "chatgpt".to_string(),
            quota_compatible: true,
        });
    apply_runtime_profile_probe_result(shared, profile_name, auth, Ok(usage))
}

pub(super) fn runtime_proxy_current_profile(shared: &RuntimeRotationProxyShared) -> Result<String> {
    Ok(shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?
        .current_profile
        .clone())
}

pub(super) fn runtime_profile_in_retry_backoff(
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

pub(super) fn runtime_profile_in_transport_backoff(
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

pub(super) fn runtime_profile_inflight_count(
    runtime: &RuntimeRotationState,
    profile_name: &str,
) -> usize {
    runtime
        .profile_inflight
        .get(profile_name)
        .copied()
        .unwrap_or(0)
}

pub(super) fn runtime_profile_inflight_hard_limit_context(context: &str) -> usize {
    runtime_profile_inflight_weight(context)
}

pub(super) fn runtime_profile_inflight_hard_limited_for_context(
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

pub(super) fn runtime_profile_in_selection_backoff(
    runtime: &RuntimeRotationState,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
    now: i64,
) -> bool {
    runtime_profile_in_retry_backoff(runtime, profile_name, now)
        || runtime_profile_in_transport_backoff(runtime, profile_name, route_kind, now)
}

pub(super) fn runtime_profile_health_score(
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

pub(super) fn runtime_route_coupled_kinds(
    route_kind: RuntimeRouteKind,
) -> &'static [RuntimeRouteKind] {
    match route_kind {
        RuntimeRouteKind::Responses => &[RuntimeRouteKind::Websocket],
        RuntimeRouteKind::Websocket => &[RuntimeRouteKind::Responses],
        RuntimeRouteKind::Compact => &[RuntimeRouteKind::Standard],
        RuntimeRouteKind::Standard => &[RuntimeRouteKind::Compact],
    }
}

pub(super) fn runtime_profile_route_coupling_score(
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

pub(super) fn runtime_profile_route_coupling_score_from_map(
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

pub(super) fn runtime_profile_selection_jitter(
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

pub(super) fn prune_runtime_profile_retry_backoff(runtime: &mut RuntimeRotationState, now: i64) {
    runtime
        .profile_retry_backoff_until
        .retain(|_, until| *until > now);
}

pub(super) fn prune_runtime_profile_transport_backoff(
    runtime: &mut RuntimeRotationState,
    now: i64,
) {
    runtime
        .profile_transport_backoff_until
        .retain(|_, until| *until > now);
}

pub(super) fn prune_runtime_profile_route_circuits(runtime: &mut RuntimeRotationState, now: i64) {
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

pub(super) fn prune_runtime_profile_selection_backoff(
    runtime: &mut RuntimeRotationState,
    now: i64,
) {
    prune_runtime_profile_retry_backoff(runtime, now);
    prune_runtime_profile_transport_backoff(runtime, now);
    prune_runtime_profile_route_circuits(runtime, now);
}

pub(super) fn runtime_profile_name_in_selection_backoff(
    profile_name: &str,
    retry_backoff_until: &BTreeMap<String, i64>,
    transport_backoff_until: &BTreeMap<String, i64>,
    route_circuit_open_until: &BTreeMap<String, i64>,
    route_kind: RuntimeRouteKind,
    now: i64,
) -> bool {
    retry_backoff_until
        .get(profile_name)
        .copied()
        .is_some_and(|until| until > now)
        || runtime_profile_transport_backoff_until_from_map(
            transport_backoff_until,
            profile_name,
            route_kind,
            now,
        )
        .is_some()
        || route_circuit_open_until
            .get(&runtime_profile_route_circuit_key(profile_name, route_kind))
            .copied()
            .is_some_and(|until| until > now)
}

pub(super) fn runtime_profile_backoff_sort_key(
    profile_name: &str,
    retry_backoff_until: &BTreeMap<String, i64>,
    transport_backoff_until: &BTreeMap<String, i64>,
    route_circuit_open_until: &BTreeMap<String, i64>,
    route_kind: RuntimeRouteKind,
    now: i64,
) -> (usize, i64, i64, i64) {
    let retry_until = retry_backoff_until
        .get(profile_name)
        .copied()
        .filter(|until| *until > now);
    let transport_until = runtime_profile_transport_backoff_until_from_map(
        transport_backoff_until,
        profile_name,
        route_kind,
        now,
    );
    let circuit_until = route_circuit_open_until
        .get(&runtime_profile_route_circuit_key(profile_name, route_kind))
        .copied()
        .filter(|until| *until > now);

    match (circuit_until, transport_until, retry_until) {
        (None, None, None) => (0, 0, 0, 0),
        (Some(circuit_until), None, None) => (1, circuit_until, 0, 0),
        (None, Some(transport_until), None) => (2, transport_until, 0, 0),
        (None, None, Some(retry_until)) => (3, retry_until, 0, 0),
        (Some(circuit_until), Some(transport_until), None) => (
            4,
            circuit_until.min(transport_until),
            circuit_until.max(transport_until),
            0,
        ),
        (Some(circuit_until), None, Some(retry_until)) => (
            5,
            circuit_until.min(retry_until),
            circuit_until.max(retry_until),
            0,
        ),
        (None, Some(transport_until), Some(retry_until)) => (
            6,
            transport_until.min(retry_until),
            transport_until.max(retry_until),
            0,
        ),
        (Some(circuit_until), Some(transport_until), Some(retry_until)) => (
            7,
            circuit_until.min(transport_until.min(retry_until)),
            circuit_until.max(transport_until.max(retry_until)),
            retry_until,
        ),
    }
}

pub(super) fn runtime_profile_health_sort_key(
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

pub(super) fn runtime_profile_inflight_sort_key(
    profile_name: &str,
    profile_inflight: &BTreeMap<String, usize>,
) -> usize {
    profile_inflight.get(profile_name).copied().unwrap_or(0)
}

pub(super) fn runtime_profile_inflight_weight(context: &str) -> usize {
    match context {
        "websocket_session" | "responses_http" => 2,
        _ => 1,
    }
}

pub(super) fn runtime_route_kind_inflight_context(route_kind: RuntimeRouteKind) -> &'static str {
    match route_kind {
        RuntimeRouteKind::Responses => "responses_http",
        RuntimeRouteKind::Compact => "compact_http",
        RuntimeRouteKind::Websocket => "websocket_session",
        RuntimeRouteKind::Standard => "standard_http",
    }
}

pub(super) fn runtime_profile_inflight_soft_limit(
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

pub(super) fn runtime_quota_pressure_band_reason(band: RuntimeQuotaPressureBand) -> &'static str {
    match band {
        RuntimeQuotaPressureBand::Healthy => "quota_healthy",
        RuntimeQuotaPressureBand::Thin => "quota_thin",
        RuntimeQuotaPressureBand::Critical => "quota_critical",
        RuntimeQuotaPressureBand::Exhausted => "quota_exhausted",
        RuntimeQuotaPressureBand::Unknown => "quota_unknown",
    }
}

pub(super) fn runtime_quota_window_status_reason(status: RuntimeQuotaWindowStatus) -> &'static str {
    match status {
        RuntimeQuotaWindowStatus::Ready => "ready",
        RuntimeQuotaWindowStatus::Thin => "thin",
        RuntimeQuotaWindowStatus::Critical => "critical",
        RuntimeQuotaWindowStatus::Exhausted => "exhausted",
        RuntimeQuotaWindowStatus::Unknown => "unknown",
    }
}

pub(super) fn runtime_quota_window_summary(
    usage: &UsageResponse,
    label: &str,
) -> RuntimeQuotaWindowSummary {
    let Some(window) = required_main_window_snapshot(usage, label) else {
        return RuntimeQuotaWindowSummary {
            status: RuntimeQuotaWindowStatus::Unknown,
            remaining_percent: 0,
            reset_at: i64::MAX,
        };
    };
    let status = if window.remaining_percent == 0 {
        RuntimeQuotaWindowStatus::Exhausted
    } else if window.remaining_percent <= 5 {
        RuntimeQuotaWindowStatus::Critical
    } else if window.remaining_percent <= 15 {
        RuntimeQuotaWindowStatus::Thin
    } else {
        RuntimeQuotaWindowStatus::Ready
    };
    RuntimeQuotaWindowSummary {
        status,
        remaining_percent: window.remaining_percent,
        reset_at: window.reset_at,
    }
}

pub(super) fn runtime_quota_summary_for_route(
    usage: &UsageResponse,
    route_kind: RuntimeRouteKind,
) -> RuntimeQuotaSummary {
    RuntimeQuotaSummary {
        five_hour: runtime_quota_window_summary(usage, "5h"),
        weekly: runtime_quota_window_summary(usage, "weekly"),
        route_band: runtime_quota_pressure_band_for_route(usage, route_kind),
    }
}

pub(super) fn runtime_quota_summary_blocking_reset_at(
    summary: RuntimeQuotaSummary,
    route_kind: RuntimeRouteKind,
) -> Option<i64> {
    let floor_percent = runtime_quota_precommit_floor_percent(route_kind);
    [summary.five_hour, summary.weekly]
        .into_iter()
        .filter(|window| runtime_quota_window_precommit_guard(*window, floor_percent))
        .map(|window| window.reset_at)
        .filter(|reset_at| *reset_at != i64::MAX)
        .max()
}

pub(super) fn runtime_profile_usage_snapshot_from_usage(
    usage: &UsageResponse,
) -> RuntimeProfileUsageSnapshot {
    let five_hour = runtime_quota_window_summary(usage, "5h");
    let weekly = runtime_quota_window_summary(usage, "weekly");
    RuntimeProfileUsageSnapshot {
        checked_at: Local::now().timestamp(),
        five_hour_status: five_hour.status,
        five_hour_remaining_percent: five_hour.remaining_percent,
        five_hour_reset_at: five_hour.reset_at,
        weekly_status: weekly.status,
        weekly_remaining_percent: weekly.remaining_percent,
        weekly_reset_at: weekly.reset_at,
    }
}

pub(super) fn runtime_quota_summary_from_usage_snapshot(
    snapshot: &RuntimeProfileUsageSnapshot,
    route_kind: RuntimeRouteKind,
) -> RuntimeQuotaSummary {
    runtime_quota_summary_from_usage_snapshot_at(snapshot, route_kind, Local::now().timestamp())
}

pub(super) fn runtime_quota_summary_from_usage_snapshot_at(
    snapshot: &RuntimeProfileUsageSnapshot,
    route_kind: RuntimeRouteKind,
    now: i64,
) -> RuntimeQuotaSummary {
    let five_hour = runtime_quota_window_summary_from_usage_snapshot_at(
        snapshot.five_hour_status,
        snapshot.five_hour_remaining_percent,
        snapshot.five_hour_reset_at,
        now,
    );
    let weekly = runtime_quota_window_summary_from_usage_snapshot_at(
        snapshot.weekly_status,
        snapshot.weekly_remaining_percent,
        snapshot.weekly_reset_at,
        now,
    );
    let route_band = [
        five_hour.status,
        weekly.status,
        match route_kind {
            RuntimeRouteKind::Responses | RuntimeRouteKind::Websocket => weekly.status,
            RuntimeRouteKind::Compact | RuntimeRouteKind::Standard => five_hour.status,
        },
    ]
    .into_iter()
    .fold(RuntimeQuotaPressureBand::Healthy, |band, status| {
        band.max(match status {
            RuntimeQuotaWindowStatus::Ready => RuntimeQuotaPressureBand::Healthy,
            RuntimeQuotaWindowStatus::Thin => RuntimeQuotaPressureBand::Thin,
            RuntimeQuotaWindowStatus::Critical => RuntimeQuotaPressureBand::Critical,
            RuntimeQuotaWindowStatus::Exhausted => RuntimeQuotaPressureBand::Exhausted,
            RuntimeQuotaWindowStatus::Unknown => RuntimeQuotaPressureBand::Unknown,
        })
    });
    RuntimeQuotaSummary {
        five_hour,
        weekly,
        route_band,
    }
}

pub(super) fn runtime_quota_window_summary_from_usage_snapshot_at(
    status: RuntimeQuotaWindowStatus,
    remaining_percent: i64,
    reset_at: i64,
    now: i64,
) -> RuntimeQuotaWindowSummary {
    if reset_at != i64::MAX && reset_at <= now {
        return RuntimeQuotaWindowSummary {
            status: RuntimeQuotaWindowStatus::Ready,
            remaining_percent: 100,
            reset_at,
        };
    }
    RuntimeQuotaWindowSummary {
        status,
        remaining_percent,
        reset_at,
    }
}

pub(super) fn runtime_profile_usage_snapshot_hold_active(
    snapshot: &RuntimeProfileUsageSnapshot,
    now: i64,
) -> bool {
    [
        (snapshot.five_hour_status, snapshot.five_hour_reset_at),
        (snapshot.weekly_status, snapshot.weekly_reset_at),
    ]
    .into_iter()
    .any(|(status, reset_at)| {
        matches!(status, RuntimeQuotaWindowStatus::Exhausted)
            && reset_at != i64::MAX
            && reset_at > now
    })
}

pub(super) fn runtime_profile_usage_snapshot_hold_expired(
    snapshot: &RuntimeProfileUsageSnapshot,
    now: i64,
) -> bool {
    [
        (snapshot.five_hour_status, snapshot.five_hour_reset_at),
        (snapshot.weekly_status, snapshot.weekly_reset_at),
    ]
    .into_iter()
    .any(|(status, reset_at)| {
        matches!(status, RuntimeQuotaWindowStatus::Exhausted)
            && reset_at != i64::MAX
            && reset_at <= now
    })
}

pub(super) fn runtime_proxy_quota_reset_at_from_message(message: &str) -> Option<i64> {
    let marker = message.to_ascii_lowercase().find("try again at ")?;
    let candidate = message
        .get(marker + "try again at ".len()..)?
        .trim()
        .trim_end_matches('.');
    let now = Local::now();
    if let Some((time_text, meridiem)) = candidate
        .split_whitespace()
        .collect::<Vec<_>>()
        .get(..2)
        .and_then(|parts| {
            if parts.len() == 2 {
                Some((parts[0], parts[1]))
            } else {
                None
            }
        })
        && let Ok(time) =
            chrono::NaiveTime::parse_from_str(&format!("{time_text} {meridiem}"), "%I:%M %p")
    {
        let mut naive = now.date_naive().and_time(time);
        let mut parsed = Local
            .from_local_datetime(&naive)
            .single()
            .or_else(|| Local.from_local_datetime(&naive).earliest())?;
        if parsed.timestamp() <= now.timestamp() {
            naive = naive.checked_add_signed(chrono::Duration::days(1))?;
            parsed = Local
                .from_local_datetime(&naive)
                .single()
                .or_else(|| Local.from_local_datetime(&naive).earliest())?;
        }
        return Some(parsed.timestamp());
    }
    let mut parts = candidate
        .split_whitespace()
        .map(|part| part.to_string())
        .collect::<Vec<_>>();
    if parts.len() < 5 {
        return None;
    }
    let day_digits = parts[1]
        .trim_end_matches(',')
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .collect::<String>();
    if day_digits.is_empty() {
        return None;
    }
    parts[1] = format!("{day_digits},");
    let normalized = parts[..5].join(" ");
    let naive = chrono::NaiveDateTime::parse_from_str(&normalized, "%b %d, %Y %I:%M %p").ok()?;
    Local
        .from_local_datetime(&naive)
        .single()
        .or_else(|| Local.from_local_datetime(&naive).earliest())
        .map(|datetime| datetime.timestamp())
}

pub(super) fn runtime_profile_known_quota_reset_at(
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

pub(super) fn mark_runtime_profile_quota_quarantine(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
    quota_message: Option<&str>,
) -> Result<()> {
    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let now = Local::now().timestamp();
    prune_runtime_profile_selection_backoff(&mut runtime, now);
    let resolved_reset_at = quota_message
        .and_then(runtime_proxy_quota_reset_at_from_message)
        .or_else(|| runtime_profile_known_quota_reset_at(&runtime, profile_name, route_kind))
        .filter(|reset_at| *reset_at > now);
    let until = resolved_reset_at
        .unwrap_or_else(|| now.saturating_add(RUNTIME_PROFILE_QUOTA_QUARANTINE_FALLBACK_SECONDS));
    runtime.profile_probe_cache.remove(profile_name);
    let snapshot = runtime
        .profile_usage_snapshots
        .entry(profile_name.to_string())
        .or_insert(RuntimeProfileUsageSnapshot {
            checked_at: now,
            five_hour_status: RuntimeQuotaWindowStatus::Unknown,
            five_hour_remaining_percent: 0,
            five_hour_reset_at: i64::MAX,
            weekly_status: RuntimeQuotaWindowStatus::Unknown,
            weekly_remaining_percent: 0,
            weekly_reset_at: i64::MAX,
        });
    snapshot.checked_at = now;
    snapshot.five_hour_status = RuntimeQuotaWindowStatus::Exhausted;
    snapshot.five_hour_remaining_percent = 0;
    snapshot.five_hour_reset_at = if snapshot.five_hour_reset_at == i64::MAX {
        until
    } else {
        snapshot.five_hour_reset_at.max(until)
    };
    snapshot.weekly_status = RuntimeQuotaWindowStatus::Exhausted;
    snapshot.weekly_remaining_percent = 0;
    snapshot.weekly_reset_at = if snapshot.weekly_reset_at == i64::MAX {
        until
    } else {
        snapshot.weekly_reset_at.max(until)
    };
    runtime
        .profile_retry_backoff_until
        .entry(profile_name.to_string())
        .and_modify(|current| *current = (*current).max(until))
        .or_insert(until);
    schedule_runtime_state_save_from_runtime(
        shared,
        &runtime,
        &format!("profile_retry_backoff:{profile_name}"),
    );
    drop(runtime);
    runtime_proxy_log(
        shared,
        format!(
            "profile_quota_quarantine profile={profile_name} route={} until={} reset_at={} message={}",
            runtime_route_kind_label(route_kind),
            until,
            resolved_reset_at.unwrap_or(i64::MAX),
            quota_message.unwrap_or("-"),
        ),
    );
    runtime_proxy_log(
        shared,
        format!("profile_retry_backoff profile={profile_name} until={until}"),
    );
    Ok(())
}

pub(super) fn usage_from_runtime_usage_snapshot(
    snapshot: &RuntimeProfileUsageSnapshot,
) -> UsageResponse {
    UsageResponse {
        email: None,
        plan_type: None,
        rate_limit: Some(WindowPair {
            primary_window: Some(UsageWindow {
                used_percent: Some((100 - snapshot.five_hour_remaining_percent).clamp(0, 100)),
                reset_at: (snapshot.five_hour_reset_at != i64::MAX)
                    .then_some(snapshot.five_hour_reset_at),
                limit_window_seconds: Some(18_000),
            }),
            secondary_window: Some(UsageWindow {
                used_percent: Some((100 - snapshot.weekly_remaining_percent).clamp(0, 100)),
                reset_at: (snapshot.weekly_reset_at != i64::MAX)
                    .then_some(snapshot.weekly_reset_at),
                limit_window_seconds: Some(604_800),
            }),
        }),
        code_review_rate_limit: None,
        additional_rate_limits: Vec::new(),
    }
}

pub(super) fn runtime_profile_backoffs_snapshot(
    runtime: &RuntimeRotationState,
) -> RuntimeProfileBackoffs {
    RuntimeProfileBackoffs {
        retry_backoff_until: runtime.profile_retry_backoff_until.clone(),
        transport_backoff_until: runtime.profile_transport_backoff_until.clone(),
        route_circuit_open_until: runtime.profile_route_circuit_open_until.clone(),
    }
}

pub(super) fn runtime_continuation_store_snapshot(
    runtime: &RuntimeRotationState,
) -> RuntimeContinuationStore {
    RuntimeContinuationStore {
        response_profile_bindings: runtime.state.response_profile_bindings.clone(),
        session_profile_bindings: runtime.state.session_profile_bindings.clone(),
        turn_state_bindings: runtime.turn_state_bindings.clone(),
        session_id_bindings: runtime.session_id_bindings.clone(),
        statuses: runtime.continuation_statuses.clone(),
    }
}

pub(super) fn runtime_soften_persisted_backoff_map_for_startup(
    backoffs: &mut BTreeMap<String, i64>,
    now: i64,
    max_future_seconds: i64,
) -> bool {
    let max_until = now.saturating_add(max_future_seconds.max(0));
    let mut changed = false;
    backoffs.retain(|_, until| {
        if *until <= now {
            changed = true;
            return false;
        }
        let next_until = (*until).min(max_until);
        if next_until != *until {
            changed = true;
        }
        *until = next_until;
        true
    });
    changed
}

pub(super) fn runtime_route_kind_from_label(label: &str) -> Option<RuntimeRouteKind> {
    match label {
        "responses" => Some(RuntimeRouteKind::Responses),
        "compact" => Some(RuntimeRouteKind::Compact),
        "websocket" => Some(RuntimeRouteKind::Websocket),
        "standard" => Some(RuntimeRouteKind::Standard),
        _ => None,
    }
}

pub(super) fn runtime_profile_route_circuit_probe_seconds(
    profile_scores: &BTreeMap<String, RuntimeProfileHealth>,
    route_profile_key: &str,
    now: i64,
) -> i64 {
    let Some((route_label, profile_name)) =
        runtime_profile_route_key_parts(route_profile_key, "__route_circuit__:")
    else {
        return RUNTIME_PROFILE_CIRCUIT_HALF_OPEN_PROBE_SECONDS;
    };
    let Some(route_kind) = runtime_route_kind_from_label(route_label) else {
        return RUNTIME_PROFILE_CIRCUIT_HALF_OPEN_PROBE_SECONDS;
    };
    let score = runtime_profile_effective_health_score_from_map(
        profile_scores,
        &runtime_profile_route_health_key(profile_name, route_kind),
        now,
    );
    runtime_profile_circuit_half_open_probe_seconds(score)
}

pub(super) fn runtime_soften_persisted_route_circuits_for_startup(
    route_circuit_open_until: &mut BTreeMap<String, i64>,
    profile_scores: &BTreeMap<String, RuntimeProfileHealth>,
    now: i64,
) -> bool {
    let mut changed = false;
    route_circuit_open_until.retain(|route_profile_key, until| {
        if *until <= now {
            changed = true;
            return false;
        }
        let max_until = now.saturating_add(runtime_profile_route_circuit_probe_seconds(
            profile_scores,
            route_profile_key,
            now,
        ));
        let next_until = (*until).min(max_until);
        if next_until != *until {
            changed = true;
        }
        *until = next_until;
        true
    });
    changed
}

pub(super) fn runtime_soften_persisted_backoffs_for_startup(
    backoffs: &mut RuntimeProfileBackoffs,
    profile_scores: &BTreeMap<String, RuntimeProfileHealth>,
    now: i64,
) -> bool {
    let mut changed = runtime_soften_persisted_backoff_map_for_startup(
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

pub(super) const RUNTIME_PROFILE_CIRCUIT_OPEN_THRESHOLD: u32 = 4;
pub(super) const RUNTIME_PROFILE_CIRCUIT_OPEN_SECONDS: i64 = 20;
pub(super) const RUNTIME_PROFILE_CIRCUIT_OPEN_MAX_SECONDS: i64 = if cfg!(test) { 320 } else { 600 };
pub(super) const RUNTIME_PROFILE_CIRCUIT_HALF_OPEN_PROBE_SECONDS: i64 = 5;
pub(super) const RUNTIME_PROFILE_CIRCUIT_HALF_OPEN_PROBE_MAX_SECONDS: i64 =
    if cfg!(test) { 20 } else { 60 };
pub(super) const RUNTIME_PROFILE_CIRCUIT_REOPEN_DECAY_SECONDS: i64 =
    if cfg!(test) { 12 } else { 1_800 };
pub(super) const RUNTIME_PROFILE_CIRCUIT_REOPEN_MAX_STAGE: u32 = 4;

pub(super) fn runtime_profile_circuit_open_seconds(score: u32, reopen_stage: u32) -> i64 {
    let multiplier = 1_i64
        .checked_shl(
            score
                .saturating_sub(RUNTIME_PROFILE_CIRCUIT_OPEN_THRESHOLD)
                .min(3)
                .saturating_add(reopen_stage.min(RUNTIME_PROFILE_CIRCUIT_REOPEN_MAX_STAGE)),
        )
        .unwrap_or(i64::MAX);
    RUNTIME_PROFILE_CIRCUIT_OPEN_SECONDS
        .saturating_mul(multiplier)
        .min(RUNTIME_PROFILE_CIRCUIT_OPEN_MAX_SECONDS)
}

pub(super) fn runtime_profile_circuit_half_open_probe_seconds(score: u32) -> i64 {
    let multiplier = 1_i64
        .checked_shl(
            score
                .saturating_sub(RUNTIME_PROFILE_CIRCUIT_OPEN_THRESHOLD)
                .min(3),
        )
        .unwrap_or(i64::MAX);
    RUNTIME_PROFILE_CIRCUIT_HALF_OPEN_PROBE_SECONDS
        .saturating_mul(multiplier)
        .min(RUNTIME_PROFILE_CIRCUIT_HALF_OPEN_PROBE_MAX_SECONDS)
}

pub(super) fn runtime_profile_route_circuit_key(
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) -> String {
    format!(
        "__route_circuit__:{}:{profile_name}",
        runtime_route_kind_label(route_kind)
    )
}

pub(super) fn runtime_profile_route_circuit_profile_name(key: &str) -> &str {
    key.rsplit(':').next().unwrap_or(key)
}

pub(super) fn runtime_profile_route_circuit_health_key(key: &str) -> String {
    key.replacen("__route_circuit__", "__route_health__", 1)
}

pub(super) fn runtime_profile_route_circuit_reopen_key(
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) -> String {
    format!(
        "__route_circuit_reopen__:{}:{profile_name}",
        runtime_route_kind_label(route_kind)
    )
}

pub(super) fn runtime_profile_route_circuit_open_until(
    runtime: &RuntimeRotationState,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
    now: i64,
) -> Option<i64> {
    runtime
        .profile_route_circuit_open_until
        .get(&runtime_profile_route_circuit_key(profile_name, route_kind))
        .copied()
        .filter(|until| *until > now)
}

pub(super) fn reserve_runtime_profile_route_circuit_half_open_probe(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) -> Result<bool> {
    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let now = Local::now().timestamp();
    let key = runtime_profile_route_circuit_key(profile_name, route_kind);
    let route_label = runtime_route_kind_label(route_kind);
    let Some(until) = runtime.profile_route_circuit_open_until.get(&key).copied() else {
        return Ok(true);
    };
    if until > now {
        return Ok(false);
    }
    let health_key = runtime_profile_route_circuit_health_key(&key);
    let reopen_key = runtime_profile_route_circuit_reopen_key(profile_name, route_kind);
    let health_score =
        runtime_profile_effective_health_score_from_map(&runtime.profile_health, &health_key, now);
    if health_score == 0 {
        runtime.profile_route_circuit_open_until.remove(&key);
        runtime.profile_health.remove(&reopen_key);
        schedule_runtime_state_save_from_runtime(
            shared,
            &runtime,
            &format!("profile_circuit_clear:{profile_name}:{route_label}"),
        );
        return Ok(true);
    }

    let probe_seconds = runtime_profile_circuit_half_open_probe_seconds(health_score);
    let reserve_until = now.saturating_add(probe_seconds);
    runtime
        .profile_route_circuit_open_until
        .insert(key, reserve_until);
    schedule_runtime_state_save_from_runtime(
        shared,
        &runtime,
        &format!("profile_circuit_half_open_probe:{profile_name}:{route_label}"),
    );
    drop(runtime);
    runtime_proxy_log(
        shared,
        format!(
            "profile_circuit_half_open_probe profile={profile_name} route={} until={reserve_until} health={health_score} probe_seconds={probe_seconds}",
            route_label
        ),
    );
    Ok(true)
}

pub(super) fn clear_runtime_profile_circuit_for_route(
    runtime: &mut RuntimeRotationState,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) -> bool {
    runtime
        .profile_route_circuit_open_until
        .remove(&runtime_profile_route_circuit_key(profile_name, route_kind))
        .is_some()
}

pub(super) fn runtime_quota_source_label(source: RuntimeQuotaSource) -> &'static str {
    match source {
        RuntimeQuotaSource::LiveProbe => "probe_cache",
        RuntimeQuotaSource::PersistedSnapshot => "persisted_snapshot",
    }
}

pub(super) fn runtime_usage_snapshot_is_usable(
    snapshot: &RuntimeProfileUsageSnapshot,
    now: i64,
) -> bool {
    if runtime_profile_usage_snapshot_hold_active(snapshot, now) {
        return true;
    }
    if runtime_profile_usage_snapshot_hold_expired(snapshot, now) {
        return false;
    }
    now.saturating_sub(snapshot.checked_at) <= RUNTIME_PROFILE_USAGE_CACHE_STALE_GRACE_SECONDS
}

pub(super) fn runtime_quota_summary_log_fields(summary: RuntimeQuotaSummary) -> String {
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

pub(super) type RuntimeQuotaPressureSortKey = (
    RuntimeQuotaPressureBand,
    i64,
    i64,
    i64,
    Reverse<i64>,
    Reverse<i64>,
    Reverse<i64>,
    i64,
    i64,
);

pub(super) fn runtime_quota_pressure_sort_key_for_route(
    usage: &UsageResponse,
    route_kind: RuntimeRouteKind,
) -> RuntimeQuotaPressureSortKey {
    let score = ready_profile_score_for_route(usage, route_kind);
    (
        runtime_quota_pressure_band_for_route(usage, route_kind),
        score.total_pressure,
        score.weekly_pressure,
        score.five_hour_pressure,
        Reverse(score.reserve_floor),
        Reverse(score.weekly_remaining),
        Reverse(score.five_hour_remaining),
        score.weekly_reset_at,
        score.five_hour_reset_at,
    )
}

pub(super) fn runtime_quota_pressure_sort_key_for_route_from_summary(
    summary: RuntimeQuotaSummary,
) -> RuntimeQuotaPressureSortKey {
    (
        summary.route_band,
        match summary.route_band {
            RuntimeQuotaPressureBand::Healthy => 0,
            RuntimeQuotaPressureBand::Thin => 1,
            RuntimeQuotaPressureBand::Critical => 2,
            RuntimeQuotaPressureBand::Exhausted => 3,
            RuntimeQuotaPressureBand::Unknown => 4,
        },
        match summary.weekly.status {
            RuntimeQuotaWindowStatus::Ready => 0,
            RuntimeQuotaWindowStatus::Thin => 1,
            RuntimeQuotaWindowStatus::Critical => 2,
            RuntimeQuotaWindowStatus::Exhausted => 3,
            RuntimeQuotaWindowStatus::Unknown => 4,
        },
        match summary.five_hour.status {
            RuntimeQuotaWindowStatus::Ready => 0,
            RuntimeQuotaWindowStatus::Thin => 1,
            RuntimeQuotaWindowStatus::Critical => 2,
            RuntimeQuotaWindowStatus::Exhausted => 3,
            RuntimeQuotaWindowStatus::Unknown => 4,
        },
        Reverse(
            summary
                .weekly
                .remaining_percent
                .min(summary.five_hour.remaining_percent),
        ),
        Reverse(summary.weekly.remaining_percent),
        Reverse(summary.five_hour.remaining_percent),
        summary.weekly.reset_at,
        summary.five_hour.reset_at,
    )
}

pub(super) fn runtime_quota_pressure_band_for_route(
    usage: &UsageResponse,
    route_kind: RuntimeRouteKind,
) -> RuntimeQuotaPressureBand {
    let Some(weekly) = required_main_window_snapshot(usage, "weekly") else {
        return RuntimeQuotaPressureBand::Unknown;
    };
    let Some(five_hour) = required_main_window_snapshot(usage, "5h") else {
        return RuntimeQuotaPressureBand::Unknown;
    };

    let weekly_remaining = weekly.remaining_percent;
    let five_hour_remaining = five_hour.remaining_percent;
    if weekly_remaining == 0 || five_hour_remaining == 0 {
        return RuntimeQuotaPressureBand::Exhausted;
    }

    let (thin_weekly, thin_five_hour, critical_weekly, critical_five_hour) = match route_kind {
        RuntimeRouteKind::Responses | RuntimeRouteKind::Websocket => (20, 10, 10, 5),
        RuntimeRouteKind::Compact | RuntimeRouteKind::Standard => (10, 5, 5, 3),
    };

    if weekly_remaining <= critical_weekly || five_hour_remaining <= critical_five_hour {
        RuntimeQuotaPressureBand::Critical
    } else if weekly_remaining <= thin_weekly || five_hour_remaining <= thin_five_hour {
        RuntimeQuotaPressureBand::Thin
    } else {
        RuntimeQuotaPressureBand::Healthy
    }
}

pub(super) fn runtime_profile_effective_health_score(
    entry: &RuntimeProfileHealth,
    now: i64,
) -> u32 {
    runtime_profile_effective_score(entry, now, RUNTIME_PROFILE_HEALTH_DECAY_SECONDS)
}

pub(super) fn runtime_profile_effective_score(
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

pub(super) fn runtime_profile_effective_health_score_from_map(
    profile_health: &BTreeMap<String, RuntimeProfileHealth>,
    key: &str,
    now: i64,
) -> u32 {
    profile_health
        .get(key)
        .map(|entry| runtime_profile_effective_health_score(entry, now))
        .unwrap_or(0)
}

pub(super) fn runtime_profile_effective_score_from_map(
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

pub(super) fn runtime_profile_global_health_score(
    runtime: &RuntimeRotationState,
    profile_name: &str,
    now: i64,
) -> u32 {
    runtime_profile_effective_health_score_from_map(&runtime.profile_health, profile_name, now)
}

pub(super) fn runtime_profile_route_health_key(
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) -> String {
    format!(
        "__route_health__:{}:{profile_name}",
        runtime_route_kind_label(route_kind)
    )
}

pub(super) fn runtime_profile_route_bad_pairing_key(
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) -> String {
    format!(
        "__route_bad_pairing__:{}:{profile_name}",
        runtime_route_kind_label(route_kind)
    )
}

pub(super) fn runtime_profile_route_success_streak_key(
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) -> String {
    format!(
        "__route_success__:{}:{profile_name}",
        runtime_route_kind_label(route_kind)
    )
}

pub(super) fn runtime_profile_route_performance_key(
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) -> String {
    format!(
        "__route_performance__:{}:{profile_name}",
        runtime_route_kind_label(route_kind)
    )
}

pub(super) fn runtime_profile_route_health_score(
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

pub(super) fn runtime_profile_route_performance_score(
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

pub(super) fn runtime_profile_latency_penalty(
    elapsed_ms: u64,
    route_kind: RuntimeRouteKind,
    stage: &str,
) -> u32 {
    let (good_ms, warn_ms, poor_ms, severe_ms) = match (route_kind, stage) {
        (RuntimeRouteKind::Responses, "ttfb") | (RuntimeRouteKind::Websocket, "connect") => {
            (120, 300, 700, 1_500)
        }
        (RuntimeRouteKind::Compact, _) | (RuntimeRouteKind::Standard, _) => (80, 180, 400, 900),
        _ => (100, 250, 600, 1_200),
    };
    match elapsed_ms {
        elapsed if elapsed <= good_ms => 0,
        elapsed if elapsed <= warn_ms => 2,
        elapsed if elapsed <= poor_ms => 4,
        elapsed if elapsed <= severe_ms => 7,
        _ => RUNTIME_PROFILE_LATENCY_PENALTY_MAX,
    }
}

pub(super) fn update_runtime_profile_route_performance(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
    next_score: u32,
    reason: &str,
) -> Result<()> {
    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let now = Local::now().timestamp();
    let key = runtime_profile_route_performance_key(profile_name, route_kind);
    if next_score == 0 {
        runtime.profile_health.remove(&key);
    } else {
        runtime.profile_health.insert(
            key,
            RuntimeProfileHealth {
                score: next_score.min(RUNTIME_PROFILE_LATENCY_PENALTY_MAX),
                updated_at: now,
            },
        );
    }
    drop(runtime);
    runtime_proxy_log(
        shared,
        format!(
            "profile_latency profile={profile_name} route={} score={} reason={reason}",
            runtime_route_kind_label(route_kind),
            next_score.min(RUNTIME_PROFILE_LATENCY_PENALTY_MAX),
        ),
    );
    Ok(())
}

pub(super) fn note_runtime_profile_latency_observation(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
    stage: &str,
    elapsed_ms: u64,
) {
    let current_score = shared
        .runtime
        .lock()
        .ok()
        .map(|runtime| {
            let now = Local::now().timestamp();
            runtime_profile_effective_score_from_map(
                &runtime.profile_health,
                &runtime_profile_route_performance_key(profile_name, route_kind),
                now,
                RUNTIME_PROFILE_PERFORMANCE_DECAY_SECONDS,
            )
        })
        .unwrap_or(0);
    let observed = runtime_profile_latency_penalty(elapsed_ms, route_kind, stage);
    let next_score = if observed == 0 {
        current_score.saturating_sub(2)
    } else {
        (((current_score as u64) * 2) + (observed as u64)).div_ceil(3) as u32
    };
    let _ = update_runtime_profile_route_performance(
        shared,
        profile_name,
        route_kind,
        next_score,
        &format!("{stage}_{elapsed_ms}ms"),
    );
}

pub(super) fn note_runtime_profile_latency_failure(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
    stage: &str,
) {
    let current_score = shared
        .runtime
        .lock()
        .ok()
        .map(|runtime| {
            let now = Local::now().timestamp();
            runtime_profile_effective_score_from_map(
                &runtime.profile_health,
                &runtime_profile_route_performance_key(profile_name, route_kind),
                now,
                RUNTIME_PROFILE_PERFORMANCE_DECAY_SECONDS,
            )
        })
        .unwrap_or(0);
    let next_score = current_score
        .saturating_add(RUNTIME_PROFILE_TRANSPORT_FAILURE_HEALTH_PENALTY)
        .min(RUNTIME_PROFILE_LATENCY_PENALTY_MAX);
    let _ = update_runtime_profile_route_performance(
        shared,
        profile_name,
        route_kind,
        next_score,
        stage,
    );
}

pub(super) fn runtime_route_kind_label(route_kind: RuntimeRouteKind) -> &'static str {
    match route_kind {
        RuntimeRouteKind::Responses => "responses",
        RuntimeRouteKind::Compact => "compact",
        RuntimeRouteKind::Websocket => "websocket",
        RuntimeRouteKind::Standard => "standard",
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RuntimeTransportFailureKind {
    Dns,
    ConnectTimeout,
    ConnectRefused,
    ConnectReset,
    TlsHandshake,
    ConnectionAborted,
    BrokenPipe,
    UnexpectedEof,
    ReadTimeout,
    UpstreamClosedBeforeCommit,
    Other,
}

pub(super) fn runtime_transport_failure_kind_label(
    kind: RuntimeTransportFailureKind,
) -> &'static str {
    match kind {
        RuntimeTransportFailureKind::Dns => "dns",
        RuntimeTransportFailureKind::ConnectTimeout => "connect_timeout",
        RuntimeTransportFailureKind::ConnectRefused => "connection_refused",
        RuntimeTransportFailureKind::ConnectReset => "connection_reset",
        RuntimeTransportFailureKind::TlsHandshake => "tls_handshake",
        RuntimeTransportFailureKind::ConnectionAborted => "connection_aborted",
        RuntimeTransportFailureKind::BrokenPipe => "broken_pipe",
        RuntimeTransportFailureKind::UnexpectedEof => "unexpected_eof",
        RuntimeTransportFailureKind::ReadTimeout => "read_timeout",
        RuntimeTransportFailureKind::UpstreamClosedBeforeCommit => "upstream_closed_before_commit",
        RuntimeTransportFailureKind::Other => "other",
    }
}

pub(super) fn runtime_upstream_connect_failure_marker(
    failure_kind: Option<RuntimeTransportFailureKind>,
) -> &'static str {
    match failure_kind {
        Some(RuntimeTransportFailureKind::ConnectTimeout)
        | Some(RuntimeTransportFailureKind::ReadTimeout) => "upstream_connect_timeout",
        Some(RuntimeTransportFailureKind::Dns) => "upstream_connect_dns_error",
        Some(RuntimeTransportFailureKind::TlsHandshake) => "upstream_tls_handshake_error",
        _ => "upstream_connect_error",
    }
}

pub(super) fn log_runtime_upstream_connect_failure(
    shared: &RuntimeRotationProxyShared,
    request_id: u64,
    transport: &str,
    profile_name: &str,
    failure_kind: Option<RuntimeTransportFailureKind>,
    error: &impl std::fmt::Display,
) {
    runtime_proxy_log(
        shared,
        format!(
            "request={request_id} transport={transport} {} profile={profile_name} class={} error={error}",
            runtime_upstream_connect_failure_marker(failure_kind),
            failure_kind
                .map(runtime_transport_failure_kind_label)
                .unwrap_or("unknown"),
        ),
    );
}

pub(super) fn runtime_transport_failure_kind_from_message(
    message: &str,
) -> Option<RuntimeTransportFailureKind> {
    let message = message.to_ascii_lowercase();
    if message.contains("dns")
        || message.contains("failed to lookup address information")
        || message.contains("no such host")
        || message.contains("name or service not known")
    {
        Some(RuntimeTransportFailureKind::Dns)
    } else if message.contains("tls")
        || message.contains("handshake")
        || message.contains("certificate")
    {
        Some(RuntimeTransportFailureKind::TlsHandshake)
    } else if message.contains("connection refused") {
        Some(RuntimeTransportFailureKind::ConnectRefused)
    } else if message.contains("timed out") || message.contains("timeout") {
        Some(RuntimeTransportFailureKind::ConnectTimeout)
    } else if message.contains("connection reset") {
        Some(RuntimeTransportFailureKind::ConnectReset)
    } else if message.contains("broken pipe") {
        Some(RuntimeTransportFailureKind::BrokenPipe)
    } else if message.contains("unexpected eof") {
        Some(RuntimeTransportFailureKind::UnexpectedEof)
    } else if message.contains("connection aborted") {
        Some(RuntimeTransportFailureKind::ConnectionAborted)
    } else if message.contains("stream closed before response.completed")
        || message.contains("closed before response.completed")
    {
        Some(RuntimeTransportFailureKind::UpstreamClosedBeforeCommit)
    } else if message.contains("unable to connect") {
        Some(RuntimeTransportFailureKind::Other)
    } else {
        None
    }
}

pub(super) fn runtime_transport_failure_kind_from_io_error(
    err: &io::Error,
) -> Option<RuntimeTransportFailureKind> {
    match err.kind() {
        io::ErrorKind::TimedOut => Some(RuntimeTransportFailureKind::ConnectTimeout),
        io::ErrorKind::ConnectionRefused => Some(RuntimeTransportFailureKind::ConnectRefused),
        io::ErrorKind::ConnectionReset => Some(RuntimeTransportFailureKind::ConnectReset),
        io::ErrorKind::ConnectionAborted => Some(RuntimeTransportFailureKind::ConnectionAborted),
        io::ErrorKind::BrokenPipe => Some(RuntimeTransportFailureKind::BrokenPipe),
        io::ErrorKind::UnexpectedEof => Some(RuntimeTransportFailureKind::UnexpectedEof),
        _ => runtime_transport_failure_kind_from_message(&err.to_string()),
    }
}

pub(super) fn runtime_transport_failure_kind_from_reqwest(
    err: &reqwest::Error,
) -> Option<RuntimeTransportFailureKind> {
    if err.is_timeout() {
        return Some(RuntimeTransportFailureKind::ReadTimeout);
    }
    std::error::Error::source(err)
        .and_then(|source| source.downcast_ref::<io::Error>())
        .and_then(runtime_transport_failure_kind_from_io_error)
        .or_else(|| runtime_transport_failure_kind_from_message(&err.to_string()))
}

pub(super) fn runtime_transport_failure_kind_from_ws(
    err: &WsError,
) -> Option<RuntimeTransportFailureKind> {
    match err {
        WsError::Io(io) => runtime_transport_failure_kind_from_io_error(io),
        WsError::Tls(_) => Some(RuntimeTransportFailureKind::TlsHandshake),
        WsError::ConnectionClosed | WsError::AlreadyClosed => {
            Some(RuntimeTransportFailureKind::UpstreamClosedBeforeCommit)
        }
        _ => runtime_transport_failure_kind_from_message(&err.to_string()),
    }
}

pub(super) fn runtime_proxy_transport_failure_kind(
    err: &anyhow::Error,
) -> Option<RuntimeTransportFailureKind> {
    for cause in err.chain() {
        if let Some(reqwest_error) = cause.downcast_ref::<reqwest::Error>()
            && let Some(kind) = runtime_transport_failure_kind_from_reqwest(reqwest_error)
        {
            return Some(kind);
        }
        if let Some(ws_error) = cause.downcast_ref::<WsError>()
            && let Some(kind) = runtime_transport_failure_kind_from_ws(ws_error)
        {
            return Some(kind);
        }
        if let Some(io_error) = cause.downcast_ref::<io::Error>()
            && let Some(kind) = runtime_transport_failure_kind_from_io_error(io_error)
        {
            return Some(kind);
        }
        if let Some(kind) = runtime_transport_failure_kind_from_message(&cause.to_string()) {
            return Some(kind);
        }
    }
    None
}

pub(super) fn runtime_profile_transport_health_penalty(kind: RuntimeTransportFailureKind) -> u32 {
    match kind {
        RuntimeTransportFailureKind::Dns
        | RuntimeTransportFailureKind::ConnectTimeout
        | RuntimeTransportFailureKind::ConnectRefused
        | RuntimeTransportFailureKind::ConnectReset
        | RuntimeTransportFailureKind::TlsHandshake => {
            RUNTIME_PROFILE_CONNECT_FAILURE_HEALTH_PENALTY
        }
        RuntimeTransportFailureKind::BrokenPipe
        | RuntimeTransportFailureKind::ConnectionAborted
        | RuntimeTransportFailureKind::UnexpectedEof
        | RuntimeTransportFailureKind::ReadTimeout
        | RuntimeTransportFailureKind::UpstreamClosedBeforeCommit
        | RuntimeTransportFailureKind::Other => RUNTIME_PROFILE_TRANSPORT_FAILURE_HEALTH_PENALTY,
    }
}

pub(super) fn reset_runtime_profile_success_streak(
    runtime: &mut RuntimeRotationState,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) {
    runtime
        .profile_health
        .remove(&runtime_profile_route_success_streak_key(
            profile_name,
            route_kind,
        ));
}

pub(super) fn bump_runtime_profile_bad_pairing_score(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
    delta: u32,
    reason: &str,
) -> Result<()> {
    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let now = Local::now().timestamp();
    let key = runtime_profile_route_bad_pairing_key(profile_name, route_kind);
    let next_score = runtime_profile_effective_score_from_map(
        &runtime.profile_health,
        &key,
        now,
        RUNTIME_PROFILE_BAD_PAIRING_DECAY_SECONDS,
    )
    .saturating_add(delta)
    .min(RUNTIME_PROFILE_HEALTH_MAX_SCORE);
    reset_runtime_profile_success_streak(&mut runtime, profile_name, route_kind);
    runtime.profile_health.insert(
        key,
        RuntimeProfileHealth {
            score: next_score,
            updated_at: now,
        },
    );
    schedule_runtime_state_save_from_runtime(
        shared,
        &runtime,
        &format!(
            "profile_bad_pairing:{profile_name}:{}",
            runtime_route_kind_label(route_kind)
        ),
    );
    drop(runtime);
    runtime_proxy_log(
        shared,
        format!(
            "profile_bad_pairing profile={profile_name} route={} score={next_score} delta={delta} reason={reason}",
            runtime_route_kind_label(route_kind)
        ),
    );
    Ok(())
}

pub(super) fn bump_runtime_profile_health_score(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
    delta: u32,
    reason: &str,
) -> Result<()> {
    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let now = Local::now().timestamp();
    let key = runtime_profile_route_health_key(profile_name, route_kind);
    let next_score = runtime
        .profile_health
        .get(&key)
        .map(|entry| runtime_profile_effective_health_score(entry, now))
        .unwrap_or(0)
        .saturating_add(delta)
        .min(RUNTIME_PROFILE_HEALTH_MAX_SCORE);
    reset_runtime_profile_success_streak(&mut runtime, profile_name, route_kind);
    runtime.profile_health.insert(
        key,
        RuntimeProfileHealth {
            score: next_score,
            updated_at: now,
        },
    );
    let circuit_until = if next_score >= RUNTIME_PROFILE_CIRCUIT_OPEN_THRESHOLD {
        let circuit_key = runtime_profile_route_circuit_key(profile_name, route_kind);
        let reopen_stage = if runtime
            .profile_route_circuit_open_until
            .contains_key(&circuit_key)
        {
            runtime_profile_effective_score_from_map(
                &runtime.profile_health,
                &runtime_profile_route_circuit_reopen_key(profile_name, route_kind),
                now,
                RUNTIME_PROFILE_CIRCUIT_REOPEN_DECAY_SECONDS,
            )
            .saturating_add(1)
            .min(RUNTIME_PROFILE_CIRCUIT_REOPEN_MAX_STAGE)
        } else {
            0
        };
        if reopen_stage == 0 {
            runtime
                .profile_health
                .remove(&runtime_profile_route_circuit_reopen_key(
                    profile_name,
                    route_kind,
                ));
        } else {
            runtime.profile_health.insert(
                runtime_profile_route_circuit_reopen_key(profile_name, route_kind),
                RuntimeProfileHealth {
                    score: reopen_stage,
                    updated_at: now,
                },
            );
        }
        let until = now.saturating_add(runtime_profile_circuit_open_seconds(
            next_score,
            reopen_stage,
        ));
        runtime
            .profile_route_circuit_open_until
            .entry(circuit_key)
            .and_modify(|current| *current = (*current).max(until))
            .or_insert(until);
        Some((until, reopen_stage))
    } else {
        None
    };
    schedule_runtime_state_save_from_runtime(
        shared,
        &runtime,
        &format!(
            "profile_health:{profile_name}:{}",
            runtime_route_kind_label(route_kind)
        ),
    );
    drop(runtime);
    runtime_proxy_log(
        shared,
        format!(
            "profile_health profile={profile_name} route={} score={next_score} delta={delta} reason={reason}",
            runtime_route_kind_label(route_kind)
        ),
    );
    if let Some((until, reopen_stage)) = circuit_until {
        runtime_proxy_log(
            shared,
            format!(
                "profile_circuit_open profile={profile_name} route={} until={until} reopen_stage={reopen_stage} reason={reason} score={next_score}",
                runtime_route_kind_label(route_kind)
            ),
        );
    }
    Ok(())
}

pub(super) fn recover_runtime_profile_health_for_route(
    runtime: &mut RuntimeRotationState,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
    now: i64,
) {
    let key = runtime_profile_route_health_key(profile_name, route_kind);
    let streak_key = runtime_profile_route_success_streak_key(profile_name, route_kind);
    let Some(current_score) = runtime
        .profile_health
        .get(&key)
        .map(|entry| runtime_profile_effective_health_score(entry, now))
    else {
        runtime.profile_health.remove(&streak_key);
        return;
    };

    let next_streak = runtime_profile_effective_score_from_map(
        &runtime.profile_health,
        &streak_key,
        now,
        RUNTIME_PROFILE_SUCCESS_STREAK_DECAY_SECONDS,
    )
    .saturating_add(1)
    .min(RUNTIME_PROFILE_SUCCESS_STREAK_MAX);
    let recovery = RUNTIME_PROFILE_HEALTH_SUCCESS_RECOVERY_SCORE
        .saturating_add(next_streak.saturating_sub(1).min(1));
    let next_score = current_score.saturating_sub(recovery);
    if next_score == 0 {
        runtime.profile_health.remove(&key);
        runtime.profile_health.remove(&streak_key);
    } else {
        runtime.profile_health.insert(
            key,
            RuntimeProfileHealth {
                score: next_score,
                updated_at: now,
            },
        );
        runtime.profile_health.insert(
            streak_key,
            RuntimeProfileHealth {
                score: next_streak,
                updated_at: now,
            },
        );
    }
}

pub(super) fn mark_runtime_profile_retry_backoff(
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
        format!("profile_retry_backoff profile={profile_name} until={until}"),
    );
    Ok(())
}

pub(super) fn mark_runtime_profile_transport_backoff(
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
        format!(
            "profile_transport_backoff profile={profile_name} route={} until={until} seconds={next_backoff_seconds} context={context}",
            runtime_route_kind_label(route_kind)
        ),
    );
    Ok(())
}

pub(super) fn note_runtime_profile_transport_failure(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
    context: &str,
    err: &anyhow::Error,
) {
    let Some(failure_kind) = runtime_proxy_transport_failure_kind(err) else {
        return;
    };
    runtime_proxy_log(
        shared,
        format!(
            "profile_transport_failure profile={profile_name} route={} class={} context={context}",
            runtime_route_kind_label(route_kind),
            runtime_transport_failure_kind_label(failure_kind),
        ),
    );
    let _ = bump_runtime_profile_health_score(
        shared,
        profile_name,
        route_kind,
        runtime_profile_transport_health_penalty(failure_kind),
        context,
    );
    let _ = bump_runtime_profile_bad_pairing_score(
        shared,
        profile_name,
        route_kind,
        RUNTIME_PROFILE_BAD_PAIRING_PENALTY,
        context,
    );
    note_runtime_profile_latency_failure(shared, profile_name, route_kind, context);
    let _ = mark_runtime_profile_transport_backoff(shared, profile_name, route_kind, context);
}

pub(super) fn clear_runtime_profile_transport_backoff_for_route(
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

pub(super) fn commit_runtime_proxy_profile_selection(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) -> Result<bool> {
    commit_runtime_proxy_profile_selection_with_policy(shared, profile_name, route_kind, true)
}

pub(super) fn commit_runtime_proxy_profile_selection_with_policy(
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

pub(super) fn clear_runtime_profile_health_for_route(
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

pub(super) fn commit_runtime_proxy_profile_selection_with_notice(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) -> Result<()> {
    let _ = commit_runtime_proxy_profile_selection(shared, profile_name, route_kind)?;
    Ok(())
}

pub(super) fn send_runtime_proxy_upstream_request(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    turn_state_override: Option<&str>,
) -> Result<reqwest::Response> {
    let started_at = Instant::now();
    let runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?
        .clone();
    let auth = runtime_profile_usage_auth(shared, profile_name)?;
    let upstream_url =
        runtime_proxy_upstream_url(&runtime.upstream_base_url, &request.path_and_query);
    let method = reqwest::Method::from_bytes(request.method.as_bytes()).with_context(|| {
        format!(
            "failed to proxy unsupported HTTP method '{}' for runtime auto-rotate",
            request.method
        )
    })?;

    let mut upstream_request = shared.async_client.request(method, &upstream_url);
    for (name, value) in &request.headers {
        if turn_state_override.is_some() && name.eq_ignore_ascii_case("x-codex-turn-state") {
            continue;
        }
        if should_skip_runtime_request_header(name) {
            continue;
        }
        upstream_request = upstream_request.header(name.as_str(), value.as_str());
    }
    if let Some(turn_state) = turn_state_override {
        upstream_request = upstream_request.header("x-codex-turn-state", turn_state);
    }

    upstream_request = upstream_request
        .header("Authorization", format!("Bearer {}", auth.access_token))
        .body(request.body.clone());

    if let Some(user_agent) = runtime_proxy_effective_user_agent(&request.headers) {
        upstream_request = upstream_request.header("User-Agent", user_agent);
    } else {
        upstream_request = upstream_request.header("User-Agent", "codex-cli");
    }

    if let Some(account_id) = auth.account_id.as_deref() {
        upstream_request = upstream_request.header("ChatGPT-Account-Id", account_id);
    }

    runtime_proxy_log(
        shared,
        format!(
            "request={request_id} transport=http upstream_start profile={profile_name} method={} url={} turn_state_override={:?} previous_response_id={:?}",
            request.method,
            upstream_url,
            turn_state_override,
            runtime_request_previous_response_id(request)
        ),
    );
    if runtime_take_fault_injection("PRODEX_RUNTIME_FAULT_UPSTREAM_CONNECT_ERROR_ONCE") {
        bail!("injected runtime upstream connect failure");
    }
    let response = match shared
        .async_runtime
        .block_on(async move { upstream_request.send().await })
    {
        Ok(response) => response,
        Err(err) => {
            log_runtime_upstream_connect_failure(
                shared,
                request_id,
                "http",
                profile_name,
                runtime_transport_failure_kind_from_reqwest(&err),
                &err,
            );
            return Err(anyhow::Error::new(err).context(format!(
                "failed to proxy runtime request for profile '{}' to {}",
                profile_name, upstream_url
            )));
        }
    };
    runtime_proxy_log(
        shared,
        format!(
            "request={request_id} transport=http upstream_response profile={profile_name} status={} content_type={:?} turn_state={:?}",
            response.status().as_u16(),
            response
                .headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok()),
            runtime_proxy_header_value(response.headers(), "x-codex-turn-state")
        ),
    );
    note_runtime_profile_latency_observation(
        shared,
        profile_name,
        runtime_proxy_request_lane(&request.path_and_query, false),
        "connect",
        started_at.elapsed().as_millis() as u64,
    );
    Ok(response)
}

pub(super) fn send_runtime_proxy_upstream_responses_request(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    turn_state_override: Option<&str>,
) -> Result<reqwest::Response> {
    let started_at = Instant::now();
    let runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?
        .clone();
    let auth = runtime_profile_usage_auth(shared, profile_name)?;
    let upstream_url =
        runtime_proxy_upstream_url(&runtime.upstream_base_url, &request.path_and_query);
    let method = reqwest::Method::from_bytes(request.method.as_bytes()).with_context(|| {
        format!(
            "failed to proxy unsupported HTTP method '{}' for runtime auto-rotate",
            request.method
        )
    })?;

    let mut upstream_request = shared.async_client.request(method, &upstream_url);
    for (name, value) in &request.headers {
        if turn_state_override.is_some() && name.eq_ignore_ascii_case("x-codex-turn-state") {
            continue;
        }
        if should_skip_runtime_request_header(name) {
            continue;
        }
        upstream_request = upstream_request.header(name.as_str(), value.as_str());
    }
    if let Some(turn_state) = turn_state_override {
        upstream_request = upstream_request.header("x-codex-turn-state", turn_state);
    }

    upstream_request = upstream_request
        .header("Authorization", format!("Bearer {}", auth.access_token))
        .body(request.body.clone());

    if let Some(user_agent) = runtime_proxy_effective_user_agent(&request.headers) {
        upstream_request = upstream_request.header("User-Agent", user_agent);
    } else {
        upstream_request = upstream_request.header("User-Agent", "codex-cli");
    }

    if let Some(account_id) = auth.account_id.as_deref() {
        upstream_request = upstream_request.header("ChatGPT-Account-Id", account_id);
    }

    runtime_proxy_log(
        shared,
        format!(
            "request={request_id} transport=http upstream_async_start profile={profile_name} method={} url={} turn_state_override={:?} previous_response_id={:?}",
            request.method,
            upstream_url,
            turn_state_override,
            runtime_request_previous_response_id(request)
        ),
    );
    if runtime_take_fault_injection("PRODEX_RUNTIME_FAULT_UPSTREAM_CONNECT_ERROR_ONCE") {
        bail!("injected runtime upstream connect failure");
    }
    let response = match shared
        .async_runtime
        .block_on(async move { upstream_request.send().await })
    {
        Ok(response) => response,
        Err(err) => {
            log_runtime_upstream_connect_failure(
                shared,
                request_id,
                "http",
                profile_name,
                runtime_transport_failure_kind_from_reqwest(&err),
                &err,
            );
            return Err(anyhow::Error::new(err).context(format!(
                "failed to proxy runtime request for profile '{}' to {}",
                profile_name, upstream_url
            )));
        }
    };
    runtime_proxy_log(
        shared,
        format!(
            "request={request_id} transport=http upstream_async_response profile={profile_name} status={} content_type={:?} turn_state={:?}",
            response.status().as_u16(),
            response
                .headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok()),
            runtime_proxy_header_value(response.headers(), "x-codex-turn-state")
        ),
    );
    note_runtime_profile_latency_observation(
        shared,
        profile_name,
        RuntimeRouteKind::Responses,
        "connect",
        started_at.elapsed().as_millis() as u64,
    );
    Ok(response)
}

pub(super) fn runtime_proxy_upstream_url(base_url: &str, path_and_query: &str) -> String {
    let base_url = base_url.trim_end_matches('/');
    let normalized_path_and_query = runtime_proxy_normalize_openai_path(path_and_query);
    if base_url.contains("/backend-api")
        && let Some(suffix) = normalized_path_and_query
            .as_ref()
            .strip_prefix("/backend-api")
    {
        return format!("{base_url}{suffix}");
    }
    if normalized_path_and_query.starts_with('/') {
        return format!("{base_url}{normalized_path_and_query}");
    }
    format!("{base_url}/{normalized_path_and_query}")
}

pub(super) fn runtime_proxy_upstream_websocket_url(
    base_url: &str,
    path_and_query: &str,
) -> Result<String> {
    let upstream_url = runtime_proxy_upstream_url(base_url, path_and_query);
    let mut url = reqwest::Url::parse(&upstream_url)
        .with_context(|| format!("failed to parse upstream websocket URL {}", upstream_url))?;
    match url.scheme() {
        "http" => {
            url.set_scheme("ws").map_err(|_| {
                anyhow::anyhow!("failed to set websocket scheme for {upstream_url}")
            })?;
        }
        "https" => {
            url.set_scheme("wss").map_err(|_| {
                anyhow::anyhow!("failed to set websocket scheme for {upstream_url}")
            })?;
        }
        "ws" | "wss" => {}
        scheme => bail!(
            "unsupported upstream websocket scheme '{scheme}' in {}",
            upstream_url
        ),
    }
    Ok(url.to_string())
}

pub(super) fn should_skip_runtime_request_header(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    matches!(
        lower.as_str(),
        "authorization"
            | "chatgpt-account-id"
            | "connection"
            | "content-length"
            | "host"
            | "transfer-encoding"
            | "upgrade"
    ) || lower.starts_with("sec-websocket-")
        || lower.starts_with("x-prodex-internal-")
}

pub(super) fn runtime_proxy_effective_user_agent(headers: &[(String, String)]) -> Option<&str> {
    headers.iter().find_map(|(name, value)| {
        name.eq_ignore_ascii_case("user-agent")
            .then_some(value.as_str())
            .filter(|value| !value.is_empty())
    })
}

pub(super) fn should_skip_runtime_response_header(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "connection"
            | "content-encoding"
            | "content-length"
            | "date"
            | "server"
            | "transfer-encoding"
    )
}

pub(super) fn forward_runtime_proxy_response(
    shared: &RuntimeRotationProxyShared,
    response: reqwest::Response,
    prelude: Vec<u8>,
) -> Result<tiny_http::ResponseBox> {
    let parts = buffer_runtime_proxy_async_response_parts(shared, response, prelude)?;
    Ok(build_runtime_proxy_response_from_parts(parts))
}

pub(super) fn forward_runtime_proxy_response_with_limit(
    shared: &RuntimeRotationProxyShared,
    response: reqwest::Response,
    prelude: Vec<u8>,
    max_bytes: usize,
) -> Result<tiny_http::ResponseBox> {
    let parts =
        buffer_runtime_proxy_async_response_parts_with_limit(shared, response, prelude, max_bytes)?;
    Ok(build_runtime_proxy_response_from_parts(parts))
}

pub(super) struct RuntimeResponsesSuccessContext<'a> {
    request_id: u64,
    request_previous_response_id: Option<&'a str>,
    request_session_id: Option<&'a str>,
    request_turn_state: Option<&'a str>,
    turn_state_override: Option<&'a str>,
    shared: &'a RuntimeRotationProxyShared,
    profile_name: &'a str,
    inflight_guard: RuntimeProfileInFlightGuard,
}

pub(super) fn prepare_runtime_proxy_responses_success(
    context: RuntimeResponsesSuccessContext<'_>,
    response: reqwest::Response,
) -> Result<RuntimeResponsesAttempt> {
    let RuntimeResponsesSuccessContext {
        request_id,
        request_previous_response_id,
        request_session_id,
        request_turn_state,
        turn_state_override,
        shared,
        profile_name,
        inflight_guard,
    } = context;

    let response_header_turn_state =
        runtime_proxy_header_value(response.headers(), "x-codex-turn-state");
    remember_runtime_successful_previous_response_owner(
        shared,
        profile_name,
        request_previous_response_id,
        RuntimeRouteKind::Responses,
    )?;
    remember_runtime_session_id(
        shared,
        profile_name,
        request_session_id,
        RuntimeRouteKind::Responses,
    )?;
    let is_sse = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.contains("text/event-stream"));
    runtime_proxy_log(
        shared,
        format!(
            "request={request_id} transport=http prepare_success profile={profile_name} sse={is_sse} turn_state={:?}",
            response_header_turn_state
        ),
    );
    if !is_sse {
        let buffered_started_at = Instant::now();
        let parts = buffer_runtime_proxy_async_response_parts(shared, response, Vec::new())?;
        let response_turn_state = response_header_turn_state
            .or_else(|| turn_state_override.map(str::to_string))
            .or_else(|| extract_runtime_turn_state_from_body_bytes(&parts.body));
        remember_runtime_turn_state(
            shared,
            profile_name,
            response_turn_state.as_deref(),
            RuntimeRouteKind::Responses,
        )?;
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=http buffered_response_complete profile={profile_name} phase=responses_unary status={} content_type={} body_bytes={} elapsed_ms={}",
                parts.status,
                runtime_buffered_response_content_type(&parts).unwrap_or("-"),
                parts.body.len(),
                buffered_started_at.elapsed().as_millis(),
            ),
        );
        let response_ids = extract_runtime_response_ids_from_body_bytes(&parts.body);
        if !response_ids.is_empty() {
            remember_runtime_response_ids_with_turn_state(
                shared,
                profile_name,
                &response_ids,
                response_turn_state.as_deref(),
                RuntimeRouteKind::Responses,
            )?;
            let _ = release_runtime_compact_lineage(
                shared,
                profile_name,
                request_session_id,
                request_turn_state,
                "response_committed",
            );
        }
        return Ok(RuntimeResponsesAttempt::Success {
            profile_name: profile_name.to_string(),
            response: RuntimeResponsesReply::Buffered(parts),
        });
    }

    let status = response.status().as_u16();
    let mut headers = Vec::new();
    for (name, value) in response.headers() {
        if should_skip_runtime_response_header(name.as_str()) {
            continue;
        }
        if let Ok(value) = value.to_str() {
            headers.push((name.to_string(), value.to_string()));
        }
    }

    let mut prefetch = RuntimePrefetchStream::spawn(
        response,
        Arc::clone(&shared.async_runtime),
        shared.log_path.clone(),
        request_id,
    );
    let lookahead = inspect_runtime_sse_lookahead(&mut prefetch, &shared.log_path, request_id)?;

    let (prelude, response_ids, lookahead_turn_state) = match lookahead {
        RuntimeSseInspection::Commit {
            prelude,
            response_ids,
            turn_state,
        } => {
            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} transport=http sse_commit profile={profile_name} prelude_bytes={} response_ids={}",
                    prelude.len(),
                    response_ids.len()
                ),
            );
            (prelude, response_ids, turn_state)
        }
        RuntimeSseInspection::QuotaBlocked(prelude) => {
            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} transport=http sse_quota_blocked profile={profile_name} prelude_bytes={}",
                    prelude.len()
                ),
            );
            return Ok(RuntimeResponsesAttempt::QuotaBlocked {
                profile_name: profile_name.to_string(),
                response: RuntimeResponsesReply::Streaming(RuntimeStreamingResponse {
                    status,
                    headers: headers.clone(),
                    body: Box::new(prefetch.into_reader(prelude)),
                    request_id,
                    profile_name: profile_name.to_string(),
                    log_path: shared.log_path.clone(),
                    shared: shared.clone(),
                    _inflight_guard: Some(inflight_guard),
                }),
            });
        }
        RuntimeSseInspection::PreviousResponseNotFound(prelude) => {
            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} transport=http route=responses previous_response_not_found profile={profile_name} stage=sse_prelude prelude_bytes={}",
                    prelude.len()
                ),
            );
            return Ok(RuntimeResponsesAttempt::PreviousResponseNotFound {
                profile_name: profile_name.to_string(),
                response: RuntimeResponsesReply::Streaming(RuntimeStreamingResponse {
                    status,
                    headers: headers.clone(),
                    body: Box::new(prefetch.into_reader(prelude)),
                    request_id,
                    profile_name: profile_name.to_string(),
                    log_path: shared.log_path.clone(),
                    shared: shared.clone(),
                    _inflight_guard: Some(inflight_guard),
                }),
                turn_state: response_header_turn_state,
            });
        }
    };
    let response_turn_state = response_header_turn_state
        .or_else(|| turn_state_override.map(str::to_string))
        .or(lookahead_turn_state);
    remember_runtime_turn_state(
        shared,
        profile_name,
        response_turn_state.as_deref(),
        RuntimeRouteKind::Responses,
    )?;
    remember_runtime_response_ids_with_turn_state(
        shared,
        profile_name,
        &response_ids,
        response_turn_state.as_deref(),
        RuntimeRouteKind::Responses,
    )?;
    if !response_ids.is_empty() {
        let _ = release_runtime_compact_lineage(
            shared,
            profile_name,
            request_session_id,
            request_turn_state,
            "response_committed",
        );
    }

    Ok(RuntimeResponsesAttempt::Success {
        profile_name: profile_name.to_string(),
        response: RuntimeResponsesReply::Streaming(RuntimeStreamingResponse {
            status,
            headers,
            body: Box::new(RuntimeSseTapReader::new(
                prefetch.into_reader(prelude.clone()),
                shared.clone(),
                profile_name.to_string(),
                &prelude,
                &response_ids,
                response_turn_state.as_deref(),
            )),
            request_id,
            profile_name: profile_name.to_string(),
            log_path: shared.log_path.clone(),
            shared: shared.clone(),
            _inflight_guard: Some(inflight_guard),
        }),
    })
}

impl RuntimeSseTapState {
    fn observe(&mut self, shared: &RuntimeRotationProxyShared, profile_name: &str, chunk: &[u8]) {
        for byte in chunk {
            self.line.push(*byte);
            if *byte != b'\n' {
                continue;
            }

            let line_text = String::from_utf8_lossy(&self.line);
            let trimmed = line_text.trim_end_matches(['\r', '\n']);
            if trimmed.is_empty() {
                let event = parse_runtime_sse_event(&self.data_lines);
                if let Some(turn_state) = event.turn_state {
                    self.turn_state = Some(turn_state);
                }
                self.remember_response_ids(
                    shared,
                    profile_name,
                    &event.response_ids,
                    RuntimeRouteKind::Responses,
                );
                self.data_lines.clear();
                self.line.clear();
                continue;
            }

            if let Some(payload) = trimmed.strip_prefix("data:") {
                self.data_lines.push(payload.trim_start().to_string());
            }
            self.line.clear();
        }
    }

    fn finish(&mut self, shared: &RuntimeRotationProxyShared, profile_name: &str) {
        let event = parse_runtime_sse_event(&self.data_lines);
        if let Some(turn_state) = event.turn_state {
            self.turn_state = Some(turn_state);
        }
        self.remember_response_ids(
            shared,
            profile_name,
            &event.response_ids,
            RuntimeRouteKind::Responses,
        );
    }

    fn remember_response_ids(
        &mut self,
        shared: &RuntimeRotationProxyShared,
        profile_name: &str,
        response_ids: &[String],
        verified_route: RuntimeRouteKind,
    ) {
        let fresh_ids = response_ids
            .iter()
            .filter(|response_id| self.remembered_response_ids.insert((*response_id).clone()))
            .cloned()
            .collect::<Vec<_>>();
        let response_ids_needing_turn_state = self
            .turn_state
            .as_deref()
            .map(|_| {
                self.remembered_response_ids
                    .iter()
                    .filter(|response_id| {
                        self.response_ids_with_turn_state
                            .insert((*response_id).clone())
                    })
                    .cloned()
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        if fresh_ids.is_empty() && response_ids_needing_turn_state.is_empty() {
            return;
        }
        if !fresh_ids.is_empty() {
            let _ = remember_runtime_response_ids_with_turn_state(
                shared,
                profile_name,
                &fresh_ids,
                self.turn_state.as_deref(),
                verified_route,
            );
        }
        if !response_ids_needing_turn_state.is_empty() {
            let rebound_ids = response_ids_needing_turn_state
                .into_iter()
                .filter(|response_id| !fresh_ids.contains(response_id))
                .collect::<Vec<_>>();
            if !rebound_ids.is_empty() {
                let _ = remember_runtime_response_ids_with_turn_state(
                    shared,
                    profile_name,
                    &rebound_ids,
                    self.turn_state.as_deref(),
                    verified_route,
                );
            }
        }
    }
}

pub(super) struct RuntimeSseTapReader {
    inner: Box<dyn Read + Send>,
    shared: RuntimeRotationProxyShared,
    profile_name: String,
    state: RuntimeSseTapState,
}

impl Read for RuntimePrefetchReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.finished {
            return Ok(0);
        }

        loop {
            let read = self.pending.read(buf)?;
            if read > 0 {
                return Ok(read);
            }

            let next = if let Some(chunk) = self.backlog.pop_front() {
                Some(chunk)
            } else {
                match self
                    .receiver
                    .recv_timeout(Duration::from_millis(runtime_proxy_stream_idle_timeout_ms()))
                {
                    Ok(chunk) => {
                        if let RuntimePrefetchChunk::Data(bytes) = &chunk {
                            runtime_prefetch_release_queued_bytes(&self.shared, bytes.len());
                        }
                        Some(chunk)
                    }
                    Err(RecvTimeoutError::Timeout) => {
                        self.finished = true;
                        return Err(io::Error::new(
                            io::ErrorKind::TimedOut,
                            "runtime upstream stream idle timed out",
                        ));
                    }
                    Err(RecvTimeoutError::Disconnected) => {
                        if let Some((kind, message)) = runtime_prefetch_terminal_error(&self.shared)
                        {
                            self.finished = true;
                            return Err(io::Error::new(kind, message));
                        }
                        None
                    }
                }
            };

            match next {
                Some(RuntimePrefetchChunk::Data(chunk)) => {
                    self.pending = Cursor::new(chunk);
                }
                Some(RuntimePrefetchChunk::End) | None => {
                    self.finished = true;
                    return Ok(0);
                }
                Some(RuntimePrefetchChunk::Error(kind, message)) => {
                    self.finished = true;
                    return Err(io::Error::new(kind, message));
                }
            }
        }
    }
}

impl Drop for RuntimePrefetchReader {
    fn drop(&mut self) {
        self.worker_abort.abort();
    }
}

impl RuntimeSseTapReader {
    pub(super) fn new(
        inner: impl Read + Send + 'static,
        shared: RuntimeRotationProxyShared,
        profile_name: String,
        prelude: &[u8],
        remembered_response_ids: &[String],
        turn_state: Option<&str>,
    ) -> Self {
        let mut state = RuntimeSseTapState {
            remembered_response_ids: remembered_response_ids.iter().cloned().collect(),
            response_ids_with_turn_state: turn_state
                .map(|_| remembered_response_ids.iter().cloned().collect())
                .unwrap_or_default(),
            turn_state: turn_state.map(str::to_string),
            ..RuntimeSseTapState::default()
        };
        state.observe(&shared, &profile_name, prelude);
        Self {
            inner: Box::new(inner),
            shared,
            profile_name,
            state,
        }
    }
}

impl Read for RuntimeSseTapReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let read = match self.inner.read(buf) {
            Ok(read) => read,
            Err(err) => {
                let transport_error =
                    anyhow::Error::new(io::Error::new(err.kind(), err.to_string()));
                note_runtime_profile_transport_failure(
                    &self.shared,
                    &self.profile_name,
                    RuntimeRouteKind::Responses,
                    "sse_read",
                    &transport_error,
                );
                return Err(err);
            }
        };
        if read == 0 {
            self.state.finish(&self.shared, &self.profile_name);
            return Ok(0);
        }
        self.state
            .observe(&self.shared, &self.profile_name, &buf[..read]);
        Ok(read)
    }
}

pub(super) fn write_runtime_streaming_response(
    writer: Box<dyn Write + Send + 'static>,
    mut response: RuntimeStreamingResponse,
) -> io::Result<()> {
    let mut writer = writer;
    let flush_each_chunk = response.headers.iter().any(|(name, value)| {
        name.eq_ignore_ascii_case("content-type")
            && value.to_ascii_lowercase().contains("text/event-stream")
    });
    let started_at = Instant::now();
    let log_writer_error = |stage: &str,
                            chunk_count: usize,
                            total_bytes: usize,
                            err: &io::Error| {
        runtime_proxy_log_to_path(
            &response.log_path,
            &format!(
                "local_writer_error request={} transport=http profile={} stage={} chunks={} bytes={} elapsed_ms={} error={}",
                response.request_id,
                response.profile_name,
                stage,
                chunk_count,
                total_bytes,
                started_at.elapsed().as_millis(),
                err
            ),
        );
    };
    runtime_proxy_log_to_path(
        &response.log_path,
        &format!(
            "request={} transport=http stream_start profile={} status={}",
            response.request_id, response.profile_name, response.status
        ),
    );
    let status = reqwest::StatusCode::from_u16(response.status)
        .ok()
        .and_then(|status| status.canonical_reason().map(str::to_string))
        .unwrap_or_else(|| "OK".to_string());
    write!(
        writer,
        "HTTP/1.1 {} {}\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n",
        response.status, status
    )
    .map_err(|err| {
        log_writer_error("headers_start", 0, 0, &err);
        err
    })?;
    for (name, value) in response.headers {
        write!(writer, "{name}: {value}\r\n").map_err(|err| {
            log_writer_error("header_line", 0, 0, &err);
            err
        })?;
    }
    writer.write_all(b"\r\n").inspect_err(|err| {
        log_writer_error("headers_end", 0, 0, err);
    })?;
    writer.flush().inspect_err(|err| {
        log_writer_error("headers_flush", 0, 0, err);
    })?;

    let mut buffer = [0_u8; 8192];
    let mut total_bytes = 0usize;
    let mut chunk_count = 0usize;
    loop {
        let read = match response.body.read(&mut buffer) {
            Ok(read) => read,
            Err(err) => {
                runtime_proxy_log_to_path(
                    &response.log_path,
                    &format!(
                        "request={} transport=http stream_read_error profile={} chunks={} bytes={} elapsed_ms={} error={}",
                        response.request_id,
                        response.profile_name,
                        chunk_count,
                        total_bytes,
                        started_at.elapsed().as_millis(),
                        err
                    ),
                );
                let transport_error =
                    anyhow::Error::new(io::Error::new(err.kind(), err.to_string()));
                if is_runtime_proxy_transport_failure(&transport_error) {
                    note_runtime_profile_latency_failure(
                        &response.shared,
                        &response.profile_name,
                        RuntimeRouteKind::Responses,
                        "stream_read_error",
                    );
                }
                return Err(err);
            }
        };
        if read == 0 {
            break;
        }
        if chunk_count == 0
            && runtime_take_fault_injection("PRODEX_RUNTIME_FAULT_STREAM_READ_ERROR_ONCE")
        {
            let err = io::Error::new(
                io::ErrorKind::ConnectionReset,
                "injected runtime stream read failure",
            );
            runtime_proxy_log_to_path(
                &response.log_path,
                &format!(
                    "request={} transport=http stream_read_error profile={} chunks={} bytes={} elapsed_ms={} error={}",
                    response.request_id,
                    response.profile_name,
                    chunk_count,
                    total_bytes,
                    started_at.elapsed().as_millis(),
                    err
                ),
            );
            note_runtime_profile_latency_failure(
                &response.shared,
                &response.profile_name,
                RuntimeRouteKind::Responses,
                "stream_read_error",
            );
            return Err(err);
        }
        chunk_count += 1;
        total_bytes += read;
        if chunk_count == 1 {
            runtime_proxy_log_to_path(
                &response.log_path,
                &format!(
                    "request={} transport=http first_local_chunk profile={} bytes={} elapsed_ms={}",
                    response.request_id,
                    response.profile_name,
                    read,
                    started_at.elapsed().as_millis()
                ),
            );
            note_runtime_profile_latency_observation(
                &response.shared,
                &response.profile_name,
                RuntimeRouteKind::Responses,
                "ttfb",
                started_at.elapsed().as_millis() as u64,
            );
        }
        write!(writer, "{:X}\r\n", read).map_err(|err| {
            log_writer_error("chunk_size", chunk_count, total_bytes, &err);
            err
        })?;
        writer.write_all(&buffer[..read]).inspect_err(|err| {
            log_writer_error("chunk_body", chunk_count, total_bytes, err);
        })?;
        writer.write_all(b"\r\n").inspect_err(|err| {
            log_writer_error("chunk_suffix", chunk_count, total_bytes, err);
        })?;
        if flush_each_chunk || chunk_count == 1 {
            writer.flush().inspect_err(|err| {
                log_writer_error("chunk_flush", chunk_count, total_bytes, err);
            })?;
        }
    }
    writer.write_all(b"0\r\n\r\n").inspect_err(|err| {
        log_writer_error("trailer", chunk_count, total_bytes, err);
    })?;
    writer.flush().inspect_err(|err| {
        log_writer_error("trailer_flush", chunk_count, total_bytes, err);
    })?;
    runtime_proxy_log_to_path(
        &response.log_path,
        &format!(
            "request={} transport=http stream_complete profile={} chunks={} bytes={} elapsed_ms={}",
            response.request_id,
            response.profile_name,
            chunk_count,
            total_bytes,
            started_at.elapsed().as_millis()
        ),
    );
    note_runtime_profile_latency_observation(
        &response.shared,
        &response.profile_name,
        RuntimeRouteKind::Responses,
        "stream_complete",
        started_at.elapsed().as_millis() as u64,
    );
    Ok(())
}

pub(super) fn build_runtime_proxy_text_response_parts(
    status: u16,
    message: &str,
) -> RuntimeBufferedResponseParts {
    RuntimeBufferedResponseParts {
        status,
        headers: vec![(
            "Content-Type".to_string(),
            b"text/plain; charset=utf-8".to_vec(),
        )],
        body: message.as_bytes().to_vec().into(),
    }
}

pub(super) fn build_runtime_proxy_text_response(
    status: u16,
    message: &str,
) -> tiny_http::ResponseBox {
    build_runtime_proxy_response_from_parts(build_runtime_proxy_text_response_parts(
        status, message,
    ))
}

pub(super) fn runtime_proxy_header_value(
    headers: &reqwest::header::HeaderMap,
    name: &str,
) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub(super) fn runtime_proxy_tungstenite_header_value(
    headers: &tungstenite::http::HeaderMap,
    name: &str,
) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub(super) fn is_runtime_proxy_transport_failure(err: &anyhow::Error) -> bool {
    runtime_proxy_transport_failure_kind(err).is_some()
}

pub(super) fn build_runtime_proxy_json_error_parts(
    status: u16,
    code: &str,
    message: &str,
) -> RuntimeBufferedResponseParts {
    let body = serde_json::json!({
        "error": {
            "code": code,
            "message": message,
        }
    })
    .to_string();
    RuntimeBufferedResponseParts {
        status,
        headers: vec![("Content-Type".to_string(), b"application/json".to_vec())],
        body: body.into_bytes().into(),
    }
}

pub(super) fn build_runtime_proxy_json_error_response(
    status: u16,
    code: &str,
    message: &str,
) -> tiny_http::ResponseBox {
    build_runtime_proxy_response_from_parts(build_runtime_proxy_json_error_parts(
        status, code, message,
    ))
}

#[derive(Debug, Default)]
pub(super) struct RuntimeManagedResponseBody {
    bytes: Vec<u8>,
}

impl RuntimeManagedResponseBody {
    pub(super) fn into_vec(mut self) -> Vec<u8> {
        std::mem::take(&mut self.bytes)
    }
}

impl From<Vec<u8>> for RuntimeManagedResponseBody {
    fn from(bytes: Vec<u8>) -> Self {
        Self { bytes }
    }
}

impl std::ops::Deref for RuntimeManagedResponseBody {
    type Target = Vec<u8>;

    fn deref(&self) -> &Self::Target {
        &self.bytes
    }
}

impl<'a> IntoIterator for &'a RuntimeManagedResponseBody {
    type Item = &'a u8;
    type IntoIter = std::slice::Iter<'a, u8>;

    fn into_iter(self) -> Self::IntoIter {
        self.bytes.iter()
    }
}

impl Drop for RuntimeManagedResponseBody {
    fn drop(&mut self) {
        let released_bytes = std::mem::take(&mut self.bytes).capacity();
        let _ = runtime_maybe_trim_process_heap(released_bytes);
    }
}

pub(super) struct RuntimeBufferedResponseParts {
    pub(super) status: u16,
    pub(super) headers: Vec<(String, Vec<u8>)>,
    pub(super) body: RuntimeManagedResponseBody,
}

#[derive(Debug)]
struct RuntimeBufferedResponseBodyReader {
    cursor: Cursor<Vec<u8>>,
}

impl RuntimeBufferedResponseBodyReader {
    fn new(body: Vec<u8>) -> Self {
        Self {
            cursor: Cursor::new(body),
        }
    }
}

impl Read for RuntimeBufferedResponseBodyReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.cursor.read(buf)
    }
}

impl Drop for RuntimeBufferedResponseBodyReader {
    fn drop(&mut self) {
        let released_bytes = std::mem::take(self.cursor.get_mut()).capacity();
        let _ = runtime_maybe_trim_process_heap(released_bytes);
    }
}

pub(super) fn is_runtime_responses_path(path_and_query: &str) -> bool {
    let normalized_path_and_query = runtime_proxy_normalize_openai_path(path_and_query);
    path_without_query(normalized_path_and_query.as_ref()).ends_with("/codex/responses")
}

pub(super) fn is_runtime_anthropic_messages_path(path_and_query: &str) -> bool {
    path_without_query(path_and_query).ends_with(RUNTIME_PROXY_ANTHROPIC_MESSAGES_PATH)
}

pub(super) fn is_runtime_compact_path(path_and_query: &str) -> bool {
    let normalized_path_and_query = runtime_proxy_normalize_openai_path(path_and_query);
    path_without_query(normalized_path_and_query.as_ref()).ends_with("/responses/compact")
}

pub(super) fn path_without_query(path_and_query: &str) -> &str {
    path_and_query
        .split_once('?')
        .map(|(path, _)| path)
        .unwrap_or(path_and_query)
}

pub(super) fn runtime_proxy_openai_suffix(path: &str) -> Option<&str> {
    if let Some(suffix) = path.strip_prefix(LEGACY_RUNTIME_PROXY_OPENAI_MOUNT_PATH_PREFIX)
        && let Some(version_suffix_index) = suffix.find('/')
    {
        return Some(&suffix[version_suffix_index..]);
    }

    if let Some(suffix) = path.strip_prefix(RUNTIME_PROXY_OPENAI_MOUNT_PATH)
        && (suffix.is_empty() || suffix.starts_with('/'))
    {
        return Some(suffix);
    }

    None
}

pub(super) fn runtime_proxy_normalize_openai_path(path_and_query: &str) -> Cow<'_, str> {
    let (path, query) = match path_and_query.split_once('?') {
        Some((path, query)) => (path, Some(query)),
        None => (path_and_query, None),
    };
    let Some(suffix) = runtime_proxy_openai_suffix(path) else {
        return Cow::Borrowed(path_and_query);
    };

    let mut normalized =
        String::with_capacity(path_and_query.len() + RUNTIME_PROXY_OPENAI_UPSTREAM_PATH.len());
    normalized.push_str(RUNTIME_PROXY_OPENAI_UPSTREAM_PATH);
    normalized.push_str(suffix);
    if let Some(query) = query {
        normalized.push('?');
        normalized.push_str(query);
    }
    Cow::Owned(normalized)
}

impl RuntimePrefetchStream {
    fn spawn(
        response: reqwest::Response,
        async_runtime: Arc<TokioRuntime>,
        log_path: PathBuf,
        request_id: u64,
    ) -> Self {
        let (sender, receiver) =
            mpsc::sync_channel::<RuntimePrefetchChunk>(RUNTIME_PROXY_PREFETCH_QUEUE_CAPACITY);
        let shared = Arc::new(RuntimePrefetchSharedState::default());
        let worker_shared = Arc::clone(&shared);
        let worker = async_runtime.spawn(async move {
            runtime_prefetch_response_chunks(response, sender, worker_shared, log_path, request_id)
                .await;
        });
        let worker_abort = worker.abort_handle();
        Self {
            receiver: Some(receiver),
            shared,
            backlog: VecDeque::new(),
            worker_abort: Some(worker_abort),
        }
    }

    fn recv_timeout(
        &mut self,
        timeout: Duration,
    ) -> std::result::Result<RuntimePrefetchChunk, RecvTimeoutError> {
        if let Some(chunk) = self.backlog.pop_front() {
            return Ok(chunk);
        }
        let chunk = self
            .receiver
            .as_ref()
            .expect("runtime prefetch receiver should remain available")
            .recv_timeout(timeout)?;
        if let RuntimePrefetchChunk::Data(bytes) = &chunk {
            runtime_prefetch_release_queued_bytes(&self.shared, bytes.len());
        }
        Ok(chunk)
    }

    fn push_backlog(&mut self, chunk: RuntimePrefetchChunk) {
        self.backlog.push_back(chunk);
    }

    fn into_reader(mut self, prelude: Vec<u8>) -> RuntimePrefetchReader {
        RuntimePrefetchReader {
            receiver: self
                .receiver
                .take()
                .expect("runtime prefetch receiver should remain available"),
            shared: Arc::clone(&self.shared),
            backlog: std::mem::take(&mut self.backlog),
            pending: Cursor::new(prelude),
            finished: false,
            worker_abort: self
                .worker_abort
                .take()
                .expect("runtime prefetch abort handle should remain available"),
        }
    }
}

impl Drop for RuntimePrefetchStream {
    fn drop(&mut self) {
        if let Some(worker_abort) = self.worker_abort.take() {
            worker_abort.abort();
        }
    }
}

pub(super) fn runtime_prefetch_set_terminal_error(
    shared: &RuntimePrefetchSharedState,
    kind: io::ErrorKind,
    message: impl Into<String>,
) {
    let mut terminal_error = shared
        .terminal_error
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if terminal_error.is_none() {
        *terminal_error = Some((kind, message.into()));
    }
}

pub(super) fn runtime_prefetch_terminal_error(
    shared: &RuntimePrefetchSharedState,
) -> Option<(io::ErrorKind, String)> {
    shared
        .terminal_error
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone()
}

pub(super) fn runtime_prefetch_release_queued_bytes(
    shared: &RuntimePrefetchSharedState,
    bytes: usize,
) {
    if bytes > 0 {
        shared.queued_bytes.fetch_sub(bytes, Ordering::SeqCst);
    }
}

pub(super) async fn runtime_prefetch_send_with_wait(
    sender: &SyncSender<RuntimePrefetchChunk>,
    shared: &RuntimePrefetchSharedState,
    chunk: Vec<u8>,
) -> RuntimePrefetchSendOutcome {
    let started_at = Instant::now();
    let retry_delay = Duration::from_millis(runtime_proxy_prefetch_backpressure_retry_ms());
    let timeout = Duration::from_millis(runtime_proxy_prefetch_backpressure_timeout_ms());
    let buffered_limit = runtime_proxy_prefetch_max_buffered_bytes().max(1);
    let mut pending = RuntimePrefetchChunk::Data(chunk);
    let mut retries = 0usize;
    loop {
        let chunk_bytes = match &pending {
            RuntimePrefetchChunk::Data(bytes) => bytes.len(),
            RuntimePrefetchChunk::End | RuntimePrefetchChunk::Error(_, _) => 0,
        };
        let queued_bytes = shared.queued_bytes.load(Ordering::SeqCst);
        if queued_bytes.saturating_add(chunk_bytes) > buffered_limit {
            if started_at.elapsed() >= timeout {
                return RuntimePrefetchSendOutcome::TimedOut {
                    message: format!(
                        "runtime prefetch buffered bytes exceeded safe limit ({} > {})",
                        queued_bytes.saturating_add(chunk_bytes),
                        buffered_limit
                    ),
                };
            }
            retries = retries.saturating_add(1);
            let remaining = timeout.saturating_sub(started_at.elapsed());
            let sleep_for = retry_delay.min(remaining);
            if !sleep_for.is_zero() {
                tokio::time::sleep(sleep_for).await;
            }
            continue;
        }
        match sender.try_send(pending) {
            Ok(()) => {
                if chunk_bytes > 0 {
                    shared.queued_bytes.fetch_add(chunk_bytes, Ordering::SeqCst);
                }
                return RuntimePrefetchSendOutcome::Sent {
                    wait_ms: started_at.elapsed().as_millis(),
                    retries,
                };
            }
            Err(TrySendError::Disconnected(_)) => {
                return RuntimePrefetchSendOutcome::Disconnected;
            }
            Err(TrySendError::Full(returned)) => {
                if started_at.elapsed() >= timeout {
                    return RuntimePrefetchSendOutcome::TimedOut {
                        message: format!(
                            "runtime prefetch backlog exceeded bounded capacity ({})",
                            RUNTIME_PROXY_PREFETCH_QUEUE_CAPACITY
                        ),
                    };
                }
                pending = returned;
                retries = retries.saturating_add(1);
                let remaining = timeout.saturating_sub(started_at.elapsed());
                let sleep_for = retry_delay.min(remaining);
                if !sleep_for.is_zero() {
                    tokio::time::sleep(sleep_for).await;
                }
            }
        }
    }
}

pub(super) async fn runtime_prefetch_response_chunks(
    mut response: reqwest::Response,
    sender: SyncSender<RuntimePrefetchChunk>,
    shared: Arc<RuntimePrefetchSharedState>,
    log_path: PathBuf,
    request_id: u64,
) {
    let mut saw_data = false;
    loop {
        match response.chunk().await {
            Ok(None) => {
                runtime_proxy_log_to_path(
                    &log_path,
                    &format!(
                        "request={request_id} transport=http upstream_stream_end saw_data={saw_data}"
                    ),
                );
                let _ = sender.try_send(RuntimePrefetchChunk::End);
                break;
            }
            Ok(Some(chunk)) => {
                if !saw_data {
                    saw_data = true;
                    runtime_proxy_log_to_path(
                        &log_path,
                        &format!(
                            "request={request_id} transport=http first_upstream_chunk bytes={}",
                            chunk.len()
                        ),
                    );
                }
                if chunk.len() > RUNTIME_PROXY_PREFETCH_MAX_CHUNK_BYTES {
                    let message = format!(
                        "runtime upstream chunk exceeded prefetch limit ({} > {})",
                        chunk.len(),
                        RUNTIME_PROXY_PREFETCH_MAX_CHUNK_BYTES
                    );
                    runtime_prefetch_set_terminal_error(
                        &shared,
                        io::ErrorKind::InvalidData,
                        message.clone(),
                    );
                    runtime_proxy_log_to_path(
                        &log_path,
                        &format!(
                            "request={request_id} transport=http prefetch_chunk_too_large bytes={} limit={} error={message}",
                            chunk.len(),
                            RUNTIME_PROXY_PREFETCH_MAX_CHUNK_BYTES,
                        ),
                    );
                    let _ = sender.try_send(RuntimePrefetchChunk::Error(
                        io::ErrorKind::InvalidData,
                        message,
                    ));
                    break;
                }
                let chunk_bytes = chunk.len();
                match runtime_prefetch_send_with_wait(&sender, &shared, chunk.to_vec()).await {
                    RuntimePrefetchSendOutcome::Sent { wait_ms, retries } => {
                        if retries > 0 {
                            runtime_proxy_log_to_path(
                                &log_path,
                                &format!(
                                    "request={request_id} transport=http prefetch_backpressure_recovered bytes={chunk_bytes} retries={retries} wait_ms={wait_ms}",
                                ),
                            );
                        }
                    }
                    RuntimePrefetchSendOutcome::TimedOut { message } => {
                        runtime_prefetch_set_terminal_error(
                            &shared,
                            io::ErrorKind::WouldBlock,
                            message.clone(),
                        );
                        runtime_proxy_log_to_path(
                            &log_path,
                            &format!(
                                "request={request_id} transport=http prefetch_backpressure_timeout bytes={chunk_bytes} capacity={} error={message}",
                                RUNTIME_PROXY_PREFETCH_QUEUE_CAPACITY,
                            ),
                        );
                        break;
                    }
                    RuntimePrefetchSendOutcome::Disconnected => {
                        runtime_proxy_log_to_path(
                            &log_path,
                            &format!(
                                "request={request_id} transport=http prefetch_receiver_disconnected"
                            ),
                        );
                        break;
                    }
                }
            }
            Err(err) => {
                let kind = runtime_reqwest_error_kind(&err);
                runtime_prefetch_set_terminal_error(&shared, kind, err.to_string());
                runtime_proxy_log_to_path(
                    &log_path,
                    &format!(
                        "request={request_id} transport=http upstream_stream_error kind={kind:?} error={err}"
                    ),
                );
                let _ = sender.try_send(RuntimePrefetchChunk::Error(kind, err.to_string()));
                break;
            }
        }
    }
}

pub(super) fn inspect_runtime_sse_lookahead(
    prefetch: &mut RuntimePrefetchStream,
    log_path: &Path,
    request_id: u64,
) -> Result<RuntimeSseInspection> {
    let deadline = Instant::now() + Duration::from_millis(runtime_proxy_sse_lookahead_timeout_ms());
    let mut buffered = Vec::new();

    loop {
        if buffered.len() >= RUNTIME_PROXY_SSE_LOOKAHEAD_BYTES {
            break;
        }
        let now = Instant::now();
        if now >= deadline {
            break;
        }
        let remaining = deadline.saturating_duration_since(now);
        match prefetch.recv_timeout(remaining) {
            Ok(RuntimePrefetchChunk::Data(chunk)) => {
                buffered.extend_from_slice(&chunk);
                match inspect_runtime_sse_buffer(&buffered)? {
                    RuntimeSseInspectionProgress::Commit {
                        response_ids,
                        turn_state,
                    } => {
                        runtime_proxy_log_to_path(
                            log_path,
                            &format!(
                                "request={request_id} transport=http lookahead_commit bytes={} response_ids={}",
                                buffered.len(),
                                response_ids.len()
                            ),
                        );
                        return Ok(RuntimeSseInspection::Commit {
                            prelude: buffered,
                            response_ids,
                            turn_state,
                        });
                    }
                    RuntimeSseInspectionProgress::Hold { .. } => {}
                    RuntimeSseInspectionProgress::QuotaBlocked => {
                        runtime_proxy_log_to_path(
                            log_path,
                            &format!(
                                "request={request_id} transport=http lookahead_retryable_signal bytes={}",
                                buffered.len()
                            ),
                        );
                        return Ok(RuntimeSseInspection::QuotaBlocked(buffered));
                    }
                    RuntimeSseInspectionProgress::PreviousResponseNotFound => {
                        runtime_proxy_log_to_path(
                            log_path,
                            &format!(
                                "request={request_id} transport=http lookahead_retryable_signal bytes={}",
                                buffered.len()
                            ),
                        );
                        return Ok(RuntimeSseInspection::PreviousResponseNotFound(buffered));
                    }
                }
            }
            Ok(RuntimePrefetchChunk::End) => break,
            Ok(RuntimePrefetchChunk::Error(kind, message)) => {
                if buffered.is_empty() {
                    runtime_proxy_log_to_path(
                        log_path,
                        &format!(
                            "request={request_id} transport=http lookahead_error_before_bytes kind={kind:?} error={message}"
                        ),
                    );
                    return Err(anyhow::Error::new(io::Error::new(kind, message))
                        .context("failed to inspect runtime auto-rotate SSE stream"));
                }
                prefetch.push_backlog(RuntimePrefetchChunk::Error(kind, message));
                break;
            }
            Err(RecvTimeoutError::Timeout) => {
                runtime_proxy_log_to_path(
                    log_path,
                    &format!(
                        "request={request_id} transport=http lookahead_timeout bytes={}",
                        buffered.len()
                    ),
                );
                break;
            }
            Err(RecvTimeoutError::Disconnected) => {
                runtime_proxy_log_to_path(
                    log_path,
                    &format!(
                        "request={request_id} transport=http lookahead_channel_disconnected bytes={}",
                        buffered.len()
                    ),
                );
                break;
            }
        }
    }

    match inspect_runtime_sse_buffer(&buffered)? {
        RuntimeSseInspectionProgress::Commit {
            response_ids,
            turn_state,
        }
        | RuntimeSseInspectionProgress::Hold {
            response_ids,
            turn_state,
        } => {
            if !buffered.is_empty() {
                runtime_proxy_log_to_path(
                    log_path,
                    &format!(
                        "request={request_id} transport=http lookahead_budget_exhausted bytes={} response_ids={}",
                        buffered.len(),
                        response_ids.len()
                    ),
                );
            }
            Ok(RuntimeSseInspection::Commit {
                prelude: buffered,
                response_ids,
                turn_state,
            })
        }
        RuntimeSseInspectionProgress::QuotaBlocked => {
            Ok(RuntimeSseInspection::QuotaBlocked(buffered))
        }
        RuntimeSseInspectionProgress::PreviousResponseNotFound => {
            Ok(RuntimeSseInspection::PreviousResponseNotFound(buffered))
        }
    }
}

pub(super) fn inspect_runtime_sse_buffer(buffered: &[u8]) -> Result<RuntimeSseInspectionProgress> {
    let mut line = Vec::new();
    let mut data_lines = Vec::new();
    let mut response_ids = BTreeSet::new();
    let mut saw_commit_ready_event = false;
    let mut turn_state = None::<String>;

    for byte in buffered {
        line.push(*byte);
        if *byte != b'\n' {
            continue;
        }

        let line_text = String::from_utf8_lossy(&line);
        let trimmed = line_text.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            let event = parse_runtime_sse_event(&data_lines);
            if event.quota_blocked {
                return Ok(RuntimeSseInspectionProgress::QuotaBlocked);
            }
            if event.previous_response_not_found {
                return Ok(RuntimeSseInspectionProgress::PreviousResponseNotFound);
            }
            response_ids.extend(event.response_ids);
            if event.turn_state.is_some() {
                turn_state = event.turn_state;
            }
            if !data_lines.is_empty()
                && !event
                    .event_type
                    .as_deref()
                    .is_some_and(runtime_proxy_precommit_hold_event_kind)
            {
                saw_commit_ready_event = true;
            }
            data_lines.clear();
            line.clear();
            continue;
        }

        if let Some(payload) = trimmed.strip_prefix("data:") {
            data_lines.push(payload.trim_start().to_string());
        }
        line.clear();
    }

    if saw_commit_ready_event {
        Ok(RuntimeSseInspectionProgress::Commit {
            response_ids: response_ids.into_iter().collect(),
            turn_state,
        })
    } else {
        Ok(RuntimeSseInspectionProgress::Hold {
            response_ids: response_ids.into_iter().collect(),
            turn_state,
        })
    }
}

pub(super) fn buffer_runtime_proxy_async_response_parts(
    shared: &RuntimeRotationProxyShared,
    response: reqwest::Response,
    prelude: Vec<u8>,
) -> Result<RuntimeBufferedResponseParts> {
    buffer_runtime_proxy_async_response_parts_with_limit(
        shared,
        response,
        prelude,
        RUNTIME_PROXY_BUFFERED_RESPONSE_MAX_BYTES,
    )
}

pub(super) fn buffer_runtime_proxy_async_response_parts_with_limit(
    shared: &RuntimeRotationProxyShared,
    mut response: reqwest::Response,
    prelude: Vec<u8>,
    max_bytes: usize,
) -> Result<RuntimeBufferedResponseParts> {
    let status = response.status().as_u16();
    let mut headers = Vec::new();
    for (name, value) in response.headers() {
        if should_skip_runtime_response_header(name.as_str()) {
            continue;
        }
        headers.push((name.as_str().to_string(), value.as_bytes().to_vec()));
    }
    let body = shared.async_runtime.block_on(async move {
        let mut body = prelude;
        loop {
            let next = response
                .chunk()
                .await
                .context("failed to read upstream runtime response body chunk")?;
            let Some(chunk) = next else {
                break;
            };
            if body.len().saturating_add(chunk.len()) > max_bytes {
                return Err(anyhow::Error::new(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "runtime buffered response exceeded safe size limit ({})",
                        max_bytes
                    ),
                )));
            }
            body.extend_from_slice(&chunk);
        }
        Ok::<Vec<u8>, anyhow::Error>(body)
    })?;
    Ok(RuntimeBufferedResponseParts {
        status,
        headers,
        body: body.into(),
    })
}

pub(super) fn runtime_reqwest_error_kind(err: &reqwest::Error) -> io::ErrorKind {
    match runtime_transport_failure_kind_from_reqwest(err) {
        Some(
            RuntimeTransportFailureKind::ConnectTimeout | RuntimeTransportFailureKind::ReadTimeout,
        ) => io::ErrorKind::TimedOut,
        Some(RuntimeTransportFailureKind::ConnectRefused) => io::ErrorKind::ConnectionRefused,
        Some(RuntimeTransportFailureKind::ConnectReset) => io::ErrorKind::ConnectionReset,
        Some(RuntimeTransportFailureKind::ConnectionAborted) => io::ErrorKind::ConnectionAborted,
        Some(RuntimeTransportFailureKind::BrokenPipe) => io::ErrorKind::BrokenPipe,
        Some(RuntimeTransportFailureKind::UnexpectedEof) => io::ErrorKind::UnexpectedEof,
        _ => io::ErrorKind::Other,
    }
}

pub(super) fn build_runtime_proxy_response_from_parts(
    parts: RuntimeBufferedResponseParts,
) -> tiny_http::ResponseBox {
    let status = TinyStatusCode(parts.status);
    let headers = parts
        .headers
        .into_iter()
        .filter_map(|(name, value)| TinyHeader::from_bytes(name.as_bytes(), value).ok())
        .collect::<Vec<_>>();
    let body_len = parts.body.len();
    TinyResponse::new(
        status,
        headers,
        Box::new(RuntimeBufferedResponseBodyReader::new(
            parts.body.into_vec(),
        )),
        Some(body_len),
        None,
    )
    .boxed()
}

pub(super) fn runtime_buffered_response_content_type(
    parts: &RuntimeBufferedResponseParts,
) -> Option<&str> {
    parts.headers.iter().find_map(|(name, value)| {
        name.eq_ignore_ascii_case("content-type")
            .then(|| std::str::from_utf8(value).ok())
            .flatten()
            .map(str::trim)
            .filter(|value| !value.is_empty())
    })
}

pub(super) fn parse_runtime_sse_payload(data_lines: &[String]) -> Option<serde_json::Value> {
    if data_lines.is_empty() {
        return None;
    }

    let payload = data_lines.join("\n");
    serde_json::from_str::<serde_json::Value>(&payload).ok()
}

pub(super) fn parse_runtime_sse_event(data_lines: &[String]) -> RuntimeParsedSseEvent {
    let Some(value) = parse_runtime_sse_payload(data_lines) else {
        return RuntimeParsedSseEvent::default();
    };

    RuntimeParsedSseEvent {
        quota_blocked: extract_runtime_proxy_quota_message_from_value(&value).is_some(),
        previous_response_not_found: extract_runtime_proxy_previous_response_message_from_value(
            &value,
        )
        .is_some(),
        response_ids: extract_runtime_response_ids_from_value(&value),
        event_type: runtime_response_event_type_from_value(&value),
        turn_state: extract_runtime_turn_state_from_value(&value),
    }
}

pub(super) fn extract_runtime_proxy_quota_message(body: &[u8]) -> Option<String> {
    if let Ok(value) = serde_json::from_slice::<serde_json::Value>(body)
        && let Some(message) = extract_runtime_proxy_quota_message_from_value(&value)
    {
        return Some(message);
    }

    extract_runtime_proxy_quota_message_from_text(&String::from_utf8_lossy(body))
}

pub(super) fn extract_runtime_proxy_quota_message_from_response_reply(
    response: &RuntimeResponsesReply,
) -> Option<String> {
    match response {
        RuntimeResponsesReply::Buffered(parts) => extract_runtime_proxy_quota_message(&parts.body),
        RuntimeResponsesReply::Streaming(_) => None,
    }
}

pub(super) fn extract_runtime_proxy_quota_message_from_websocket_payload(
    payload: &RuntimeWebsocketErrorPayload,
) -> Option<String> {
    match payload {
        RuntimeWebsocketErrorPayload::Text(text) => {
            extract_runtime_proxy_quota_message_from_text(text)
        }
        RuntimeWebsocketErrorPayload::Binary(bytes) => extract_runtime_proxy_quota_message(bytes),
        RuntimeWebsocketErrorPayload::Empty => None,
    }
}

pub(super) fn extract_runtime_proxy_overload_message_from_websocket_payload(
    payload: &RuntimeWebsocketErrorPayload,
) -> Option<String> {
    match payload {
        RuntimeWebsocketErrorPayload::Text(text) => {
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(text)
                && let Some(message) = extract_runtime_proxy_overload_message_from_value(&value)
            {
                return Some(message);
            }
            extract_runtime_proxy_overload_message_from_text(text)
        }
        RuntimeWebsocketErrorPayload::Binary(bytes) => {
            if let Ok(value) = serde_json::from_slice::<serde_json::Value>(bytes)
                && let Some(message) = extract_runtime_proxy_overload_message_from_value(&value)
            {
                return Some(message);
            }
            extract_runtime_proxy_overload_message_from_text(&String::from_utf8_lossy(bytes))
        }
        RuntimeWebsocketErrorPayload::Empty => None,
    }
}

pub(super) fn extract_runtime_proxy_previous_response_message(body: &[u8]) -> Option<String> {
    serde_json::from_slice::<serde_json::Value>(body)
        .ok()
        .and_then(|value| extract_runtime_proxy_previous_response_message_from_value(&value))
}

pub(super) fn extract_runtime_proxy_overload_message(status: u16, body: &[u8]) -> Option<String> {
    if let Ok(value) = serde_json::from_slice::<serde_json::Value>(body)
        && let Some(message) = extract_runtime_proxy_overload_message_from_value(&value)
    {
        return Some(message);
    }

    let body_text = String::from_utf8_lossy(body).trim().to_string();
    if matches!(status, 429 | 500 | 502 | 503 | 504 | 529)
        && let Some(message) = extract_runtime_proxy_overload_message_from_text(&body_text)
    {
        return Some(message);
    }

    (status == 500).then(|| {
        if body_text.is_empty() {
            "Upstream Codex backend is currently experiencing high demand.".to_string()
        } else {
            body_text
        }
    })
}

pub(super) fn extract_runtime_proxy_overload_message_from_value(
    value: &serde_json::Value,
) -> Option<String> {
    let direct_error = value.get("error");
    let response_error = value
        .get("response")
        .and_then(|response| response.get("error"));
    for error in [direct_error, response_error].into_iter().flatten() {
        let code = error.get("code").and_then(serde_json::Value::as_str);
        let message = error
            .get("message")
            .and_then(serde_json::Value::as_str)
            .or_else(|| error.get("detail").and_then(serde_json::Value::as_str));
        if matches!(code, Some("server_is_overloaded" | "slow_down")) {
            return Some(
                message
                    .unwrap_or("Upstream Codex backend is currently overloaded.")
                    .to_string(),
            );
        }
        if let Some(message) = message.filter(|message| runtime_proxy_overload_message(message)) {
            return Some(message.to_string());
        }
    }

    match value {
        serde_json::Value::Array(values) => values
            .iter()
            .find_map(extract_runtime_proxy_overload_message_from_value),
        serde_json::Value::Object(map) => map
            .values()
            .find_map(extract_runtime_proxy_overload_message_from_value),
        _ => None,
    }
}

pub(super) fn extract_runtime_proxy_overload_message_from_text(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }

    runtime_proxy_overload_message(trimmed).then(|| trimmed.to_string())
}

pub(super) fn extract_runtime_proxy_quota_message_from_value(
    value: &serde_json::Value,
) -> Option<String> {
    if let Some(message) = extract_runtime_proxy_quota_message_candidate(value) {
        return Some(message);
    }

    match value {
        serde_json::Value::Array(values) => values
            .iter()
            .find_map(extract_runtime_proxy_quota_message_from_value),
        serde_json::Value::Object(map) => map
            .values()
            .find_map(extract_runtime_proxy_quota_message_from_value),
        _ => None,
    }
}

pub(super) fn extract_runtime_proxy_quota_message_candidate(
    value: &serde_json::Value,
) -> Option<String> {
    match value {
        serde_json::Value::String(message) => {
            runtime_proxy_usage_limit_message(message).then(|| message.to_string())
        }
        serde_json::Value::Object(map) => {
            let message = map
                .get("message")
                .and_then(serde_json::Value::as_str)
                .or_else(|| map.get("detail").and_then(serde_json::Value::as_str))
                .or_else(|| map.get("error").and_then(serde_json::Value::as_str));
            let code = map.get("code").and_then(serde_json::Value::as_str);
            let error_type = map.get("type").and_then(serde_json::Value::as_str);
            let code_matches = matches!(code, Some("insufficient_quota" | "rate_limit_exceeded"));
            let type_matches = matches!(error_type, Some("usage_limit_reached"));
            let message_matches = message.is_some_and(runtime_proxy_usage_limit_message);
            if !(code_matches || type_matches || message_matches) {
                return None;
            }

            Some(
                message
                    .unwrap_or("Upstream Codex account quota was exhausted.")
                    .to_string(),
            )
        }
        _ => None,
    }
}

pub(super) fn extract_runtime_proxy_quota_message_from_text(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }

    let lower = trimmed.to_ascii_lowercase();
    if runtime_proxy_usage_limit_message(trimmed)
        || lower.contains("usage_limit_reached")
        || lower.contains("insufficient_quota")
        || lower.contains("rate_limit_exceeded")
    {
        Some(trimmed.to_string())
    } else {
        None
    }
}

pub(super) fn runtime_proxy_usage_limit_message(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("you've hit your usage limit")
        || lower.contains("you have hit your usage limit")
        || lower.contains("the usage limit has been reached")
        || lower.contains("usage limit has been reached")
        || lower.contains("usage limit")
            && (lower.contains("try again at")
                || lower.contains("request to your admin")
                || lower.contains("more access now"))
}

pub(super) fn runtime_proxy_overload_message(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("selected model is at capacity")
        || (lower.contains("model is at capacity")
            && (lower.contains("try a different model") || lower.contains("please try again")))
        || lower.contains("backend under high demand")
        || lower.contains("experiencing high demand")
        || lower.contains("server is overloaded")
        || lower.contains("currently overloaded")
}

pub(super) fn runtime_proxy_body_snippet(body: &[u8], max_chars: usize) -> String {
    let normalized = String::from_utf8_lossy(body)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if normalized.is_empty() {
        return "-".to_string();
    }

    let snippet = normalized.chars().take(max_chars).collect::<String>();
    if normalized.chars().count() > max_chars {
        format!("{snippet}...")
    } else {
        snippet
    }
}

pub(super) fn extract_runtime_proxy_previous_response_message_from_value(
    value: &serde_json::Value,
) -> Option<String> {
    let direct_error = value.get("error");
    let response_error = value
        .get("response")
        .and_then(|response| response.get("error"));
    for error in [direct_error, response_error].into_iter().flatten() {
        let code = error.get("code").and_then(serde_json::Value::as_str)?;
        if code != "previous_response_not_found" {
            continue;
        }
        return Some(
            error
                .get("message")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("Previous response could not be found on the selected Codex account.")
                .to_string(),
        );
    }
    None
}

#[cfg(test)]
pub(super) fn extract_runtime_response_ids_from_payload(payload: &str) -> Vec<String> {
    serde_json::from_str::<serde_json::Value>(payload)
        .ok()
        .map(|value| extract_runtime_response_ids_from_value(&value))
        .unwrap_or_default()
}

pub(super) fn extract_runtime_response_ids_from_body_bytes(body: &[u8]) -> Vec<String> {
    serde_json::from_slice::<serde_json::Value>(body)
        .ok()
        .map(|value| extract_runtime_response_ids_from_value(&value))
        .unwrap_or_default()
}

pub(super) fn extract_runtime_turn_state_from_body_bytes(body: &[u8]) -> Option<String> {
    serde_json::from_slice::<serde_json::Value>(body)
        .ok()
        .and_then(|value| extract_runtime_turn_state_from_value(&value))
}

pub(super) fn push_runtime_response_id(response_ids: &mut Vec<String>, id: Option<&str>) {
    if let Some(id) = id
        && !response_ids.iter().any(|existing| existing == id)
    {
        response_ids.push(id.to_string());
    }
}

pub(super) fn extract_runtime_response_ids_from_value(value: &serde_json::Value) -> Vec<String> {
    let mut response_ids = Vec::new();

    push_runtime_response_id(
        &mut response_ids,
        value
            .get("response")
            .and_then(|response| response.get("id"))
            .and_then(serde_json::Value::as_str),
    );
    push_runtime_response_id(
        &mut response_ids,
        value.get("response_id").and_then(serde_json::Value::as_str),
    );

    if value
        .get("object")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|object| object == "response" || object.ends_with(".response"))
    {
        push_runtime_response_id(
            &mut response_ids,
            value.get("id").and_then(serde_json::Value::as_str),
        );
    }

    response_ids
}

pub(super) fn extract_runtime_turn_state_from_value(value: &serde_json::Value) -> Option<String> {
    value
        .get("response")
        .and_then(|response| response.get("headers"))
        .and_then(extract_runtime_turn_state_from_headers_value)
        .or_else(|| {
            value
                .get("headers")
                .and_then(extract_runtime_turn_state_from_headers_value)
        })
}

pub(super) fn extract_runtime_turn_state_from_headers_value(
    value: &serde_json::Value,
) -> Option<String> {
    let headers = value.as_object()?;
    headers.iter().find_map(|(name, value)| {
        if name.eq_ignore_ascii_case("x-codex-turn-state") {
            match value {
                serde_json::Value::String(value) => Some(value.clone()),
                serde_json::Value::Array(items) => items.iter().find_map(|item| match item {
                    serde_json::Value::String(value) => Some(value.clone()),
                    _ => None,
                }),
                _ => None,
            }
        } else {
            None
        }
    })
}
