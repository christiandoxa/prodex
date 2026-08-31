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

#[test]
fn ping_openai_sends_one_canonical_model_request() {
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
    let homes = sorted_log_lines(&home_log);
    assert_eq!(
        homes.len(),
        1,
        "application ping must send one model request"
    );
    assert!(
        [
            fixture.main_home.to_string_lossy().to_string(),
            fixture.second_home.to_string_lossy().to_string(),
            third_home.to_string_lossy().to_string(),
        ]
        .contains(&homes[0]),
        "canonical ping selected an unknown profile: {}",
        homes[0]
    );
    let args = fs::read_to_string(&fixture.codex_args_log).expect("failed to read args log");
    let args = args.lines().collect::<Vec<_>>();
    assert_eq!(args.iter().filter(|arg| **arg == "--json").count(), 1);
    assert_eq!(args.iter().filter(|arg| **arg == "exec").count(), 1);
    assert_eq!(args.iter().filter(|arg| **arg == "ping").count(), 1);
    assert!(args.contains(&"--sandbox"));
    assert!(args.contains(&"read-only"));
    assert!(args.contains(&"--ephemeral"));
    assert!(args.contains(&"--ignore-user-config"));
    assert!(args.contains(&"--skip-git-repo-check"));
    assert!(!args.contains(&"--dangerously-bypass-approvals-and-sandbox"));
}

#[test]
fn ping_openai_uses_a_profile_that_is_ready_on_weekly_quota_only() {
    let fixture = setup_fixture();
    let weekly_home = add_managed_profile(&fixture, "weekly-only", "weekly-only-account");
    let home_log = fixture._temp_dir.path.join("ping-weekly-only-homes.log");
    let home_log_string = home_log.display().to_string();

    let output = run_prodex_with_env(
        &fixture,
        &["ping", "openai"],
        &[("TEST_CODEX_LOG_APPEND", home_log_string.as_str())],
    );
    assert!(
        output.status.success(),
        "prodex ping openai failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let homes = sorted_log_lines(&home_log);
    assert_eq!(
        homes.len(),
        1,
        "application ping must send one model request"
    );
    assert!(
        [
            fixture.main_home.to_string_lossy().to_string(),
            fixture.second_home.to_string_lossy().to_string(),
            weekly_home.to_string_lossy().to_string(),
        ]
        .contains(&homes[0])
    );
}

#[test]
fn ping_openai_uses_ready_snapshots_when_live_quota_probe_fails() {
    let fixture = setup_fixture();
    let third_home = add_managed_profile(&fixture, "third", "third-account");
    let quota = run_prodex(&fixture, &["quota", "--all", "--once"]);
    assert!(
        quota.status.success(),
        "failed to seed quota snapshots: {}",
        String::from_utf8_lossy(&quota.stderr)
    );
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

    assert!(
        output.status.success(),
        "prodex ping openai failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let homes = sorted_log_lines(&home_log);
    assert_eq!(
        homes.len(),
        1,
        "snapshot-backed ping must send one model request"
    );
    assert!(
        [
            fixture.main_home.to_string_lossy().to_string(),
            fixture.second_home.to_string_lossy().to_string(),
            third_home.to_string_lossy().to_string(),
        ]
        .contains(&homes[0])
    );
}

#[test]
fn ping_openai_ignores_unrelated_profile_session_files() {
    let fixture = setup_fixture();
    let broken_home = add_managed_profile(&fixture, "broken", "unknown-account");
    fs::write(broken_home.join("sessions"), b"not a directory")
        .expect("failed to create broken sessions path");
    let home_log = fixture._temp_dir.path.join("ping-error-homes.log");
    let home_log_string = home_log.display().to_string();

    let output = run_prodex_with_env(
        &fixture,
        &["ping", "openai"],
        &[("TEST_CODEX_LOG_APPEND", home_log_string.as_str())],
    );
    assert!(
        output.status.success(),
        "isolated ping should not depend on the profile session directory: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let homes = sorted_log_lines(&home_log);
    assert_eq!(homes.len(), 1, "isolated ping must send one model request");
    assert!(
        [
            broken_home.to_string_lossy().to_string(),
            fixture.main_home.to_string_lossy().to_string(),
            fixture.second_home.to_string_lossy().to_string(),
        ]
        .contains(&homes[0])
    );
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

    assert!(
        output.status.success(),
        "prodex ping openai failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let homes = sorted_log_lines(&home_log);
    assert_eq!(
        homes.len(),
        1,
        "application ping must not probe every quota bucket"
    );
    assert_eq!(homes[0], spark_home.to_string_lossy());
    let args = fs::read_to_string(&fixture.codex_args_log).expect("failed to read args log");
    let args = args.lines().collect::<Vec<_>>();
    assert_eq!(args.iter().filter(|arg| **arg == "--json").count(), 1);
    assert_eq!(args.iter().filter(|arg| **arg == "exec").count(), 1);
    assert_eq!(args.iter().filter(|arg| **arg == "ping").count(), 1);
    assert!(!args.contains(&"gpt-5.3-codex-spark"));
    assert!(!args.contains(&"--dangerously-bypass-approvals-and-sandbox"));
}
