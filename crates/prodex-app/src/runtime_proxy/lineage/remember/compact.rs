//! Compact-route continuation lineage recording.

use super::*;

pub(crate) fn remember_runtime_compact_lineage(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    session_id: Option<&str>,
    turn_state: Option<&str>,
    verified_route: RuntimeRouteKind,
) -> Result<()> {
    let session_id = session_id
        .map(str::trim)
        .filter(|value| prodex_runtime_state::RuntimeHardBindingIdentity::session(value).is_some());
    let turn_state = turn_state.map(str::trim).filter(|value| {
        prodex_runtime_state::RuntimeHardBindingIdentity::turn_state(value).is_some()
    });
    if session_id.is_none() && turn_state.is_none() {
        return Ok(());
    }

    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let runtime_state = &mut *runtime;
    let bound_at = Local::now().timestamp();
    let binding_identity = runtime_profile_binding_identity(runtime_state, profile_name);
    let mut changed = false;

    if let Some(session_id) = session_id {
        let key = runtime_compact_session_lineage_key(session_id);
        changed = remember_runtime_compact_binding(
            &mut runtime_state.session_id_bindings,
            &mut runtime_state.continuation_statuses,
            key,
            profile_name,
            bound_at,
            binding_identity.as_ref(),
            verified_route,
            RuntimeContinuationBindingKind::SessionId,
        ) || changed;
    }

    if let Some(turn_state) = turn_state {
        let key = runtime_compact_turn_state_lineage_key(turn_state);
        changed = remember_runtime_compact_binding(
            &mut runtime_state.turn_state_bindings,
            &mut runtime_state.continuation_statuses,
            key,
            profile_name,
            bound_at,
            binding_identity.as_ref(),
            verified_route,
            RuntimeContinuationBindingKind::TurnState,
        ) || changed;
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
            RuntimeStateMutation::CompactLineage(profile_name.to_string()),
        );
        drop(runtime);
    } else {
        drop(runtime);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn remember_runtime_compact_binding(
    bindings: &mut std::collections::BTreeMap<String, ResponseProfileBinding>,
    statuses: &mut RuntimeContinuationStatuses,
    key: String,
    profile_name: &str,
    bound_at: i64,
    binding_identity: Option<&prodex_provider_spi::RuntimeProviderBindingIdentity>,
    verified_route: RuntimeRouteKind,
    binding_kind: RuntimeContinuationBindingKind,
) -> bool {
    let (binding_changed, should_refresh_binding) =
        remember_hard_binding(bindings, &key, profile_name, bound_at, binding_identity);
    let mut changed = binding_changed;
    if should_refresh_binding
        || runtime_continuation_status_should_refresh_verified(
            statuses,
            binding_kind,
            &key,
            bound_at,
            Some(verified_route),
        )
    {
        changed = runtime_mark_continuation_status_verified(
            statuses,
            binding_kind,
            &key,
            bound_at,
            Some(verified_route),
        ) || changed;
    }
    changed
}
