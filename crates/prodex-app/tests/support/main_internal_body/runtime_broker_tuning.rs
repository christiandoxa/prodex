use super::*;

#[test]
fn runtime_proxy_worker_count_env_override_beats_policy_file() {
    let temp_dir = TestDir::new();
    let prodex_home = temp_dir.path.join("prodex");
    fs::create_dir_all(&prodex_home).expect("prodex home should exist");
    fs::write(
        prodex_home.join("policy.toml"),
        r#"
version = 1

[runtime_proxy]
worker_count = 11
"#,
    )
    .expect("policy file should write");
    let _prodex_guard = TestEnvVarGuard::set("PRODEX_HOME", &prodex_home.display().to_string());
    let _worker_guard = TestEnvVarGuard::set("PRODEX_RUNTIME_PROXY_WORKER_COUNT", "13");

    clear_runtime_policy_cache();
    assert_eq!(runtime_proxy_worker_count(), 13);
    clear_runtime_policy_cache();
}

#[test]
fn runtime_tuning_snapshot_reports_effective_policy_and_env_values() {
    let temp_dir = TestDir::new();
    let prodex_home = temp_dir.path.join("prodex");
    fs::create_dir_all(&prodex_home).expect("prodex home should exist");
    fs::write(
        prodex_home.join("policy.toml"),
        r#"
version = 1

[runtime_proxy]
worker_count = 8
long_lived_worker_count = 5
probe_refresh_worker_count = 4
async_worker_count = 3
long_lived_queue_capacity = 88
active_request_limit = 40
profile_inflight_soft_limit = 9
profile_inflight_hard_limit = 10
responses_active_limit = 9
compact_active_limit = 5
websocket_active_limit = 7
standard_active_limit = 6
http_connect_timeout_ms = 444
stream_idle_timeout_ms = 555
sse_lookahead_timeout_ms = 66
websocket_connect_timeout_ms = 777
websocket_happy_eyeballs_delay_ms = 88
websocket_precommit_progress_timeout_ms = 999
websocket_previous_response_reuse_stale_ms = 1001
websocket_connect_worker_count = 4
websocket_connect_queue_capacity = 12
websocket_connect_overflow_capacity = 0
websocket_dns_worker_count = 2
websocket_dns_queue_capacity = 8
websocket_dns_overflow_capacity = 0
admission_wait_budget_ms = 111
pressure_admission_wait_budget_ms = 22
long_lived_queue_wait_budget_ms = 333
pressure_long_lived_queue_wait_budget_ms = 44
"#,
    )
    .expect("policy file should write");
    let _env_guards = vec![
        TestEnvVarGuard::set("PRODEX_HOME", &prodex_home.display().to_string()),
        TestEnvVarGuard::set("PRODEX_RUNTIME_PROXY_WORKER_COUNT", "12"),
        TestEnvVarGuard::unset("PRODEX_RUNTIME_PROXY_LONG_LIVED_WORKER_COUNT"),
        TestEnvVarGuard::unset("PRODEX_RUNTIME_PROBE_REFRESH_WORKER_COUNT"),
        TestEnvVarGuard::unset("PRODEX_RUNTIME_PROXY_ASYNC_WORKER_COUNT"),
        TestEnvVarGuard::unset("PRODEX_RUNTIME_PROXY_LONG_LIVED_QUEUE_CAPACITY"),
        TestEnvVarGuard::unset("PRODEX_RUNTIME_PROXY_ACTIVE_REQUEST_LIMIT"),
        TestEnvVarGuard::unset("PRODEX_RUNTIME_PROXY_PROFILE_INFLIGHT_SOFT_LIMIT"),
        TestEnvVarGuard::unset("PRODEX_RUNTIME_PROXY_PROFILE_INFLIGHT_HARD_LIMIT"),
        TestEnvVarGuard::set("PRODEX_RUNTIME_PROXY_RESPONSES_ACTIVE_LIMIT", "31"),
        TestEnvVarGuard::unset("PRODEX_RUNTIME_PROXY_COMPACT_ACTIVE_LIMIT"),
        TestEnvVarGuard::unset("PRODEX_RUNTIME_PROXY_WEBSOCKET_ACTIVE_LIMIT"),
        TestEnvVarGuard::unset("PRODEX_RUNTIME_PROXY_STANDARD_ACTIVE_LIMIT"),
        TestEnvVarGuard::set("PRODEX_RUNTIME_PROXY_HTTP_CONNECT_TIMEOUT_MS", "1234"),
        TestEnvVarGuard::unset("PRODEX_RUNTIME_PROXY_STREAM_IDLE_TIMEOUT_MS"),
        TestEnvVarGuard::unset("PRODEX_RUNTIME_PROXY_SSE_LOOKAHEAD_TIMEOUT_MS"),
        TestEnvVarGuard::unset("PRODEX_RUNTIME_PROXY_WEBSOCKET_CONNECT_TIMEOUT_MS"),
        TestEnvVarGuard::unset("PRODEX_RUNTIME_PROXY_WEBSOCKET_HAPPY_EYEBALLS_DELAY_MS"),
        TestEnvVarGuard::unset("PRODEX_RUNTIME_PROXY_WEBSOCKET_PRECOMMIT_PROGRESS_TIMEOUT_MS"),
        TestEnvVarGuard::unset("PRODEX_RUNTIME_PROXY_WEBSOCKET_PREVIOUS_RESPONSE_REUSE_STALE_MS"),
        TestEnvVarGuard::set("PRODEX_RUNTIME_WEBSOCKET_CONNECT_WORKER_COUNT", "6"),
        TestEnvVarGuard::unset("PRODEX_RUNTIME_WEBSOCKET_CONNECT_QUEUE_CAPACITY"),
        TestEnvVarGuard::unset("PRODEX_RUNTIME_WEBSOCKET_CONNECT_OVERFLOW_CAPACITY"),
        TestEnvVarGuard::unset("PRODEX_RUNTIME_WEBSOCKET_DNS_WORKER_COUNT"),
        TestEnvVarGuard::unset("PRODEX_RUNTIME_WEBSOCKET_DNS_QUEUE_CAPACITY"),
        TestEnvVarGuard::set("PRODEX_RUNTIME_WEBSOCKET_DNS_OVERFLOW_CAPACITY", "9"),
        TestEnvVarGuard::unset("PRODEX_RUNTIME_PROXY_ADMISSION_WAIT_BUDGET_MS"),
        TestEnvVarGuard::unset("PRODEX_RUNTIME_PROXY_PRESSURE_ADMISSION_WAIT_BUDGET_MS"),
        TestEnvVarGuard::unset("PRODEX_RUNTIME_PROXY_LONG_LIVED_QUEUE_WAIT_BUDGET_MS"),
        TestEnvVarGuard::unset("PRODEX_RUNTIME_PROXY_PRESSURE_LONG_LIVED_QUEUE_WAIT_BUDGET_MS"),
    ];

    clear_runtime_policy_cache();
    let snapshot = collect_runtime_tuning_snapshot(&RuntimeConfig::compatibility_current());
    clear_runtime_policy_cache();

    assert_eq!(snapshot.worker_count, 12);
    assert_eq!(snapshot.long_lived_worker_count, 5);
    assert_eq!(snapshot.async_worker_count, 3);
    assert_eq!(snapshot.probe_refresh_worker_count, 4);
    assert_eq!(snapshot.long_lived_queue_capacity, 88);
    assert_eq!(snapshot.active_request_limit, 40);
    assert_eq!(snapshot.lane_limits.responses, 31);
    assert_eq!(snapshot.lane_limits.compact, 5);
    assert_eq!(snapshot.lane_limits.websocket, 7);
    assert_eq!(snapshot.lane_limits.standard, 6);
    assert_eq!(
        snapshot.precommit_attempt_limit,
        RUNTIME_PROXY_PRECOMMIT_ATTEMPT_LIMIT
    );
    assert_eq!(
        snapshot.precommit_budget_ms,
        RUNTIME_PROXY_PRECOMMIT_BUDGET_MS
    );
    assert_eq!(
        snapshot.pressure_precommit_attempt_limit,
        RUNTIME_PROXY_PRESSURE_PRECOMMIT_ATTEMPT_LIMIT
    );
    assert_eq!(
        snapshot.pressure_precommit_budget_ms,
        RUNTIME_PROXY_PRESSURE_PRECOMMIT_BUDGET_MS
    );
    assert_eq!(
        snapshot.continuation_precommit_attempt_limit,
        RUNTIME_PROXY_PRECOMMIT_CONTINUATION_ATTEMPT_LIMIT
    );
    assert_eq!(
        snapshot.continuation_precommit_budget_ms,
        RUNTIME_PROXY_PRECOMMIT_CONTINUATION_BUDGET_MS
    );
    assert_eq!(snapshot.admission_wait_budget_ms, 111);
    assert_eq!(snapshot.pressure_admission_wait_budget_ms, 22);
    assert_eq!(snapshot.long_lived_queue_wait_budget_ms, 333);
    assert_eq!(snapshot.pressure_long_lived_queue_wait_budget_ms, 44);
    assert_eq!(snapshot.http_connect_timeout_ms, 1234);
    assert_eq!(snapshot.stream_idle_timeout_ms, 555);
    assert_eq!(snapshot.sse_lookahead_timeout_ms, 66);
    assert_eq!(snapshot.websocket_connect_timeout_ms, 777);
    assert_eq!(snapshot.websocket_happy_eyeballs_delay_ms, 88);
    assert_eq!(snapshot.websocket_precommit_progress_timeout_ms, 999);
    assert_eq!(snapshot.websocket_previous_response_reuse_stale_ms, 1001);
    assert_eq!(snapshot.websocket_connect_worker_count, 6);
    assert_eq!(snapshot.websocket_connect_queue_capacity, 12);
    assert_eq!(snapshot.websocket_connect_overflow_capacity, 0);
    assert_eq!(snapshot.websocket_dns_worker_count, 2);
    assert_eq!(snapshot.websocket_dns_queue_capacity, 8);
    assert_eq!(snapshot.websocket_dns_overflow_capacity, 9);
    assert_eq!(snapshot.profile_inflight_soft_limit, 9);
    assert_eq!(snapshot.profile_inflight_hard_limit, 10);
    assert_eq!(
        format_runtime_tuning_workers(&snapshot),
        "workers proxy=12, long-lived=5, async=3, probe-refresh=4; active=40, queue=88; lanes responses=31, compact=5, websocket=7, standard=6; ws-connect workers=6, queue=12, overflow=0; ws-dns workers=2, queue=8, overflow=9"
    );
    assert_eq!(
        format_runtime_tuning_budgets(&snapshot),
        format!(
            "precommit={}x/{}ms, pressure-precommit={}x/{}ms, continuation={}x/{}ms; admission=111ms, pressure-admission=22ms, long-lived=333ms, pressure-long-lived=44ms",
            RUNTIME_PROXY_PRECOMMIT_ATTEMPT_LIMIT,
            RUNTIME_PROXY_PRECOMMIT_BUDGET_MS,
            RUNTIME_PROXY_PRESSURE_PRECOMMIT_ATTEMPT_LIMIT,
            RUNTIME_PROXY_PRESSURE_PRECOMMIT_BUDGET_MS,
            RUNTIME_PROXY_PRECOMMIT_CONTINUATION_ATTEMPT_LIMIT,
            RUNTIME_PROXY_PRECOMMIT_CONTINUATION_BUDGET_MS
        )
    );
    assert_eq!(
        format_runtime_tuning_transport(&snapshot),
        "http-connect=1234ms, stream-idle=555ms, sse-lookahead=66ms; ws-connect=777ms, ws-progress=999ms, ws-happy=88ms, ws-stale-reuse=1001ms; inflight soft/hard=9/10"
    );
}

#[test]
fn cleanup_runtime_broker_stale_leases_removes_dead_pid_files() {
    let temp_dir = TestDir::new();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let broker_key = "lease-test";
    let lease_dir = runtime_broker_lease_dir(&paths, broker_key);
    fs::create_dir_all(&lease_dir).expect("lease dir should exist");
    let stale_path = lease_dir.join("999999999-stale.lease");
    let live_path = lease_dir.join(format!("{}-live.lease", std::process::id()));
    fs::write(&stale_path, "stale").expect("stale lease should write");
    fs::write(&live_path, "live").expect("live lease should write");

    let live_count = cleanup_runtime_broker_stale_leases(&paths, broker_key);

    assert_eq!(live_count, 1);
    assert!(!stale_path.exists(), "dead-pid lease should be removed");
    assert!(live_path.exists(), "current-pid lease should remain");
}

#[test]
fn runtime_proxy_endpoint_child_lease_uses_requested_pid_and_cleans_up() {
    let temp_dir = TestDir::new();
    let lease_dir = temp_dir.path.join("leases");
    let endpoint = RuntimeProxyEndpoint {
        listen_addr: "127.0.0.1:33475".parse().expect("listen addr should parse"),
        openai_mount_path: RUNTIME_PROXY_OPENAI_MOUNT_PATH.to_string(),
        local_model_provider_id: None,
        realtime_ws_base_url: None,
        realtime_ws_model: None,
        lease_dir: lease_dir.clone(),
        broker_session_affinity_control: None,
        _lease: None,
        _direct_proxy: None,
    };

    let child_pid = 424242u32;
    let lease = endpoint
        .create_child_lease(child_pid)
        .expect("child lease should be created");
    let mut entries = fs::read_dir(&lease_dir)
        .expect("lease dir should exist")
        .collect::<Result<Vec<_>, _>>()
        .expect("lease dir should be readable");
    assert_eq!(entries.len(), 1, "expected exactly one child lease file");
    let lease_path = entries
        .pop()
        .expect("child lease entry should exist")
        .path();
    let file_name = lease_path
        .file_name()
        .and_then(|value| value.to_str())
        .expect("lease file name should be utf-8")
        .to_string();
    assert!(
        file_name.starts_with("424242-"),
        "child lease should use the child pid in its filename: {file_name}"
    );
    assert_eq!(
        fs::read_to_string(&lease_path).expect("lease file should be readable"),
        "pid=424242\n"
    );

    drop(lease);
    let remaining = fs::read_dir(&lease_dir)
        .expect("lease dir should still exist")
        .count();
    assert_eq!(remaining, 0, "dropping the lease should remove the file");
}

#[test]
fn runtime_broker_version_replacement_defers_while_child_lease_is_live() {
    let temp_dir = TestDir::new();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let broker_key = "version-lease-test";
    let lease_dir = runtime_broker_lease_dir(&paths, broker_key);
    fs::create_dir_all(&lease_dir).expect("lease dir should exist");
    let live_lease = lease_dir.join(format!("{}-child.lease", std::process::id()));
    fs::write(&live_lease, "live").expect("live child lease should write");
    let registry = RuntimeBrokerRegistry {
        pid: std::process::id(),
        listen_addr: "127.0.0.1:33475".to_string(),
        started_at: Local::now().timestamp(),
        upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
        include_code_review: false,
        upstream_no_proxy: false,
        smart_context_enabled: false,
        current_profile: "main".to_string(),
        instance_id: "old-instance".to_string(),
        prodex_version: Some("0.0.0-old".to_string()),
        executable_path: None,
        executable_sha256: None,
        openai_mount_path: Some(RUNTIME_PROXY_OPENAI_MOUNT_PATH.to_string()),
    };

    let outcome =
        replace_runtime_broker_if_version_mismatch_with_health(&paths, broker_key, &registry, None);

    assert_eq!(
        outcome,
        RuntimeBrokerVersionGuardOutcome::DeferredActiveRequests
    );
    assert!(
        live_lease.exists(),
        "live child lease must keep the old broker available for its Codex TUI"
    );
}

#[test]
fn runtime_broker_process_args_expose_only_the_hidden_subcommand() {
    let args: Vec<String> = runtime_broker_process_args()
        .into_iter()
        .map(|value| value.to_string_lossy().into_owned())
        .collect();

    assert_eq!(args, ["__runtime-broker"]);
}

#[test]
fn runtime_broker_key_is_scoped_to_prodex_binary_identity() {
    let first = runtime_broker_key_for_binary_identity(
        "https://chatgpt.com/backend-api",
        false,
        false,
        "version=0.39.0;sha256=alpha",
    );
    let second = runtime_broker_key_for_binary_identity(
        "https://chatgpt.com/backend-api",
        false,
        false,
        "version=0.39.0;sha256=beta",
    );
    let third = runtime_broker_key_for_binary_identity(
        "https://chatgpt.com/backend-api",
        false,
        false,
        "version=0.40.0;sha256=alpha",
    );
    let review = runtime_broker_key_for_binary_identity(
        "https://chatgpt.com/backend-api",
        true,
        false,
        "version=0.39.0;sha256=alpha",
    );
    let no_proxy = runtime_broker_key_for_binary_identity(
        "https://chatgpt.com/backend-api",
        false,
        true,
        "version=0.39.0;sha256=alpha",
    );

    assert_ne!(
        first, second,
        "runtime broker keys must not reuse brokers from a different binary build"
    );
    assert_ne!(
        first, no_proxy,
        "runtime broker keys must not reuse brokers with a different upstream proxy mode"
    );
    assert_ne!(
        first, third,
        "runtime broker keys must not reuse brokers from a different prodex version"
    );
    assert_ne!(
        first, review,
        "review and non-review broker routes should stay isolated"
    );
}

#[test]
fn runtime_broker_key_is_scoped_to_smart_context_mode() {
    let normal = runtime_broker_key_for_binary_identity_with_smart_context(
        "https://chatgpt.com/backend-api",
        false,
        false,
        false,
        None,
        "version=0.71.0;sha256=alpha",
    );
    let smart = runtime_broker_key_for_binary_identity_with_smart_context(
        "https://chatgpt.com/backend-api",
        false,
        false,
        true,
        None,
        "version=0.71.0;sha256=alpha",
    );

    assert_ne!(
        normal, smart,
        "smart-context broker must not reuse a non-smart broker"
    );
}
