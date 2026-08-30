use super::*;

#[test]
fn probe_refresh_pause_logs_effective_policy_and_env_pause_ms() {
    let temp_dir = TestDir::isolated();
    let prodex_home = temp_dir.path.join("prodex-policy");
    fs::create_dir_all(&prodex_home).expect("prodex home should exist");
    fs::write(
        prodex_home.join("policy.toml"),
        r#"
version = 1

[runtime_proxy]
sync_probe_pressure_pause_ms = 2
"#,
    )
    .expect("policy file should write");

    let _env_lock = TestEnvVarGuard::lock();
    let _home_guard = TestEnvVarGuard::set(
        "PRODEX_HOME",
        prodex_home.to_str().expect("prodex home path"),
    );
    let _pause_unset_guard =
        TestEnvVarGuard::unset("PRODEX_RUNTIME_PROXY_SYNC_PROBE_PRESSURE_PAUSE_MS");
    clear_runtime_policy_cache();

    let shared = RuntimeProxyFixtureBuilder::new().build_shared(&temp_dir);
    runtime_proxy_probe_refresh_pause(&shared, RuntimeRouteKind::Responses);
    runtime_proxy_flush_logs_for_path(&shared.log_path).expect("runtime log should flush");
    let policy_log = read_runtime_proxy_test_log(&shared.log_path);
    assert!(
        policy_log.contains("runtime_proxy_probe_refresh_pause route=responses pause_ms=2"),
        "policy pause override should be logged as effective pause: {policy_log}"
    );

    let env_temp_dir = TestDir::isolated();
    let _pause_env_guard =
        TestEnvVarGuard::set("PRODEX_RUNTIME_PROXY_SYNC_PROBE_PRESSURE_PAUSE_MS", "1");
    clear_runtime_policy_cache();

    let env_shared = RuntimeProxyFixtureBuilder::new().build_shared(&env_temp_dir);
    runtime_proxy_probe_refresh_pause(&env_shared, RuntimeRouteKind::Responses);
    runtime_proxy_flush_logs_for_path(&env_shared.log_path).expect("runtime log should flush");
    let env_log = read_runtime_proxy_test_log(&env_shared.log_path);
    assert!(
        env_log.contains("runtime_proxy_probe_refresh_pause route=responses pause_ms=1"),
        "env pause override should beat policy and be logged as effective pause: {env_log}"
    );

    clear_runtime_policy_cache();
}

#[test]
fn fresh_selection_retries_after_scheduled_cold_start_probe() {
    let _env_lock = TestEnvVarGuard::lock();
    let _pause_guard = ci_runtime_proxy_timeout_guard(
        "PRODEX_RUNTIME_PROXY_SYNC_PROBE_PRESSURE_PAUSE_MS",
        2_000,
        4_000,
    );
    clear_runtime_policy_cache();

    let _probe_refresh = RuntimeProbeRefreshTestGuard::new();
    let temp_dir = TestDir::isolated();
    let backend = RuntimeProxyBackend::start();
    let shared = runtime_shared_for_cold_start_probe_selection(&temp_dir, backend.base_url());
    shared
        .runtime
        .lock()
        .expect("runtime lock should succeed")
        .profile_usage_snapshots
        .insert(
            "second".to_string(),
            ready_runtime_usage_snapshot(Local::now().timestamp(), 0),
        );

    let candidate = next_runtime_response_candidate(&shared, &BTreeSet::new())
        .expect("candidate lookup should succeed");
    assert_eq!(candidate, Some("second".to_string()));
    assert_eq!(backend.usage_accounts(), vec!["second-account".to_string()]);

    drop(shared);
    drop(backend);
    wait_for_runtime_background_queues_idle();
    clear_runtime_policy_cache();
}
