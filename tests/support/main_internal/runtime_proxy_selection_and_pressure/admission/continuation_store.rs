use super::*;

#[test]
fn merge_runtime_continuation_store_keeps_compact_session_release_tombstone() {
    let profiles = BTreeMap::from([(
        "main".to_string(),
        ProfileEntry {
            codex_home: PathBuf::from("/tmp/main"),
            managed: true,
            email: Some("main@example.com".to_string()),
            provider: ProfileProvider::Openai,
        },
    )]);
    let now = Local::now().timestamp();
    let key = runtime_compact_session_lineage_key("sess-compact");

    let existing = RuntimeContinuationStore {
        session_id_bindings: BTreeMap::from([(
            key.clone(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: now - 30,
            },
        )]),
        statuses: RuntimeContinuationStatuses {
            session_id: BTreeMap::from([(
                key.clone(),
                RuntimeContinuationBindingStatus {
                    state: RuntimeContinuationBindingLifecycle::Verified,
                    confidence: 2,
                    last_touched_at: Some(now - 30),
                    last_verified_at: Some(now - 30),
                    last_verified_route: Some("compact".to_string()),
                    last_not_found_at: None,
                    not_found_streak: 0,
                    success_count: 1,
                    failure_count: 0,
                },
            )]),
            ..RuntimeContinuationStatuses::default()
        },
        ..RuntimeContinuationStore::default()
    };
    let incoming = RuntimeContinuationStore {
        statuses: RuntimeContinuationStatuses {
            session_id: BTreeMap::from([(
                key.clone(),
                RuntimeContinuationBindingStatus {
                    last_verified_route: Some("compact".to_string()),
                    ..dead_continuation_status(now)
                },
            )]),
            ..RuntimeContinuationStatuses::default()
        },
        ..RuntimeContinuationStore::default()
    };

    let merged = merge_runtime_continuation_store(&existing, &incoming, &profiles);
    assert!(
        !merged.session_id_bindings.contains_key(&key),
        "compact session binding should be removed when a newer release tombstone exists"
    );
    assert_eq!(
        merged
            .statuses
            .session_id
            .get(&key)
            .map(|status| status.state),
        Some(RuntimeContinuationBindingLifecycle::Dead)
    );
}

fn single_profile_test_profiles(temp_dir: &TestDir) -> BTreeMap<String, ProfileEntry> {
    let profile_home = temp_dir.path.join("homes/main");
    write_auth_json(&profile_home.join("auth.json"), "main-account");
    BTreeMap::from([(
        "main".to_string(),
        ProfileEntry {
            codex_home: profile_home,
            managed: true,
            email: Some("main@example.com".to_string()),
            provider: ProfileProvider::Openai,
        },
    )])
}

fn insert_continuation_binding(
    store: &mut RuntimeContinuationStore,
    kind: RuntimeContinuationBindingKind,
    key: &str,
    bound_at: i64,
) {
    let binding = ResponseProfileBinding {
        profile_name: "main".to_string(),
        bound_at,
    };
    match kind {
        RuntimeContinuationBindingKind::Response => {
            store
                .response_profile_bindings
                .insert(key.to_string(), binding);
        }
        RuntimeContinuationBindingKind::TurnState => {
            store.turn_state_bindings.insert(key.to_string(), binding);
        }
        RuntimeContinuationBindingKind::SessionId => {
            store
                .session_profile_bindings
                .insert(key.to_string(), binding.clone());
            store.session_id_bindings.insert(key.to_string(), binding);
        }
    }
}

fn insert_dead_continuation_status(
    store: &mut RuntimeContinuationStore,
    kind: RuntimeContinuationBindingKind,
    key: &str,
    dead_at: i64,
) {
    let status = dead_continuation_status(dead_at);
    match kind {
        RuntimeContinuationBindingKind::Response => {
            store.statuses.response.insert(key.to_string(), status);
        }
        RuntimeContinuationBindingKind::TurnState => {
            store.statuses.turn_state.insert(key.to_string(), status);
        }
        RuntimeContinuationBindingKind::SessionId => {
            store.statuses.session_id.insert(key.to_string(), status);
        }
    }
}

fn continuation_binding_present(
    store: &RuntimeContinuationStore,
    kind: RuntimeContinuationBindingKind,
    key: &str,
) -> bool {
    match kind {
        RuntimeContinuationBindingKind::Response => {
            store.response_profile_bindings.contains_key(key)
        }
        RuntimeContinuationBindingKind::TurnState => store.turn_state_bindings.contains_key(key),
        RuntimeContinuationBindingKind::SessionId => {
            store.session_profile_bindings.contains_key(key)
                && store.session_id_bindings.contains_key(key)
        }
    }
}

fn continuation_dead_status_present(
    store: &RuntimeContinuationStore,
    kind: RuntimeContinuationBindingKind,
    key: &str,
) -> bool {
    let status = match kind {
        RuntimeContinuationBindingKind::Response => store.statuses.response.get(key),
        RuntimeContinuationBindingKind::TurnState => store.statuses.turn_state.get(key),
        RuntimeContinuationBindingKind::SessionId => store.statuses.session_id.get(key),
    };
    status.is_some_and(|status| status.state == RuntimeContinuationBindingLifecycle::Dead)
}


#[test]
fn runtime_continuation_status_pruning_uses_evidence_over_age() {
    let temp_dir = TestDir::isolated();
    let profile_home = temp_dir.path.join("homes/main");
    write_auth_json(&profile_home.join("auth.json"), "main-account");
    let profiles = BTreeMap::from([(
        "main".to_string(),
        ProfileEntry {
            codex_home: profile_home,
            managed: true,
            email: Some("main@example.com".to_string()),
            provider: ProfileProvider::Openai,
        },
    )]);
    let now = Local::now().timestamp();
    let stale_bound_at = now - 365 * 24 * 60 * 60;
    let mut statuses = RuntimeContinuationStatuses::default();

    assert!(runtime_mark_continuation_status_verified(
        &mut statuses,
        RuntimeContinuationBindingKind::Response,
        "resp-main",
        now,
        Some(RuntimeRouteKind::Responses),
    ));
    assert_eq!(
        statuses
            .response
            .get("resp-main")
            .and_then(|status| status.last_verified_route.as_deref()),
        Some("responses")
    );
    assert!(runtime_mark_continuation_status_suspect(
        &mut statuses,
        RuntimeContinuationBindingKind::Response,
        "resp-main",
        now + 1,
    ));

    let retained = compact_runtime_continuation_store(
        RuntimeContinuationStore {
            response_profile_bindings: BTreeMap::from([(
                "resp-main".to_string(),
                ResponseProfileBinding {
                    profile_name: "main".to_string(),
                    bound_at: stale_bound_at,
                },
            )]),
            statuses: statuses.clone(),
            ..RuntimeContinuationStore::default()
        },
        &profiles,
    );
    assert!(retained.response_profile_bindings.contains_key("resp-main"));
    assert_eq!(
        retained
            .statuses
            .response
            .get("resp-main")
            .map(|status| status.state),
        Some(RuntimeContinuationBindingLifecycle::Suspect)
    );

    assert!(runtime_mark_continuation_status_suspect(
        &mut statuses,
        RuntimeContinuationBindingKind::Response,
        "resp-main",
        now + 2,
    ));

    let pruned = compact_runtime_continuation_store(
        RuntimeContinuationStore {
            response_profile_bindings: BTreeMap::from([(
                "resp-main".to_string(),
                ResponseProfileBinding {
                    profile_name: "main".to_string(),
                    bound_at: stale_bound_at,
                },
            )]),
            statuses,
            ..RuntimeContinuationStore::default()
        },
        &profiles,
    );
    assert!(!pruned.response_profile_bindings.contains_key("resp-main"));
    assert_eq!(
        pruned
            .statuses
            .response
            .get("resp-main")
            .map(|status| status.state),
        Some(RuntimeContinuationBindingLifecycle::Dead)
    );
}

#[test]
fn runtime_dead_continuation_tombstone_blocks_stale_binding_resurrection() {
    let temp_dir = TestDir::isolated();
    let profile_home = temp_dir.path.join("homes/main");
    write_auth_json(&profile_home.join("auth.json"), "main-account");
    let profiles = BTreeMap::from([(
        "main".to_string(),
        ProfileEntry {
            codex_home: profile_home,
            managed: true,
            email: Some("main@example.com".to_string()),
            provider: ProfileProvider::Openai,
        },
    )]);
    let now = Local::now().timestamp();
    let mut tombstone_statuses = RuntimeContinuationStatuses::default();

    assert!(runtime_mark_continuation_status_verified(
        &mut tombstone_statuses,
        RuntimeContinuationBindingKind::Response,
        "resp-main",
        now - 2,
        Some(RuntimeRouteKind::Responses),
    ));
    assert!(runtime_mark_continuation_status_dead(
        &mut tombstone_statuses,
        RuntimeContinuationBindingKind::Response,
        "resp-main",
        now,
    ));

    let merged = merge_runtime_continuation_store(
        &RuntimeContinuationStore {
            statuses: tombstone_statuses,
            ..RuntimeContinuationStore::default()
        },
        &RuntimeContinuationStore {
            response_profile_bindings: BTreeMap::from([(
                "resp-main".to_string(),
                ResponseProfileBinding {
                    profile_name: "main".to_string(),
                    bound_at: now - 1,
                },
            )]),
            ..RuntimeContinuationStore::default()
        },
        &profiles,
    );

    assert!(
        !merged.response_profile_bindings.contains_key("resp-main"),
        "dead tombstone should prune stale resurrected binding"
    );
    assert_eq!(
        merged
            .statuses
            .response
            .get("resp-main")
            .map(|status| status.state),
        Some(RuntimeContinuationBindingLifecycle::Dead)
    );
}

#[test]
fn runtime_continuation_tombstone_merge_boundaries_cover_all_binding_kinds() {
    #[derive(Clone, Copy)]
    struct TombstoneCase {
        name: &'static str,
        existing_binding_at: Option<i64>,
        incoming_binding_at: Option<i64>,
        existing_dead_at: Option<i64>,
        incoming_dead_at: Option<i64>,
        expect_binding: bool,
        expect_dead: bool,
    }

    let now = Local::now().timestamp();
    let cases = [
        TombstoneCase {
            name: "existing_dead_prunes_older_incoming_binding",
            existing_binding_at: None,
            incoming_binding_at: Some(now - 1),
            existing_dead_at: Some(now),
            incoming_dead_at: None,
            expect_binding: false,
            expect_dead: true,
        },
        TombstoneCase {
            name: "existing_dead_prunes_same_second_incoming_binding",
            existing_binding_at: None,
            incoming_binding_at: Some(now),
            existing_dead_at: Some(now),
            incoming_dead_at: None,
            expect_binding: false,
            expect_dead: true,
        },
        TombstoneCase {
            name: "newer_incoming_binding_shadows_existing_dead",
            existing_binding_at: None,
            incoming_binding_at: Some(now + 1),
            existing_dead_at: Some(now),
            incoming_dead_at: None,
            expect_binding: true,
            expect_dead: false,
        },
        TombstoneCase {
            name: "incoming_dead_prunes_existing_binding",
            existing_binding_at: Some(now),
            incoming_binding_at: None,
            existing_dead_at: None,
            incoming_dead_at: Some(now + 1),
            expect_binding: false,
            expect_dead: true,
        },
        TombstoneCase {
            name: "incoming_same_second_dead_prunes_existing_binding",
            existing_binding_at: Some(now),
            incoming_binding_at: None,
            existing_dead_at: None,
            incoming_dead_at: Some(now),
            expect_binding: false,
            expect_dead: true,
        },
        TombstoneCase {
            name: "newer_existing_binding_shadows_incoming_dead",
            existing_binding_at: Some(now + 1),
            incoming_binding_at: None,
            existing_dead_at: None,
            incoming_dead_at: Some(now),
            expect_binding: true,
            expect_dead: false,
        },
    ];

    for kind in [
        RuntimeContinuationBindingKind::Response,
        RuntimeContinuationBindingKind::TurnState,
        RuntimeContinuationBindingKind::SessionId,
    ] {
        let key = match kind {
            RuntimeContinuationBindingKind::Response => "resp-main",
            RuntimeContinuationBindingKind::TurnState => "turn-main",
            RuntimeContinuationBindingKind::SessionId => "sess-main",
        };

        for case in cases {
            let temp_dir = TestDir::isolated();
            let profiles = single_profile_test_profiles(&temp_dir);
            let mut existing = RuntimeContinuationStore::default();
            let mut incoming = RuntimeContinuationStore::default();

            if let Some(bound_at) = case.existing_binding_at {
                insert_continuation_binding(&mut existing, kind, key, bound_at);
            }
            if let Some(bound_at) = case.incoming_binding_at {
                insert_continuation_binding(&mut incoming, kind, key, bound_at);
            }
            if let Some(dead_at) = case.existing_dead_at {
                insert_dead_continuation_status(&mut existing, kind, key, dead_at);
            }
            if let Some(dead_at) = case.incoming_dead_at {
                insert_dead_continuation_status(&mut incoming, kind, key, dead_at);
            }

            let merged = merge_runtime_continuation_store(&existing, &incoming, &profiles);

            assert_eq!(
                continuation_binding_present(&merged, kind, key),
                case.expect_binding,
                "kind={kind:?} case={}",
                case.name
            );
            assert_eq!(
                continuation_dead_status_present(&merged, kind, key),
                case.expect_dead,
                "kind={kind:?} case={}",
                case.name
            );
        }
    }
}

#[test]
fn runtime_dead_continuation_tombstone_overrides_same_second_verified_status() {
    let temp_dir = TestDir::isolated();
    let profile_home = temp_dir.path.join("homes/main");
    write_auth_json(&profile_home.join("auth.json"), "main-account");
    let profiles = BTreeMap::from([(
        "main".to_string(),
        ProfileEntry {
            codex_home: profile_home,
            managed: true,
            email: Some("main@example.com".to_string()),
            provider: ProfileProvider::Openai,
        },
    )]);
    let now = Local::now().timestamp();
    let mut existing_statuses = RuntimeContinuationStatuses::default();
    let mut incoming_statuses = RuntimeContinuationStatuses::default();

    assert!(runtime_mark_continuation_status_verified(
        &mut existing_statuses,
        RuntimeContinuationBindingKind::Response,
        "resp-main",
        now,
        Some(RuntimeRouteKind::Responses),
    ));
    assert!(runtime_mark_continuation_status_dead(
        &mut incoming_statuses,
        RuntimeContinuationBindingKind::Response,
        "resp-main",
        now,
    ));

    let merged = merge_runtime_continuation_store(
        &RuntimeContinuationStore {
            response_profile_bindings: BTreeMap::from([(
                "resp-main".to_string(),
                ResponseProfileBinding {
                    profile_name: "main".to_string(),
                    bound_at: now,
                },
            )]),
            statuses: existing_statuses,
            ..RuntimeContinuationStore::default()
        },
        &RuntimeContinuationStore {
            statuses: incoming_statuses,
            ..RuntimeContinuationStore::default()
        },
        &profiles,
    );

    assert!(
        !merged.response_profile_bindings.contains_key("resp-main"),
        "same-second dead tombstone should still prune the released binding"
    );
    assert_eq!(
        merged
            .statuses
            .response
            .get("resp-main")
            .map(|status| status.state),
        Some(RuntimeContinuationBindingLifecycle::Dead)
    );
}

#[test]
fn runtime_newer_binding_overrides_older_dead_tombstone() {
    let temp_dir = TestDir::isolated();
    let profile_home = temp_dir.path.join("homes/main");
    write_auth_json(&profile_home.join("auth.json"), "main-account");
    let profiles = BTreeMap::from([(
        "main".to_string(),
        ProfileEntry {
            codex_home: profile_home,
            managed: true,
            email: Some("main@example.com".to_string()),
            provider: ProfileProvider::Openai,
        },
    )]);
    let now = Local::now().timestamp();
    let mut statuses = RuntimeContinuationStatuses::default();

    assert!(runtime_mark_continuation_status_verified(
        &mut statuses,
        RuntimeContinuationBindingKind::Response,
        "resp-main",
        now - 5,
        Some(RuntimeRouteKind::Responses),
    ));
    assert!(runtime_mark_continuation_status_dead(
        &mut statuses,
        RuntimeContinuationBindingKind::Response,
        "resp-main",
        now - 3,
    ));

    let compacted = compact_runtime_continuation_store(
        RuntimeContinuationStore {
            response_profile_bindings: BTreeMap::from([(
                "resp-main".to_string(),
                ResponseProfileBinding {
                    profile_name: "main".to_string(),
                    bound_at: now,
                },
            )]),
            statuses,
            ..RuntimeContinuationStore::default()
        },
        &profiles,
    );

    assert_eq!(
        compacted
            .response_profile_bindings
            .get("resp-main")
            .map(|binding| binding.profile_name.as_str()),
        Some("main")
    );
    assert!(
        !compacted.statuses.response.contains_key("resp-main"),
        "older dead tombstone should not suppress a newer binding"
    );
}

#[test]
fn runtime_continuation_store_compaction_prunes_response_bindings_to_limit() {
    let temp_dir = TestDir::isolated();
    let profile_home = temp_dir.path.join("homes/main");
    write_auth_json(&profile_home.join("auth.json"), "main-account");
    let profiles = BTreeMap::from([(
        "main".to_string(),
        ProfileEntry {
            codex_home: profile_home,
            managed: true,
            email: Some("main@example.com".to_string()),
            provider: ProfileProvider::Openai,
        },
    )]);
    let now = Local::now().timestamp();
    let mut response_profile_bindings = BTreeMap::new();
    for index in 0..(RESPONSE_PROFILE_BINDING_LIMIT + 1) {
        response_profile_bindings.insert(
            format!("resp-{index:06}"),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: now - (RESPONSE_PROFILE_BINDING_LIMIT + 1) as i64 + index as i64,
            },
        );
    }
    let hot_response_id = "resp-000000".to_string();
    let displaced_response_id = "resp-000001".to_string();

    let compacted = compact_runtime_continuation_store(
        RuntimeContinuationStore {
            response_profile_bindings,
            statuses: RuntimeContinuationStatuses {
                response: BTreeMap::from([(
                    hot_response_id.clone(),
                    RuntimeContinuationBindingStatus {
                        state: RuntimeContinuationBindingLifecycle::Verified,
                        confidence: RUNTIME_CONTINUATION_CONFIDENCE_MAX,
                        last_touched_at: Some(now),
                        last_verified_at: Some(now),
                        last_verified_route: Some("responses".to_string()),
                        last_not_found_at: None,
                        not_found_streak: 0,
                        success_count: 3,
                        failure_count: 0,
                    },
                )]),
                ..RuntimeContinuationStatuses::default()
            },
            ..RuntimeContinuationStore::default()
        },
        &profiles,
    );

    assert_eq!(
        compacted.response_profile_bindings.len(),
        RESPONSE_PROFILE_BINDING_LIMIT
    );
    assert!(
        compacted
            .response_profile_bindings
            .contains_key(&hot_response_id),
        "verified hot response binding should survive compaction even when it is oldest"
    );
    assert!(
        compacted
            .response_profile_bindings
            .contains_key(&format!("resp-{0:06}", RESPONSE_PROFILE_BINDING_LIMIT)),
        "newest cold response binding should still be retained"
    );
    assert!(
        !compacted
            .response_profile_bindings
            .contains_key(&displaced_response_id),
        "colder bindings should be pruned ahead of the oldest verified binding"
    );
}

#[test]
fn runtime_continuation_store_compaction_prunes_response_statuses_to_limit() {
    let temp_dir = TestDir::isolated();
    let profile_home = temp_dir.path.join("homes/main");
    write_auth_json(&profile_home.join("auth.json"), "main-account");
    let profiles = BTreeMap::from([(
        "main".to_string(),
        ProfileEntry {
            codex_home: profile_home,
            managed: true,
            email: Some("main@example.com".to_string()),
            provider: ProfileProvider::Openai,
        },
    )]);
    let now = Local::now().timestamp();
    let mut response_profile_bindings = BTreeMap::new();
    let mut response_statuses = BTreeMap::new();
    for index in 0..(RUNTIME_CONTINUATION_RESPONSE_STATUS_LIMIT + 3) {
        let response_id = format!("resp-{index:06}");
        response_profile_bindings.insert(
            response_id.clone(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: now - 1_000 + index as i64,
            },
        );
        response_statuses.insert(
            response_id,
            RuntimeContinuationBindingStatus {
                state: RuntimeContinuationBindingLifecycle::Warm,
                confidence: 1,
                last_touched_at: Some(now - 1_000 + index as i64),
                last_verified_at: None,
                last_verified_route: None,
                last_not_found_at: None,
                not_found_streak: 0,
                success_count: 0,
                failure_count: 0,
            },
        );
    }
    response_statuses.insert(
        "resp-hot".to_string(),
        RuntimeContinuationBindingStatus {
            state: RuntimeContinuationBindingLifecycle::Verified,
            confidence: RUNTIME_CONTINUATION_CONFIDENCE_MAX,
            last_touched_at: Some(now),
            last_verified_at: Some(now),
            last_verified_route: Some("responses".to_string()),
            last_not_found_at: None,
            not_found_streak: 0,
            success_count: 5,
            failure_count: 0,
        },
    );
    response_profile_bindings.insert(
        "resp-hot".to_string(),
        ResponseProfileBinding {
            profile_name: "main".to_string(),
            bound_at: now - 2_000,
        },
    );

    let compacted = compact_runtime_continuation_store(
        RuntimeContinuationStore {
            response_profile_bindings,
            statuses: RuntimeContinuationStatuses {
                response: response_statuses,
                ..RuntimeContinuationStatuses::default()
            },
            ..RuntimeContinuationStore::default()
        },
        &profiles,
    );

    assert_eq!(
        compacted.statuses.response.len(),
        RUNTIME_CONTINUATION_RESPONSE_STATUS_LIMIT
    );
    assert!(
        compacted.statuses.response.contains_key("resp-hot"),
        "verified status should survive hard status cap"
    );
    assert!(
        !compacted.statuses.response.contains_key("resp-000000"),
        "coldest status should be dropped deterministically"
    );
}

#[test]
fn runtime_continuation_store_compaction_keeps_verified_hot_binding_over_newer_cold_binding() {
    let temp_dir = TestDir::isolated();
    let profile_home = temp_dir.path.join("homes/main");
    write_auth_json(&profile_home.join("auth.json"), "main-account");
    let profiles = BTreeMap::from([(
        "main".to_string(),
        ProfileEntry {
            codex_home: profile_home,
            managed: true,
            email: Some("main@example.com".to_string()),
            provider: ProfileProvider::Openai,
        },
    )]);
    let now = Local::now().timestamp();

    let compacted = compact_runtime_continuation_store(
        RuntimeContinuationStore {
            response_profile_bindings: BTreeMap::from([
                (
                    "resp-hot".to_string(),
                    ResponseProfileBinding {
                        profile_name: "main".to_string(),
                        bound_at: now - 3,
                    },
                ),
                (
                    "resp-cold-1".to_string(),
                    ResponseProfileBinding {
                        profile_name: "main".to_string(),
                        bound_at: now - 2,
                    },
                ),
                (
                    "resp-cold-2".to_string(),
                    ResponseProfileBinding {
                        profile_name: "main".to_string(),
                        bound_at: now - 1,
                    },
                ),
            ]),
            statuses: RuntimeContinuationStatuses {
                response: BTreeMap::from([(
                    "resp-hot".to_string(),
                    RuntimeContinuationBindingStatus {
                        state: RuntimeContinuationBindingLifecycle::Verified,
                        confidence: RUNTIME_CONTINUATION_CONFIDENCE_MAX,
                        success_count: 3,
                        failure_count: 0,
                        not_found_streak: 0,
                        last_touched_at: Some(now),
                        last_not_found_at: None,
                        last_verified_at: Some(now),
                        last_verified_route: Some("responses".to_string()),
                    },
                )]),
                ..RuntimeContinuationStatuses::default()
            },
            ..RuntimeContinuationStore::default()
        },
        &profiles,
    );

    let mut retained = compacted
        .response_profile_bindings
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    retained.sort();

    assert_eq!(retained.len(), 3);
    assert!(retained.contains(&"resp-hot".to_string()));

    let mut pruned = RuntimeContinuationStore {
        response_profile_bindings: compacted.response_profile_bindings,
        statuses: compacted.statuses,
        ..RuntimeContinuationStore::default()
    };
    prune_runtime_continuation_response_bindings(
        &mut pruned.response_profile_bindings,
        &pruned.statuses.response,
        2,
    );
    assert!(pruned.response_profile_bindings.contains_key("resp-hot"));
    assert!(
        !pruned.response_profile_bindings.contains_key("resp-cold-1"),
        "the cold binding should be pruned before the older verified binding"
    );
}
