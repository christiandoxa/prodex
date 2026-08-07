use super::*;

#[test]
fn profile_import_auth_update_journal_recovery_keeps_committed_auth() {
    let sandbox_dir = ProfileCommandsTestDir::new("profile-commands-env");
    let _env = ProfileCommandsTestEnv::new(&sandbox_dir.path);
    let target_dir = ProfileCommandsTestDir::new("import-journal-cleanup");
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

    import_profile_export_payload(&target_paths, &mut existing_state, &payload)
        .expect("import should update same-name profile");

    let journals = profile_commands_import_auth_journal_paths(&target_paths);
    assert_eq!(journals.len(), 1, "auth overwrite journal should be staged");
    assert!(
        fs::read_to_string(&journals[0])
            .expect("auth overwrite journal should be readable")
            .contains("old-token"),
        "journal should preserve the replaced token"
    );

    existing_state
        .save(&target_paths)
        .expect("state should save after import");
    let mut recovered_state = AppState::load(&target_paths).expect("state should reload");
    let recovered = crate::profile_commands::import_export::repair_profile_import_auth_journals(
        &target_paths,
        &mut recovered_state,
    )
    .expect("committed import should be finalized");

    assert_eq!(recovered, 1);
    assert_eq!(
        profile_commands_read_access_token(&existing_home),
        "fresh-token",
        "recovery must not roll back a committed import"
    );
    assert_eq!(
        recovered_state.profiles["main"].email.as_deref(),
        Some("main@example.com")
    );
    assert!(
        profile_commands_import_auth_journal_paths(&target_paths).is_empty(),
        "recovery should clean committed auth overwrite journals"
    );
    assert!(
        prodex_profile_export::profile_lifecycle_journal_paths(&target_paths.root)
            .expect("lifecycle journals should be readable")
            .is_empty(),
        "recovery should clean the import lifecycle journal"
    );
}

#[test]
fn profile_import_save_failure_rolls_back_existing_gemini_profile() {
    let sandbox_dir = ProfileCommandsTestDir::new("profile-commands-env");
    let _env = ProfileCommandsTestEnv::new(&sandbox_dir.path);
    let paths = AppPaths::discover().expect("app paths should resolve");
    let existing_home = paths.managed_profiles_root.join("gemini-main");
    create_codex_home_if_missing(&existing_home).expect("existing home should exist");
    let old_secret = serde_json::json!({
        "auth_mode": "gemini_oauth",
        "access_token": "old-gemini-access-token",
        "refresh_token": "old-gemini-refresh-token",
        "token_type": "Bearer",
        "scope": "https://www.googleapis.com/auth/cloud-platform",
        "expiry_date": 1800000000000_i64,
        "email": "old-gemini@example.com",
        "project_id": "old-project"
    })
    .to_string();
    write_secret_text_file(&existing_home.join(GEMINI_OAUTH_SECRET_FILE), &old_secret).unwrap();
    AppState {
        profiles: BTreeMap::from([(
            "gemini-main".to_string(),
            ProfileEntry {
                codex_home: existing_home.clone(),
                managed: true,
                email: Some("old-gemini@example.com".to_string()),
                provider: ProfileProvider::Gemini {
                    email: "old-gemini@example.com".to_string(),
                    project_id: Some("old-project".to_string()),
                },
            },
        )]),
        ..AppState::default()
    }
    .save(&paths)
    .unwrap();
    let new_secret = serde_json::json!({
        "auth_mode": "gemini_oauth",
        "access_token": "new-gemini-access-token",
        "refresh_token": "new-gemini-refresh-token",
        "token_type": "Bearer",
        "scope": "https://www.googleapis.com/auth/cloud-platform",
        "expiry_date": 1900000000000_i64,
        "email": "new-gemini@example.com",
        "project_id": "new-project"
    })
    .to_string();
    let payload = ProfileExportPayload {
        exported_at: Local::now().to_rfc3339(),
        source_prodex_version: env!("CARGO_PKG_VERSION").to_string(),
        active_profile: Some("gemini-main".to_string()),
        profiles: vec![ExportedProfile {
            name: "gemini-main".to_string(),
            email: Some("new-gemini@example.com".to_string()),
            source_managed: true,
            provider: ProfileProvider::Gemini {
                email: "new-gemini@example.com".to_string(),
                project_id: Some("new-project".to_string()),
            },
            auth_json: String::new(),
            secret_files: vec![prodex_profile_export::ExportedSecretFile {
                path: GEMINI_OAUTH_SECRET_FILE.to_string(),
                text: new_secret,
            }],
        }],
    };
    let bundle_path = sandbox_dir.path.join("gemini-import.json");
    let bundle = serialize_profile_export_payload(&payload, None).unwrap();
    prodex_profile_export::write_profile_export_bundle(&bundle_path, &bundle).unwrap();
    let backup_path = state_last_good_file_path(&paths);
    fs::remove_file(&backup_path).expect("state backup should be removed");
    fs::create_dir(&backup_path).expect("state backup blocker should be created");

    handle_import_profiles(ImportProfileArgs {
        path: bundle_path,
        name: None,
        activate: false,
        insecure: false,
    })
    .expect_err("state save failure should fail Gemini import");

    let state = AppState::load(&paths).unwrap();
    assert_eq!(
        state.profiles["gemini-main"].email.as_deref(),
        Some("old-gemini@example.com")
    );
    assert_eq!(state.profiles["gemini-main"].provider.label(), "gemini");
    assert_eq!(
        fs::read_to_string(existing_home.join(GEMINI_OAUTH_SECRET_FILE)).unwrap(),
        old_secret
    );
    let journal = prodex_profile_export::read_profile_import_auth_update_journal(
        profile_commands_import_auth_journal_paths(&paths)
            .first()
            .expect("rollback journal should remain"),
    )
    .unwrap();
    assert!(!journal.restore_auth_json);
    assert_eq!(journal.previous_secret_files.len(), 1);
    assert!(journal.previous_provider_json.is_some());
}

#[test]
fn profile_lifecycle_recovery_rolls_back_partial_auth_when_metadata_is_unchanged() {
    let sandbox_dir = ProfileCommandsTestDir::new("profile-commands-env");
    let _env = ProfileCommandsTestEnv::new(&sandbox_dir.path);
    let target_dir = ProfileCommandsTestDir::new("lifecycle-partial-auth");
    let paths = profile_commands_test_paths(&target_dir.path);
    let home = paths.managed_profiles_root.join("main");
    create_codex_home_if_missing(&home).expect("profile home should exist");
    let old_auth =
        profile_commands_auth_json_with_email("main@example.com", "old-token", "main-account");
    write_secret_text_file(&home.join("auth.json"), &old_auth).expect("old auth should be written");
    let profile = ProfileEntry {
        codex_home: home.clone(),
        managed: true,
        email: Some("main@example.com".to_string()),
        provider: ProfileProvider::Openai,
    };
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([("main".to_string(), profile.clone())]),
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
        &profile,
        Some("main".to_string()),
        crate::profile_commands::import_export::ProfileAuthUpdate {
            next_auth_json: Some(new_auth.clone()),
            next_provider_json: Some(serde_json::to_string(&profile.provider).unwrap()),
            next_secret_files: Vec::new(),
            previous_secret_file_paths: &[],
            temporary_home: None,
        },
    )
    .expect("lifecycle should be journaled");
    write_secret_text_file(&home.join("auth.json"), "partial-credential-write")
        .expect("partial auth should be written");

    let mut caller_state = AppState::load(&paths).expect("state should load");
    let recovered = crate::profile_commands::import_export::repair_profile_import_auth_journals(
        &paths,
        &mut caller_state,
    )
    .expect("partial auth should recover");

    assert_eq!(recovered, 1);
    assert_eq!(
        fs::read_to_string(home.join("auth.json")).unwrap(),
        old_auth
    );
    assert_eq!(caller_state.profiles["main"], profile);
    assert_eq!(AppState::load(&paths).unwrap().profiles["main"], profile);
    assert!(
        prodex_profile_export::profile_lifecycle_journal_paths(&paths.root)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn profile_import_recovery_removes_orphaned_credential_staging_home() {
    let sandbox_dir = ProfileCommandsTestDir::new("profile-commands-env");
    let _env = ProfileCommandsTestEnv::new(&sandbox_dir.path);
    let target_dir = ProfileCommandsTestDir::new("import-orphan-staging");
    let paths = profile_commands_test_paths(&target_dir.path);
    let staging_home = prodex_profile_export::profile_import_staging_home(
        &paths.managed_profiles_root,
        "main",
        "crashed",
    );
    create_codex_home_if_missing(&staging_home).expect("staging home should exist");
    write_secret_text_file(&staging_home.join("auth.json"), "credential-sentinel")
        .expect("staged credential should be written");

    crate::profile_commands::import_export::load_profile_state_with_profile_recovery(
        &paths, true,
    )
    .expect("startup recovery should remove orphaned staging");

    assert!(!staging_home.exists());
}

#[test]
fn profile_import_recovery_rejects_missing_promoted_home_after_primary_backup_failure() {
    let sandbox_dir = ProfileCommandsTestDir::new("profile-commands-env");
    let _env = ProfileCommandsTestEnv::new(&sandbox_dir.path);
    let target_dir = ProfileCommandsTestDir::new("import-primary-backup-failure");
    let paths = profile_commands_test_paths(&target_dir.path);
    let before = AppState::default();
    before.save(&paths).expect("initial state should save");

    let final_home = paths.managed_profiles_root.join("main");
    let desired = ProfileEntry {
        codex_home: final_home.clone(),
        managed: true,
        email: Some("main@example.com".to_string()),
        provider: ProfileProvider::Openai,
    };
    let lifecycle_path = crate::profile_commands::import_export::write_profile_lifecycle_plan(
        &paths,
        "import",
        &crate::profile_commands::import_export::ProfileLifecyclePlan {
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
                    source: paths
                        .managed_profiles_root
                        .join(".import-main-crashed")
                        .display()
                        .to_string(),
                    destination: final_home.display().to_string(),
                    rollback:
                        crate::profile_commands::import_export::ProfileLifecyclePromoteRollback::Remove,
                },
            ],
            auth_journal_paths: Vec::new(),
        },
    )
    .expect("import lifecycle should be journaled");
    create_codex_home_if_missing(&final_home).expect("final home should exist");

    let backup_path = state_last_good_file_path(&paths);
    fs::remove_file(&backup_path).expect("state backup should be removable");
    fs::create_dir(&backup_path).expect("state backup blocker should be created");
    let mut after = before.clone();
    after.profiles.insert("main".to_string(), desired);
    after.active_profile = Some("main".to_string());
    after
        .save(&paths)
        .expect_err("primary write should report backup failure");
    fs::remove_dir(&backup_path).expect("state backup blocker should be removable");
    fs::remove_dir_all(&final_home).expect("rollback should remove promoted home");

    let recovered = crate::profile_commands::import_export::recover_profile_lifecycle_journals(
        &paths, &mut after, false,
    )
    .expect("crash recovery should roll back missing promoted home");

    assert_eq!(recovered.recovered, 1);
    assert!(after.profiles.is_empty());
    assert!(!final_home.exists());
    assert!(!lifecycle_path.exists());
    assert!(AppState::load(&paths).unwrap().profiles.is_empty());
}

fn openai_import_payload(auth_json: String) -> ProfileExportPayload {
    ProfileExportPayload {
        exported_at: Local::now().to_rfc3339(),
        source_prodex_version: env!("CARGO_PKG_VERSION").to_string(),
        active_profile: Some("main".to_string()),
        profiles: vec![ExportedProfile {
            name: "main".to_string(),
            email: Some("main@example.com".to_string()),
            source_managed: true,
            provider: ProfileProvider::Openai,
            auth_json,
            secret_files: Vec::new(),
        }],
    }
}

fn assert_no_import_staging_homes(paths: &AppPaths) {
    let staging_homes = fs::read_dir(&paths.managed_profiles_root)
        .expect("managed profile root should be readable")
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry
                .file_name()
                .to_str()
                .is_some_and(|name| name.starts_with(".import-"))
        })
        .collect::<Vec<_>>();
    assert!(
        staging_homes.is_empty(),
        "failed import must remove all staging homes: {:?}",
        staging_homes
    );
}

#[test]
fn failed_import_secret_write_removes_staging_home() {
    let sandbox_dir = ProfileCommandsTestDir::new("profile-commands-env");
    let _env = ProfileCommandsTestEnv::new(&sandbox_dir.path);
    let target_dir = ProfileCommandsTestDir::new("import-secret-write-failure");
    let paths = profile_commands_test_paths(&target_dir.path);
    let mut auth = serde_json::from_str::<serde_json::Value>(
        &profile_commands_auth_json_with_email("main@example.com", "temporary-token", "main"),
    )
    .expect("auth fixture should parse");
    auth["last_refresh"] = serde_json::Value::String("x".repeat(2 * 1024 * 1024));
    let payload = openai_import_payload(auth.to_string());
    let mut state = AppState::default();

    let error = import_profile_export_payload(&paths, &mut state, &payload)
        .expect_err("oversized secret write should fail");

    assert!(
        format!("{error:#}").contains("safe size limit"),
        "unexpected secret write error: {error:#}"
    );
    assert!(state.profiles.is_empty());
    assert_no_import_staging_homes(&paths);
}

#[test]
fn failed_import_lifecycle_plan_creation_removes_staging_home() {
    let sandbox_dir = ProfileCommandsTestDir::new("profile-commands-env");
    let _env = ProfileCommandsTestEnv::new(&sandbox_dir.path);
    let target_dir = ProfileCommandsTestDir::new("import-lifecycle-plan-failure");
    let paths = profile_commands_test_paths(&target_dir.path);
    fs::write(
        paths.root.join("profile-lifecycle-journal"),
        "block lifecycle journal directory",
    )
    .expect("lifecycle journal blocker should be written");
    let payload = openai_import_payload(profile_commands_auth_json_with_email(
        "main@example.com",
        "temporary-token",
        "main",
    ));
    let mut state = AppState::default();

    import_profile_export_payload(&paths, &mut state, &payload)
        .expect_err("lifecycle journal creation should fail");

    assert!(state.profiles.is_empty());
    assert_no_import_staging_homes(&paths);
}
