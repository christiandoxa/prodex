use super::*;

#[test]
fn doctor_quota_checks_profiles_in_parallel() {
    let fixture = setup_fixture();
    fixture.usage_server.set_delay_ms(80);

    let output = run_prodex(&fixture, &["doctor", "--quota"]);

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        fixture.usage_server.max_concurrent_requests() >= 2,
        "doctor quota checks never overlapped"
    );
}

#[test]
fn doctor_warns_and_repairs_orphan_profile_import_auth_journals() {
    let fixture = setup_fixture();
    let journal_root = fixture.prodex_home.join("profile-import-auth-journal");
    let journal_path = journal_root.join("main-test.json");
    fs::create_dir_all(&journal_root).expect("failed to create journal root");

    let previous_auth = json!({
        "tokens": {
            "access_token": "old-token",
            "account_id": "main-account"
        }
    });
    write_json(
        &fixture.main_home.join("auth.json"),
        &json!({
            "tokens": {
                "access_token": "fresh-token",
                "account_id": "main-account"
            }
        }),
    );
    let mut state = read_state(&fixture.prodex_home);
    state["profiles"]["main"]["email"] = json!("imported@example.com");
    write_json(&fixture.prodex_home.join("state.json"), &state);
    write_json(
        &journal_path,
        &json!({
            "version": 1,
            "profile_name": "main",
            "codex_home": fixture.main_home.display().to_string(),
            "previous_email": "main@example.com",
            "previous_auth_json": previous_auth.to_string(),
            "created_at": "2026-04-28T00:00:00+00:00"
        }),
    );

    let output = run_prodex(&fixture, &["doctor"]);

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("profile-import-auth-journal contains 1 orphan journal(s)"),
        "doctor should surface orphan import auth journal, stdout: {stdout}"
    );
    assert!(
        stdout.contains("--repair-import-auth-journals"),
        "doctor should show repair command, stdout: {stdout}"
    );
    assert_eq!(read_access_token(&fixture.main_home), "fresh-token");
    assert!(journal_path.exists(), "plain doctor should not repair");

    let output = run_prodex(&fixture, &["doctor", "--repair-import-auth-journals"]);

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Repaired 1 orphan journal(s)."),
        "doctor repair should report recovered journal, stdout: {stdout}"
    );
    assert_eq!(read_access_token(&fixture.main_home), "old-token");
    assert!(
        !journal_path.exists(),
        "doctor repair should remove recovered journal"
    );
    let state = read_state(&fixture.prodex_home);
    assert_eq!(
        state["profiles"]["main"]["email"].as_str(),
        Some("main@example.com")
    );
}

#[test]
fn quota_raw_uses_builtin_usage_client() {
    let fixture = setup_fixture();

    let output = run_prodex(&fixture, &["quota", "--profile", "second", "--raw"]);

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let usage: Value =
        serde_json::from_slice(&output.stdout).expect("failed to parse raw quota output");
    assert_eq!(usage["email"], "second@example.com");
    assert_eq!(usage["plan_type"], "plus");
    assert_eq!(
        usage["rate_limit"]["secondary_window"]["limit_window_seconds"],
        604_800
    );
}

#[test]
fn quota_all_detail_shows_main_reset_times() {
    let fixture = setup_fixture();

    let output = run_prodex(&fixture, &["quota", "--all", "--detail", "--once"]);

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Quota Overview"));
    assert!(stdout.contains("REMAINING"));
    assert!(stdout.contains("status: Blocked: 5h exhausted until "));
    assert!(stdout.contains("status: Ready"));
    assert!(stdout.contains("resets: 5h "));
    assert!(stdout.contains("| weekly "));
}

#[test]
fn quota_all_detail_sorts_by_status_then_nearest_main_reset() {
    let fixture = setup_fixture();
    add_managed_profile(&fixture, "third", "third-account");

    let output = run_prodex(&fixture, &["quota", "--all", "--detail", "--once"]);

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let profile_lines = stdout
        .lines()
        .enumerate()
        .filter_map(|(index, line)| {
            let trimmed = line.trim_start();
            if trimmed.starts_with("second") {
                Some(("second", index))
            } else if trimmed.starts_with("third") {
                Some(("third", index))
            } else if trimmed.starts_with("main") {
                Some(("main", index))
            } else {
                None
            }
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    let second_index = *profile_lines
        .get("second")
        .expect("second profile should be rendered");
    let third_index = *profile_lines
        .get("third")
        .expect("third profile should be rendered");
    let main_index = *profile_lines
        .get("main")
        .expect("main profile should be rendered");

    assert!(
        second_index < third_index,
        "ready profile with sooner reset should sort first"
    );
    assert!(
        third_index < main_index,
        "blocked profiles should sort after ready profiles"
    );
}
