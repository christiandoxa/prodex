use super::*;

#[test]
fn clear_runtime_stale_previous_response_binding_marks_dead_tombstone() {
    let temp_dir = TestDir::isolated();
    let main_home = temp_dir.path.join("homes/main");
    write_auth_json(&main_home.join("auth.json"), "main-account");

    let shared = runtime_rotation_proxy_shared(
        &temp_dir,
        RuntimeRotationState {
            paths: AppPaths {
                root: temp_dir.path.join("prodex"),
                state_file: temp_dir.path.join("prodex/state.json"),
                managed_profiles_root: temp_dir.path.join("prodex/profiles"),
                shared_codex_root: temp_dir.path.join("shared"),
                legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
            },
            state: AppState {
                active_profile: Some("main".to_string()),
                profiles: BTreeMap::from([(
                    "main".to_string(),
                    ProfileEntry {
                        codex_home: main_home,
                        managed: true,
                        email: Some("main@example.com".to_string()),
                        provider: ProfileProvider::Openai,
                    },
                )]),
                last_run_selected_at: BTreeMap::new(),
                response_profile_bindings: BTreeMap::new(),
                session_profile_bindings: BTreeMap::new(),
            },
            upstream_base_url: "http://127.0.0.1:1/backend-api".to_string(),
            include_code_review: false,
            current_profile: "main".to_string(),
            profile_usage_auth: BTreeMap::new(),
            turn_state_bindings: BTreeMap::new(),
            session_id_bindings: BTreeMap::new(),
            continuation_statuses: RuntimeContinuationStatuses::default(),
            profile_probe_cache: BTreeMap::new(),
            profile_usage_snapshots: BTreeMap::new(),
            profile_retry_backoff_until: BTreeMap::new(),
            profile_transport_backoff_until: BTreeMap::new(),
            profile_route_circuit_open_until: BTreeMap::new(),
            profile_health: BTreeMap::new(),
        },
        usize::MAX,
    );

    remember_runtime_response_ids_with_turn_state(
        &shared,
        "main",
        &[String::from("resp-main")],
        Some("turn-main"),
        RuntimeRouteKind::Responses,
    )
    .expect("response affinity should be recorded");
    wait_for_runtime_background_queues_idle();

    assert!(
        clear_runtime_stale_previous_response_binding(&shared, "main", Some("resp-main"))
            .expect("stale clear should succeed")
    );
    wait_for_runtime_background_queues_idle();

    let runtime = shared.runtime.lock().expect("runtime lock should succeed");
    assert!(
        !runtime
            .state
            .response_profile_bindings
            .contains_key("resp-main"),
        "cleared stale binding should be removed from live affinity"
    );
    assert_eq!(
        runtime
            .continuation_statuses
            .response
            .get("resp-main")
            .map(|status| status.state),
        Some(RuntimeContinuationBindingLifecycle::Dead)
    );
}

#[test]
fn clear_runtime_dead_response_bindings_clears_turn_state_when_last_lineage_dies() {
    let temp_dir = TestDir::isolated();
    let shared = runtime_shared_for_dead_response_binding_cleanup(&temp_dir);
    let response_ids = vec!["resp-main".to_string(), "resp-main-next".to_string()];
    let first_lineage = runtime_response_turn_state_lineage_key("resp-main", "turn-main");
    let second_lineage = runtime_response_turn_state_lineage_key("resp-main-next", "turn-main");

    remember_runtime_response_ids_with_turn_state(
        &shared,
        "main",
        &response_ids,
        Some("turn-main"),
        RuntimeRouteKind::Responses,
    )
    .expect("response affinity should be recorded");
    remember_runtime_turn_state(
        &shared,
        "main",
        Some("turn-main"),
        RuntimeRouteKind::Responses,
    )
    .expect("turn-state affinity should be recorded");
    wait_for_runtime_background_queues_idle();

    assert!(
        clear_runtime_dead_response_bindings(&shared, "main", &response_ids, "test")
            .expect("dead binding clear should succeed")
    );
    wait_for_runtime_background_queues_idle();

    let runtime = shared.runtime.lock().expect("runtime lock should succeed");
    assert!(
        !runtime
            .state
            .response_profile_bindings
            .contains_key("resp-main"),
        "dead response id should be removed from live affinity"
    );
    assert!(
        !runtime
            .state
            .response_profile_bindings
            .contains_key("resp-main-next"),
        "follow-up response id should be removed from live affinity"
    );
    assert!(
        !runtime
            .state
            .response_profile_bindings
            .contains_key(&first_lineage),
        "dead response-turn_state lineage should be removed"
    );
    assert!(
        !runtime
            .state
            .response_profile_bindings
            .contains_key(&second_lineage),
        "all response-turn_state lineage should be removed when the chain dies"
    );
    assert!(
        !runtime.turn_state_bindings.contains_key("turn-main"),
        "direct turn_state affinity should be removed once no response lineage remains"
    );
    assert_eq!(
        runtime
            .continuation_statuses
            .response
            .get("resp-main")
            .map(|status| status.state),
        Some(RuntimeContinuationBindingLifecycle::Dead)
    );
    assert_eq!(
        runtime
            .continuation_statuses
            .response
            .get("resp-main-next")
            .map(|status| status.state),
        Some(RuntimeContinuationBindingLifecycle::Dead)
    );
    assert_eq!(
        runtime
            .continuation_statuses
            .turn_state
            .get("turn-main")
            .map(|status| status.state),
        Some(RuntimeContinuationBindingLifecycle::Dead)
    );
}

#[test]
fn clear_runtime_dead_response_bindings_keeps_turn_state_when_other_lineage_survives() {
    let temp_dir = TestDir::isolated();
    let shared = runtime_shared_for_dead_response_binding_cleanup(&temp_dir);
    let remaining_lineage = runtime_response_turn_state_lineage_key("resp-main-next", "turn-main");

    remember_runtime_response_ids_with_turn_state(
        &shared,
        "main",
        &[String::from("resp-main"), String::from("resp-main-next")],
        Some("turn-main"),
        RuntimeRouteKind::Responses,
    )
    .expect("response affinity should be recorded");
    remember_runtime_turn_state(
        &shared,
        "main",
        Some("turn-main"),
        RuntimeRouteKind::Responses,
    )
    .expect("turn-state affinity should be recorded");
    wait_for_runtime_background_queues_idle();

    assert!(
        clear_runtime_dead_response_bindings(
            &shared,
            "main",
            &[String::from("resp-main")],
            "test",
        )
        .expect("dead binding clear should succeed")
    );
    wait_for_runtime_background_queues_idle();

    let runtime = shared.runtime.lock().expect("runtime lock should succeed");
    assert!(
        !runtime
            .state
            .response_profile_bindings
            .contains_key("resp-main"),
        "cleared dead response id should be removed"
    );
    assert!(
        runtime
            .state
            .response_profile_bindings
            .contains_key("resp-main-next"),
        "live sibling response id should survive"
    );
    assert!(
        runtime
            .state
            .response_profile_bindings
            .contains_key(&remaining_lineage),
        "remaining response-turn_state lineage should survive"
    );
    assert!(
        runtime.turn_state_bindings.contains_key("turn-main"),
        "direct turn_state affinity should survive while another live lineage exists"
    );
    assert_eq!(
        runtime
            .continuation_statuses
            .response
            .get("resp-main")
            .map(|status| status.state),
        Some(RuntimeContinuationBindingLifecycle::Dead)
    );
    assert_eq!(
        runtime
            .continuation_statuses
            .turn_state
            .get("turn-main")
            .map(|status| status.state),
        Some(RuntimeContinuationBindingLifecycle::Verified)
    );
}

#[test]
fn clear_runtime_dead_response_bindings_clears_all_removed_turn_state_affinities() {
    let temp_dir = TestDir::isolated();
    let shared = runtime_shared_for_dead_response_binding_cleanup(&temp_dir);
    let response_ids = vec![String::from("resp-main")];

    remember_runtime_response_ids_with_turn_state(
        &shared,
        "main",
        &response_ids,
        Some("turn-first"),
        RuntimeRouteKind::Responses,
    )
    .expect("first response-turn_state lineage should be recorded");
    remember_runtime_response_ids_with_turn_state(
        &shared,
        "main",
        &response_ids,
        Some("turn-second"),
        RuntimeRouteKind::Responses,
    )
    .expect("second response-turn_state lineage should be recorded");
    remember_runtime_turn_state(
        &shared,
        "main",
        Some("turn-first"),
        RuntimeRouteKind::Responses,
    )
    .expect("first direct turn-state affinity should be recorded");
    remember_runtime_turn_state(
        &shared,
        "main",
        Some("turn-second"),
        RuntimeRouteKind::Responses,
    )
    .expect("second direct turn-state affinity should be recorded");
    wait_for_runtime_background_queues_idle();

    assert!(
        clear_runtime_dead_response_bindings(&shared, "main", &response_ids, "test")
            .expect("dead binding clear should succeed")
    );
    wait_for_runtime_background_queues_idle();

    let runtime = shared.runtime.lock().expect("runtime lock should succeed");
    assert!(
        !runtime.turn_state_bindings.contains_key("turn-first"),
        "first removed turn_state should be cleared"
    );
    assert!(
        !runtime.turn_state_bindings.contains_key("turn-second"),
        "second removed turn_state should be cleared"
    );
    assert_eq!(
        runtime
            .continuation_statuses
            .turn_state
            .get("turn-first")
            .map(|status| status.state),
        Some(RuntimeContinuationBindingLifecycle::Dead)
    );
    assert_eq!(
        runtime
            .continuation_statuses
            .turn_state
            .get("turn-second")
            .map(|status| status.state),
        Some(RuntimeContinuationBindingLifecycle::Dead)
    );
}
