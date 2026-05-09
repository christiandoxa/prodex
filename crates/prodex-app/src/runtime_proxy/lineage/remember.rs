use super::*;
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
