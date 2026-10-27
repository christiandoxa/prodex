use super::*;
use std::time::{Duration, SystemTime};

#[test]
fn login_uses_local_profile_identity_without_querying_other_accounts() {
    let fixture = setup_fixture();
    let mut state = read_state(&fixture.prodex_home);
    state["profiles"]["main"]["email"] = json!("main@example.com");
    state["profiles"]["second"]["email"] = json!("second@example.com");
    write_json(&fixture.prodex_home.join("state.json"), &state);
    let output = run_prodex_with_env(
        &fixture,
        &["login"],
        &[("TEST_LOGIN_ACCOUNT_ID", "main-account")],
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(read_state(&fixture.prodex_home)["active_profile"], "main");
    assert_eq!(
        fixture.usage_server.max_concurrent_requests(),
        0,
        "saving fresh login credentials must not query quota for other profiles under the lifecycle lock"
    );
}

fn assert_login_preserves_history(args: &[&str], existing: bool) {
    let fixture = setup_fixture();
    let mut state = read_state(&fixture.prodex_home);
    if existing {
        state["profiles"]["main"]["email"] = json!("main@example.com");
        state["profiles"].as_object_mut().unwrap().remove("second");
        write_json(
            &fixture.main_home.join("auth.json"),
            &json!({"tokens": {
                "id_token": chatgpt_id_token("main@example.com"),
                "access_token": "main-token", "account_id": "main-account"
            }}),
        );
    } else {
        state = json!({"profiles": {}});
    }
    write_json(&fixture.prodex_home.join("state.json"), &state);
    let session = fixture
        .shared_codex_home
        .join("sessions/2026/01/01/rollout-synthetic.jsonl");
    fs::create_dir_all(session.parent().unwrap()).unwrap();
    let contents = "{\"timestamp\":\"2026-01-01T00:00:00Z\",\"type\":\"session_meta\",\"payload\":{\"id\":\"01900000-0000-7000-8000-000000000001\"}}\n";
    fs::write(&session, contents).unwrap();
    let timestamp = SystemTime::UNIX_EPOCH + Duration::from_secs(1_800_000_000);
    fs::File::options()
        .write(true)
        .open(&session)
        .unwrap()
        .set_modified(timestamp)
        .unwrap();
    let before = fs::metadata(&session).unwrap().modified().unwrap();
    let output = if args.contains(&"--with-api-key") {
        run_prodex_with_env_and_stdin(&fixture, args, &[], "synthetic-api-key\n")
    } else {
        run_prodex_with_env(&fixture, args, &[("TEST_LOGIN_ACCOUNT_ID", "main-account")])
    };
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read_to_string(&session).unwrap(),
        contents,
        "login must not rewrite unrelated rollout contents"
    );
    assert_eq!(
        fs::metadata(&session).unwrap().modified().unwrap(),
        before,
        "login must not run historical session maintenance before saving credentials"
    );
    assert!(
        !fixture
            .prodex_home
            .join("shared-codex-session-maintenance-v1.json")
            .exists(),
        "session-maintenance bookkeeping is not part of login"
    );
    let saved = read_state(&fixture.prodex_home);
    let name = saved["active_profile"].as_str().unwrap();
    let home = std::path::Path::new(saved["profiles"][name]["codex_home"].as_str().unwrap());
    assert!(home.join("auth.json").is_file());
    assert_eq!(
        fs::canonicalize(home.join("sessions")).unwrap(),
        fs::canonicalize(fixture.shared_codex_home.join("sessions")).unwrap(),
        "deferred maintenance must retain shared-session wiring"
    );
}

#[test]
fn new_login_saves_credentials_without_historical_session_maintenance() {
    assert_login_preserves_history(&["login"], false);
}

#[test]
fn existing_login_saves_credentials_without_historical_session_maintenance() {
    assert_login_preserves_history(&["login"], true);
}

#[test]
fn named_login_saves_credentials_without_historical_session_maintenance() {
    assert_login_preserves_history(&["login", "--profile", "main"], true);
}

#[test]
fn api_key_login_saves_credentials_without_historical_session_maintenance() {
    assert_login_preserves_history(&["login", "--with-api-key"], false);
}

#[test]
fn named_api_key_login_saves_credentials_without_historical_session_maintenance() {
    assert_login_preserves_history(&["login", "--profile", "main", "--with-api-key"], true);
}
