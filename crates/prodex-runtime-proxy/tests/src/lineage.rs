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

#[test]
fn compiled_lineage_latest_binding_prefers_last_matching_timestamp_and_exact_unicode_profile() {
    let first = runtime_response_turn_state_lineage_key("resp:雪", "first");
    let duplicate = runtime_response_turn_state_lineage_key("resp:雪", "last:🦀");
    let other = runtime_response_turn_state_lineage_key("resp:雪", "other-profile");
    let mismatch = runtime_response_turn_state_lineage_key("resp", "wrong-response");
    let malformed = "__response_turn_state__:6:resp:雪:bad-length";
    let entries = [
        binding(&first, "雪", i64::MIN),
        binding(&other, "other", i64::MAX),
        binding(&mismatch, "雪", i64::MAX),
        binding(malformed, "雪", i64::MAX),
        binding(&first, "雪", 100),
        binding(&duplicate, "雪", 100),
    ];
    assert_eq!(
        runtime_previous_response_turn_state_from_bindings(entries, "resp:雪", Some("雪")),
        Some("last:🦀".to_owned()),
        "later original binding wins an equal signed timestamp",
    );
    assert_eq!(
        runtime_previous_response_turn_state_from_bindings(entries, "resp:雪", None),
        Some("other-profile".to_owned()),
        "unbound lookup selects greatest timestamp across profiles",
    );
    assert_eq!(
        runtime_previous_response_turn_state_from_bindings(entries, "resp:雪", Some("missing")),
        None,
    );
    assert_eq!(
        runtime_previous_response_turn_state_from_bindings(entries, "resp", Some("雪")),
        Some("wrong-response".to_owned()),
    );
    assert_eq!(
        runtime_previous_response_turn_state_from_bindings(Vec::new(), "resp:雪", None),
        None,
    );
}

#[test]
fn compiled_lineage_latest_binding_reduces_over_65536_rows_with_last_input_tie() {
    let old_key = runtime_response_turn_state_lineage_key("resp", "old");
    let new_key = runtime_response_turn_state_lineage_key("resp", "new");
    let malformed = "non-lineage-key";
    let entries = (0..65_537)
        .map(|index| {
            let key = if index == 0 {
                old_key.as_str()
            } else if index == 65_536 {
                new_key.as_str()
            } else {
                malformed
            };
            binding(
                key,
                "main",
                if index == 0 || index == 65_536 {
                    42
                } else {
                    i64::MAX
                },
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        runtime_previous_response_turn_state_from_bindings(entries, "resp", Some("main")),
        Some("new".to_owned()),
        "second-stage compiled Mojo selection resolves cross-batch timestamp ties",
    );
}
