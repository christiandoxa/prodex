use super::*;
pub(crate) fn schedule_runtime_binding_touch_save(
    shared: &RuntimeRotationProxyShared,
    runtime: &RuntimeRotationState,
    reason: &str,
) {
    schedule_runtime_state_save_from_runtime(shared, runtime, reason);
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

    let runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    Ok(runtime_previous_response_turn_state_from_bindings(
        &runtime.state.response_profile_bindings,
        previous_response_id,
        bound_profile,
    ))
}

pub(crate) fn clear_runtime_response_turn_state_lineage(
    bindings: &mut BTreeMap<String, ResponseProfileBinding>,
    previous_response_id: &str,
) -> bool {
    !drain_runtime_response_turn_state_lineage(bindings, previous_response_id, None).is_empty()
}

pub(super) fn drain_runtime_response_turn_state_lineage(
    bindings: &mut BTreeMap<String, ResponseProfileBinding>,
    previous_response_id: &str,
    bound_profile: Option<&str>,
) -> BTreeSet<String> {
    let plan = runtime_proxy_crate::runtime_response_turn_state_lineage_drain_plan(
        bindings.iter().map(|(key, binding)| {
            runtime_proxy_crate::RuntimeResponseTurnStateLineageBinding {
                key,
                profile_name: &binding.profile_name,
                bound_at: binding.bound_at,
            }
        }),
        previous_response_id,
        bound_profile,
    );
    for key in plan.keys {
        bindings.remove(&key);
    }
    plan.removed_turn_states
}

pub(super) fn runtime_previous_response_turn_state_from_bindings(
    bindings: &BTreeMap<String, ResponseProfileBinding>,
    previous_response_id: &str,
    bound_profile: Option<&str>,
) -> Option<String> {
    runtime_proxy_crate::runtime_previous_response_turn_state_from_bindings(
        bindings.iter().map(|(key, binding)| {
            runtime_proxy_crate::RuntimeResponseTurnStateLineageBinding {
                key,
                profile_name: &binding.profile_name,
                bound_at: binding.bound_at,
            }
        }),
        previous_response_id,
        bound_profile,
    )
}

pub(super) fn runtime_live_response_turn_states_for_profile(
    bindings: &BTreeMap<String, ResponseProfileBinding>,
    profile_name: &str,
    filter: &BTreeSet<String>,
) -> BTreeSet<String> {
    runtime_proxy_crate::runtime_live_response_turn_states_for_profile(
        bindings.iter().map(|(key, binding)| {
            runtime_proxy_crate::RuntimeResponseTurnStateLineageBinding {
                key,
                profile_name: &binding.profile_name,
                bound_at: binding.bound_at,
            }
        }),
        profile_name,
        filter,
    )
}
