use super::*;

#[test]
fn perform_prodex_cleanup_deduplicates_profiles_by_email() {
    let temp_dir = TestDir::isolated();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    fs::create_dir_all(&paths.root).expect("prodex root should exist");
    fs::create_dir_all(&paths.managed_profiles_root).expect("managed profiles root should exist");

    let primary_home = paths.managed_profiles_root.join("primary");
    let duplicate_home = paths.managed_profiles_root.join("duplicate");
    fs::create_dir_all(&primary_home).expect("primary home should exist");
    fs::create_dir_all(&duplicate_home).expect("duplicate home should exist");

    let mut state = AppState {
        active_profile: Some("duplicate".to_string()),
        profiles: BTreeMap::from([
            (
                "primary".to_string(),
                ProfileEntry {
                    codex_home: primary_home.clone(),
                    managed: true,
                    email: Some("main@example.com".to_string()),
                    provider: ProfileProvider::Openai,
                },
            ),
            (
                "duplicate".to_string(),
                ProfileEntry {
                    codex_home: duplicate_home.clone(),
                    managed: true,
                    email: Some("Main@Example.com".to_string()),
                    provider: ProfileProvider::Openai,
                },
            ),
        ]),
        last_run_selected_at: BTreeMap::from([
            ("primary".to_string(), 10),
            ("duplicate".to_string(), 5),
        ]),
        response_profile_bindings: BTreeMap::from([(
            "resp-1".to_string(),
            ResponseProfileBinding {
                profile_name: "primary".to_string(),
                bound_at: 11,
            },
        )]),
        session_profile_bindings: BTreeMap::from([(
            "sess-1".to_string(),
            ResponseProfileBinding {
                profile_name: "primary".to_string(),
                bound_at: 12,
            },
        )]),
    };
    state.save(&paths).expect("state should save");

    let continuations = RuntimeContinuationStore {
        response_profile_bindings: BTreeMap::from([(
            "resp-2".to_string(),
            ResponseProfileBinding {
                profile_name: "primary".to_string(),
                bound_at: 13,
            },
        )]),
        session_profile_bindings: BTreeMap::from([(
            "sess-2".to_string(),
            ResponseProfileBinding {
                profile_name: "primary".to_string(),
                bound_at: 14,
            },
        )]),
        turn_state_bindings: BTreeMap::from([(
            "turn-2".to_string(),
            ResponseProfileBinding {
                profile_name: "primary".to_string(),
                bound_at: 15,
            },
        )]),
        session_id_bindings: BTreeMap::from([(
            "sid-2".to_string(),
            ResponseProfileBinding {
                profile_name: "primary".to_string(),
                bound_at: 16,
            },
        )]),
        statuses: RuntimeContinuationStatuses::default(),
    };
    save_runtime_continuations_for_profiles(&paths, &continuations, &state.profiles)
        .expect("continuations should save");
    save_runtime_continuation_journal_for_profiles(&paths, &continuations, &state.profiles, 123)
        .expect("continuation journal should save");

    let summary = perform_prodex_cleanup(&paths, &mut state).expect("cleanup should succeed");

    assert_eq!(summary.duplicate_profiles_removed, 1);
    assert_eq!(summary.duplicate_managed_profile_homes_removed, 1);
    assert_eq!(state.active_profile.as_deref(), Some("duplicate"));
    assert_eq!(state.profiles.len(), 1);
    assert!(state.profiles.contains_key("duplicate"));
    assert_eq!(state.last_run_selected_at.get("duplicate"), Some(&10));
    assert_eq!(
        state.response_profile_bindings["resp-1"].profile_name,
        "duplicate"
    );
    assert_eq!(
        state.session_profile_bindings["sess-1"].profile_name,
        "duplicate"
    );
    assert!(
        !primary_home.exists(),
        "duplicate managed home should be removed"
    );
    assert!(
        duplicate_home.exists(),
        "canonical managed home should remain"
    );

    let saved_state = AppState::load(&paths).expect("saved state should load");
    assert_eq!(saved_state.active_profile.as_deref(), Some("duplicate"));
    assert!(saved_state.profiles.contains_key("duplicate"));
    assert!(!saved_state.profiles.contains_key("primary"));

    let restored_continuations = load_runtime_continuations_with_recovery(&paths, &state.profiles)
        .expect("continuations should load")
        .value;
    assert_eq!(
        restored_continuations.response_profile_bindings["resp-2"].profile_name,
        "duplicate"
    );
    assert_eq!(
        restored_continuations.session_profile_bindings["sess-2"].profile_name,
        "duplicate"
    );
    assert_eq!(
        restored_continuations.turn_state_bindings["turn-2"].profile_name,
        "duplicate"
    );
    assert_eq!(
        restored_continuations.session_id_bindings["sid-2"].profile_name,
        "duplicate"
    );

    let restored_journal = load_runtime_continuation_journal_with_recovery(&paths, &state.profiles)
        .expect("continuation journal should load")
        .value;
    assert_eq!(
        restored_journal.continuations.response_profile_bindings["resp-2"].profile_name,
        "duplicate"
    );
}

#[test]
fn perform_prodex_cleanup_refuses_duplicate_managed_home_outside_managed_root() {
    let temp_dir = TestDir::isolated();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    fs::create_dir_all(&paths.root).expect("prodex root should exist");
    fs::create_dir_all(&paths.managed_profiles_root).expect("managed profiles root should exist");

    let inside_home = paths.managed_profiles_root.join("inside");
    let outside_home = temp_dir.path.join("outside-managed");
    fs::create_dir_all(&inside_home).expect("inside home should exist");
    fs::create_dir_all(&outside_home).expect("outside home should exist");

    let mut state = AppState {
        active_profile: Some("inside".to_string()),
        profiles: BTreeMap::from([
            (
                "inside".to_string(),
                ProfileEntry {
                    codex_home: inside_home.clone(),
                    managed: true,
                    email: Some("main@example.com".to_string()),
                    provider: ProfileProvider::Openai,
                },
            ),
            (
                "outside".to_string(),
                ProfileEntry {
                    codex_home: outside_home.clone(),
                    managed: true,
                    email: Some("Main@Example.com".to_string()),
                    provider: ProfileProvider::Openai,
                },
            ),
        ]),
        last_run_selected_at: BTreeMap::from([
            ("inside".to_string(), 10),
            ("outside".to_string(), 1),
        ]),
        ..AppState::default()
    };
    state.save(&paths).expect("state should save");

    let err = perform_prodex_cleanup(&paths, &mut state)
        .expect_err("duplicate managed home outside root should be refused");

    assert!(
        err.to_string().contains("outside managed profiles root"),
        "unexpected error: {err:#}"
    );
    assert!(
        outside_home.exists(),
        "outside managed home should not be deleted"
    );
    assert!(
        AppState::load(&paths)
            .expect("saved state should load")
            .profiles
            .contains_key("outside"),
        "saved state should stay unchanged after failed cleanup"
    );
}

#[cfg(unix)]
#[test]
fn perform_prodex_cleanup_refuses_symlink_managed_profiles_root() {
    let temp_dir = TestDir::isolated();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let outside_profiles = temp_dir.path.join("outside-profiles");
    fs::create_dir_all(&paths.root).expect("prodex root should exist");
    fs::create_dir_all(&outside_profiles).expect("outside root should exist");
    std::os::unix::fs::symlink(&outside_profiles, &paths.managed_profiles_root)
        .expect("managed profiles root symlink should be created");

    let primary_home = paths.managed_profiles_root.join("primary");
    let duplicate_home = paths.managed_profiles_root.join("duplicate");
    fs::create_dir_all(&primary_home).expect("primary home should exist through symlink");
    fs::create_dir_all(&duplicate_home).expect("duplicate home should exist through symlink");

    let mut state = AppState {
        active_profile: Some("duplicate".to_string()),
        profiles: BTreeMap::from([
            (
                "primary".to_string(),
                ProfileEntry {
                    codex_home: primary_home,
                    managed: true,
                    email: Some("main@example.com".to_string()),
                    provider: ProfileProvider::Openai,
                },
            ),
            (
                "duplicate".to_string(),
                ProfileEntry {
                    codex_home: duplicate_home,
                    managed: true,
                    email: Some("Main@Example.com".to_string()),
                    provider: ProfileProvider::Openai,
                },
            ),
        ]),
        last_run_selected_at: BTreeMap::from([
            ("primary".to_string(), 10),
            ("duplicate".to_string(), 5),
        ]),
        ..AppState::default()
    };

    let err = perform_prodex_cleanup(&paths, &mut state)
        .expect_err("cleanup should reject symlinked managed root");

    assert!(
        err.to_string().contains("must not be a symbolic link"),
        "unexpected cleanup error: {err:#}"
    );
    assert!(
        outside_profiles.join("primary").exists(),
        "cleanup must not delete through a symlinked managed profiles root"
    );
}

#[test]
fn perform_prodex_cleanup_keeps_same_email_profiles_when_workspace_differs() {
    let temp_dir = TestDir::isolated();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    create_codex_home_if_missing(&paths.root).expect("prodex root should exist");
    create_codex_home_if_missing(&paths.managed_profiles_root)
        .expect("managed profiles root should exist");

    let first_home = paths.managed_profiles_root.join("first");
    let second_home = paths.managed_profiles_root.join("second");
    create_codex_home_if_missing(&first_home).expect("first home should exist");
    create_codex_home_if_missing(&second_home).expect("second home should exist");
    write_auth_json(&first_home.join("auth.json"), "acct-one");
    write_auth_json(&second_home.join("auth.json"), "acct-two");

    let mut state = AppState {
        active_profile: Some("first".to_string()),
        profiles: BTreeMap::from([
            (
                "first".to_string(),
                ProfileEntry {
                    codex_home: first_home.clone(),
                    managed: true,
                    email: Some("main@example.com".to_string()),
                    provider: ProfileProvider::Openai,
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home.clone(),
                    managed: true,
                    email: Some("Main@Example.com".to_string()),
                    provider: ProfileProvider::Openai,
                },
            ),
        ]),
        ..AppState::default()
    };

    let summary = perform_prodex_cleanup(&paths, &mut state).expect("cleanup should succeed");

    assert_eq!(summary.duplicate_profiles_removed, 0);
    assert_eq!(state.profiles.len(), 2);
    assert!(first_home.exists());
    assert!(second_home.exists());
}
