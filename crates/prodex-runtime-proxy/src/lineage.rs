use std::collections::BTreeSet;

#[cfg(test)]
use prodex_runtime_state::RUNTIME_RESPONSE_TURN_STATE_LINEAGE_PREFIX;
#[cfg(test)]
use prodex_runtime_state::runtime_response_turn_state_lineage_key;
use prodex_runtime_state::runtime_response_turn_state_lineage_parts;

#[derive(Debug, Clone, Copy)]
pub struct RuntimeResponseTurnStateLineageBinding<'a> {
    pub key: &'a str,
    pub profile_name: &'a str,
    pub bound_at: i64,
}

#[cfg(test)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeResponseTurnStateLineageDrainPlan {
    pub keys: Vec<String>,
    pub removed_turn_states: BTreeSet<String>,
}

#[cfg(test)]
pub(crate) fn runtime_response_turn_state_lineage_drain_plan<'a>(
    bindings: impl IntoIterator<Item = RuntimeResponseTurnStateLineageBinding<'a>>,
    previous_response_id: &str,
    bound_profile: Option<&str>,
) -> RuntimeResponseTurnStateLineageDrainPlan {
    let prefix = runtime_response_turn_state_lineage_prefix(previous_response_id);
    let mut keys = bindings
        .into_iter()
        .filter(|entry| entry.key.starts_with(&prefix))
        .filter(|entry| bound_profile.is_none_or(|profile_name| entry.profile_name == profile_name))
        .map(|entry| entry.key.to_string())
        .collect::<Vec<_>>();
    keys.sort();

    let mut removed_turn_states = BTreeSet::new();
    for key in &keys {
        if let Some((_, turn_state)) = runtime_response_turn_state_lineage_parts(key) {
            removed_turn_states.insert(turn_state.to_string());
        }
    }

    RuntimeResponseTurnStateLineageDrainPlan {
        keys,
        removed_turn_states,
    }
}

pub fn runtime_previous_response_turn_state_from_bindings<'a>(
    bindings: impl IntoIterator<Item = RuntimeResponseTurnStateLineageBinding<'a>>,
    previous_response_id: &str,
    bound_profile: Option<&str>,
) -> Option<String> {
    use prodex_mojo_core::runtime_lineage::{
        RuntimeLineageLatestBindingFact, runtime_lineage_latest_binding_index,
    };
    const BINDING_BATCH: usize = 65_536;
    let originals = bindings.into_iter().collect::<Vec<_>>();
    // The existing compiled lineage key parser acquires original borrowed
    // response components; Mojo owns all response/profile filtering and
    // maximum-bound_at selection from those parsed facts.
    let facts = originals
        .iter()
        .map(|binding| RuntimeLineageLatestBindingFact {
            response_id: runtime_response_turn_state_lineage_parts(binding.key)
                .map(|(response, _)| response),
            profile_name: binding.profile_name,
            bound_at: binding.bound_at,
        })
        .collect::<Vec<_>>();
    let mut winners = Vec::new();
    for (batch, chunk) in facts.chunks(BINDING_BATCH).enumerate() {
        let index =
            runtime_lineage_latest_binding_index(chunk, previous_response_id, bound_profile)
                .expect("compiled Mojo lineage turn-state binding selection failed");
        if let Some(index) = index {
            winners.push(batch * BINDING_BATCH + index);
        }
    }
    let finalists = winners
        .iter()
        .map(|index| facts[*index])
        .collect::<Vec<_>>();
    let selected =
        runtime_lineage_latest_binding_index(&finalists, previous_response_id, bound_profile)
            .expect("compiled Mojo lineage turn-state finalist selection failed")?;
    let binding = originals[winners[selected]];
    runtime_response_turn_state_lineage_parts(binding.key)
        .map(|(_, turn_state)| turn_state.to_string())
}

pub fn runtime_live_response_turn_states_for_profile<'a>(
    bindings: impl IntoIterator<Item = RuntimeResponseTurnStateLineageBinding<'a>>,
    profile_name: &str,
    filter: &BTreeSet<String>,
) -> BTreeSet<String> {
    bindings
        .into_iter()
        .filter(|entry| entry.profile_name == profile_name)
        .filter_map(|entry| runtime_response_turn_state_lineage_parts(entry.key))
        .filter(|(_, turn_state)| filter.contains(*turn_state))
        .map(|(_, turn_state)| turn_state.to_string())
        .collect()
}

#[cfg(test)]
fn runtime_response_turn_state_lineage_prefix(previous_response_id: &str) -> String {
    format!(
        "{RUNTIME_RESPONSE_TURN_STATE_LINEAGE_PREFIX}{}:{previous_response_id}:",
        previous_response_id.len()
    )
}

#[cfg(test)]
#[path = "../tests/src/lineage.rs"]
mod tests;
