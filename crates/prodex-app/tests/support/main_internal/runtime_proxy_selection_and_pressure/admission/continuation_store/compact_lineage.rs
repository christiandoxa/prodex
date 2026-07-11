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
