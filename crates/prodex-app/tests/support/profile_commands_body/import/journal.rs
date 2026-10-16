use super::*;

#[test]
fn profile_lifecycle_rollback_restores_caller_and_durable_before_state() {
    let sandbox_dir = ProfileCommandsTestDir::new("profile-commands-env");
    let _env = ProfileCommandsTestEnv::new(&sandbox_dir.path);
    let target_dir = ProfileCommandsTestDir::new("lifecycle-state-rollback");
    let paths = profile_commands_test_paths(&target_dir.path);
    let old_home = paths.managed_profiles_root.join("old");
    let side_home = paths.managed_profiles_root.join("side");
    let new_home = paths.managed_profiles_root.join("new");
    let old = ProfileEntry {
        codex_home: old_home,
        managed: true,
        email: Some("old@example.com".to_string()),
        provider: ProfileProvider::Openai,
    };
    let side = ProfileEntry {
        codex_home: side_home,
        managed: true,
        email: Some("side@example.com".to_string()),
        provider: ProfileProvider::Openai,
    };
    let new = ProfileEntry {
        codex_home: new_home,
        managed: true,
        email: Some("new@example.com".to_string()),
        provider: ProfileProvider::Openai,
    };
    let before = AppState {
        active_profile: Some("old".to_string()),
        profiles: BTreeMap::from([
            ("old".to_string(), old.clone()),
            ("side".to_string(), side.clone()),
        ]),
        last_run_selected_at: BTreeMap::from([("old".to_string(), Local::now().timestamp() - 1)]),
        ..AppState::default()
    };
    before.save(&paths).expect("initial state should save");
    let mut partial = before.clone();
    partial.profiles.insert("new".to_string(), new.clone());
    partial.active_profile = Some("new".to_string());
    partial
        .last_run_selected_at
        .insert("new".to_string(), Local::now().timestamp());
    partial.save(&paths).expect("partial state should save");

    let mut after_old = old.clone();
    after_old.email = Some("changed@example.com".to_string());
    let plan = crate::profile_commands::import_export::ProfileLifecyclePlan {
        profile_states: vec![
            crate::profile_commands::import_export::lifecycle_profile_state(
                "old",
                Some(&old),
                Some(&after_old),
            )
            .unwrap(),
            crate::profile_commands::import_export::lifecycle_profile_state(
                "new",
                None,
                Some(&new),
            )
            .unwrap(),
        ],
        previous_active_profile: Some("old".to_string()),
        next_active_profile: Some("new".to_string()),
        home_actions: Vec::new(),
        auth_journal_paths: Vec::new(),
    };
    crate::profile_commands::import_export::write_profile_lifecycle_plan(&paths, "import", &plan)
        .expect("lifecycle should be journaled");

    let mut caller_state = partial;
    let recovered = crate::profile_commands::import_export::recover_profile_lifecycle_journals(
        &paths,
        &mut caller_state,
        false,
    )
    .expect("partial lifecycle should roll back");

    assert_eq!(recovered.recovered, 1);
    assert_eq!(caller_state.active_profile.as_deref(), Some("old"));
    assert_eq!(caller_state.profiles.len(), 2);
    assert_eq!(caller_state.profiles["old"], old);
    assert_eq!(caller_state.profiles["side"], side);
    assert!(!caller_state.profiles.contains_key("new"));
    let durable = AppState::load(&paths).expect("durable state should load");
    assert_eq!(durable.active_profile.as_deref(), Some("old"));
    assert_eq!(durable.profiles, caller_state.profiles);
}

#[test]
fn profile_import_auth_update_journal_recovers_orphaned_auth_overwrite() {
    let sandbox_dir = ProfileCommandsTestDir::new("profile-commands-env");
    let _env = ProfileCommandsTestEnv::new(&sandbox_dir.path);
    let target_dir = ProfileCommandsTestDir::new("import-journal-recovery");
    let target_paths = profile_commands_test_paths(&target_dir.path);
    let existing_home = target_paths.managed_profiles_root.join("main");
    create_codex_home_if_missing(&existing_home).expect("existing home should exist");
    write_secret_text_file(
        &existing_home.join("auth.json"),
        &profile_commands_auth_json_with_email("main@example.com", "old-token", "main-account"),
    )
    .expect("existing auth should be written");

    let mut existing_state = AppState {
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: existing_home.clone(),
                managed: true,
                email: Some("main@example.com".to_string()),
                provider: ProfileProvider::Openai,
            },
        )]),
        ..AppState::default()
    };
    let payload = ProfileExportPayload {
        exported_at: Local::now().to_rfc3339(),
        source_prodex_version: env!("CARGO_PKG_VERSION").to_string(),
        active_profile: Some("main".to_string()),
        profiles: vec![ExportedProfile {
            name: "main".to_string(),
            email: Some("imported@example.com".to_string()),
            source_managed: true,
            provider: ProfileProvider::Openai,
            auth_json: profile_commands_auth_json_with_email(
                "imported@example.com",
                "fresh-token",
                "main-account",
            ),
            secret_files: Vec::new(),
        }],
    };

    import_profile_export_payload(&target_paths, &mut existing_state, &payload)
        .expect("import should update same-name profile");
    assert_eq!(
        profile_commands_read_access_token(&existing_home),
        "fresh-token".to_string(),
        "test should simulate crash after auth overwrite"
    );
    assert_eq!(
        existing_state
            .profiles
            .get("main")
            .and_then(|profile| profile.email.as_deref()),
        Some("imported@example.com")
    );
    assert_eq!(
        profile_commands_import_auth_journal_paths(&target_paths).len(),
        1,
        "auth overwrite journal should be orphaned"
    );
    assert_eq!(
        crate::profile_commands::import_export::count_profile_import_auth_journals(&target_paths)
            .expect("journal count should succeed"),
        1,
        "orphaned journal should be visible outside import"
    );

    let recovered = crate::profile_commands::import_export::repair_profile_import_auth_journals(
        &target_paths,
        &mut existing_state,
    )
    .expect("orphaned journal should recover");

    assert_eq!(recovered, 1);
    assert_eq!(
        profile_commands_read_access_token(&existing_home),
        "old-token".to_string(),
        "recovery should restore previous auth"
    );
    assert_eq!(
        existing_state
            .profiles
            .get("main")
            .and_then(|profile| profile.email.as_deref()),
        Some("main@example.com"),
        "recovery should restore previous email"
    );
    assert!(
        profile_commands_import_auth_journal_paths(&target_paths).is_empty(),
        "recovery should remove recovered auth overwrite journals"
    );
}

#[test]
fn profile_import_auth_update_journal_rejects_mismatched_codex_home() {
    let sandbox_dir = ProfileCommandsTestDir::new("profile-commands-env");
    let _env = ProfileCommandsTestEnv::new(&sandbox_dir.path);
    let target_dir = ProfileCommandsTestDir::new("import-journal-mismatch");
    let target_paths = profile_commands_test_paths(&target_dir.path);
    let existing_home = target_paths.managed_profiles_root.join("main");
    let attack_home = target_dir.path.join("outside-target");
    create_codex_home_if_missing(&existing_home).expect("existing home should exist");
    create_codex_home_if_missing(&attack_home).expect("attack home should exist");
    write_secret_text_file(
        &existing_home.join("auth.json"),
        &profile_commands_auth_json_with_email("main@example.com", "old-token", "main-account"),
    )
    .expect("existing auth should be written");

    let mut state = AppState {
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: existing_home.clone(),
                managed: true,
                email: Some("main@example.com".to_string()),
                provider: ProfileProvider::Openai,
            },
        )]),
        ..AppState::default()
    };
    let journal_root =
        prodex_profile_export::ensure_profile_import_auth_update_journal_root(&target_paths.root)
            .expect("journal root should exist");
    let journal = prodex_profile_export::ImportedExistingProfileAuthUpdateJournal::new(
        "main".to_string(),
        attack_home.display().to_string(),
        Some("main@example.com".to_string()),
        Some(profile_commands_auth_json_with_email(
            "main@example.com",
            "forged-token",
            "main-account",
        )),
        Local::now().to_rfc3339(),
    );
    prodex_profile_export::write_profile_import_auth_update_journal(
        journal_root.join("main-forged.json"),
        &journal,
    )
    .expect("forged journal should be written");

    let err = crate::profile_commands::import_export::repair_profile_import_auth_journals(
        &target_paths,
        &mut state,
    )
    .expect_err("mismatched journal should be rejected");

    assert!(
        err.to_string().contains("targets"),
        "unexpected recovery error: {err:#}"
    );
    assert_eq!(
        profile_commands_read_access_token(&existing_home),
        "old-token"
    );
    assert!(!attack_home.join("auth.json").exists());
}

#[test]

fn profile_import_save_failure_rolls_back_auth_and_keeps_recoverable_journal() {
    let sandbox_dir = ProfileCommandsTestDir::new("profile-commands-env");
    let _env = ProfileCommandsTestEnv::new(&sandbox_dir.path);
    let paths = AppPaths::discover().expect("app paths should resolve");
    let existing_home = paths.managed_profiles_root.join("primary");
    create_codex_home_if_missing(&existing_home).expect("existing home should exist");
    write_secret_text_file(
        &existing_home.join("auth.json"),
        &profile_commands_auth_json_with_email("main@example.com", "old-token", "main-account"),
    )
    .expect("existing auth should be written");

    AppState {
        active_profile: None,
        profiles: BTreeMap::from([(
            "primary".to_string(),
            ProfileEntry {
                codex_home: existing_home.clone(),
                managed: true,
                email: Some("main@example.com".to_string()),
                provider: ProfileProvider::Openai,
            },
        )]),
        ..AppState::default()
    }
    .save(&paths)
    .expect("initial state should save");

    let payload = ProfileExportPayload {
        exported_at: Local::now().to_rfc3339(),
        source_prodex_version: env!("CARGO_PKG_VERSION").to_string(),
        active_profile: Some("backup-main".to_string()),
        profiles: vec![ExportedProfile {
            name: "backup-main".to_string(),
            email: Some("main@example.com".to_string()),
            source_managed: true,
            provider: ProfileProvider::Openai,
            auth_json: profile_commands_auth_json_with_email(
                "main@example.com",
                "fresh-token",
                "main-account",
            ),
            secret_files: Vec::new(),
        }],
    };
    let bundle_path = sandbox_dir.path.join("duplicate-import.json");
    let bundle = serialize_profile_export_payload(&payload, None)
        .expect("profile export payload should serialize");
    prodex_profile_export::write_profile_export_bundle(&bundle_path, &bundle)
        .expect("profile export bundle should be written");

    let fault_count = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
        .rem_euclid(100_000)
        .saturating_add(10)
        .to_string();
    let _fault = TestEnvVarGuard::set("PRODEX_RUNTIME_FAULT_STATE_SAVE_ERROR_ONCE", &fault_count);
    let err = handle_import_profiles(ImportProfileArgs {
        path: bundle_path,
        name: None,
        activate: false,
        insecure: false,
    })
    .expect_err("state save failure should fail import");

    assert!(
        err.to_string()
            .contains("injected runtime state save failure"),
        "unexpected import error: {err:#}"
    );
    let state = AppState::load(&paths).expect("state should load after failed import");
    assert_eq!(state.active_profile, None);
    assert_eq!(
        state
            .profiles
            .get("primary")
            .and_then(|profile| profile.email.as_deref()),
        Some("main@example.com")
    );
    assert_eq!(
        profile_commands_read_access_token(&existing_home),
        "old-token".to_string(),
        "save failure should keep normal auth rollback behavior"
    );

    let journals = profile_commands_import_auth_journal_paths(&paths);
    assert_eq!(
        journals.len(),
        1,
        "failed save should preserve a recoverable auth overwrite journal"
    );
    let journal = fs::read_to_string(&journals[0]).expect("auth overwrite journal should exist");
    assert!(journal.contains("primary"));
    assert!(journal.contains("old-token"));
}

#[test]
fn import_current_updates_existing_profile_token_for_duplicate_email() {
    let sandbox_dir = ProfileCommandsTestDir::new("profile-commands-env");
    let _env = ProfileCommandsTestEnv::new(&sandbox_dir.path);
    let paths = AppPaths::discover().expect("app paths should resolve");
    let existing_home = paths.managed_profiles_root.join("primary");
    let current_home = paths.shared_codex_root.clone();
    create_codex_home_if_missing(&existing_home).expect("existing home should exist");
    create_codex_home_if_missing(&current_home).expect("current home should exist");
    write_secret_text_file(
        &existing_home.join("auth.json"),
        &profile_commands_auth_json_with_email("main@example.com", "old-token", "main-account"),
    )
    .expect("existing auth should be written");
    write_secret_text_file(
        &current_home.join("auth.json"),
        &profile_commands_auth_json_with_email("main@example.com", "new-token", "main-account"),
    )
    .expect("current auth should be written");

    AppState {
        active_profile: None,
        profiles: BTreeMap::from([(
            "primary".to_string(),
            ProfileEntry {
                codex_home: existing_home.clone(),
                managed: true,
                email: Some("main@example.com".to_string()),
                provider: ProfileProvider::Openai,
            },
        )]),
        ..AppState::default()
    }
    .save(&paths)
    .expect("state should save");

    handle_import_current_profile(ImportCurrentArgs {
        name: "duplicate".to_string(),
        insecure: false,
    })
    .expect("import-current should update duplicate auth");

    let state = AppState::load(&paths).expect("state should load");
    assert_eq!(state.active_profile.as_deref(), Some("primary"));
    assert_eq!(state.profiles.len(), 1);
    assert_eq!(
        state
            .profiles
            .get("primary")
            .and_then(|profile| profile.email.as_deref()),
        Some("main@example.com")
    );
    assert_eq!(
        profile_commands_read_access_token(&existing_home),
        "new-token".to_string()
    );
    assert!(
        !paths.managed_profiles_root.join("duplicate").exists(),
        "duplicate import-current should not create a new managed home"
    );
}
