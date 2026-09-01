use super::*;

fn sorted_log_lines(path: &std::path::Path) -> Vec<String> {
    let mut lines = fs::read_to_string(path)
        .expect("failed to read ping homes log")
        .lines()
        .map(str::to_string)
        .collect::<Vec<_>>();
    lines.sort();
    lines
}

fn assert_profiles_were_probed(path: &std::path::Path, profiles: &[std::path::PathBuf]) {
    let homes = sorted_log_lines(path);
    let mut expected = profiles
        .iter()
        .map(|profile| profile.to_string_lossy().to_string())
        .collect::<Vec<_>>();
    expected.sort();
    assert_eq!(homes, expected);
}

fn assert_ping_args(fixture: &Fixture, expected_count: usize) {
    let args = fs::read_to_string(&fixture.codex_args_log)
        .expect("failed to read codex args log")
        .lines()
        .map(str::to_string)
        .collect::<Vec<_>>();
    assert_eq!(
        args.iter().filter(|arg| arg.as_str() == "--json").count(),
        expected_count
    );
    assert_eq!(
        args.iter().filter(|arg| arg.as_str() == "exec").count(),
        expected_count
    );
    assert_eq!(
        args.iter().filter(|arg| arg.as_str() == "ping").count(),
        expected_count
    );
    assert!(args.iter().any(|arg| arg == "--sandbox"));
    assert!(args.iter().any(|arg| arg == "read-only"));
    assert!(args.iter().any(|arg| arg == "--ephemeral"));
    assert!(args.iter().any(|arg| arg == "--ignore-user-config"));
    assert!(args.iter().any(|arg| arg == "--ignore-rules"));
    assert!(args.iter().any(|arg| arg == "--skip-git-repo-check"));
    assert!(
        !args
            .iter()
            .any(|arg| arg == "--dangerously-bypass-approvals-and-sandbox")
    );
}

#[test]
fn ping_openai_probes_every_configured_openai_profile() {
    let fixture = setup_fixture();
    let third_home = add_managed_profile(&fixture, "third", "third-account");
    let args_log = fixture.codex_args_log.display().to_string();
    let home_log = fixture._temp_dir.path.join("ping-homes.log");
    let home_log_string = home_log.display().to_string();

    let output = run_prodex_with_env(
        &fixture,
        &["ping", "openai"],
        &[
            ("TEST_CODEX_ARGS_LOG", args_log.as_str()),
            ("TEST_CODEX_ARGS_LOG_APPEND", "1"),
            ("TEST_CODEX_LOG_APPEND", home_log_string.as_str()),
        ],
    );

    assert!(
        output.status.success(),
        "prodex ping openai failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_profiles_were_probed(
        &home_log,
        &[
            fixture.main_home.clone(),
            fixture.second_home.clone(),
            third_home,
        ],
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Profiles discovered: 3"), "{stdout}");
    assert!(stdout.contains("Profiles tested: 3"), "{stdout}");
    assert!(stdout.contains("Healthy: 3"), "{stdout}");
    assert!(stdout.contains("Pool usable: yes"), "{stdout}");
    assert_ping_args(&fixture, 3);
}

#[test]
fn ping_openai_includes_profile_that_is_ready_on_weekly_quota_only() {
    let fixture = setup_fixture();
    let weekly_home = add_managed_profile(&fixture, "weekly-only", "weekly-only-account");
    let home_log = fixture._temp_dir.path.join("ping-weekly-only-homes.log");
    let home_log_string = home_log.display().to_string();

    let output = run_prodex_with_env(
        &fixture,
        &["ping", "openai"],
        &[("TEST_CODEX_LOG_APPEND", home_log_string.as_str())],
    );

    assert!(output.status.success());
    assert_profiles_were_probed(
        &home_log,
        &[
            fixture.main_home.clone(),
            fixture.second_home.clone(),
            weekly_home,
        ],
    );
}

#[test]
fn ping_openai_uses_all_profiles_when_live_quota_probe_is_unavailable() {
    let fixture = setup_fixture();
    let third_home = add_managed_profile(&fixture, "third", "third-account");
    let home_log = fixture._temp_dir.path.join("ping-snapshot-homes.log");
    let home_log_string = home_log.display().to_string();

    let output = run_prodex_with_env(
        &fixture,
        &[
            "ping",
            "openai",
            "--base-url",
            "http://127.0.0.1:1/backend-api",
        ],
        &[("TEST_CODEX_LOG_APPEND", home_log_string.as_str())],
    );

    assert!(output.status.success());
    assert_profiles_were_probed(
        &home_log,
        &[
            fixture.main_home.clone(),
            fixture.second_home.clone(),
            third_home,
        ],
    );
}

#[test]
fn ping_openai_continues_after_first_structured_turn_failure() {
    let fixture = setup_fixture();
    let third_home = add_managed_profile(&fixture, "third", "third-account");
    let home_log = fixture._temp_dir.path.join("ping-protocol-homes.log");
    let home_log_string = home_log.display().to_string();

    let output = run_prodex_with_env(
        &fixture,
        &["ping", "openai"],
        &[
            ("TEST_CODEX_FAILURE_PROFILE", "main"),
            ("TEST_CODEX_FAILURE_KIND", "protocol"),
            ("TEST_CODEX_LOG_APPEND", home_log_string.as_str()),
        ],
    );

    assert!(!output.status.success());
    assert_profiles_were_probed(
        &home_log,
        &[
            fixture.main_home.clone(),
            fixture.second_home.clone(),
            third_home,
        ],
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("main  PROTOCOL_FAILED"), "{stdout}");
    assert!(stdout.contains("second  OK"), "{stdout}");
    assert!(stdout.contains("third  OK"), "{stdout}");
    assert!(stdout.contains("Healthy: 2"), "{stdout}");
}

#[test]
fn ping_openai_reports_exhaustion_and_continues() {
    let fixture = setup_fixture();
    let home_log = fixture._temp_dir.path.join("ping-exhausted-homes.log");
    let home_log_string = home_log.display().to_string();

    let output = run_prodex_with_env(
        &fixture,
        &["ping", "openai"],
        &[
            ("TEST_CODEX_FAILURE_PROFILE", "main"),
            ("TEST_CODEX_FAILURE_KIND", "exhausted"),
            ("TEST_CODEX_LOG_APPEND", home_log_string.as_str()),
        ],
    );

    assert!(!output.status.success());
    assert_profiles_were_probed(
        &home_log,
        &[fixture.main_home.clone(), fixture.second_home.clone()],
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("main  EXHAUSTED"), "{stdout}");
    assert!(stdout.contains("Healthy: 1"), "{stdout}");
    assert!(stdout.contains("Exhausted: 1"), "{stdout}");
    assert!(stdout.contains("Pool usable: yes"), "{stdout}");
}

#[test]
fn ping_openai_reports_middle_503_without_calling_it_quota() {
    let fixture = setup_fixture();
    let third_home = add_managed_profile(&fixture, "third", "third-account");
    let home_log = fixture._temp_dir.path.join("ping-overload-homes.log");
    let home_log_string = home_log.display().to_string();

    let output = run_prodex_with_env(
        &fixture,
        &["ping", "openai"],
        &[
            ("TEST_CODEX_FAILURE_PROFILE", "second"),
            ("TEST_CODEX_FAILURE_KIND", "overloaded"),
            ("TEST_CODEX_LOG_APPEND", home_log_string.as_str()),
        ],
    );

    assert!(!output.status.success());
    assert_profiles_were_probed(
        &home_log,
        &[
            fixture.main_home.clone(),
            fixture.second_home.clone(),
            third_home,
        ],
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("main  OK"), "{stdout}");
    assert!(stdout.contains("second  UPSTREAM_OVERLOADED"), "{stdout}");
    assert!(stdout.contains("third  OK"), "{stdout}");
    assert!(stdout.contains("Exhausted: 0"), "{stdout}");
    assert!(stdout.contains("Pool usable: yes"), "{stdout}");
}

#[test]
fn ping_openai_reports_generic_429_as_rate_limited() {
    let fixture = setup_fixture();
    let third_home = add_managed_profile(&fixture, "third", "third-account");
    let home_log = fixture._temp_dir.path.join("ping-rate-limited-homes.log");
    let home_log_string = home_log.display().to_string();

    let output = run_prodex_with_env(
        &fixture,
        &["ping", "openai"],
        &[
            ("TEST_CODEX_FAILURE_PROFILE", "second"),
            ("TEST_CODEX_FAILURE_KIND", "rate_limited"),
            ("TEST_CODEX_LOG_APPEND", home_log_string.as_str()),
        ],
    );

    assert!(!output.status.success());
    assert_profiles_were_probed(
        &home_log,
        &[
            fixture.main_home.clone(),
            fixture.second_home.clone(),
            third_home,
        ],
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("second  RATE_LIMITED"), "{stdout}");
    assert!(stdout.contains("Exhausted: 0"), "{stdout}");
}

#[test]
fn ping_openai_reports_malformed_output_and_continues() {
    let fixture = setup_fixture();
    let third_home = add_managed_profile(&fixture, "third", "third-account");
    let home_log = fixture._temp_dir.path.join("ping-malformed-homes.log");
    let home_log_string = home_log.display().to_string();

    let output = run_prodex_with_env(
        &fixture,
        &["ping", "openai"],
        &[
            ("TEST_CODEX_FAILURE_PROFILE", "main"),
            ("TEST_CODEX_FAILURE_KIND", "malformed"),
            ("TEST_CODEX_LOG_APPEND", home_log_string.as_str()),
        ],
    );

    assert!(!output.status.success());
    assert_profiles_were_probed(
        &home_log,
        &[
            fixture.main_home.clone(),
            fixture.second_home.clone(),
            third_home,
        ],
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("main  PROTOCOL_FAILED"), "{stdout}");
    assert!(stdout.contains("second  OK"), "{stdout}");
    assert!(stdout.contains("third  OK"), "{stdout}");
}

#[test]
fn ping_openai_reports_child_spawn_failure_and_continues() {
    let fixture = setup_fixture();
    let home_log = fixture._temp_dir.path.join("ping-spawn-homes.log");
    let home_log_string = home_log.display().to_string();

    let output = run_prodex_with_env(
        &fixture,
        &["ping", "openai"],
        &[
            ("TEST_CODEX_FAILURE_PROFILE", "main"),
            ("TEST_CODEX_FAILURE_KIND", "spawn"),
            ("TEST_CODEX_LOG_APPEND", home_log_string.as_str()),
        ],
    );

    assert!(!output.status.success());
    assert_profiles_were_probed(
        &home_log,
        &[fixture.main_home.clone(), fixture.second_home.clone()],
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("main  SPAWN_FAILED"), "{stdout}");
    assert!(stdout.contains("second  OK"), "{stdout}");
}

#[test]
fn ping_openai_reports_fast_nonzero_exit_detail_and_continues() {
    let fixture = setup_fixture();
    let home_log = fixture._temp_dir.path.join("ping-process-homes.log");
    let home_log_string = home_log.display().to_string();

    let output = run_prodex_with_env(
        &fixture,
        &["ping", "openai"],
        &[
            ("TEST_CODEX_FAILURE_PROFILE", "main"),
            ("TEST_CODEX_FAILURE_KIND", "process"),
            ("TEST_CODEX_LOG_APPEND", home_log_string.as_str()),
        ],
    );

    assert!(!output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("main  PROCESS_FAILED"), "{stdout}");
    assert!(stdout.contains("exit code 23"), "{stdout}");
    assert!(stdout.contains("fast child failure"), "{stdout}");
    assert!(!stdout.contains("fixture-token"), "{stdout}");
    assert!(stdout.contains("second  OK"), "{stdout}");
}

#[test]
fn ping_openai_json_contains_per_profile_results() {
    let fixture = setup_fixture();
    let output = run_prodex(&fixture, &["ping", "openai", "--json"]);
    assert!(
        output.status.success(),
        "prodex ping openai --json failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: Value = serde_json::from_slice(&output.stdout).expect("ping JSON should parse");
    assert_eq!(value["provider"], "openai");
    assert_eq!(value["status"], "ok");
    assert_eq!(value["profiles"].as_array().unwrap().len(), 2);
    assert_eq!(value["summary"]["profiles_discovered"], 2);
    assert_eq!(value["summary"]["profiles_tested"], 2);
    assert_eq!(value["summary"]["pool_usable"], true);
}

#[test]
fn ping_openai_profile_selector_pins_one_profile() {
    let fixture = setup_fixture();
    let home_log = fixture._temp_dir.path.join("ping-pinned-homes.log");
    let home_log_string = home_log.display().to_string();

    let output = run_prodex_with_env(
        &fixture,
        &["ping", "openai", "--profile", "second"],
        &[("TEST_CODEX_LOG_APPEND", home_log_string.as_str())],
    );

    assert!(output.status.success());
    assert_profiles_were_probed(&home_log, std::slice::from_ref(&fixture.second_home));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Profiles discovered: 1"), "{stdout}");
    assert!(stdout.contains("Pool usable: yes"), "{stdout}");
}

#[test]
fn ping_openai_does_not_probe_spark_separately() {
    let fixture = setup_fixture();
    let spark_home = add_managed_profile(&fixture, "spark", "spark-account");
    let args_log = fixture.codex_args_log.display().to_string();
    let home_log = fixture._temp_dir.path.join("ping-spark-homes.log");
    let home_log_string = home_log.display().to_string();

    let output = run_prodex_with_env(
        &fixture,
        &["ping", "openai"],
        &[
            ("TEST_CODEX_ARGS_LOG", args_log.as_str()),
            ("TEST_CODEX_ARGS_LOG_APPEND", "1"),
            ("TEST_CODEX_LOG_APPEND", home_log_string.as_str()),
        ],
    );

    assert!(output.status.success());
    assert_profiles_were_probed(
        &home_log,
        &[
            fixture.main_home.clone(),
            fixture.second_home.clone(),
            spark_home,
        ],
    );
    let args = fs::read_to_string(&fixture.codex_args_log).expect("failed to read args log");
    assert!(!args.lines().any(|arg| arg == "gpt-5.3-codex-spark"));
}

#[test]
fn ping_openai_ignores_unrelated_profile_session_files_without_aborting_inventory() {
    let fixture = setup_fixture();
    let broken_home = add_managed_profile(&fixture, "broken", "unknown-account");
    fs::write(broken_home.join("sessions"), b"not a directory")
        .expect("failed to create broken sessions path");
    let state_before = fs::read(fixture.prodex_home.join("state.json"))
        .expect("state should be readable before ping");
    let sessions_before = fs::read(broken_home.join("sessions"))
        .expect("unrelated session state should be readable before ping");
    let home_log = fixture._temp_dir.path.join("ping-error-homes.log");
    let home_log_string = home_log.display().to_string();

    let output = run_prodex_with_env(
        &fixture,
        &["ping", "openai"],
        &[("TEST_CODEX_LOG_APPEND", home_log_string.as_str())],
    );

    assert!(
        output.status.success(),
        "unrelated profile session state should not block ping: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let homes = sorted_log_lines(&home_log);
    assert!(homes.contains(&fixture.main_home.to_string_lossy().to_string()));
    assert!(homes.contains(&fixture.second_home.to_string_lossy().to_string()));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Profiles tested: 3"), "{stdout}");
    assert!(stdout.contains("Healthy: 3"), "{stdout}");
    assert_eq!(
        fs::read(fixture.prodex_home.join("state.json")).unwrap(),
        state_before
    );
    assert_eq!(
        fs::read(broken_home.join("sessions")).unwrap(),
        sessions_before
    );
}
