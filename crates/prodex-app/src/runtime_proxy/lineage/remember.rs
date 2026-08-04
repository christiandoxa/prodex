//! Runtime continuation lineage recording helpers.

use super::*;

mod compact;

pub(crate) use compact::remember_runtime_compact_lineage;

pub(crate) fn runtime_profile_binding_identity(
    runtime: &RuntimeRotationState,
    profile_name: &str,
) -> Option<prodex_provider_spi::RuntimeProviderBindingIdentity> {
    let profile = runtime.state.profiles.get(profile_name)?;
    let provider = match profile.provider {
        prodex_state::ProfileProvider::Openai => prodex_provider_core::ProviderId::OpenAi,
        prodex_state::ProfileProvider::Gemini { .. } => prodex_provider_core::ProviderId::Gemini,
        prodex_state::ProfileProvider::Anthropic { .. } => {
            prodex_provider_core::ProviderId::Anthropic
        }
        prodex_state::ProfileProvider::Copilot { .. } => prodex_provider_core::ProviderId::Copilot,
        prodex_state::ProfileProvider::Kiro { .. } => prodex_provider_core::ProviderId::Kiro,
        prodex_state::ProfileProvider::Agy { .. } => prodex_provider_core::ProviderId::Local,
    };
    prodex_provider_spi::RuntimeProviderBindingIdentity::from_profile(
        provider,
        profile_name,
        &runtime.upstream_base_url,
    )
}

pub(crate) fn remember_runtime_external_binding_identity(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    binding_identity: &prodex_provider_spi::RuntimeProviderBindingIdentity,
    response_ids: &[String],
    turn_state: Option<&str>,
    session_id: Option<&str>,
) -> Result<()> {
    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let bound_at = Local::now().timestamp();
    let mut changed = false;
    for response_id in response_ids
        .iter()
        .filter(|value| prodex_runtime_state::RuntimeHardBindingIdentity::response(value).is_some())
    {
        changed = remember_hard_binding(
            &mut runtime.state.response_profile_bindings,
            response_id,
            profile_name,
            bound_at,
            Some(binding_identity),
        )
        .0 || changed;
        if let Some(turn_state) = turn_state {
            let key = runtime_response_turn_state_lineage_key(response_id, turn_state);
            changed = remember_hard_binding(
                &mut runtime.state.response_profile_bindings,
                &key,
                profile_name,
                bound_at,
                Some(binding_identity),
            )
            .0 || changed;
        }
    }
    if let Some(turn_state) = turn_state.map(str::trim).filter(|value| {
        prodex_runtime_state::RuntimeHardBindingIdentity::turn_state(value).is_some()
    }) {
        changed = remember_hard_binding(
            &mut runtime.turn_state_bindings,
            turn_state,
            profile_name,
            bound_at,
            Some(binding_identity),
        )
        .0 || changed;
        let key = runtime_compact_turn_state_lineage_key(turn_state);
        changed = remember_hard_binding(
            &mut runtime.turn_state_bindings,
            &key,
            profile_name,
            bound_at,
            Some(binding_identity),
        )
        .0 || changed;
    }
    if let Some(session_id) = session_id
        .map(str::trim)
        .filter(|value| prodex_runtime_state::RuntimeHardBindingIdentity::session(value).is_some())
    {
        changed = remember_hard_binding(
            &mut runtime.session_id_bindings,
            session_id,
            profile_name,
            bound_at,
            Some(binding_identity),
        )
        .0 || changed;
        let key = runtime_compact_session_lineage_key(session_id);
        changed = remember_hard_binding(
            &mut runtime.session_id_bindings,
            &key,
            profile_name,
            bound_at,
            Some(binding_identity),
        )
        .0 || changed;
        changed = remember_hard_binding(
            &mut runtime.state.session_profile_bindings,
            session_id,
            profile_name,
            bound_at,
            Some(binding_identity),
        )
        .0 || changed;
    }
    if changed {
        prune_profile_bindings(
            &mut runtime.state.response_profile_bindings,
            RESPONSE_PROFILE_BINDING_LIMIT,
        );
        prune_profile_bindings(
            &mut runtime.turn_state_bindings,
            TURN_STATE_PROFILE_BINDING_LIMIT,
        );
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
            RuntimeStateMutation::ResponseIds(profile_name.to_string()),
        );
    }
    Ok(())
}

pub(crate) fn remember_runtime_turn_state(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    turn_state: Option<&str>,
    verified_route: RuntimeRouteKind,
) -> Result<()> {
    let Some(turn_state) = turn_state.map(str::trim).filter(|value| {
        prodex_runtime_state::RuntimeHardBindingIdentity::turn_state(value).is_some()
    }) else {
        return Ok(());
    };

    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let bound_at = Local::now().timestamp();
    let binding_identity = runtime_profile_binding_identity(&runtime, profile_name);
    let mut changed = false;
    let (binding_changed, should_refresh_binding) = remember_hard_binding(
        &mut runtime.turn_state_bindings,
        turn_state,
        profile_name,
        bound_at,
        binding_identity.as_ref(),
    );
    changed = binding_changed || changed;
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
            RuntimeStateMutation::TurnState(profile_name.to_string()),
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
    let Some(session_id) = session_id
        .map(str::trim)
        .filter(|value| prodex_runtime_state::RuntimeHardBindingIdentity::session(value).is_some())
    else {
        return Ok(());
    };

    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let bound_at = Local::now().timestamp();
    let binding_identity = runtime_profile_binding_identity(&runtime, profile_name);
    let mut changed = false;
    let mut should_refresh_binding = false;
    let (binding_changed, binding_refresh) = remember_hard_binding(
        &mut runtime.session_id_bindings,
        session_id,
        profile_name,
        bound_at,
        binding_identity.as_ref(),
    );
    changed = binding_changed || changed;
    should_refresh_binding = binding_refresh || should_refresh_binding;
    let (binding_changed, binding_refresh) = remember_hard_binding(
        &mut runtime.state.session_profile_bindings,
        session_id,
        profile_name,
        bound_at,
        binding_identity.as_ref(),
    );
    changed = binding_changed || changed;
    should_refresh_binding = binding_refresh || should_refresh_binding;
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
            RuntimeStateMutation::SessionId(profile_name.to_string()),
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
    let response_ids = response_ids
        .iter()
        .map(String::as_str)
        .filter(|response_id| {
            prodex_runtime_state::RuntimeHardBindingIdentity::response(response_id).is_some()
        })
        .collect::<Vec<_>>();
    if response_ids.is_empty() {
        return Ok(());
    }

    let turn_state = turn_state.map(str::trim).filter(|value| {
        prodex_runtime_state::RuntimeHardBindingIdentity::turn_state(value).is_some()
    });
    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let bound_at = Local::now().timestamp();
    let binding_identity = runtime_profile_binding_identity(&runtime, profile_name);
    let mut changed = false;
    let mut response_turn_state_changed = false;
    for response_id in &response_ids {
        let (binding_changed, should_refresh_binding) = remember_runtime_response_binding(
            &mut runtime,
            response_id,
            profile_name,
            bound_at,
            binding_identity.as_ref(),
        );
        changed = binding_changed || changed;
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
            let turn_state_changed = remember_runtime_response_turn_state_binding(
                &mut runtime,
                key,
                profile_name,
                bound_at,
                binding_identity.as_ref(),
            );
            changed = turn_state_changed || changed;
            response_turn_state_changed = turn_state_changed || response_turn_state_changed;
        }
    }
    if !changed {
        drop(runtime);
        return Ok(());
    }

    prune_profile_bindings(
        &mut runtime.state.response_profile_bindings,
        RESPONSE_PROFILE_BINDING_LIMIT,
    );
    schedule_runtime_state_save_from_runtime(
        shared,
        &runtime,
        RuntimeStateMutation::ResponseIds(profile_name.to_string()),
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
        return Ok(());
    }
    if turn_state.is_none() {
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
    Ok(())
}

fn remember_runtime_response_binding(
    runtime: &mut RuntimeRotationState,
    response_id: &str,
    profile_name: &str,
    bound_at: i64,
    binding_identity: Option<&prodex_provider_spi::RuntimeProviderBindingIdentity>,
) -> (bool, bool) {
    let mut changed =
        clear_runtime_previous_response_negative_cache(runtime, response_id, profile_name);
    let (binding_changed, should_refresh) = remember_hard_binding(
        &mut runtime.state.response_profile_bindings,
        response_id,
        profile_name,
        bound_at,
        binding_identity,
    );
    changed = binding_changed || changed;
    (changed, should_refresh)
}

fn remember_runtime_response_turn_state_binding(
    runtime: &mut RuntimeRotationState,
    key: String,
    profile_name: &str,
    bound_at: i64,
    binding_identity: Option<&prodex_provider_spi::RuntimeProviderBindingIdentity>,
) -> bool {
    remember_hard_binding(
        &mut runtime.state.response_profile_bindings,
        &key,
        profile_name,
        bound_at,
        binding_identity,
    )
    .0
}

fn remember_hard_binding(
    bindings: &mut BTreeMap<String, ResponseProfileBinding>,
    key: &str,
    profile_name: &str,
    bound_at: i64,
    binding_identity: Option<&prodex_provider_spi::RuntimeProviderBindingIdentity>,
) -> (bool, bool) {
    if !prodex_runtime_state::runtime_lineage_key_is_bounded(key)
        || !prodex_runtime_state::runtime_identity_component_is_valid(profile_name)
    {
        return (false, false);
    }
    match bindings.get_mut(key) {
        Some(binding) if binding.profile_name == profile_name => {
            let identity_added = binding.binding_identity.is_none() && binding_identity.is_some();
            if binding
                .binding_identity
                .as_ref()
                .zip(binding_identity)
                .is_some_and(|(previous, current)| previous != current)
            {
                binding.profile_name = prodex_state::HARD_BINDING_CONFLICT_PROFILE.to_string();
                binding.binding_identity = None;
                binding.bound_at = binding.bound_at.max(bound_at);
                return (true, true);
            }
            if binding.binding_identity.is_none()
                && let Some(binding_identity) = binding_identity
            {
                binding.binding_identity = Some(binding_identity.clone());
            }
            if binding.bound_at < bound_at {
                binding.bound_at = bound_at;
            }
            (identity_added, identity_added)
        }
        Some(binding) => {
            binding.profile_name = prodex_state::HARD_BINDING_CONFLICT_PROFILE.to_string();
            binding.binding_identity = None;
            binding.bound_at = binding.bound_at.max(bound_at);
            (true, true)
        }
        None => {
            bindings.insert(
                key.to_string(),
                ResponseProfileBinding {
                    profile_name: profile_name.to_string(),
                    bound_at,
                    binding_identity: binding_identity.cloned(),
                },
            );
            (true, true)
        }
    }
}

pub(crate) fn remember_runtime_successful_previous_response_owner(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    previous_response_id: Option<&str>,
    verified_route: RuntimeRouteKind,
) -> Result<()> {
    let Some(previous_response_id) = previous_response_id.map(str::trim).filter(|value| {
        prodex_runtime_state::RuntimeHardBindingIdentity::response(value).is_some()
    }) else {
        return Ok(());
    };

    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let bound_at = Local::now().timestamp();
    let binding_identity = runtime_profile_binding_identity(&runtime, profile_name);
    let mut changed = clear_runtime_previous_response_negative_cache(
        &mut runtime,
        previous_response_id,
        profile_name,
    );
    let (binding_changed, should_refresh_binding) = remember_hard_binding(
        &mut runtime.state.response_profile_bindings,
        previous_response_id,
        profile_name,
        bound_at,
        binding_identity.as_ref(),
    );
    let binding_conflicted = runtime
        .state
        .response_profile_bindings
        .get(previous_response_id)
        .is_some_and(|binding| binding.profile_name == prodex_state::HARD_BINDING_CONFLICT_PROFILE);
    changed = binding_changed || changed;
    let should_refresh_status = should_refresh_binding
        || runtime_continuation_status_map(
            &runtime.continuation_statuses,
            RuntimeContinuationBindingKind::Response,
        )
        .get(previous_response_id)
        .is_none_or(|status| {
            status.state != prodex_runtime_state::RuntimeContinuationBindingLifecycle::Verified
                || status.last_verified_route.as_deref()
                    != Some(runtime_route_kind_label(verified_route))
        });
    if should_refresh_status {
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
            RuntimeStateMutation::PreviousResponseOwner(profile_name.to_string()),
        );
        drop(runtime);
        if !binding_conflicted {
            runtime_proxy_log(
                shared,
                format!(
                    "binding previous_response_owner profile={profile_name} response_id={previous_response_id}"
                ),
            );
        }
    } else {
        drop(runtime);
    }
    Ok(())
}
