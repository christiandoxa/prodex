use super::*;

#[test]
fn persist_login_home_preserves_refresh_token_from_codex_login_home() {
    let target_dir = ProfileCommandsTestDir::new("login-refresh-token-new");
    let login_home = target_dir.path.join("login-home");
    let codex_home = target_dir.path.join("profiles/main");
    create_codex_home_if_missing(&login_home).expect("login home should exist");
    write_secret_text_file(
        &login_home.join("auth.json"),
        &profile_commands_auth_json_without_email(
            "fresh-access-token",
            "main-account",
            "fresh-refresh-token",
        ),
    )
    .expect("login auth should be written");

    persist_login_home(&login_home, &codex_home).expect("login home should persist");

    assert_eq!(
        profile_commands_read_access_token(&codex_home),
        "fresh-access-token"
    );
    assert_eq!(
        profile_commands_read_refresh_token(&codex_home),
        "fresh-refresh-token"
    );
    assert!(
        !login_home.exists(),
        "temporary login home should be consumed after persistence"
    );
}

#[test]
fn update_existing_profile_auth_preserves_refresh_token_from_login_auth_json() {
    let target_dir = ProfileCommandsTestDir::new("login-refresh-token-existing");
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
    let mut state = AppState {
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

    update_existing_profile_auth(
        &target_paths,
        &mut state,
        "main",
        Some("main@example.com"),
        &profile_commands_auth_json_with_email_and_refresh(
            "main@example.com",
            "fresh-access-token",
            "main-account",
            Some("fresh-refresh-token"),
        ),
        true,
    )
    .expect("existing profile auth should update");

    assert_eq!(state.active_profile.as_deref(), Some("main"));
    assert_eq!(
        profile_commands_read_access_token(&existing_home),
        "fresh-access-token"
    );
    assert_eq!(
        profile_commands_read_refresh_token(&existing_home),
        "fresh-refresh-token"
    );
}
