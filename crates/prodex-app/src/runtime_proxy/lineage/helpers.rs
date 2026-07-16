use anyhow::Result;
use std::collections::{BTreeMap, BTreeSet};

use crate::{ResponseProfileBinding, RuntimeStateMutation};

use super::{
    RuntimeRotationProxyShared, RuntimeRotationState, schedule_runtime_state_save_from_runtime,
};
pub(crate) fn schedule_runtime_binding_touch_save(
    shared: &RuntimeRotationProxyShared,
    runtime: &RuntimeRotationState,
    mutation: RuntimeStateMutation,
) {
    schedule_runtime_state_save_from_runtime(shared, runtime, mutation);
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
    let prefix = runtime_response_turn_state_lineage_prefix(previous_response_id);
    let mut removed_turn_states = BTreeSet::new();
    let keys = bindings
        .range(prefix.clone()..)
        .take_while(|(key, _)| key.starts_with(&prefix))
        .filter(|(_, binding)| {
            bound_profile.is_none_or(|profile_name| binding.profile_name == profile_name)
        })
        .filter_map(|(key, _)| {
            let (_, turn_state) =
                runtime_proxy_crate::runtime_response_turn_state_lineage_parts(key)?;
            removed_turn_states.insert(turn_state.to_string());
            Some(key.clone())
        })
        .collect::<Vec<_>>();
    for key in keys {
        bindings.remove(&key);
    }
    removed_turn_states
}

fn runtime_response_turn_state_lineage_prefix(previous_response_id: &str) -> String {
    format!(
        "{}{}:{previous_response_id}:",
        runtime_proxy_crate::RUNTIME_RESPONSE_TURN_STATE_LINEAGE_PREFIX,
        previous_response_id.len()
    )
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
