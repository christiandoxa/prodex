use super::*;

#[test]
fn sync_probe_pressure_pause_logs_effective_policy_and_env_pause_ms() {
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

    let shared = RuntimeProxyFixtureBuilder::new().build_shared(&temp_dir);
    shared.local_overload_backoff_until.store(
        Local::now().timestamp().max(0) as u64 + 60,
        Ordering::SeqCst,
    );
    assert!(runtime_proxy_sync_probe_pressure_mode_active_for_route(
        &shared,
        RuntimeRouteKind::Responses
    ));

    let _env_lock = TestEnvVarGuard::lock();
    let _home_guard = TestEnvVarGuard::set(
        "PRODEX_HOME",
        prodex_home.to_str().expect("prodex home path"),
    );
    let _pause_unset_guard =
        TestEnvVarGuard::unset("PRODEX_RUNTIME_PROXY_SYNC_PROBE_PRESSURE_PAUSE_MS");
    clear_runtime_policy_cache();

    runtime_proxy_sync_probe_pressure_pause(&shared, RuntimeRouteKind::Responses);
    runtime_proxy_flush_logs_for_path(&shared.log_path);
    let policy_log = fs::read_to_string(&shared.log_path).expect("runtime log should be readable");
    assert!(
        policy_log.contains("runtime_proxy_sync_probe_pressure_pause route=responses pause_ms=2"),
        "policy pause override should be logged as effective pause: {policy_log}"
    );

    let _ = fs::remove_file(&shared.log_path);
    let _pause_env_guard =
        TestEnvVarGuard::set("PRODEX_RUNTIME_PROXY_SYNC_PROBE_PRESSURE_PAUSE_MS", "1");
    clear_runtime_policy_cache();

    runtime_proxy_sync_probe_pressure_pause(&shared, RuntimeRouteKind::Responses);
    runtime_proxy_flush_logs_for_path(&shared.log_path);
    let env_log = fs::read_to_string(&shared.log_path).expect("runtime log should be readable");
    assert!(
        env_log.contains("runtime_proxy_sync_probe_pressure_pause route=responses pause_ms=1"),
        "env pause override should beat policy and be logged as effective pause: {env_log}"
    );

    clear_runtime_policy_cache();
}
