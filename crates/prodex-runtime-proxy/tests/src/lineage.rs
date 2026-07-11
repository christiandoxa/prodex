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
