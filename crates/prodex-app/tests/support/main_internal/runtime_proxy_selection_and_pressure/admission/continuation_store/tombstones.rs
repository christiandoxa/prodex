use super::*;

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
        existing_binding_offset: Option<i64>,
        incoming_binding_offset: Option<i64>,
        existing_dead_offset: Option<i64>,
        incoming_dead_offset: Option<i64>,
        expect_binding: bool,
        expect_dead: bool,
    }

    let cases = [
        TombstoneCase {
            name: "existing_dead_prunes_older_incoming_binding",
            existing_binding_offset: None,
            incoming_binding_offset: Some(-1),
            existing_dead_offset: Some(0),
            incoming_dead_offset: None,
            expect_binding: false,
            expect_dead: true,
        },
        TombstoneCase {
            name: "existing_dead_prunes_same_second_incoming_binding",
            existing_binding_offset: None,
            incoming_binding_offset: Some(0),
            existing_dead_offset: Some(0),
            incoming_dead_offset: None,
            expect_binding: false,
            expect_dead: true,
        },
        TombstoneCase {
            name: "newer_incoming_binding_shadows_existing_dead",
            existing_binding_offset: None,
            incoming_binding_offset: Some(1),
            existing_dead_offset: Some(0),
            incoming_dead_offset: None,
            expect_binding: true,
            expect_dead: false,
        },
        TombstoneCase {
            name: "incoming_dead_prunes_existing_binding",
            existing_binding_offset: Some(0),
            incoming_binding_offset: None,
            existing_dead_offset: None,
            incoming_dead_offset: Some(1),
            expect_binding: false,
            expect_dead: true,
        },
        TombstoneCase {
            name: "incoming_same_second_dead_prunes_existing_binding",
            existing_binding_offset: Some(0),
            incoming_binding_offset: None,
            existing_dead_offset: None,
            incoming_dead_offset: Some(0),
            expect_binding: false,
            expect_dead: true,
        },
        TombstoneCase {
            name: "newer_existing_binding_shadows_incoming_dead",
            existing_binding_offset: Some(1),
            incoming_binding_offset: None,
            existing_dead_offset: None,
            incoming_dead_offset: Some(0),
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
            let now = Local::now().timestamp();

            if let Some(offset) = case.existing_binding_offset {
                insert_continuation_binding(&mut existing, kind, key, now + offset);
            }
            if let Some(offset) = case.incoming_binding_offset {
                insert_continuation_binding(&mut incoming, kind, key, now + offset);
            }
            if let Some(offset) = case.existing_dead_offset {
                insert_dead_continuation_status(&mut existing, kind, key, now + offset);
            }
            if let Some(offset) = case.incoming_dead_offset {
                insert_dead_continuation_status(&mut incoming, kind, key, now + offset);
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
