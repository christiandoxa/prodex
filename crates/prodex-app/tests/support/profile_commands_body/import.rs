use super::*;
#[path = "import/lifecycle.rs"]
mod lifecycle;
#[path = "import/claude.rs"]
mod claude;
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
fn profile_import_rejects_existing_managed_profile_home_outside_root() {
    let sandbox_dir = ProfileCommandsTestDir::new("profile-commands-env");
    let _env = ProfileCommandsTestEnv::new(&sandbox_dir.path);
    let target_dir = ProfileCommandsTestDir::new("import-existing-outside-root");
    let target_paths = profile_commands_test_paths(&target_dir.path);
    let outside_root = target_dir.path.join("outside");
    profile_commands_create_private_test_dir(&outside_root);
    let outside_home = outside_root.join("main");
    profile_commands_create_private_test_dir(&outside_home);
    write_secret_text_file(
        &outside_home.join("auth.json"),
        &profile_commands_auth_json_with_email("main@example.com", "old-token", "main-account"),
    )
    .expect("outside auth should be written");

    let mut existing_state = AppState {
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

    let err = import_profile_export_payload(&target_paths, &mut existing_state, &payload)
        .expect_err("managed profile outside root should be rejected");

    assert!(
        err.to_string().contains("is outside"),
        "unexpected error: {err:#}"
    );
    assert_eq!(
        profile_commands_read_access_token(&outside_home),
        "old-token",
        "import must not overwrite auth outside managed profile root"
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
        err.to_string()
            .contains("missing secret file 'gemini_oauth.json'"),
        "unexpected error: {err:#}"
    );
    assert!(state.profiles.is_empty());
}

#[test]
fn profile_import_rejects_provider_profile_with_unexpected_secret_file() {
    let sandbox_dir = ProfileCommandsTestDir::new("profile-commands-env");
    let _env = ProfileCommandsTestEnv::new(&sandbox_dir.path);
    let target_dir = ProfileCommandsTestDir::new("import-provider-unexpected-secret");
    let target_paths = profile_commands_test_paths(&target_dir.path);
    let payload = ProfileExportPayload {
        exported_at: Local::now().to_rfc3339(),
        source_prodex_version: env!("CARGO_PKG_VERSION").to_string(),
        active_profile: Some("main".to_string()),
        profiles: vec![ExportedProfile {
            name: "main".to_string(),
            email: None,
            source_managed: true,
            provider: ProfileProvider::Openai,
            auth_json: String::new(),
            secret_files: vec![prodex_profile_export::ExportedSecretFile {
                path: "unexpected.json".to_string(),
                text: "credential-sentinel".to_string(),
            }],
        }],
    };
    let mut state = AppState::default();

    let err = import_profile_export_payload(&target_paths, &mut state, &payload)
        .expect_err("provider import with an unexpected secret file should fail");
    assert!(
        err.to_string()
            .contains("unexpected secret file 'unexpected.json'"),
        "unexpected error: {err:#}"
    );
    assert!(state.profiles.is_empty());
}

#[test]
fn profile_import_rejects_same_name_cross_provider_update() {
    let sandbox_dir = ProfileCommandsTestDir::new("profile-commands-env");
    let _env = ProfileCommandsTestEnv::new(&sandbox_dir.path);
    let target_dir = ProfileCommandsTestDir::new("import-provider-mismatch");
    let target_paths = profile_commands_test_paths(&target_dir.path);
    let existing_home = target_paths.managed_profiles_root.join("main");
    create_codex_home_if_missing(&existing_home).expect("existing home should exist");
    let mut state = AppState {
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: existing_home,
                managed: true,
                email: Some("gemini@example.com".to_string()),
                provider: ProfileProvider::Gemini {
                    email: "gemini@example.com".to_string(),
                    project_id: None,
                },
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
            email: Some("agy@example.com".to_string()),
            source_managed: true,
            provider: ProfileProvider::Agy {
                account: Some("agy@example.com".to_string()),
            },
            auth_json: String::new(),
            secret_files: Vec::new(),
        }],
    };

    let error = import_profile_export_payload(&target_paths, &mut state, &payload)
        .expect_err("same-name cross-provider update must fail");

    assert!(error.to_string().contains("cannot be imported as 'agy'"));
    assert_eq!(state.profiles["main"].provider.label(), "gemini");
}

#[cfg(unix)]
#[test]
fn profile_import_rejects_symlink_managed_profiles_root() {
    let sandbox_dir = ProfileCommandsTestDir::new("profile-commands-env");
    let _env = ProfileCommandsTestEnv::new(&sandbox_dir.path);
    let target_dir = ProfileCommandsTestDir::new("import-symlink-managed-root");
    let target_paths = profile_commands_test_paths(&target_dir.path);
    let outside = target_dir.path.join("outside-profiles");
    profile_commands_replace_test_dir_with_symlink(&target_paths.managed_profiles_root, &outside);
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
    let mut state = AppState::default();

    let err = import_profile_export_payload(&target_paths, &mut state, &payload)
        .expect_err("symlink managed root should be rejected");

    assert!(
        err.to_string().contains("must not be a symbolic link"),
        "unexpected error: {err:#}"
    );
    assert!(state.profiles.is_empty());
    assert!(
        !outside.join("main").exists(),
        "import must not stage profiles through a symlinked managed root"
    );
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
    assert_eq!(
        commit.updated_existing_names,
        vec!["gemini-main".to_string()]
    );
    assert_eq!(
        existing_state.active_profile.as_deref(),
        Some("gemini-main")
    );
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
        profile_commands_read_access_token(
            &target_paths.managed_profiles_root.join("second-login")
        ),
        "second-token".to_string()
    );
}
#[path = "import/journal.rs"]
mod journal;
