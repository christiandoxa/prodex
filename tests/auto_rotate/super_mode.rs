use super::*;

#[test]
fn super_dry_run_presidio_flag_reports_redaction_enabled() {
    let fixture = setup_fixture();

    let output = run_prodex(
        &fixture,
        &[
            "super",
            "--dry-run",
            "--skip-quota-check",
            "--presidio",
            "exec",
            "hello",
        ],
    );

    assert!(
        output.status.success(),
        "dry-run failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stdout.contains("Presidio redaction: enabled"),
        "dry-run should report Presidio redaction, stdout: {stdout}"
    );
    assert!(
        !stderr.contains("Use Presidio for data safety?"),
        "explicit --presidio should skip prompt, stderr: {stderr}"
    );
}

#[cfg(unix)]
#[test]
fn super_interactive_pty_prompt_y_enables_presidio_redaction() {
    let fixture = setup_fixture();
    let args_log = fixture.codex_args_log.display().to_string();
    let runtime_log_dir = fixture._temp_dir.path.join("runtime-logs");
    fs::create_dir_all(&runtime_log_dir).expect("failed to create runtime log dir");
    let runtime_log_dir_arg = runtime_log_dir.display().to_string();

    let run = run_prodex_with_pty_prompt_answer(
        &fixture,
        &["super", "--skip-quota-check", "exec", "hello"],
        &[
            ("TEST_CODEX_ARGS_LOG", args_log.as_str()),
            ("PRODEX_RUNTIME_LOG_DIR", runtime_log_dir_arg.as_str()),
        ],
        "Use Presidio for data safety? [y/N] ",
        "y\n",
    );

    assert!(
        run.output.status.success(),
        "pty run failed: tty={} stdout={} stderr={}",
        run.tty_output,
        String::from_utf8_lossy(&run.output.stdout),
        String::from_utf8_lossy(&run.output.stderr)
    );
    assert!(
        run.tty_output
            .contains("Use Presidio for data safety? [y/N] "),
        "PTY prompt output missing prompt: {}",
        run.tty_output
    );

    let codex_args =
        fs::read_to_string(&fixture.codex_args_log).expect("failed to read codex args log");
    let args = codex_args.lines().collect::<Vec<_>>();
    assert!(
        args.ends_with(&[
            "--dangerously-bypass-approvals-and-sandbox",
            "exec",
            "hello"
        ]),
        "Super should still launch Codex with user args, args: {args:?}"
    );
    let latest_log_pointer = runtime_log_dir.join("prodex-runtime-latest.path");
    let latest_log = crate::test_wait::wait_for_poll(
        "latest runtime log pointer",
        std::time::Duration::from_secs(5),
        std::time::Duration::from_millis(10),
        || {
            fs::read_to_string(&latest_log_pointer)
                .ok()
                .map(|path| path.trim().to_string())
                .filter(|path| !path.is_empty())
        },
    );
    let log = fs::read_to_string(latest_log).expect("failed to read runtime log");
    assert!(
        log.contains("presidio_redaction_enabled=true"),
        "answering y should enable runtime Presidio redaction, log: {log}"
    );
}
