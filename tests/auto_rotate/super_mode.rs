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
    for expected in [
        "--dangerously-bypass-approvals-and-sandbox",
        "--dangerously-bypass-hook-trust",
        "trust_level",
    ] {
        assert!(
            stdout.contains(expected),
            "Super dry-run should expose {expected}, stdout: {stdout}"
        );
    }
    assert!(
        !stderr.contains("Use Presidio for data safety?"),
        "explicit --presidio should skip prompt, stderr: {stderr}"
    );
}

#[cfg(unix)]
#[test]
fn s_pty_renders_presidio_tui() {
    let fixture = setup_fixture();
    fs::write(
        fixture.prodex_home.join("presidio.toml"),
        "enabled = true\n",
    )
    .expect("failed to seed enabled Presidio config");

    let run = run_prodex_with_pty_until_prompt(
        &fixture,
        &["s", "--skip-quota-check", "exec", "hello"],
        &[],
        "Use Presidio for data safety?",
    );

    assert!(
        !run.output.status.success(),
        "the prompt-only test should stop the launch: tty={} stdout={} stderr={}",
        run.tty_output,
        String::from_utf8_lossy(&run.output.stdout),
        String::from_utf8_lossy(&run.output.stderr)
    );
    assert!(
        run.tty_output.contains("Use Presidio for data safety?"),
        "Super should ask for Presidio permission: {}",
        run.tty_output
    );
    assert!(
        run.tty_output.contains("Presidio opt-in"),
        "Super should render the Presidio prompt as a TUI: {}",
        run.tty_output
    );
}
