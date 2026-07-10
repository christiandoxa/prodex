//! Compact-route continuation lineage recording.

use super::*;

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
