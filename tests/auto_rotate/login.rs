use super::*;

#[test]
fn login_without_profile_creates_profile_from_email() {
    let fixture = setup_fixture();
    write_json(
        &fixture.prodex_home.join("state.json"),
        &json!({
            "profiles": {}
        }),
    );

    let output = run_prodex_with_env(
        &fixture,
        &["login"],
        &[("TEST_LOGIN_ACCOUNT_ID", "main-account")],
    );

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let state = read_state(&fixture.prodex_home);
    assert_eq!(state["active_profile"], "main_example.com");
    assert_eq!(
        state["profiles"]["main_example.com"]["email"],
        "main@example.com"
    );
    assert_eq!(
        state["profiles"].as_object().map(|profiles| profiles.len()),
        Some(1)
    );
    assert!(
        state["profiles"]["main_example.com"]["codex_home"]
            .as_str()
            .expect("codex_home should be a string")
            .ends_with("/profiles/main_example.com")
    );
    assert!(
        fixture
            .prodex_home
            .join("profiles/main_example.com/auth.json")
            .is_file()
    );
    assert!(
        String::from_utf8_lossy(&output.stdout)
            .contains("Logged in as main@example.com. Created profile 'main_example.com'.")
    );
}

#[test]
fn login_without_profile_reuses_existing_profile_for_same_email() {
    let fixture = setup_fixture();
    write_json(
        &fixture.prodex_home.join("state.json"),
        &json!({
            "active_profile": "primary",
            "profiles": {
                "primary": {
                    "codex_home": fixture.main_home,
                    "managed": true
                }
            }
        }),
    );

    let output = run_prodex_with_env(
        &fixture,
        &["login"],
        &[("TEST_LOGIN_ACCOUNT_ID", "main-account")],
    );

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let state = read_state(&fixture.prodex_home);
    assert_eq!(state["active_profile"], "primary");
    assert_eq!(state["profiles"]["primary"]["email"], "main@example.com");
    assert_eq!(
        state["profiles"].as_object().map(|profiles| profiles.len()),
        Some(1)
    );
    assert!(!fixture.prodex_home.join("profiles/primary").exists());
    assert!(String::from_utf8_lossy(&output.stdout).contains(
        "Logged in as main@example.com. Updated auth token for existing profile 'primary'."
    ));
}

#[test]
fn login_without_profile_does_not_reuse_email_derived_profile_name_for_other_email() {
    let fixture = setup_fixture();
    let id_token = chatgpt_id_token("main@example.com");
    write_json(
        &fixture.main_home.join("auth.json"),
        &json!({
            "tokens": {
                "id_token": id_token,
                "access_token": "test-token",
                "account_id": "main-account"
            }
        }),
    );
    write_json(
        &fixture.prodex_home.join("state.json"),
        &json!({
            "active_profile": "customeradroit_gmail.com",
            "profiles": {
                "customeradroit_gmail.com": {
                    "codex_home": fixture.main_home,
                    "managed": true,
                    "email": "main@example.com"
                }
            }
        }),
    );

    let output = run_prodex_with_env(
        &fixture,
        &["login"],
        &[("TEST_LOGIN_ACCOUNT_ID", "main-account")],
    );

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let state = read_state(&fixture.prodex_home);
    assert_eq!(state["active_profile"], "main_example.com");
    assert_eq!(
        state["profiles"]["main_example.com"]["email"],
        "main@example.com"
    );
    assert!(
        String::from_utf8_lossy(&output.stdout)
            .contains("Logged in as main@example.com. Created profile 'main_example.com'.")
    );
}

#[test]
fn login_without_profile_updates_token_only_for_duplicate_email() {
    let fixture = setup_fixture();
    let id_token = chatgpt_id_token("main@example.com");
    write_json(
        &fixture.prodex_home.join("state.json"),
        &json!({
            "active_profile": "primary",
            "profiles": {
                "primary": {
                    "codex_home": fixture.main_home,
                    "managed": true
                }
            }
        }),
    );

    let output = run_prodex_with_env(
        &fixture,
        &["login"],
        &[
            ("TEST_LOGIN_ACCOUNT_ID", "main-account"),
            ("TEST_LOGIN_ACCESS_TOKEN", "fresh-token"),
            ("TEST_LOGIN_ID_TOKEN", id_token.as_str()),
            ("TEST_SESSION_MARKER", "duplicate-login"),
        ],
    );

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let auth_json = fs::read_to_string(fixture.main_home.join("auth.json"))
        .expect("updated auth.json should exist");
    assert_eq!(
        serde_json::from_str::<Value>(&auth_json)
            .expect("auth.json should parse")["tokens"]["access_token"]
            .as_str(),
        Some("fresh-token")
    );
    assert!(
        !fixture
            .shared_codex_home
            .join("sessions/duplicate-login.json")
            .exists(),
        "duplicate login should not copy session state into the existing profile"
    );
}

#[test]
fn login_without_profile_looks_up_existing_profiles_in_parallel() {
    let fixture = setup_fixture();
    add_managed_profile(&fixture, "third", "third-account");
    fixture.usage_server.set_delay_ms(80);

    let output = run_prodex_with_env(
        &fixture,
        &["login"],
        &[("TEST_LOGIN_ACCOUNT_ID", "main-account")],
    );

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains(
        "Logged in as main@example.com. Updated auth token for existing profile 'main'."
    ));
    assert!(
        fixture.usage_server.max_concurrent_requests() >= 2,
        "login profile lookup never overlapped"
    );
}

#[test]
fn login_without_profile_adds_suffix_when_email_name_is_taken() {
    let fixture = setup_fixture();
    write_json(
        &fixture.prodex_home.join("state.json"),
        &json!({
            "active_profile": "main_example.com",
            "profiles": {
                "main_example.com": {
                    "codex_home": fixture.second_home,
                    "managed": true,
                    "email": "second@example.com"
                }
            }
        }),
    );

    let output = run_prodex_with_env(
        &fixture,
        &["login"],
        &[("TEST_LOGIN_ACCOUNT_ID", "main-account")],
    );

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let state = read_state(&fixture.prodex_home);
    assert_eq!(state["active_profile"], "main_example.com-2");
    assert_eq!(
        state["profiles"]["main_example.com-2"]["email"],
        "main@example.com"
    );
    assert_eq!(
        state["profiles"]["main_example.com"]["email"],
        "second@example.com"
    );
    assert!(
        fixture
            .prodex_home
            .join("profiles/main_example.com-2/auth.json")
            .is_file()
    );
}

#[test]
fn login_without_profile_uses_auth_email_before_quota_lookup() {
    let fixture = setup_fixture();
    write_json(
        &fixture.prodex_home.join("state.json"),
        &json!({
            "profiles": {}
        }),
    );
    let id_token = chatgpt_id_token("token@example.com");

    let output = run_prodex_with_env(
        &fixture,
        &["login"],
        &[
            ("TEST_LOGIN_ACCOUNT_ID", "main-account"),
            ("TEST_LOGIN_ID_TOKEN", id_token.as_str()),
            ("CODEX_CHATGPT_BASE_URL", "http://127.0.0.1:1"),
        ],
    );

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let state = read_state(&fixture.prodex_home);
    assert_eq!(state["active_profile"], "token_example.com");
    assert_eq!(
        state["profiles"]["token_example.com"]["email"],
        "token@example.com"
    );
}

#[test]
fn login_without_profile_falls_back_to_usage_email_when_id_token_is_missing() {
    let fixture = setup_fixture();
    write_json(
        &fixture.prodex_home.join("state.json"),
        &json!({
            "profiles": {}
        }),
    );

    let output = run_prodex_with_env(
        &fixture,
        &["login"],
        &[
            ("TEST_LOGIN_ACCOUNT_ID", "main-account"),
            ("CODEX_CHATGPT_BASE_URL", fixture.usage_base_url.as_str()),
        ],
    );

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let state = read_state(&fixture.prodex_home);
    assert_eq!(state["active_profile"], "main_example.com");
    assert_eq!(
        state["profiles"]["main_example.com"]["email"],
        "main@example.com"
    );

    let auth_json = fs::read_to_string(
        fixture
            .prodex_home
            .join("profiles/main_example.com/auth.json"),
    )
    .expect("failed to read created auth.json");
    assert!(
        !auth_json.contains("\"id_token\""),
        "auth.json should not contain an id_token when login falls back to usage email"
    );
}
