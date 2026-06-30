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
        vec!["exec", "ping", "exec", "ping"]
    );
}
