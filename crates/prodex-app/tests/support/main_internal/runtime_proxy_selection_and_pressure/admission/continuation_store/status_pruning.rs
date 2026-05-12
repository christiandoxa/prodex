use super::*;

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
