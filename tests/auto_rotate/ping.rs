use super::*;

#[test]
fn ping_openai_sends_ping_to_each_ready_openai_profile() {
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
    let homes = fs::read_to_string(home_log).expect("failed to read ping homes log");
    assert_eq!(
        homes.lines().collect::<Vec<_>>(),
        vec![
            fixture.second_home.to_string_lossy().to_string(),
            third_home.to_string_lossy().to_string(),
        ]
    );
    let args = fs::read_to_string(&fixture.codex_args_log).expect("failed to read args log");
    assert_eq!(
        args.lines().collect::<Vec<_>>(),
        vec![
            "--dangerously-bypass-approvals-and-sandbox",
            "exec",
            "ping",
            "--dangerously-bypass-approvals-and-sandbox",
            "exec",
            "ping",
        ]
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
    assert_eq!(
        fs::read_to_string(home_log)
            .expect("ready snapshot profiles should be pinged")
            .lines()
            .collect::<Vec<_>>(),
        vec![
            fixture.second_home.to_string_lossy().to_string(),
            third_home.to_string_lossy().to_string(),
        ]
    );
}

#[test]
fn ping_openai_continues_after_one_ready_profile_child_error() {
    let fixture = setup_fixture();
    let broken_home = add_managed_profile(&fixture, "broken", "third-account");
    fs::write(broken_home.join("sessions"), b"not a directory")
        .expect("failed to create broken sessions path");
    let home_log = fixture._temp_dir.path.join("ping-error-homes.log");
    let home_log_string = home_log.display().to_string();

    let output = run_prodex_with_env(
        &fixture,
        &["ping", "openai"],
        &[("TEST_CODEX_LOG_APPEND", home_log_string.as_str())],
    );

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("ping failed for broken"));
    assert_eq!(
        fs::read_to_string(home_log)
            .expect("later ready profile should still be pinged")
            .lines()
            .collect::<Vec<_>>(),
        vec![fixture.second_home.to_string_lossy().to_string()]
    );
}

#[test]
fn ping_openai_sends_extra_spark_ping_when_profile_has_spark_limit() {
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
    let homes = fs::read_to_string(home_log).expect("failed to read ping homes log");
    assert_eq!(
        homes.lines().collect::<Vec<_>>(),
        vec![
            fixture.second_home.to_string_lossy().to_string(),
            spark_home.to_string_lossy().to_string(),
            spark_home.to_string_lossy().to_string(),
        ]
    );
    let args = fs::read_to_string(&fixture.codex_args_log).expect("failed to read args log");
    assert_eq!(
        args.lines().collect::<Vec<_>>(),
        vec![
            "--dangerously-bypass-approvals-and-sandbox",
            "exec",
            "ping",
            "--dangerously-bypass-approvals-and-sandbox",
            "exec",
            "ping",
            "--dangerously-bypass-approvals-and-sandbox",
            "--model",
            "gpt-5.3-codex-spark",
            "exec",
            "-c",
            "model_context_window=128000",
            "-c",
            "model_auto_compact_token_limit=115200",
            "ping",
        ]
    );
}
