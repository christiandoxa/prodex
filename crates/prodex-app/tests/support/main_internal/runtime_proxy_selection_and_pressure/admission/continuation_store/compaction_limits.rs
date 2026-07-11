use super::*;

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
    prodex_runtime_store::prune_runtime_continuation_response_bindings(
        &mut pruned.response_profile_bindings,
        &pruned.statuses.response,
        2,
        runtime_continuation_compaction_policy(),
    );
    assert!(pruned.response_profile_bindings.contains_key("resp-hot"));
    assert!(
        !pruned.response_profile_bindings.contains_key("resp-cold-1"),
        "the cold binding should be pruned before the older verified binding"
    );
}
