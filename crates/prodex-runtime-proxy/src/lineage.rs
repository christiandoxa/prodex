use std::collections::BTreeSet;

#[cfg(test)]
use prodex_runtime_state::runtime_response_turn_state_lineage_key;
use prodex_runtime_state::{
    RUNTIME_RESPONSE_TURN_STATE_LINEAGE_PREFIX, runtime_response_turn_state_lineage_parts,
};

#[derive(Debug, Clone, Copy)]
pub struct RuntimeResponseTurnStateLineageBinding<'a> {
    pub key: &'a str,
    pub profile_name: &'a str,
    pub bound_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeResponseTurnStateLineageDrainPlan {
    pub keys: Vec<String>,
    pub removed_turn_states: BTreeSet<String>,
}

#[derive(Debug, Clone, Copy)]
pub struct RuntimeProfileBindingOrderEntry<'a> {
    pub key: &'a str,
    pub bound_at: i64,
}

pub fn runtime_response_turn_state_lineage_drain_plan<'a>(
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
    let prefix = runtime_response_turn_state_lineage_prefix(previous_response_id);
    let mut selected = None::<(i64, String)>;
    for entry in bindings {
        if !entry.key.starts_with(&prefix) {
            continue;
        }
        if bound_profile.is_some_and(|profile_name| entry.profile_name != profile_name) {
            continue;
        }
        let Some((response_id, turn_state)) = runtime_response_turn_state_lineage_parts(entry.key)
        else {
            continue;
        };
        if response_id != previous_response_id {
            continue;
        }
        let candidate = (entry.bound_at, turn_state.to_string());
        if selected
            .as_ref()
            .is_none_or(|(current_bound_at, _)| *current_bound_at <= candidate.0)
        {
            selected = Some(candidate);
        }
    }
    selected.map(|(_, turn_state)| turn_state)
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

pub fn runtime_profile_binding_prune_keys<'a>(
    bindings: impl IntoIterator<Item = RuntimeProfileBindingOrderEntry<'a>>,
    max_entries: usize,
) -> Vec<String> {
    let mut oldest = bindings
        .into_iter()
        .map(|entry| (entry.key.to_string(), entry.bound_at))
        .collect::<Vec<_>>();
    if oldest.len() <= max_entries {
        return Vec::new();
    }

    let excess = oldest.len() - max_entries;
    oldest.sort_by_key(|(_, bound_at)| *bound_at);
    oldest
        .into_iter()
        .take(excess)
        .map(|(key, _)| key)
        .collect()
}

fn runtime_response_turn_state_lineage_prefix(previous_response_id: &str) -> String {
    format!(
        "{RUNTIME_RESPONSE_TURN_STATE_LINEAGE_PREFIX}{}:{previous_response_id}:",
        previous_response_id.len()
    )
}

#[cfg(test)]
#[path = "../tests/src/lineage.rs"]
mod tests;
