use super::*;

#[test]
fn runtime_continuations_load_legacy_last_good_backup_when_primary_is_invalid() {
    let temp_dir = TestDir::isolated();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let profiles = BTreeMap::from([(
        "main".to_string(),
        ProfileEntry {
            codex_home: temp_dir.path.join("homes/main"),
            managed: true,
            email: Some("main@example.com".to_string()),
            provider: ProfileProvider::Openai,
        },
    )]);
    let backup_store = RuntimeContinuationStore {
        response_profile_bindings: BTreeMap::from([(
            "resp-legacy".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: Local::now().timestamp(),
            },
        )]),
        ..RuntimeContinuationStore::default()
    };
    fs::create_dir_all(&paths.root).expect("prodex root should exist");
    fs::write(
        runtime_continuations_last_good_file_path(&paths),
        serde_json::to_string_pretty(&backup_store)
            .expect("legacy continuation backup should serialize"),
    )
    .expect("legacy continuation backup should write");
    fs::write(runtime_continuations_file_path(&paths), "{ not valid json")
        .expect("broken continuation primary should write");

    let loaded = load_runtime_continuations_with_recovery(&paths, &profiles)
        .expect("legacy continuation backup recovery should succeed");

    assert!(loaded.recovered_from_backup);
    assert_eq!(
        loaded
            .value
            .response_profile_bindings
            .get("resp-legacy")
            .map(|binding| binding.profile_name.as_str()),
        Some("main")
    );
}

#[test]
fn runtime_continuations_reject_stale_generation_overwrite() {
    let temp_dir = TestDir::isolated();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let profiles = BTreeMap::from([(
        "main".to_string(),
        ProfileEntry {
            codex_home: temp_dir.path.join("homes/main"),
            managed: true,
            email: Some("main@example.com".to_string()),
            provider: ProfileProvider::Openai,
        },
    )]);
    let initial_store = RuntimeContinuationStore {
        response_profile_bindings: BTreeMap::from([(
            "resp-old".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: Local::now().timestamp() - 10,
            },
        )]),
        ..RuntimeContinuationStore::default()
    };
    save_runtime_continuations_for_profiles(&paths, &initial_store, &profiles)
        .expect("initial continuation save should succeed");

    let wrapped_primary: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(runtime_continuations_file_path(&paths))
            .expect("initial continuation primary should be readable"),
    )
    .expect("initial continuation primary should be valid json");
    assert_eq!(wrapped_primary["generation"].as_u64(), Some(1));

    let newer_store = RuntimeContinuationStore {
        response_profile_bindings: BTreeMap::from([(
            "resp-new".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: Local::now().timestamp(),
            },
        )]),
        ..RuntimeContinuationStore::default()
    };
    write_versioned_runtime_sidecar(
        &runtime_continuations_file_path(&paths),
        &runtime_continuations_last_good_file_path(&paths),
        2,
        &newer_store,
    );

    let stale_store = RuntimeContinuationStore {
        response_profile_bindings: BTreeMap::from([(
            "resp-stale".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: Local::now().timestamp() - 20,
            },
        )]),
        ..RuntimeContinuationStore::default()
    };
    let err = save_runtime_continuations_for_profiles(&paths, &stale_store, &profiles)
        .expect_err("stale continuation save should be fenced");
    assert!(
        err.to_string().contains("stale runtime sidecar generation"),
        "unexpected stale-save error: {err:#}"
    );

    let wrapped_after: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(runtime_continuations_file_path(&paths))
            .expect("continuation primary should still be readable"),
    )
    .expect("continuation primary should still be valid json");
    assert_eq!(wrapped_after["generation"].as_u64(), Some(2));
    assert_eq!(
        wrapped_after["value"]["response_profile_bindings"]["resp-new"]["profile_name"],
        serde_json::Value::String("main".to_string())
    );
    assert!(
        !wrapped_after["value"]["response_profile_bindings"]
            .as_object()
            .expect("response_profile_bindings should be an object")
            .contains_key("resp-stale"),
        "stale writer must not overwrite newer continuation state"
    );
}

#[test]
fn runtime_state_snapshot_save_retries_stale_continuation_generation() {
    let temp_dir = TestDir::isolated();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let now = Local::now().timestamp();
    let profiles = BTreeMap::from([(
        "main".to_string(),
        ProfileEntry {
            codex_home: temp_dir.path.join("homes/main"),
            managed: true,
            email: Some("main@example.com".to_string()),
            provider: ProfileProvider::Openai,
        },
    )]);
    let initial_state = AppState {
        active_profile: Some("main".to_string()),
        profiles: profiles.clone(),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    initial_state
        .save(&paths)
        .expect("initial state save should succeed");

    let initial_store = RuntimeContinuationStore {
        response_profile_bindings: BTreeMap::from([(
            "resp-initial".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: now - 30,
            },
        )]),
        ..RuntimeContinuationStore::default()
    };
    save_runtime_continuations_for_profiles(&paths, &initial_store, &profiles)
        .expect("initial continuation save should succeed");

    let external_store = RuntimeContinuationStore {
        response_profile_bindings: BTreeMap::from([(
            "resp-external".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: now - 10,
            },
        )]),
        ..RuntimeContinuationStore::default()
    };
    write_versioned_runtime_sidecar(
        &runtime_continuations_file_path(&paths),
        &runtime_continuations_last_good_file_path(&paths),
        2,
        &external_store,
    );

    let snapshot = AppState {
        active_profile: Some("main".to_string()),
        profiles: profiles.clone(),
        last_run_selected_at: BTreeMap::from([("main".to_string(), now)]),
        response_profile_bindings: BTreeMap::from([(
            "resp-local".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: now,
            },
        )]),
        session_profile_bindings: BTreeMap::new(),
    };
    let revision = AtomicU64::new(1);
    assert!(
        save_runtime_state_snapshot_if_latest(RuntimeStateSnapshotSaveInput {
            paths: &paths,
            snapshot: &snapshot,
            continuations: &runtime_continuation_store_from_app_state(&snapshot),
            profile_scores: &BTreeMap::new(),
            usage_snapshots: &BTreeMap::new(),
            backoffs: &RuntimeProfileBackoffs::default(),
            revision: 1,
            latest_revision: &revision,
        })
        .expect("state snapshot save should succeed after stale retry")
    );

    let loaded = load_runtime_continuations_with_recovery(&paths, &profiles)
        .expect("continuations should reload")
        .value;
    assert_eq!(
        loaded
            .response_profile_bindings
            .get("resp-local")
            .map(|binding| binding.profile_name.as_str()),
        Some("main")
    );
    let wrapped: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(runtime_continuations_file_path(&paths))
            .expect("continuation primary should be readable"),
    )
    .expect("continuation primary should remain valid json");
    assert_eq!(wrapped["generation"].as_u64(), Some(3));
}

#[test]
fn runtime_state_snapshot_retry_does_not_resurrect_released_response_binding() {
    let temp_dir = TestDir::isolated();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let now = Local::now().timestamp();
    let profiles = BTreeMap::from([(
        "main".to_string(),
        ProfileEntry {
            codex_home: temp_dir.path.join("homes/main"),
            managed: true,
            email: Some("main@example.com".to_string()),
            provider: ProfileProvider::Openai,
        },
    )]);
    let initial_state = AppState {
        active_profile: Some("main".to_string()),
        profiles: profiles.clone(),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    initial_state
        .save(&paths)
        .expect("initial state save should succeed");

    let initial_store = RuntimeContinuationStore {
        response_profile_bindings: BTreeMap::from([(
            "resp-stale".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: now - 30,
            },
        )]),
        ..RuntimeContinuationStore::default()
    };
    save_runtime_continuations_for_profiles(&paths, &initial_store, &profiles)
        .expect("initial continuation save should succeed");

    let released_store = RuntimeContinuationStore {
        statuses: RuntimeContinuationStatuses {
            response: BTreeMap::from([("resp-stale".to_string(), dead_continuation_status(now))]),
            ..RuntimeContinuationStatuses::default()
        },
        ..RuntimeContinuationStore::default()
    };
    write_versioned_runtime_sidecar(
        &runtime_continuations_file_path(&paths),
        &runtime_continuations_last_good_file_path(&paths),
        2,
        &released_store,
    );

    let snapshot = AppState {
        active_profile: Some("main".to_string()),
        profiles: profiles.clone(),
        last_run_selected_at: BTreeMap::from([("main".to_string(), now)]),
        response_profile_bindings: BTreeMap::from([(
            "resp-stale".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: now - 10,
            },
        )]),
        session_profile_bindings: BTreeMap::new(),
    };
    let revision = AtomicU64::new(1);
    assert!(
        save_runtime_state_snapshot_if_latest(RuntimeStateSnapshotSaveInput {
            paths: &paths,
            snapshot: &snapshot,
            continuations: &runtime_continuation_store_from_app_state(&snapshot),
            profile_scores: &BTreeMap::new(),
            usage_snapshots: &BTreeMap::new(),
            backoffs: &RuntimeProfileBackoffs::default(),
            revision: 1,
            latest_revision: &revision,
        })
        .expect("state snapshot save should succeed after stale retry")
    );

    let loaded = load_runtime_continuations_with_recovery(&paths, &profiles)
        .expect("continuations should reload")
        .value;
    assert!(
        !loaded.response_profile_bindings.contains_key("resp-stale"),
        "released response binding must not be resurrected"
    );
    assert_eq!(
        loaded
            .statuses
            .response
            .get("resp-stale")
            .map(|status| status.state),
        Some(RuntimeContinuationBindingLifecycle::Dead)
    );

    let state = AppState::load(&paths).expect("state should reload");
    assert!(
        !state.response_profile_bindings.contains_key("resp-stale"),
        "state snapshot must not rewrite the released response binding"
    );
}

#[test]
fn runtime_continuation_journal_save_retries_stale_generation() {
    let temp_dir = TestDir::isolated();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let now = Local::now().timestamp();
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: temp_dir.path.join("homes/main"),
                managed: true,
                email: Some("main@example.com".to_string()),
                provider: ProfileProvider::Openai,
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    state.save(&paths).expect("state should save");

    let initial = RuntimeContinuationStore {
        response_profile_bindings: BTreeMap::from([(
            "resp-initial".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: now - 30,
            },
        )]),
        ..RuntimeContinuationStore::default()
    };
    save_runtime_continuation_journal(&paths, &initial, now - 30)
        .expect("initial journal save should succeed");

    let external = RuntimeContinuationJournal {
        saved_at: now - 10,
        continuations: RuntimeContinuationStore {
            response_profile_bindings: BTreeMap::from([(
                "resp-external".to_string(),
                ResponseProfileBinding {
                    profile_name: "main".to_string(),
                    bound_at: now - 10,
                },
            )]),
            ..RuntimeContinuationStore::default()
        },
    };
    write_versioned_runtime_sidecar(
        &runtime_continuation_journal_file_path(&paths),
        &runtime_continuation_journal_last_good_file_path(&paths),
        2,
        &external,
    );

    let incoming = RuntimeContinuationStore {
        response_profile_bindings: BTreeMap::from([(
            "resp-local".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: now,
            },
        )]),
        ..RuntimeContinuationStore::default()
    };
    save_runtime_continuation_journal(&paths, &incoming, now)
        .expect("journal save should retry stale generation");

    let loaded = load_runtime_continuation_journal_with_recovery(&paths, &state.profiles)
        .expect("journal should reload")
        .value;
    assert_eq!(loaded.saved_at, now);
    assert_eq!(
        loaded
            .continuations
            .response_profile_bindings
            .get("resp-local")
            .map(|binding| binding.profile_name.as_str()),
        Some("main")
    );
    let wrapped: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(runtime_continuation_journal_file_path(&paths))
            .expect("journal primary should be readable"),
    )
    .expect("journal primary should remain valid json");
    assert_eq!(wrapped["generation"].as_u64(), Some(3));
}

#[test]
fn runtime_continuation_journal_save_for_profiles_preserves_bindings_without_state_file() {
    let temp_dir = TestDir::isolated();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let now = Local::now().timestamp();
    let profiles = BTreeMap::from([(
        "main".to_string(),
        ProfileEntry {
            codex_home: temp_dir.path.join("homes/main"),
            managed: true,
            email: Some("main@example.com".to_string()),
            provider: ProfileProvider::Openai,
        },
    )]);
    let continuations = RuntimeContinuationStore {
        response_profile_bindings: BTreeMap::from([(
            "resp-main".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: now,
            },
        )]),
        ..RuntimeContinuationStore::default()
    };

    save_runtime_continuation_journal_for_profiles(&paths, &continuations, &profiles, now)
        .expect("journal save with explicit profiles should succeed");

    let loaded = load_runtime_continuation_journal_with_recovery(&paths, &profiles)
        .expect("journal should reload")
        .value;
    assert_eq!(loaded.saved_at, now);
    assert_eq!(
        loaded
            .continuations
            .response_profile_bindings
            .get("resp-main")
            .map(|binding| binding.profile_name.as_str()),
        Some("main")
    );
}

#[test]
fn runtime_continuation_journal_retry_does_not_resurrect_released_response_binding() {
    let temp_dir = TestDir::isolated();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let now = Local::now().timestamp();
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: temp_dir.path.join("homes/main"),
                managed: true,
                email: Some("main@example.com".to_string()),
                provider: ProfileProvider::Openai,
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    state.save(&paths).expect("state should save");

    let initial = RuntimeContinuationStore {
        response_profile_bindings: BTreeMap::from([(
            "resp-stale".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: now - 30,
            },
        )]),
        ..RuntimeContinuationStore::default()
    };
    save_runtime_continuation_journal(&paths, &initial, now - 30)
        .expect("initial journal save should succeed");

    let external = RuntimeContinuationJournal {
        saved_at: now - 5,
        continuations: RuntimeContinuationStore {
            statuses: RuntimeContinuationStatuses {
                response: BTreeMap::from([(
                    "resp-stale".to_string(),
                    dead_continuation_status(now),
                )]),
                ..RuntimeContinuationStatuses::default()
            },
            ..RuntimeContinuationStore::default()
        },
    };
    write_versioned_runtime_sidecar(
        &runtime_continuation_journal_file_path(&paths),
        &runtime_continuation_journal_last_good_file_path(&paths),
        2,
        &external,
    );

    let incoming = RuntimeContinuationStore {
        response_profile_bindings: BTreeMap::from([(
            "resp-stale".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: now - 10,
            },
        )]),
        ..RuntimeContinuationStore::default()
    };
    save_runtime_continuation_journal(&paths, &incoming, now)
        .expect("journal save should retry stale generation");

    let loaded = load_runtime_continuation_journal_with_recovery(&paths, &state.profiles)
        .expect("journal should reload")
        .value;
    assert_eq!(loaded.saved_at, now);
    assert!(
        !loaded
            .continuations
            .response_profile_bindings
            .contains_key("resp-stale"),
        "released response binding must not be resurrected in the journal"
    );
    assert_eq!(
        loaded
            .continuations
            .statuses
            .response
            .get("resp-stale")
            .map(|status| status.state),
        Some(RuntimeContinuationBindingLifecycle::Dead)
    );
}
