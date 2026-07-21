use super::*;

#[test]
fn remove_profile_preserves_managed_home_without_delete_home() {
    let temp_dir = TestDir::isolated();
    let prodex_home = temp_dir.path.join("prodex");
    let prodex_home_string = prodex_home.to_string_lossy().to_string();
    let _prodex_home = TestEnvVarGuard::set("PRODEX_HOME", &prodex_home_string);
    let paths = AppPaths::discover().expect("paths should resolve");
    fs::create_dir_all(&paths.managed_profiles_root).expect("managed profile root should exist");
    let profile_home = paths.managed_profiles_root.join("main");
    fs::create_dir_all(&profile_home).expect("managed profile home should exist");
    fs::write(profile_home.join("auth.json"), "{}").expect("auth file should exist");

    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: profile_home.clone(),
                managed: true,
                email: Some("main@example.com".to_string()),
                provider: ProfileProvider::Openai,
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    write_versioned_runtime_sidecar(
        &runtime_usage_snapshots_file_path(&paths),
        &runtime_usage_snapshots_last_good_file_path(&paths),
        0,
        &BTreeMap::<String, RuntimeProfileUsageSnapshot>::new(),
    );
    write_versioned_runtime_sidecar(
        &runtime_scores_file_path(&paths),
        &runtime_scores_last_good_file_path(&paths),
        0,
        &BTreeMap::<String, RuntimeProfileHealth>::new(),
    );
    write_versioned_runtime_sidecar(
        &runtime_backoffs_file_path(&paths),
        &runtime_backoffs_last_good_file_path(&paths),
        0,
        &RuntimeProfileBackoffs::default(),
    );
    state.save(&paths).expect("state should save");

    handle_remove_profile(RemoveProfileArgs {
        name: Some("main".to_string()),
        all: false,
        delete_home: false,
    })
    .expect("managed profile remove should succeed");

    let reloaded = AppState::load(&paths).expect("state should reload");
    assert!(!reloaded.profiles.contains_key("main"));
    assert!(
        profile_home.exists(),
        "managed profile home should remain without --delete-home"
    );
}

#[test]
fn remove_active_profile_selects_next_profile() {
    let temp_dir = TestDir::isolated();
    let prodex_home = temp_dir.path.join("prodex");
    let prodex_home_string = prodex_home.to_string_lossy().to_string();
    let _prodex_home = TestEnvVarGuard::set("PRODEX_HOME", &prodex_home_string);
    let paths = AppPaths::discover().expect("paths should resolve");
    let main_home = paths.managed_profiles_root.join("main");
    let second_home = paths.managed_profiles_root.join("second");
    fs::create_dir_all(&main_home).expect("main home should exist");
    fs::create_dir_all(&second_home).expect("second home should exist");
    let now = Local::now().timestamp();

    AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home,
                    managed: true,
                    email: None,
                    provider: ProfileProvider::Openai,
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home.clone(),
                    managed: true,
                    email: None,
                    provider: ProfileProvider::Openai,
                },
            ),
        ]),
        last_run_selected_at: BTreeMap::from([
            ("main".to_string(), now - 1),
            ("second".to_string(), now),
        ]),
        response_profile_bindings: BTreeMap::from([(
            "resp-main".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: now,
            },
        )]),
        session_profile_bindings: BTreeMap::from([(
            "sess-second".to_string(),
            ResponseProfileBinding {
                profile_name: "second".to_string(),
                bound_at: now,
            },
        )]),
    }
    .save(&paths)
    .expect("state should save");

    handle_remove_profile(RemoveProfileArgs {
        name: Some("main".to_string()),
        all: false,
        delete_home: false,
    })
    .expect("active profile remove should succeed");

    let reloaded = AppState::load(&paths).expect("state should reload");
    assert_eq!(reloaded.active_profile.as_deref(), Some("second"));
    assert!(!reloaded.profiles.contains_key("main"));
    assert!(!reloaded.last_run_selected_at.contains_key("main"));
    assert!(reloaded.response_profile_bindings.is_empty());
    assert!(reloaded.session_profile_bindings.contains_key("sess-second"));
    assert!(second_home.exists(), "remaining profile home should stay");
}

#[test]
fn remove_all_profiles_rejects_delete_home_for_external_profiles() {
    let temp_dir = TestDir::isolated();
    let prodex_home = temp_dir.path.join("prodex");
    let prodex_home_string = prodex_home.to_string_lossy().to_string();
    let _prodex_home = TestEnvVarGuard::set("PRODEX_HOME", &prodex_home_string);
    let paths = AppPaths::discover().expect("paths should resolve");
    let external_home = temp_dir.path.join("external-second");
    fs::create_dir_all(&external_home).expect("external profile home should exist");
    fs::write(external_home.join("auth.json"), "{}").expect("external auth file should exist");

    AppState {
        active_profile: Some("second".to_string()),
        profiles: BTreeMap::from([(
            "second".to_string(),
            ProfileEntry {
                codex_home: external_home.clone(),
                managed: false,
                email: Some("second@example.com".to_string()),
                provider: ProfileProvider::Openai,
            },
        )]),
        ..AppState::default()
    }
    .save(&paths)
    .expect("state should save");

    let err = handle_remove_profile(RemoveProfileArgs {
        name: None,
        all: true,
        delete_home: true,
    })
    .expect_err("bulk delete should reject external homes");
    assert!(
        err.to_string()
            .contains("refuses to delete external profiles"),
        "unexpected error: {err:#}"
    );

    let reloaded = AppState::load(&paths).expect("state should reload");
    assert!(
        reloaded.profiles.contains_key("second"),
        "profile should remain after rejected bulk delete"
    );
    assert!(
        external_home.exists(),
        "external home should remain after rejected bulk delete"
    );
}

#[test]
fn remove_profile_refuses_managed_home_outside_managed_root() {
    let temp_dir = TestDir::isolated();
    let prodex_home = temp_dir.path.join("prodex");
    let prodex_home_string = prodex_home.to_string_lossy().to_string();
    let _prodex_home = TestEnvVarGuard::set("PRODEX_HOME", &prodex_home_string);
    let paths = AppPaths::discover().expect("paths should resolve");
    let outside_home = temp_dir.path.join("outside-managed");
    fs::create_dir_all(&outside_home).expect("outside profile home should exist");
    fs::write(outside_home.join("auth.json"), "{}").expect("outside auth file should exist");

    AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: outside_home.clone(),
                managed: true,
                email: Some("main@example.com".to_string()),
                provider: ProfileProvider::Openai,
            },
        )]),
        ..AppState::default()
    }
    .save(&paths)
    .expect("state should save");

    let err = handle_remove_profile(RemoveProfileArgs {
        name: Some("main".to_string()),
        all: false,
        delete_home: true,
    })
    .expect_err("managed profile home outside root should be refused");

    assert!(
        err.to_string()
            .contains("outside managed profiles root"),
        "unexpected error: {err:#}"
    );
    assert!(
        outside_home.exists(),
        "outside profile home should not be deleted"
    );
    assert!(
        AppState::load(&paths)
            .expect("state should reload")
            .profiles
            .contains_key("main"),
        "state file should stay unchanged after failed deletion"
    );
}

#[cfg(unix)]
#[test]
fn remove_profile_refuses_symlink_managed_profiles_root() {
    let temp_dir = TestDir::isolated();
    let prodex_home = temp_dir.path.join("prodex");
    let prodex_home_string = prodex_home.to_string_lossy().to_string();
    let _prodex_home = TestEnvVarGuard::set("PRODEX_HOME", &prodex_home_string);
    let paths = AppPaths::discover().expect("paths should resolve");
    let outside_profiles = temp_dir.path.join("outside-profiles");
    fs::create_dir_all(&outside_profiles).expect("outside root should exist");
    fs::create_dir_all(&paths.root).expect("prodex root should exist");
    std::os::unix::fs::symlink(&outside_profiles, &paths.managed_profiles_root)
        .expect("managed profiles root symlink should be created");
    let profile_home = paths.managed_profiles_root.join("main");
    fs::create_dir_all(&profile_home).expect("profile home should exist through symlink");
    fs::write(profile_home.join("auth.json"), "{}").expect("auth file should exist");

    AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: profile_home.clone(),
                managed: true,
                email: Some("main@example.com".to_string()),
                provider: ProfileProvider::Openai,
            },
        )]),
        ..AppState::default()
    }
    .save(&paths)
    .expect("state should save");

    let err = handle_remove_profile(RemoveProfileArgs {
        name: Some("main".to_string()),
        all: false,
        delete_home: true,
    })
    .expect_err("managed profile root symlink should be refused");

    assert!(
        err.to_string().contains("must not be a symbolic link"),
        "unexpected error: {err:#}"
    );
    assert!(
        outside_profiles.join("main").exists(),
        "remove must not delete through a symlinked managed profiles root"
    );
    assert!(
        AppState::load(&paths)
            .expect("state should reload")
            .profiles
            .contains_key("main"),
        "state file should stay unchanged after failed deletion"
    );
}

#[test]
fn unique_profile_name_preserves_untracked_managed_directory() {
    let temp_dir = TestDir::isolated();
    let state = AppState::default();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let stale_dir = paths.managed_profiles_root.join("main_example.com");
    fs::create_dir_all(&stale_dir).expect("stale managed directory should exist");
    fs::write(stale_dir.join("stale.txt"), "old").expect("stale file should be written");

    assert_eq!(
        unique_profile_name_for_email(&paths, &state, "main@example.com"),
        "main_example.com-2"
    );
    assert!(
        stale_dir.join("stale.txt").exists(),
        "profile naming must not delete an untracked managed directory"
    );
}

#[test]
fn remove_all_profiles_clears_state_and_continuation_sidecars() {
    let temp_dir = TestDir::isolated();
    let prodex_home = temp_dir.path.join("prodex");
    let prodex_home_string = prodex_home.to_string_lossy().to_string();
    let _prodex_home = TestEnvVarGuard::set("PRODEX_HOME", &prodex_home_string);
    let paths = AppPaths::discover().expect("paths should resolve");
    fs::create_dir_all(&paths.managed_profiles_root).expect("managed profile root should exist");
    let managed_home = paths.managed_profiles_root.join("main");
    let external_home = temp_dir.path.join("external-second");
    fs::create_dir_all(&managed_home).expect("managed profile home should exist");
    fs::create_dir_all(&external_home).expect("external profile home should exist");
    fs::write(managed_home.join("auth.json"), "{}").expect("managed auth file should exist");
    fs::write(external_home.join("auth.json"), "{}").expect("external auth file should exist");

    let now = Local::now().timestamp();
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: managed_home.clone(),
                    managed: true,
                    email: Some("main@example.com".to_string()),
                    provider: ProfileProvider::Openai,
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: external_home.clone(),
                    managed: false,
                    email: Some("second@example.com".to_string()),
                    provider: ProfileProvider::Openai,
                },
            ),
        ]),
        last_run_selected_at: BTreeMap::from([
            ("main".to_string(), now - 1),
            ("second".to_string(), now),
        ]),
        response_profile_bindings: BTreeMap::from([
            (
                "resp-main".to_string(),
                ResponseProfileBinding {
                    profile_name: "main".to_string(),
                    bound_at: now,
                },
            ),
            (
                "resp-second".to_string(),
                ResponseProfileBinding {
                    profile_name: "second".to_string(),
                    bound_at: now,
                },
            ),
        ]),
        session_profile_bindings: BTreeMap::from([
            (
                "sess-main".to_string(),
                ResponseProfileBinding {
                    profile_name: "main".to_string(),
                    bound_at: now,
                },
            ),
            (
                "sess-second".to_string(),
                ResponseProfileBinding {
                    profile_name: "second".to_string(),
                    bound_at: now,
                },
            ),
        ]),
    };
    write_versioned_runtime_sidecar(
        &runtime_usage_snapshots_file_path(&paths),
        &runtime_usage_snapshots_last_good_file_path(&paths),
        0,
        &BTreeMap::<String, RuntimeProfileUsageSnapshot>::new(),
    );
    write_versioned_runtime_sidecar(
        &runtime_scores_file_path(&paths),
        &runtime_scores_last_good_file_path(&paths),
        0,
        &BTreeMap::<String, RuntimeProfileHealth>::new(),
    );
    write_versioned_runtime_sidecar(
        &runtime_backoffs_file_path(&paths),
        &runtime_backoffs_last_good_file_path(&paths),
        0,
        &RuntimeProfileBackoffs::default(),
    );
    state.save(&paths).expect("state should save");

    let continuations = RuntimeContinuationStore {
        response_profile_bindings: BTreeMap::from([
            (
                "resp-main".to_string(),
                ResponseProfileBinding {
                    profile_name: "main".to_string(),
                    bound_at: now,
                },
            ),
            (
                "resp-second".to_string(),
                ResponseProfileBinding {
                    profile_name: "second".to_string(),
                    bound_at: now,
                },
            ),
        ]),
        session_profile_bindings: BTreeMap::from([
            (
                "sess-main".to_string(),
                ResponseProfileBinding {
                    profile_name: "main".to_string(),
                    bound_at: now,
                },
            ),
            (
                "sess-second".to_string(),
                ResponseProfileBinding {
                    profile_name: "second".to_string(),
                    bound_at: now,
                },
            ),
        ]),
        turn_state_bindings: BTreeMap::from([
            (
                "turn-main".to_string(),
                ResponseProfileBinding {
                    profile_name: "main".to_string(),
                    bound_at: now,
                },
            ),
            (
                "turn-second".to_string(),
                ResponseProfileBinding {
                    profile_name: "second".to_string(),
                    bound_at: now,
                },
            ),
        ]),
        session_id_bindings: BTreeMap::from([
            (
                "sid-main".to_string(),
                ResponseProfileBinding {
                    profile_name: "main".to_string(),
                    bound_at: now,
                },
            ),
            (
                "sid-second".to_string(),
                ResponseProfileBinding {
                    profile_name: "second".to_string(),
                    bound_at: now,
                },
            ),
        ]),
        statuses: RuntimeContinuationStatuses::default(),
    };
    save_runtime_continuations_for_profiles(&paths, &continuations, &state.profiles)
        .expect("continuations should save");
    save_runtime_continuation_journal_for_profiles(&paths, &continuations, &state.profiles, now)
        .expect("continuation journal should save");

    handle_remove_profile(RemoveProfileArgs {
        name: None,
        all: true,
        delete_home: false,
    })
    .expect("bulk profile remove should succeed");

    let reloaded = AppState::load(&paths).expect("state should reload");
    assert!(
        reloaded.profiles.is_empty(),
        "all profiles should be removed"
    );
    assert!(
        reloaded.active_profile.is_none(),
        "active profile should clear"
    );
    assert!(
        reloaded.last_run_selected_at.is_empty(),
        "selection metadata should be cleared"
    );
    assert!(
        reloaded.response_profile_bindings.is_empty(),
        "response bindings should be cleared"
    );
    assert!(
        reloaded.session_profile_bindings.is_empty(),
        "session bindings should be cleared"
    );
    assert!(
        managed_home.exists(),
        "managed profile home should remain during bulk removal without --delete-home"
    );
    assert!(
        external_home.exists(),
        "external profile home should remain without --delete-home"
    );

    let restored_continuations =
        load_runtime_continuations_with_recovery(&paths, &reloaded.profiles)
            .expect("continuations should load")
            .value;
    assert!(
        restored_continuations.response_profile_bindings.is_empty(),
        "response continuation bindings should be cleared"
    );
    assert!(
        restored_continuations.session_profile_bindings.is_empty(),
        "session continuation bindings should be cleared"
    );
    assert!(
        restored_continuations.turn_state_bindings.is_empty(),
        "turn state bindings should be cleared"
    );
    assert!(
        restored_continuations.session_id_bindings.is_empty(),
        "session id bindings should be cleared"
    );

    let restored_journal =
        load_runtime_continuation_journal_with_recovery(&paths, &reloaded.profiles)
            .expect("continuation journal should load")
            .value;
    assert!(
        restored_journal
            .continuations
            .response_profile_bindings
            .is_empty(),
        "journal response bindings should be cleared"
    );
    assert!(
        restored_journal
            .continuations
            .session_profile_bindings
            .is_empty(),
        "journal session bindings should be cleared"
    );
    assert!(
        restored_journal
            .continuations
            .turn_state_bindings
            .is_empty(),
        "journal turn state bindings should be cleared"
    );
    assert!(
        restored_journal
            .continuations
            .session_id_bindings
            .is_empty(),
        "journal session id bindings should be cleared"
    );
}
