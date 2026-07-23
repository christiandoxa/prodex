use super::*;

#[test]
fn run_auto_rotates_active_profile_when_current_is_blocked() {
    let fixture = setup_fixture();

    let output = run_prodex(&fixture, &["run"]);

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Quota preflight blocked profile 'main'"));
    assert!(stderr.contains("profile 'second'"));
    assert_eq!(active_profile(&fixture.prodex_home), "second");
    assert_eq!(
        fs::read_to_string(&fixture.codex_log)
            .expect("failed to read codex log")
            .trim(),
        fixture.second_home.display().to_string()
    );
}

#[test]
fn run_auto_rotate_flag_rotates_active_profile_when_current_is_blocked() {
    let fixture = setup_fixture();

    let output = run_prodex(&fixture, &["run", "--auto-rotate"]);

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Quota preflight blocked profile 'main'"));
    assert!(stderr.contains("profile 'second'"));
    assert_eq!(active_profile(&fixture.prodex_home), "second");
    assert_eq!(
        fs::read_to_string(&fixture.codex_log)
            .expect("failed to read codex log")
            .trim(),
        fixture.second_home.display().to_string()
    );
}

#[test]
fn explicit_profile_auto_rotates_by_default() {
    let fixture = setup_fixture();

    let output = run_prodex(&fixture, &["run", "--profile", "main"]);

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stderr).contains("Auto-rotating to profile 'second'."));
    assert_eq!(active_profile(&fixture.prodex_home), "second");
    assert_eq!(
        fs::read_to_string(&fixture.codex_log)
            .expect("failed to read codex log")
            .trim(),
        fixture.second_home.display().to_string()
    );
}

#[test]
fn run_exec_preserves_prompt_and_piped_stdin() {
    let fixture = setup_fixture();
    let args_log = fixture.codex_args_log.display().to_string();
    let stdin_log = fixture.codex_stdin_log.display().to_string();

    let output = run_prodex_with_env_and_stdin(
        &fixture,
        &[
            "run",
            "--profile",
            "main",
            "--no-auto-rotate",
            "--skip-quota-check",
            "exec",
            "summarize concisely",
        ],
        &[
            ("TEST_CODEX_ARGS_LOG", args_log.as_str()),
            ("TEST_CODEX_STDIN_LOG", stdin_log.as_str()),
        ],
        "piped input\n",
    );

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(active_profile(&fixture.prodex_home), "main");
    assert_eq!(
        fs::read_to_string(&fixture.codex_log)
            .expect("failed to read codex log")
            .trim(),
        fixture.main_home.display().to_string()
    );
    assert_eq!(
        fs::read_to_string(&fixture.codex_args_log)
            .expect("failed to read codex args log")
            .lines()
            .collect::<Vec<_>>(),
        vec!["exec", "summarize concisely"]
    );
    assert_eq!(
        fs::read_to_string(&fixture.codex_stdin_log)
            .expect("failed to read codex stdin log")
            .trim_end(),
        "piped input"
    );
}

#[test]
fn explicit_profile_can_disable_auto_rotate() {
    let fixture = setup_fixture();

    let output = run_prodex(&fixture, &["run", "--profile", "main", "--no-auto-rotate"]);

    assert_eq!(output.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("Other profiles that look ready: second")
    );
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("Rerun without `--no-auto-rotate` to allow fallback.")
    );
    assert_eq!(active_profile(&fixture.prodex_home), "main");
    assert!(!fixture.codex_log.exists());
    assert_eq!(
        fixture.main_home.file_name().and_then(|name| name.to_str()),
        Some("main")
    );
}

#[test]
fn run_preflight_checks_fallback_profiles_in_parallel() {
    let fixture = setup_fixture();
    add_managed_profile(&fixture, "third", "third-account");
    fixture.usage_server.set_delay_ms(250);

    let output = run_prodex(&fixture, &["run", "--profile", "main", "--no-auto-rotate"]);

    assert_eq!(output.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("Other profiles that look ready: third, second")
    );
    assert!(
        fixture.usage_server.max_concurrent_requests() >= 2,
        "fallback profile checks never overlapped"
    );
}

#[test]
fn run_without_profile_keeps_the_active_ready_account() {
    let fixture = setup_fixture();
    add_managed_profile(&fixture, "elite", "elite-account");
    let mut state = read_state(&fixture.prodex_home);
    state["active_profile"] = Value::String("second".to_string());
    write_json(&fixture.prodex_home.join("state.json"), &state);

    let output = run_prodex(&fixture, &["run"]);

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(active_profile(&fixture.prodex_home), "second");
    assert_eq!(
        fs::read_to_string(&fixture.codex_log)
            .expect("failed to read codex log")
            .trim(),
        fixture.second_home.display().to_string()
    );
    assert!(!String::from_utf8_lossy(&output.stderr).contains("Auto-selecting profile"));
}

#[cfg(any(unix, windows))]
#[test]
fn run_recovers_when_runtime_broker_registry_points_to_a_dead_pid() {
    let fixture = setup_fixture();
    let mut child = spawn_prodex_with_env(
        &fixture,
        &["run", "--profile", "main", "--skip-quota-check"],
        &[("TEST_LONG_RUNNING_RUN", "5")],
    );

    let registry_path = wait_for_runtime_broker_registry_path(&fixture.prodex_home);
    let initial_registry: Value =
        serde_json::from_slice(&fs::read(&registry_path).expect("failed to read registry"))
            .expect("failed to parse registry");
    let initial_pid = initial_registry["pid"]
        .as_u64()
        .expect("registry pid should be numeric") as u32;
    assert!(
        initial_pid > 0,
        "registry should contain a live broker pid before failure"
    );

    #[cfg(unix)]
    let kill_status = Command::new("kill")
        .arg("-9")
        .arg(initial_pid.to_string())
        .status()
        .expect("failed to kill broker pid");
    #[cfg(windows)]
    let kill_status = {
        let initial_pid = initial_pid.to_string();
        Command::new("taskkill")
            .args(["/PID", initial_pid.as_str(), "/F"])
            .status()
            .expect("failed to kill broker pid")
    };
    assert!(kill_status.success(), "failed to kill broker pid");

    let _ = child.kill();
    let _ = child.wait();
    let _ = fs::remove_file(&fixture.codex_log);

    let mut recovered_child = spawn_prodex_with_env(
        &fixture,
        &["run", "--profile", "main", "--skip-quota-check"],
        &[("TEST_LONG_RUNNING_RUN", "5")],
    );
    let recovered_registry: Value = crate::test_wait::wait_for_poll(
        "fresh runtime broker registry",
        std::time::Duration::from_secs(30),
        std::time::Duration::from_millis(10),
        || {
            let registry: Value = serde_json::from_slice(&fs::read(&registry_path).ok()?).ok()?;
            (registry["pid"].as_u64()? as u32 != initial_pid).then_some(registry)
        },
    );
    let recovered_pid = recovered_registry["pid"]
        .as_u64()
        .expect("recovered registry pid should be numeric") as u32;
    assert_ne!(
        recovered_pid, initial_pid,
        "a stale broker registry should be replaced by a fresh broker process"
    );
    let recovered_codex_home = crate::test_wait::wait_for_poll(
        "Codex launch through recovered broker",
        std::time::Duration::from_secs(30),
        std::time::Duration::from_millis(10),
        || {
            fs::read_to_string(&fixture.codex_log)
                .ok()
                .filter(|content| !content.trim().is_empty())
        },
    );
    let _ = recovered_child.kill();
    let _ = recovered_child.wait();
    assert_eq!(
        recovered_codex_home.trim(),
        fixture.main_home.display().to_string()
    );
}
