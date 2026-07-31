use super::*;

#[test]
fn login_lifecycle_recovery_keeps_committed_existing_credentials() {
    let sandbox_dir = ProfileCommandsTestDir::new("login-lifecycle-commit");
    let _env = ProfileCommandsTestEnv::new(&sandbox_dir.path);
    let target_dir = ProfileCommandsTestDir::new("login-lifecycle-target");
    let paths = profile_commands_test_paths(&target_dir.path);
    let codex_home = paths.managed_profiles_root.join("main");
    create_codex_home_if_missing(&codex_home).expect("profile home should exist");
    write_secret_text_file(
        &codex_home.join("auth.json"),
        &profile_commands_auth_json_with_email("old@example.com", "old-token", "main-account"),
    )
    .expect("old auth should be written");
    let mut state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: codex_home.clone(),
                managed: true,
                email: Some("old@example.com".to_string()),
                provider: ProfileProvider::Openai,
            },
        )]),
        ..AppState::default()
    };
    state.save(&paths).expect("initial state should save");

    let login_home = paths.managed_profiles_root.join(".login-test");
    create_codex_home_if_missing(&login_home).expect("temporary login home should exist");
    let new_auth =
        profile_commands_auth_json_with_email("new@example.com", "new-token", "main-account");
    let mut desired = state.profiles["main"].clone();
    desired.email = Some("new@example.com".to_string());
    crate::profile_commands::import_export::prepare_existing_profile_lifecycle(
        &paths,
        "login",
        &state,
        "main",
        &desired,
        Some("main".to_string()),
        crate::profile_commands::import_export::ProfileAuthUpdate {
            next_auth_json: Some(new_auth.clone()),
            next_provider_json: Some(serde_json::to_string(&desired.provider).unwrap()),
            next_secret_files: Vec::new(),
            previous_secret_file_paths: &[],
            temporary_home: Some(&login_home),
        },
    )
    .expect("login lifecycle should be journaled");
    write_secret_text_file(&codex_home.join("auth.json"), &new_auth)
        .expect("new auth should be written");
    state.profiles.get_mut("main").unwrap().email = Some("new@example.com".to_string());
    state.save(&paths).expect("committed state should save");

    let mut recovered_state = AppState::load(&paths).expect("state should reload");
    let recovered = crate::profile_commands::import_export::repair_profile_import_auth_journals(
        &paths,
        &mut recovered_state,
    )
    .expect("committed login should recover");

    assert_eq!(recovered, 1);
    assert_eq!(profile_commands_read_access_token(&codex_home), "new-token");
    assert_eq!(
        recovered_state.profiles["main"].email.as_deref(),
        Some("new@example.com")
    );
    assert!(
        !login_home.exists(),
        "committed login temp home should be cleaned"
    );
    assert!(
        prodex_profile_export::profile_lifecycle_journal_paths(&paths.root)
            .unwrap()
            .is_empty()
    );
    assert!(
        profile_commands_import_auth_journal_paths(&paths).is_empty(),
        "committed login auth journal should be cleaned"
    );
}

#[test]
fn login_lifecycle_recovery_removes_uncommitted_new_profile_home() {
    let sandbox_dir = ProfileCommandsTestDir::new("login-lifecycle-rollback");
    let _env = ProfileCommandsTestEnv::new(&sandbox_dir.path);
    let target_dir = ProfileCommandsTestDir::new("login-lifecycle-target");
    let paths = profile_commands_test_paths(&target_dir.path);
    let login_home = paths.managed_profiles_root.join(".login-test");
    let cleanup_home = paths.managed_profiles_root.join(".login-cleanup");
    let codex_home = paths.managed_profiles_root.join("main");
    create_codex_home_if_missing(&login_home).expect("temporary login home should exist");
    create_codex_home_if_missing(&cleanup_home).expect("cleanup home should exist");
    fs::write(login_home.join("auth.json"), "pending").expect("pending auth should be written");
    let desired = ProfileEntry {
        codex_home: codex_home.clone(),
        managed: true,
        email: Some("new@example.com".to_string()),
        provider: ProfileProvider::Openai,
    };
    let plan = crate::profile_commands::import_export::ProfileLifecyclePlan {
        profile_states: vec![
            crate::profile_commands::import_export::lifecycle_profile_state(
                "main",
                None,
                Some(&desired),
            )
            .unwrap(),
        ],
        previous_active_profile: None,
        next_active_profile: Some("main".to_string()),
        home_actions: vec![
            crate::profile_commands::import_export::ProfileLifecycleHomeAction::Promote {
                source: login_home.display().to_string(),
                destination: codex_home.display().to_string(),
                rollback:
                    crate::profile_commands::import_export::ProfileLifecyclePromoteRollback::Remove,
            },
            crate::profile_commands::import_export::ProfileLifecycleHomeAction::Cleanup {
                path: cleanup_home.display().to_string(),
            },
        ],
        auth_journal_paths: Vec::new(),
    };
    let lifecycle_path = crate::profile_commands::import_export::write_profile_lifecycle_plan(
        &paths, "login", &plan,
    )
    .unwrap();
    persist_login_home(&login_home, &codex_home).expect("login home should be promoted");

    let mut state = AppState::default();
    let recovered = crate::profile_commands::import_export::recover_profile_lifecycle_journals(
        &paths, &mut state, false,
    )
    .expect("uncommitted login should recover");

    assert_eq!(recovered.recovered, 1);
    assert!(
        !login_home.exists(),
        "rollback should delete the temporary home"
    );
    assert!(
        !cleanup_home.exists(),
        "rollback should clean temporary cleanup homes"
    );
    assert!(
        !codex_home.exists(),
        "rollback should remove the uncommitted profile home"
    );
    assert!(!lifecycle_path.exists());
}

#[test]
fn login_lifecycle_recovery_detects_partial_api_key_profile_file_deletion() {
    let sandbox_dir = ProfileCommandsTestDir::new("login-api-key-file-recovery");
    let _env = ProfileCommandsTestEnv::new(&sandbox_dir.path);
    let target_dir = ProfileCommandsTestDir::new("login-api-key-file-target");
    let paths = profile_commands_test_paths(&target_dir.path);
    let codex_home = paths.managed_profiles_root.join("main");
    create_codex_home_if_missing(&codex_home).expect("profile home should exist");
    let old_auth =
        profile_commands_auth_json_with_email("main@example.com", "old-token", "main-account");
    write_secret_text_file(&codex_home.join("auth.json"), &old_auth)
        .expect("old auth should be written");
    write_secret_text_file(
        &codex_home.join(".prodex-profile.toml"),
        "base_url = \"https://example.test/v1\"\n",
    )
    .expect("old profile config should be written");
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: codex_home.clone(),
                managed: true,
                email: None,
                provider: ProfileProvider::Openai,
            },
        )]),
        ..AppState::default()
    };
    state.save(&paths).expect("initial state should save");

    let new_auth =
        profile_commands_auth_json_with_email("main@example.com", "new-token", "main-account");
    crate::profile_commands::import_export::prepare_existing_profile_lifecycle(
        &paths,
        "login",
        &state,
        "main",
        &state.profiles["main"],
        Some("main".to_string()),
        crate::profile_commands::import_export::ProfileAuthUpdate {
            next_auth_json: Some(new_auth.clone()),
            next_provider_json: Some(serde_json::to_string(&ProfileProvider::Openai).unwrap()),
            next_secret_files: vec![
                prodex_profile_export::ImportedExistingProfileFileUpdate {
                    path: ".prodex-profile.toml".to_string(),
                    text: None,
                },
            ],
            previous_secret_file_paths: &[".prodex-profile.toml"],
            temporary_home: None,
        },
    )
    .expect("API-key lifecycle should be journaled");
    write_secret_text_file(&codex_home.join("auth.json"), &new_auth)
        .expect("new auth should be written");
    state
        .save(&paths)
        .expect("unchanged profile state should save");

    let mut recovered_state = AppState::load(&paths).expect("state should reload");
    let recovered = crate::profile_commands::import_export::repair_profile_import_auth_journals(
        &paths,
        &mut recovered_state,
    )
    .expect("partial API-key login should recover");

    assert_eq!(recovered, 1);
    assert_eq!(profile_commands_read_access_token(&codex_home), "old-token");
    assert_eq!(
        fs::read_to_string(codex_home.join(".prodex-profile.toml")).unwrap(),
        "base_url = \"https://example.test/v1\"\n"
    );
}
