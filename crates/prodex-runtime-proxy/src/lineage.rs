use std::collections::BTreeSet;

pub const RUNTIME_COMPACT_SESSION_LINEAGE_PREFIX: &str = "__compact_session__:";
pub const RUNTIME_COMPACT_TURN_STATE_LINEAGE_PREFIX: &str = "__compact_turn_state__:";
pub const RUNTIME_RESPONSE_TURN_STATE_LINEAGE_PREFIX: &str = "__response_turn_state__:";

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

pub fn runtime_compact_session_lineage_key(session_id: &str) -> String {
    format!("{RUNTIME_COMPACT_SESSION_LINEAGE_PREFIX}{session_id}")
}

pub fn runtime_compact_turn_state_lineage_key(turn_state: &str) -> String {
    format!("{RUNTIME_COMPACT_TURN_STATE_LINEAGE_PREFIX}{turn_state}")
}

pub fn runtime_response_turn_state_lineage_key(response_id: &str, turn_state: &str) -> String {
    format!(
        "{RUNTIME_RESPONSE_TURN_STATE_LINEAGE_PREFIX}{}:{response_id}:{turn_state}",
        response_id.len()
    )
}

pub fn runtime_is_response_turn_state_lineage_key(key: &str) -> bool {
    key.starts_with(RUNTIME_RESPONSE_TURN_STATE_LINEAGE_PREFIX)
}

pub fn runtime_response_turn_state_lineage_parts(key: &str) -> Option<(&str, &str)> {
    let suffix = key.strip_prefix(RUNTIME_RESPONSE_TURN_STATE_LINEAGE_PREFIX)?;
    let (response_len, rest) = suffix.split_once(':')?;
    let response_len = response_len.parse::<usize>().ok()?;
    let response_and_sep = rest.get(..response_len.saturating_add(1))?;
    if response_and_sep.as_bytes().get(response_len).copied() != Some(b':') {
        return None;
    }
    let response_id = response_and_sep.get(..response_len)?;
    let turn_state = rest.get(response_len.saturating_add(1)..)?;
    (!response_id.is_empty() && !turn_state.is_empty()).then_some((response_id, turn_state))
}

pub fn runtime_is_compact_session_lineage_key(key: &str) -> bool {
    key.starts_with(RUNTIME_COMPACT_SESSION_LINEAGE_PREFIX)
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
mod tests {
    use super::*;

    fn binding<'a>(
        key: &'a str,
        profile_name: &'a str,
        bound_at: i64,
    ) -> RuntimeResponseTurnStateLineageBinding<'a> {
        RuntimeResponseTurnStateLineageBinding {
            key,
            profile_name,
            bound_at,
        }
    }

    #[test]
    fn response_turn_state_lineage_keys_round_trip_colons() {
        let key = runtime_response_turn_state_lineage_key("resp:123", "turn:abc");

        assert_eq!(
            runtime_response_turn_state_lineage_parts(&key),
            Some(("resp:123", "turn:abc"))
        );
    }

    #[test]
    fn previous_response_turn_state_selects_newest_matching_profile() {
        let old = runtime_response_turn_state_lineage_key("resp", "old");
        let new = runtime_response_turn_state_lineage_key("resp", "new");
        let other = runtime_response_turn_state_lineage_key("resp", "other-profile");
        let bindings = [
            binding(&old, "main", 10),
            binding(&new, "main", 20),
            binding(&other, "second", 30),
        ];

        assert_eq!(
            runtime_previous_response_turn_state_from_bindings(bindings, "resp", Some("main")),
            Some("new".to_string())
        );
    }

    #[test]
    fn drain_plan_includes_only_matching_profile() {
        let main = runtime_response_turn_state_lineage_key("resp", "main-turn");
        let second = runtime_response_turn_state_lineage_key("resp", "second-turn");
        let bindings = [binding(&main, "main", 10), binding(&second, "second", 20)];

        let plan = runtime_response_turn_state_lineage_drain_plan(bindings, "resp", Some("main"));

        assert_eq!(plan.keys, vec![main]);
        assert_eq!(
            plan.removed_turn_states,
            BTreeSet::from(["main-turn".to_string()])
        );
    }
}
