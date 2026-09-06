use super::*;

fn claude_import_credentials(access_token: &str) -> String {
    serde_json::json!({
        "claudeAiOauth": {
            "accessToken": access_token,
            "refreshToken": "claude-refresh-token",
            "expiresAt": 1900000000000_i64,
            "refreshTokenExpiresAt": 1900000000001_i64,
            "scopes": ["user:inference", "user:profile"],
            "subscriptionType": "pro",
            "rateLimitTier": "default_claude_ai",
            "email": "claude@example.com"
        }
    })
    .to_string()
}

fn claude_import_args(name: Option<&str>, activate: bool) -> ImportProfileArgs {
    ImportProfileArgs {
        path: PathBuf::from("claude"),
        name: name.map(str::to_string),
        activate,
        insecure: false,
    }
}

fn write_claude_import_source(source_dir: &Path, access_token: &str) {
    create_codex_home_if_missing(source_dir).expect("Claude source directory should exist");
    let path = source_dir.join(CLAUDE_CREDENTIALS_FILE);
    fs::write(path, claude_import_credentials(access_token))
        .expect("Claude credentials should be written");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(
            source_dir.join(CLAUDE_CREDENTIALS_FILE),
            fs::Permissions::from_mode(0o644),
        )
        .expect("Claude source permissions should be set");
    }
}

#[test]
fn profile_import_claude_supports_isolated_source_and_private_destination() {
    let sandbox_dir = ProfileCommandsTestDir::new("claude-isolated-import");
    let _env = ProfileCommandsTestEnv::new(&sandbox_dir.path);
    let _claude_bin = TestEnvVarGuard::set(
        "CLAUDE_BIN",
        &sandbox_dir.path.join("missing-claude").display().to_string(),
    );
    let source_dir = sandbox_dir.path.join("home/.claude");
    write_claude_import_source(&source_dir, "isolated-claude-access-token");
    #[cfg(windows)]
    let _source_override = TestEnvVarGuard::set(
        "CLAUDE_CONFIG_DIR",
        &source_dir.display().to_string(),
    );
    #[cfg(not(windows))]
    let _source_override = TestEnvVarGuard::unset("CLAUDE_CONFIG_DIR");

    handle_import_claude_profile(&claude_import_args(Some("claude-default"), true))
        .expect("isolated Claude credentials should import");

    let paths = AppPaths::discover().expect("Prodex paths should resolve");
    let state = AppState::load(&paths).expect("profile state should load");
    assert_eq!(state.profiles.len(), 1);
    let profile = state
        .profiles
        .get("claude-default")
        .expect("imported Claude profile should exist");
    assert_eq!(profile.email.as_deref(), Some("claude@example.com"));
    assert!(matches!(profile.provider, ProfileProvider::Anthropic { .. }));
    assert_eq!(
        fs::read_to_string(profile.codex_home.join(CLAUDE_CREDENTIALS_FILE))
            .expect("managed Claude credentials should be readable"),
        claude_import_credentials("isolated-claude-access-token")
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        assert_eq!(
            fs::metadata(profile.codex_home.join(CLAUDE_CREDENTIALS_FILE))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }
}

#[test]
fn profile_import_claude_supports_custom_source_and_updates_duplicate_profile() {
    let sandbox_dir = ProfileCommandsTestDir::new("claude-custom-import");
    let _env = ProfileCommandsTestEnv::new(&sandbox_dir.path);
    let source_dir = sandbox_dir.path.join("external claude credentials");
    write_claude_import_source(&source_dir, "old-claude-access-token");
    let _source_override = TestEnvVarGuard::set(
        "CLAUDE_CONFIG_DIR",
        &source_dir.display().to_string(),
    );
    let _claude_bin = TestEnvVarGuard::set(
        "CLAUDE_BIN",
        &sandbox_dir.path.join("missing-claude").display().to_string(),
    );

    handle_import_claude_profile(&claude_import_args(Some("claude-main"), true))
        .expect("custom Claude credentials should import");
    fs::write(
        source_dir.join(CLAUDE_CREDENTIALS_FILE),
        claude_import_credentials("new-claude-access-token"),
    )
    .expect("updated Claude credentials should be written");
    handle_import_claude_profile(&claude_import_args(None, false))
        .expect("duplicate Claude credentials should update existing profile");

    let paths = AppPaths::discover().expect("Prodex paths should resolve");
    let state = AppState::load(&paths).expect("profile state should load");
    assert_eq!(state.profiles.len(), 1);
    let profile = state
        .profiles
        .get("claude-main")
        .expect("duplicate import should retain profile name");
    assert_eq!(
        fs::read_to_string(profile.codex_home.join(CLAUDE_CREDENTIALS_FILE))
            .expect("updated managed Claude credentials should be readable"),
        claude_import_credentials("new-claude-access-token")
    );
}

#[test]
fn profile_import_claude_credentials_survive_managed_runtime_preparation() {
    let sandbox_dir = ProfileCommandsTestDir::new("claude-runtime-preparation");
    let _env = ProfileCommandsTestEnv::new(&sandbox_dir.path);
    let source_dir = sandbox_dir.path.join("external-claude");
    write_claude_import_source(&source_dir, "runtime-claude-access-token");
    let _source_override = TestEnvVarGuard::set(
        "CLAUDE_CONFIG_DIR",
        &source_dir.display().to_string(),
    );
    let _claude_bin = TestEnvVarGuard::set(
        "CLAUDE_BIN",
        &sandbox_dir.path.join("missing-claude").display().to_string(),
    );

    handle_import_claude_profile(&claude_import_args(Some("claude-runtime"), true))
        .expect("Claude credentials should import");

    let paths = AppPaths::discover().expect("Prodex paths should resolve");
    let profile = AppState::load(&paths)
        .expect("profile state should load")
        .profiles
        .remove("claude-runtime")
        .expect("imported Claude profile should exist");
    prodex_shared_codex_fs::prepare_managed_codex_home_for_runtime_launch_with_local_credentials(
        &paths,
        &profile.codex_home,
    )
    .expect("managed runtime home should prepare");
    prodex_shared_codex_fs::prepare_managed_codex_home_for_runtime_launch_with_local_credentials(
        &paths,
        &profile.codex_home,
    )
    .expect("managed runtime home should prepare again after restart");
    assert!(
        fs::symlink_metadata(profile.codex_home.join(CLAUDE_CREDENTIALS_FILE))
            .expect("managed Claude credentials metadata should exist")
            .file_type()
            .is_file()
    );

    assert_eq!(
        read_claude_oauth_secret(&profile.codex_home)
            .expect("managed Claude credentials should remain private")
            .access_token,
        "runtime-claude-access-token"
    );
}

#[cfg(unix)]
#[test]
fn managed_claude_credentials_do_not_copy_unattributed_legacy_shared_state() {
    use std::os::unix::fs::{PermissionsExt as _, symlink};

    let sandbox_dir = ProfileCommandsTestDir::new("claude-legacy-shared-credentials");
    let paths = profile_commands_test_paths(&sandbox_dir.path);
    let codex_home = paths.managed_profiles_root.join("claude-runtime");
    let shared_credentials = paths
        .shared_codex_root
        .join(CLAUDE_CREDENTIALS_FILE);
    create_codex_home_if_missing(&codex_home).expect("profile home should exist");
    create_codex_home_if_missing(&paths.shared_codex_root).expect("shared home should exist");
    let shared_state = r#"{"mcpServers":{"example":{"accessToken":"mcp-token"}}}"#;
    write_secret_text_file(&shared_credentials, shared_state)
        .expect("legacy shared state should be written");
    fs::set_permissions(
        &shared_credentials,
        fs::Permissions::from_mode(0o644),
    )
    .expect("legacy shared credentials permissions should be set");
    symlink(&shared_credentials, codex_home.join(CLAUDE_CREDENTIALS_FILE))
        .expect("legacy credential link should be created");

    prodex_shared_codex_fs::prepare_managed_codex_home_with_local_credentials(&paths, &codex_home)
        .expect("managed Claude home should remove its legacy link");

    let local_path = codex_home.join(CLAUDE_CREDENTIALS_FILE);
    assert!(!local_path.exists());
    assert_eq!(
        fs::read_to_string(&shared_credentials).expect("shared state should remain readable"),
        shared_state
    );

    let source_dir = sandbox_dir.path.join("fresh claude login");
    write_claude_import_source(&source_dir, "fresh-profile-access-token");
    crate::copy_claude_oauth_credentials(&source_dir, &codex_home)
        .expect("fresh Claude credentials should replace the removed link");
    assert_eq!(
        fs::metadata(&local_path)
            .expect("fresh credentials metadata should read")
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    assert_eq!(
        read_claude_oauth_secret(&codex_home)
            .expect("fresh credentials should remain readable")
            .access_token,
        "fresh-profile-access-token"
    );
    assert_eq!(
        fs::read_to_string(shared_credentials).expect("shared state should stay unchanged"),
        shared_state
    );
}

#[cfg(unix)]
#[test]
fn profile_import_claude_rejects_symlinked_source_root_and_credentials() {
    use std::os::unix::fs::symlink;

    let sandbox_dir = ProfileCommandsTestDir::new("claude-unsafe-import");
    let _env = ProfileCommandsTestEnv::new(&sandbox_dir.path);
    let real_source = sandbox_dir.path.join("real-claude");
    write_claude_import_source(&real_source, "unsafe-claude-access-token");
    let source_link = sandbox_dir.path.join("linked-claude");
    symlink(&real_source, &source_link).expect("source directory symlink should be created");
    let _source_override = TestEnvVarGuard::set(
        "CLAUDE_CONFIG_DIR",
        &source_link.display().to_string(),
    );
    let _claude_bin = TestEnvVarGuard::set(
        "CLAUDE_BIN",
        &sandbox_dir.path.join("missing-claude").display().to_string(),
    );

    let error = handle_import_claude_profile(&claude_import_args(Some("unsafe"), false))
        .expect_err("symlinked Claude source root should be rejected");
    assert!(format!("{error:#}").contains("regular secret file"));
    let paths = AppPaths::discover().expect("Prodex paths should resolve");
    assert!(AppState::load(&paths)
        .expect("profile state should load")
        .profiles
        .is_empty());

    drop(_source_override);
    let linked_credentials_source = sandbox_dir.path.join("linked-credentials-claude");
    create_codex_home_if_missing(&linked_credentials_source)
        .expect("linked credentials source directory should be created");
    let source_file_link = linked_credentials_source.join(CLAUDE_CREDENTIALS_FILE);
    symlink(
        real_source.join(CLAUDE_CREDENTIALS_FILE),
        &source_file_link,
    )
    .expect("source credentials symlink should be created");
    let _source_override = TestEnvVarGuard::set(
        "CLAUDE_CONFIG_DIR",
        &linked_credentials_source.display().to_string(),
    );
    let error = handle_import_claude_profile(&claude_import_args(Some("unsafe"), false))
        .expect_err("symlinked Claude credentials should be rejected");
    assert!(format!("{error:#}").contains("regular secret file"));
}
