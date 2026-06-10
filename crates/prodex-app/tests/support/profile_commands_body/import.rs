use super::*;

#[test]
fn profile_import_updates_existing_profile_when_name_matches() {
    let sandbox_dir = ProfileCommandsTestDir::new("profile-commands-env");
    let _env = ProfileCommandsTestEnv::new(&sandbox_dir.path);
    let target_dir = ProfileCommandsTestDir::new("import-collision");
    let target_paths = profile_commands_test_paths(&target_dir.path);
    let existing_home = target_paths.managed_profiles_root.join("main");
    create_codex_home_if_missing(&existing_home).expect("existing home should exist");
    write_secret_text_file(
        &existing_home.join("auth.json"),
        &profile_commands_auth_json_with_email("main@example.com", "old-token", "main-account"),
    )
    .expect("existing auth should be written");

    let mut existing_state = AppState {
        active_profile: None,
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
                "imported-account",
            ),
            secret_files: Vec::new(),
        }],
    };

    let commit = import_profile_export_payload(&target_paths, &mut existing_state, &payload)
        .expect("import should update same-name profile");

    assert!(commit.imported_names.is_empty());
    assert_eq!(commit.updated_existing_names, vec!["main".to_string()]);
    assert_eq!(existing_state.active_profile.as_deref(), Some("main"));
    assert_eq!(existing_state.profiles.len(), 1);
    assert_eq!(
        existing_state
            .profiles
            .get("main")
            .and_then(|profile| profile.email.as_deref()),
        Some("imported@example.com")
    );
    assert_eq!(
        profile_commands_read_access_token(&existing_home),
        "fresh-token".to_string()
    );
    assert_eq!(
        fs::read_dir(&target_paths.managed_profiles_root)
            .expect("managed root should be readable")
            .count(),
        1,
        "same-name import should not create a new managed home"
    );
}

#[test]
fn profile_import_updates_existing_profile_refresh_token() {
    let sandbox_dir = ProfileCommandsTestDir::new("profile-commands-env");
    let _env = ProfileCommandsTestEnv::new(&sandbox_dir.path);
    let target_dir = ProfileCommandsTestDir::new("import-refresh-existing");
    let target_paths = profile_commands_test_paths(&target_dir.path);
    let existing_home = target_paths.managed_profiles_root.join("main");
    create_codex_home_if_missing(&existing_home).expect("existing home should exist");
    write_secret_text_file(
        &existing_home.join("auth.json"),
        &profile_commands_auth_json_with_email_and_refresh(
            "main@example.com",
            "old-access-token",
            "main-account",
            Some("old-refresh-token"),
        ),
    )
    .expect("existing auth should be written");

    let mut existing_state = AppState {
        active_profile: None,
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
            auth_json: profile_commands_auth_json_with_email_and_refresh(
                "main@example.com",
                "fresh-access-token",
                "main-account",
                Some("fresh-refresh-token"),
            ),
            secret_files: Vec::new(),
        }],
    };

    import_profile_export_payload(&target_paths, &mut existing_state, &payload)
        .expect("import should update existing auth with refresh token");

    assert_eq!(
        profile_commands_read_access_token(&existing_home),
        "fresh-access-token"
    );
    assert_eq!(
        profile_commands_read_refresh_token(&existing_home),
        "fresh-refresh-token"
    );
}

#[test]
fn profile_import_rejects_provider_profile_missing_required_secret_file() {
    let sandbox_dir = ProfileCommandsTestDir::new("profile-commands-env");
    let _env = ProfileCommandsTestEnv::new(&sandbox_dir.path);
    let target_dir = ProfileCommandsTestDir::new("import-provider-missing-secret");
    let target_paths = profile_commands_test_paths(&target_dir.path);
    let payload = ProfileExportPayload {
        exported_at: Local::now().to_rfc3339(),
        source_prodex_version: env!("CARGO_PKG_VERSION").to_string(),
        active_profile: Some("gemini-main".to_string()),
        profiles: vec![ExportedProfile {
            name: "gemini-main".to_string(),
            email: Some("gemini@example.com".to_string()),
            source_managed: true,
            provider: ProfileProvider::Gemini {
                email: "gemini@example.com".to_string(),
                project_id: Some("gemini-project".to_string()),
            },
            auth_json: String::new(),
            secret_files: Vec::new(),
        }],
    };
    let mut state = AppState::default();

    let err = import_profile_export_payload(&target_paths, &mut state, &payload)
        .expect_err("provider import without secret file should fail");
    assert!(
        err.to_string().contains("missing secret file 'gemini_oauth.json'"),
        "unexpected error: {err:#}"
    );
    assert!(state.profiles.is_empty());
}

#[test]
fn profile_import_updates_existing_gemini_profile_when_name_matches() {
    let sandbox_dir = ProfileCommandsTestDir::new("profile-commands-env");
    let _env = ProfileCommandsTestEnv::new(&sandbox_dir.path);
    let target_dir = ProfileCommandsTestDir::new("import-existing-gemini");
    let target_paths = profile_commands_test_paths(&target_dir.path);
    let existing_home = target_paths.managed_profiles_root.join("gemini-main");
    create_codex_home_if_missing(&existing_home).expect("existing home should exist");
    write_secret_text_file(
        &existing_home.join(GEMINI_OAUTH_SECRET_FILE),
        &serde_json::json!({
            "auth_mode": "gemini_oauth",
            "access_token": "old-gemini-access-token",
            "refresh_token": "old-gemini-refresh-token",
            "token_type": "Bearer",
            "scope": "https://www.googleapis.com/auth/cloud-platform",
            "expiry_date": 1800000000000_i64,
            "email": "old-gemini@example.com",
            "project_id": "old-project"
        })
        .to_string(),
    )
    .expect("existing Gemini secret should be written");

    let mut existing_state = AppState {
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
    };
    let fresh_secret = serde_json::json!({
        "auth_mode": "gemini_oauth",
        "access_token": "fresh-gemini-access-token",
        "refresh_token": "fresh-gemini-refresh-token",
        "token_type": "Bearer",
        "scope": "https://www.googleapis.com/auth/cloud-platform",
        "expiry_date": 1900000000000_i64,
        "email": "gemini@example.com",
        "project_id": "fresh-project"
    })
    .to_string();
    let payload = ProfileExportPayload {
        exported_at: Local::now().to_rfc3339(),
        source_prodex_version: env!("CARGO_PKG_VERSION").to_string(),
        active_profile: Some("gemini-main".to_string()),
        profiles: vec![ExportedProfile {
            name: "gemini-main".to_string(),
            email: Some("gemini@example.com".to_string()),
            source_managed: true,
            provider: ProfileProvider::Gemini {
                email: "gemini@example.com".to_string(),
                project_id: Some("fresh-project".to_string()),
            },
            auth_json: String::new(),
            secret_files: vec![prodex_profile_export::ExportedSecretFile {
                path: GEMINI_OAUTH_SECRET_FILE.to_string(),
                text: fresh_secret.clone(),
            }],
        }],
    };

    let commit = import_profile_export_payload(&target_paths, &mut existing_state, &payload)
        .expect("import should update same-name Gemini profile");

    assert!(commit.imported_names.is_empty());
    assert_eq!(commit.updated_existing_names, vec!["gemini-main".to_string()]);
    assert_eq!(existing_state.active_profile.as_deref(), Some("gemini-main"));
    assert_eq!(
        existing_state.profiles.get("gemini-main"),
        Some(&ProfileEntry {
            codex_home: existing_home.clone(),
            managed: true,
            email: Some("gemini@example.com".to_string()),
            provider: ProfileProvider::Gemini {
                email: "gemini@example.com".to_string(),
                project_id: Some("fresh-project".to_string()),
            },
        })
    );
    assert_eq!(
        fs::read_to_string(existing_home.join(GEMINI_OAUTH_SECRET_FILE))
            .expect("Gemini secret should be replaced"),
        fresh_secret
    );
    assert!(!existing_home.join("auth.json").exists());
}

#[test]
fn profile_import_auth_update_journal_is_removed_after_successful_state_save() {
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

    let commit = import_profile_export_payload(&target_paths, &mut existing_state, &payload)
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
    prodex_profile_export::cleanup_imported_auth_update_journals(&commit);

    assert!(
        profile_commands_import_auth_journal_paths(&target_paths).is_empty(),
        "successful state save should clean auth overwrite journals"
    );
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
        super::import_export::count_profile_import_auth_journals(&target_paths)
            .expect("journal count should succeed"),
        1,
        "orphaned journal should be visible outside import"
    );

    let recovered =
        super::import_export::repair_profile_import_auth_journals(&target_paths, &mut existing_state)
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
fn profile_import_updates_existing_profile_when_email_matches() {
    let sandbox_dir = ProfileCommandsTestDir::new("profile-commands-env");
    let _env = ProfileCommandsTestEnv::new(&sandbox_dir.path);
    let target_dir = ProfileCommandsTestDir::new("import-duplicate-email");
    let target_paths = profile_commands_test_paths(&target_dir.path);
    let existing_home = target_paths.managed_profiles_root.join("primary");
    create_codex_home_if_missing(&existing_home).expect("existing home should exist");
    write_secret_text_file(
        &existing_home.join("auth.json"),
        &profile_commands_auth_json_with_email("main@example.com", "old-token", "main-account"),
    )
    .expect("existing auth should be written");

    let mut existing_state = AppState {
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
    };
    let payload = ProfileExportPayload {
        exported_at: Local::now().to_rfc3339(),
        source_prodex_version: env!("CARGO_PKG_VERSION").to_string(),
        active_profile: Some("backup-main".to_string()),
        profiles: vec![ExportedProfile {
            name: "backup-main".to_string(),
            email: Some("Main@Example.com".to_string()),
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

    let commit = import_profile_export_payload(&target_paths, &mut existing_state, &payload)
        .expect("import should update existing duplicate");

    assert!(commit.imported_names.is_empty());
    assert_eq!(commit.updated_existing_names, vec!["primary".to_string()]);
    assert_eq!(existing_state.active_profile.as_deref(), Some("primary"));
    assert_eq!(existing_state.profiles.len(), 1);
    assert_eq!(
        existing_state
            .profiles
            .get("primary")
            .and_then(|profile| profile.email.as_deref()),
        Some("main@example.com")
    );
    assert_eq!(
        profile_commands_read_access_token(&existing_home),
        "fresh-token".to_string()
    );
    assert!(
        !target_paths
            .managed_profiles_root
            .join("backup-main")
            .exists(),
        "duplicate import should not create a new managed home"
    );
}

#[test]
fn profile_import_keeps_distinct_workspaces_with_same_email() {
    let sandbox_dir = ProfileCommandsTestDir::new("profile-commands-env");
    let _env = ProfileCommandsTestEnv::new(&sandbox_dir.path);
    let target_dir = ProfileCommandsTestDir::new("import-same-email-different-workspace");
    let target_paths = profile_commands_test_paths(&target_dir.path);
    let existing_home = target_paths.managed_profiles_root.join("primary");
    create_codex_home_if_missing(&existing_home).expect("existing home should exist");
    write_secret_text_file(
        &existing_home.join("auth.json"),
        &profile_commands_auth_json_with_email("main@example.com", "old-token", "account-one"),
    )
    .expect("existing auth should be written");

    let mut existing_state = AppState {
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
    };
    let payload = ProfileExportPayload {
        exported_at: Local::now().to_rfc3339(),
        source_prodex_version: env!("CARGO_PKG_VERSION").to_string(),
        active_profile: None,
        profiles: vec![ExportedProfile {
            name: "business".to_string(),
            email: Some("Main@Example.com".to_string()),
            source_managed: true,
            provider: ProfileProvider::Openai,
            auth_json: profile_commands_auth_json_with_email(
                "main@example.com",
                "fresh-token",
                "account-two",
            ),
            secret_files: Vec::new(),
        }],
    };

    let commit = import_profile_export_payload(&target_paths, &mut existing_state, &payload)
        .expect("different workspace should import as a new profile");

    assert_eq!(commit.imported_names, vec!["business".to_string()]);
    assert!(commit.updated_existing_names.is_empty());
    assert_eq!(existing_state.profiles.len(), 2);
    assert_eq!(
        profile_commands_read_access_token(&existing_home),
        "old-token".to_string()
    );
    assert_eq!(
        profile_commands_read_access_token(&target_paths.managed_profiles_root.join("business")),
        "fresh-token".to_string()
    );
}

#[test]
fn profile_import_keeps_distinct_profiles_with_same_workspace_and_different_emails() {
    let sandbox_dir = ProfileCommandsTestDir::new("profile-commands-env");
    let _env = ProfileCommandsTestEnv::new(&sandbox_dir.path);
    let target_dir = ProfileCommandsTestDir::new("import-same-workspace-different-email");
    let target_paths = profile_commands_test_paths(&target_dir.path);
    let payload = ProfileExportPayload {
        exported_at: Local::now().to_rfc3339(),
        source_prodex_version: env!("CARGO_PKG_VERSION").to_string(),
        active_profile: Some("second-login".to_string()),
        profiles: vec![
            ExportedProfile {
                name: "first-login".to_string(),
                email: Some("first@example.com".to_string()),
                source_managed: true,
                provider: ProfileProvider::Openai,
                auth_json: profile_commands_auth_json_with_email(
                    "first@example.com",
                    "first-token",
                    "shared-account",
                ),
                secret_files: Vec::new(),
            },
            ExportedProfile {
                name: "second-login".to_string(),
                email: Some("second@example.com".to_string()),
                source_managed: true,
                provider: ProfileProvider::Openai,
                auth_json: profile_commands_auth_json_with_email(
                    "second@example.com",
                    "second-token",
                    "shared-account",
                ),
                secret_files: Vec::new(),
            },
        ],
    };
    let mut state = AppState::default();

    let commit = import_profile_export_payload(&target_paths, &mut state, &payload)
        .expect("same workspace with different emails should preserve both profiles");

    assert_eq!(
        commit.imported_names,
        vec!["first-login".to_string(), "second-login".to_string()]
    );
    assert!(commit.updated_existing_names.is_empty());
    assert_eq!(state.active_profile.as_deref(), Some("second-login"));
    assert_eq!(state.profiles.len(), 2);
    assert_eq!(
        state
            .profiles
            .get("first-login")
            .and_then(|profile| profile.email.as_deref()),
        Some("first@example.com")
    );
    assert_eq!(
        state
            .profiles
            .get("second-login")
            .and_then(|profile| profile.email.as_deref()),
        Some("second@example.com")
    );
    assert_eq!(
        profile_commands_read_access_token(&target_paths.managed_profiles_root.join("first-login")),
        "first-token".to_string()
    );
    assert_eq!(
        profile_commands_read_access_token(&target_paths.managed_profiles_root.join("second-login")),
        "second-token".to_string()
    );
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
