use super::*;

pub(crate) fn runtime_request_hard_binding_owner(
    shared: &RuntimeRotationProxyShared,
    previous_response_id: Option<&str>,
    turn_state: Option<&str>,
    session_id: Option<&str>,
) -> Result<prodex_runtime_state::RuntimeHardBindingOwner> {
    let identity = runtime_hard_binding_identity(previous_response_id, turn_state, session_id)?;
    let runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    Ok(runtime_hard_binding_owner_for_runtime(&runtime, &identity))
}

pub(crate) fn runtime_response_bound_profile(
    shared: &RuntimeRotationProxyShared,
    previous_response_id: &str,
    _route_kind: RuntimeRouteKind,
) -> Result<Option<String>> {
    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let status_was_stale = runtime_age_stale_verified_continuation_status(
        &mut runtime.continuation_statuses,
        RuntimeContinuationBindingKind::Response,
        previous_response_id,
        Local::now().timestamp(),
    );
    let identity = runtime_hard_binding_identity(Some(previous_response_id), None, None)?;
    let owner = runtime_hard_binding_owner_for_runtime(&runtime, &identity);
    let profile_name = runtime_hard_binding_profile_name(&owner);
    let mut persist_touch = false;
    if let prodex_runtime_state::RuntimeHardBindingOwner::Owned(profile_name) = &owner {
        let binding_touch = {
            runtime
                .state
                .response_profile_bindings
                .get_mut(previous_response_id)
                .and_then(|binding| {
                    (binding.profile_name == *profile_name)
                        .then(|| touch_runtime_continuation_binding(binding))
                })
        };
        if let Some((binding_persist, now)) = binding_touch {
            persist_touch |= binding_persist;
            persist_touch |= runtime_continuation_status_should_persist_touch(
                &runtime.continuation_statuses,
                RuntimeContinuationBindingKind::Response,
                previous_response_id,
                now,
            );
            let _ = runtime_mark_continuation_status_touched(
                &mut runtime.continuation_statuses,
                RuntimeContinuationBindingKind::Response,
                previous_response_id,
                now,
            );
        }
    }
    if persist_touch || status_was_stale {
        schedule_runtime_binding_touch_save(
            shared,
            &runtime,
            if status_was_stale {
                RuntimeStateMutation::ContinuationStale(previous_response_id.to_string())
            } else {
                RuntimeStateMutation::ResponseTouch(previous_response_id.to_string())
            },
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
    let status_was_stale = runtime_age_stale_verified_continuation_status(
        &mut runtime.continuation_statuses,
        RuntimeContinuationBindingKind::TurnState,
        turn_state,
        Local::now().timestamp(),
    );
    let identity = runtime_hard_binding_identity(None, Some(turn_state), None)?;
    let owner = runtime_hard_binding_owner_for_runtime(&runtime, &identity);
    let profile_name = runtime_hard_binding_profile_name(&owner);
    let mut persist_touch = false;
    if let prodex_runtime_state::RuntimeHardBindingOwner::Owned(profile_name) = &owner {
        let binding_touch = {
            runtime
                .turn_state_bindings
                .get_mut(turn_state)
                .and_then(|binding| {
                    (binding.profile_name == *profile_name)
                        .then(|| touch_runtime_continuation_binding(binding))
                })
        };
        if let Some((binding_persist, now)) = binding_touch {
            persist_touch |= binding_persist;
            persist_touch |= runtime_continuation_status_should_persist_touch(
                &runtime.continuation_statuses,
                RuntimeContinuationBindingKind::TurnState,
                turn_state,
                now,
            );
            let _ = runtime_mark_continuation_status_touched(
                &mut runtime.continuation_statuses,
                RuntimeContinuationBindingKind::TurnState,
                turn_state,
                now,
            );
        }
    }
    if persist_touch || status_was_stale {
        schedule_runtime_binding_touch_save(
            shared,
            &runtime,
            if status_was_stale {
                RuntimeStateMutation::ContinuationStale(turn_state.to_string())
            } else {
                RuntimeStateMutation::TurnStateTouch(turn_state.to_string())
            },
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
    let status_was_stale = runtime_age_stale_verified_continuation_status(
        &mut runtime.continuation_statuses,
        RuntimeContinuationBindingKind::SessionId,
        session_id,
        Local::now().timestamp(),
    );
    let identity = runtime_hard_binding_identity(None, None, Some(session_id))?;
    let owner = runtime_hard_binding_owner_for_runtime(&runtime, &identity);
    let profile_name = runtime_hard_binding_profile_name(&owner);
    let mut persist_touch = false;
    if let prodex_runtime_state::RuntimeHardBindingOwner::Owned(profile_name) = &owner {
        persist_touch = touch_runtime_session_binding(
            runtime.session_id_bindings.get_mut(session_id),
            profile_name,
            &mut persist_touch,
        ) || persist_touch;
        persist_touch = touch_runtime_session_binding(
            runtime.state.session_profile_bindings.get_mut(session_id),
            profile_name,
            &mut persist_touch,
        ) || persist_touch;
        persist_touch = runtime_continuation_status_should_persist_touch(
            &runtime.continuation_statuses,
            RuntimeContinuationBindingKind::SessionId,
            session_id,
            Local::now().timestamp(),
        ) || persist_touch;
        let _ = runtime_mark_continuation_status_touched(
            &mut runtime.continuation_statuses,
            RuntimeContinuationBindingKind::SessionId,
            session_id,
            Local::now().timestamp(),
        );
    }
    if persist_touch || status_was_stale {
        schedule_runtime_binding_touch_save(
            shared,
            &runtime,
            if status_was_stale {
                RuntimeStateMutation::ContinuationStale(session_id.to_string())
            } else {
                RuntimeStateMutation::SessionTouch(session_id.to_string())
            },
        );
    }
    Ok(profile_name)
}

fn runtime_hard_binding_owner_for_runtime(
    runtime: &RuntimeRotationState,
    identity: &prodex_runtime_state::RuntimeHardBindingIdentity,
) -> prodex_runtime_state::RuntimeHardBindingOwner {
    let resolution = prodex_runtime_store::runtime_hard_binding_resolution(
        identity,
        &runtime.state.response_profile_bindings,
        &runtime.turn_state_bindings,
        &runtime.session_id_bindings,
        &runtime.state.session_profile_bindings,
        &runtime.state.profiles,
    );
    match resolution {
        prodex_runtime_state::RuntimeHardBindingResolution::Owned {
            profile_name,
            binding_identity: Some(expected),
        } if runtime_profile_binding_identity(runtime, &profile_name).as_ref()
            != Some(&expected) =>
        {
            prodex_runtime_state::RuntimeHardBindingOwner::Unavailable(profile_name)
        }
        resolution => resolution.owner(),
    }
}

fn runtime_hard_binding_identity(
    previous_response_id: Option<&str>,
    turn_state: Option<&str>,
    session_id: Option<&str>,
) -> Result<prodex_runtime_state::RuntimeHardBindingIdentity> {
    let supplied = [previous_response_id, turn_state, session_id]
        .into_iter()
        .flatten()
        .any(|value| !value.trim().is_empty());
    match prodex_runtime_state::RuntimeHardBindingIdentity::new(
        previous_response_id,
        turn_state,
        session_id,
    ) {
        Some(identity) => Ok(identity),
        None if !supplied => Ok(prodex_runtime_state::RuntimeHardBindingIdentity::default()),
        None => Err(anyhow::anyhow!("runtime hard-binding identity is invalid")),
    }
}

fn runtime_hard_binding_profile_name(
    owner: &prodex_runtime_state::RuntimeHardBindingOwner,
) -> Option<String> {
    match owner {
        prodex_runtime_state::RuntimeHardBindingOwner::Owned(profile_name) => {
            Some(profile_name.clone())
        }
        prodex_runtime_state::RuntimeHardBindingOwner::Unavailable(_) => {
            Some(prodex_runtime_state::RUNTIME_HARD_BINDING_CONFLICT_PROFILE.to_string())
        }
        prodex_runtime_state::RuntimeHardBindingOwner::Conflict => {
            Some(prodex_runtime_state::RUNTIME_HARD_BINDING_CONFLICT_PROFILE.to_string())
        }
        prodex_runtime_state::RuntimeHardBindingOwner::Unbound => None,
    }
}

fn touch_runtime_continuation_binding(binding: &mut ResponseProfileBinding) -> (bool, i64) {
    let now = Local::now().timestamp();
    let persist_touch = runtime_binding_touch_should_persist(binding.bound_at, now);
    if binding.bound_at < now {
        binding.bound_at = now;
    }
    (persist_touch, now)
}

fn touch_runtime_session_binding(
    binding: Option<&mut ResponseProfileBinding>,
    profile_name: &str,
    persist_touch: &mut bool,
) -> bool {
    let Some(binding) = binding.filter(|binding| binding.profile_name == profile_name) else {
        return false;
    };
    let now = Local::now().timestamp();
    *persist_touch = runtime_binding_touch_should_persist(binding.bound_at, now);
    if binding.bound_at < now {
        binding.bound_at = now;
    }
    *persist_touch
}
