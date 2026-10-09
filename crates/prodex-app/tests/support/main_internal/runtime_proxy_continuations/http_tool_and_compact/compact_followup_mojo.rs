use super::*;

#[test]
fn compact_followup_consumer_uses_mojo_plan_for_dead_and_live_binding_precedence() {
    let calls_before = runtime_compact_followup_mojo_confirmed_calls();
    let now = Local::now().timestamp();
    let key = runtime_compact_turn_state_lineage_key("turn-mojo-consumer");
    let profile = "consumer-owner";
    let temp_dir = TestDir::isolated();
    let mut runtime = RuntimeProxyFixtureBuilder::new().build_runtime(&temp_dir);
    runtime.state.profiles.insert(
        profile.to_string(),
        ProfileEntry {
            codex_home: temp_dir.path.join("homes/consumer-owner"),
            managed: false,
            email: None,
            provider: ProfileProvider::Openai,
        },
    );
    runtime.turn_state_bindings.insert(
        key.clone(),
        ResponseProfileBinding {
            binding_identity: None,
            profile_name: profile.to_string(),
            bound_at: now.saturating_add(60),
        },
    );
    runtime.continuation_statuses.turn_state.insert(
        key.clone(),
        dead_continuation_status(now),
    );
    let shared = runtime_rotation_proxy_shared(&temp_dir, runtime, 1);

    let mut runtime = shared.runtime.lock().expect("runtime lock");
    let selected = runtime_touch_compact_lineage_binding(
        &shared,
        &mut runtime,
        &key,
        RuntimeStateMutation::CompactTurnStateTouch("turn-mojo-consumer".to_string()),
        false,
    );
    assert_eq!(selected.as_deref(), Some(profile));
    assert!(runtime.continuation_statuses.turn_state.contains_key(&key));
    assert_eq!(runtime.turn_state_bindings[&key].bound_at, now.saturating_add(60));

    assert_eq!(
        runtime_touch_compact_lineage_binding(
            &shared,
            &mut runtime,
            "missing-lineage-key",
            RuntimeStateMutation::CompactTurnStateTouch("missing".to_string()),
            false,
        ),
        None,
        "missing lineage must not acquire an owner"
    );

    let wrong_owner_key = runtime_compact_turn_state_lineage_key("turn-removed-owner");
    runtime.turn_state_bindings.insert(
        wrong_owner_key.clone(),
        ResponseProfileBinding {
            binding_identity: None,
            profile_name: "removed-profile".to_string(),
            bound_at: now,
        },
    );
    assert_eq!(
        runtime_touch_compact_lineage_binding(
            &shared,
            &mut runtime,
            &wrong_owner_key,
            RuntimeStateMutation::CompactTurnStateTouch("turn-removed-owner".to_string()),
            false,
        )
        .as_deref(),
        Some(prodex_runtime_state::RUNTIME_HARD_BINDING_CONFLICT_PROFILE),
        "unavailable binding owners must fail closed"
    );
    assert!(
        runtime_compact_followup_mojo_confirmed_calls().saturating_sub(calls_before) >= 3,
        "live, missing, and unavailable compact lineage must each reach Mojo"
    );
}
