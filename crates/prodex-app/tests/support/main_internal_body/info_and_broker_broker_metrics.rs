use super::*;

#[test]
fn runtime_proxy_broker_metrics_endpoint_reports_live_runtime_snapshot() {
    let backend = RuntimeProxyBackend::start();
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    write_auth_json(&main_home.join("auth.json"), "main-account");

    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: main_home,
                managed: true,
                email: Some("main@example.com".to_string()),
                provider: ProfileProvider::Openai,
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let proxy = start_runtime_rotation_proxy(&paths, &state, "main", backend.base_url(), false)
        .expect("runtime proxy should start");
    register_runtime_broker_metadata(
        &proxy.log_path,
        RuntimeBrokerMetadata {
            broker_key: runtime_broker_key(&backend.base_url(), false, false),
            listen_addr: proxy.listen_addr.to_string(),
            started_at: Local::now().timestamp(),
            current_profile: "main".to_string(),
            include_code_review: false,
            upstream_no_proxy: false,
            instance_id: "instance".to_string(),
            admin_token: runtime_broker_test_secret("secret"),
            prodex_version: Some(runtime_current_prodex_version().to_string()),
            executable_path: None,
            executable_sha256: None,
        },
    );
    let mut log = std::fs::OpenOptions::new()
        .append(true)
        .open(&proxy.log_path)
        .expect("runtime log should open for append");
    writeln!(
        log,
        "[2026-04-22 10:00:00.000 +00:00] request=7 transport=http route=responses stale_continuation reason=previous_response_not_found profile=main"
    )
    .expect("runtime log should append stale continuation");
    writeln!(
        log,
        "[2026-04-22 10:00:00.001 +00:00] request=7 transport=websocket route=websocket websocket_session=17 chain_dead_upstream_confirmed profile=main previous_response_id=resp-7 reason=previous_response_not_found_locked_affinity via=- event=-"
    )
    .expect("runtime log should append chain dead marker");
    drop(log);

    let response = Client::builder()
        .build()
        .expect("client")
        .get(format!(
            "http://{}/__prodex/runtime/metrics",
            proxy.listen_addr
        ))
        .header("X-Prodex-Admin-Token", "secret")
        .send()
        .expect("runtime broker metrics request should succeed");

    assert_eq!(response.status().as_u16(), 200);
    let metrics = response
        .json::<RuntimeBrokerMetrics>()
        .expect("runtime broker metrics should decode");
    assert_eq!(metrics.health.current_profile, "main");
    assert_eq!(metrics.health.instance_id, "instance");
    assert_eq!(metrics.health.persistence_role, "owner");
    assert_eq!(
        metrics.health.prodex_version.as_deref(),
        Some(runtime_current_prodex_version())
    );
    assert!(metrics.active_request_limit > 0);
    assert!(metrics.traffic.responses.limit > 0);
    assert_eq!(metrics.traffic.responses.admissions_total, 0);
    assert_eq!(metrics.traffic.responses.global_limit_rejections_total, 0);
    assert_eq!(metrics.traffic.responses.lane_limit_rejections_total, 0);
    assert_eq!(metrics.local_overload_backoff_remaining_seconds, 0);
    assert_eq!(metrics.continuations.response_bindings, 0);
    assert_eq!(
        metrics.continuity_failure_reasons.stale_continuation,
        BTreeMap::from([("previous_response_not_found".to_string(), 1)])
    );
    assert_eq!(
        metrics
            .continuity_failure_reasons
            .chain_dead_upstream_confirmed,
        BTreeMap::from([("previous_response_not_found_locked_affinity".to_string(), 1,)])
    );
}

#[test]
fn runtime_proxy_broker_prometheus_metrics_endpoint_reports_text_snapshot() {
    let backend = RuntimeProxyBackend::start();
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    write_auth_json(&main_home.join("auth.json"), "main-account");

    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: main_home,
                managed: true,
                email: Some("main@example.com".to_string()),
                provider: ProfileProvider::Openai,
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let proxy = start_runtime_rotation_proxy(&paths, &state, "main", backend.base_url(), false)
        .expect("runtime proxy should start");
    register_runtime_broker_metadata(
        &proxy.log_path,
        RuntimeBrokerMetadata {
            broker_key: runtime_broker_key(&backend.base_url(), false, false),
            listen_addr: proxy.listen_addr.to_string(),
            started_at: Local::now().timestamp(),
            current_profile: "main".to_string(),
            include_code_review: false,
            upstream_no_proxy: false,
            instance_id: "instance".to_string(),
            admin_token: runtime_broker_test_secret("secret"),
            prodex_version: Some(runtime_current_prodex_version().to_string()),
            executable_path: None,
            executable_sha256: None,
        },
    );
    let mut log = std::fs::OpenOptions::new()
        .append(true)
        .open(&proxy.log_path)
        .expect("runtime log should open for append");
    writeln!(
        log,
        "[2026-04-22 10:00:00.010 +00:00] request=9 transport=websocket route=websocket websocket_session=21 chain_retried_owner profile=main previous_response_id=resp-9 delay_ms=20 reason=previous_response_not_found_locked_affinity via=-"
    )
    .expect("runtime log should append chain retry marker");
    writeln!(
        log,
        "[2026-04-22 10:00:00.011 +00:00] request=9 websocket_session=21 stale_continuation reason=websocket_reuse_watchdog_locked_affinity profile=main event=timeout"
    )
    .expect("runtime log should append stale continuation marker");
    drop(log);

    let response = Client::builder()
        .build()
        .expect("client")
        .get(format!(
            "http://{}/__prodex/runtime/metrics/prometheus",
            proxy.listen_addr
        ))
        .header("X-Prodex-Admin-Token", "secret")
        .send()
        .expect("runtime broker prometheus request should succeed");

    assert_eq!(response.status().as_u16(), 200);
    assert_eq!(
        response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok()),
        Some("text/plain; version=0.0.4; charset=utf-8")
    );
    let body = response.text().expect("prometheus body should decode");
    assert!(body.contains("prodex_runtime_broker_info"));
    assert!(body.contains("broker_key=\""));
    assert!(!body.contains("current_profile="));
    assert!(body.contains("prodex_version=\""));
    assert!(body.contains("executable_sha256=\""));
    assert!(body.contains("prodex_runtime_broker_lane_admissions_total"));
    assert!(body.contains("prodex_runtime_broker_lane_releases_total"));
    assert!(body.contains("prodex_runtime_broker_lane_global_limit_rejections_total"));
    assert!(body.contains("prodex_runtime_broker_lane_lane_limit_rejections_total"));
    assert!(body.contains("prodex_runtime_broker_lane_release_underflows_total"));
    assert!(body.contains("prodex_runtime_broker_active_request_release_underflows_total"));
    assert!(body.contains("prodex_runtime_broker_profile_inflight_release_underflows_total"));
    assert!(body.contains("prodex_runtime_broker_continuation_binding_counts"));
    assert!(body.contains("prodex_runtime_broker_continuity_failures_total"));
    assert!(body.contains("binding_kind=\"response\""));
    assert!(body.contains("lifecycle=\"warm\""));
    assert!(body.contains("event=\"chain_retried_owner\",listen_addr=\""));
    assert!(body.contains("reason=\"websocket_reuse_watchdog_locked_affinity\""));
}

#[test]
fn runtime_broker_metrics_snapshot_tracks_lane_admissions_and_rejections() {
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    write_auth_json(&main_home.join("auth.json"), "main-account");

    let shared = runtime_rotation_proxy_shared(
        &temp_dir,
        RuntimeRotationState {
            paths: AppPaths {
                root: temp_dir.path.join("prodex"),
                state_file: temp_dir.path.join("prodex/state.json"),
                managed_profiles_root: temp_dir.path.join("prodex/profiles"),
                shared_codex_root: temp_dir.path.join("shared"),
                legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
            },
            state: AppState {
                active_profile: Some("main".to_string()),
                profiles: BTreeMap::from([(
                    "main".to_string(),
                    ProfileEntry {
                        codex_home: main_home,
                        managed: true,
                        email: Some("main@example.com".to_string()),
                        provider: ProfileProvider::Openai,
                    },
                )]),
                last_run_selected_at: BTreeMap::new(),
                response_profile_bindings: BTreeMap::new(),
                session_profile_bindings: BTreeMap::new(),
            },
            upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
            include_code_review: false,
            current_profile: "main".to_string(),
            profile_usage_auth: BTreeMap::new(),
            turn_state_bindings: BTreeMap::new(),
            session_id_bindings: BTreeMap::new(),
            continuation_statuses: RuntimeContinuationStatuses::default(),
            profile_probe_cache: BTreeMap::new(),
            profile_usage_snapshots: BTreeMap::new(),
            profile_retry_backoff_until: BTreeMap::new(),
            profile_transport_backoff_until: BTreeMap::new(),
            profile_route_circuit_open_until: BTreeMap::new(),
            profile_inflight: BTreeMap::new(),
            profile_health: BTreeMap::new(),
        },
        2,
    );

    let guard = try_acquire_runtime_proxy_active_request_slot(
        &shared,
        "http",
        "/backend-api/codex/responses",
    )
    .expect("responses admission should succeed");
    drop(guard);

    shared
        .active_request_count
        .store(shared.active_request_limit, Ordering::SeqCst);
    assert!(matches!(
        try_acquire_runtime_proxy_active_request_slot(
            &shared,
            "http",
            "/backend-api/codex/responses/compact",
        ),
        Err(RuntimeProxyAdmissionRejection::GlobalLimit)
    ));
    shared.active_request_count.store(0, Ordering::SeqCst);

    shared
        .lane_admission
        .responses_active
        .store(shared.lane_admission.limits.responses, Ordering::SeqCst);
    assert!(matches!(
        try_acquire_runtime_proxy_active_request_slot(
            &shared,
            "http",
            "/backend-api/codex/responses",
        ),
        Err(RuntimeProxyAdmissionRejection::LaneLimit(
            RuntimeRouteKind::Responses
        ))
    ));
    shared
        .lane_admission
        .responses_active
        .store(0, Ordering::SeqCst);

    let metrics = runtime_broker_metrics_snapshot(
        &shared,
        &RuntimeBrokerMetadata {
            broker_key: "broker".to_string(),
            listen_addr: "127.0.0.1:12345".to_string(),
            started_at: Local::now().timestamp(),
            current_profile: "main".to_string(),
            include_code_review: false,
            upstream_no_proxy: false,
            instance_id: "instance".to_string(),
            admin_token: runtime_broker_test_secret("secret"),
            prodex_version: None,
            executable_path: None,
            executable_sha256: None,
        },
    )
    .expect("broker metrics snapshot should succeed");

    assert_eq!(metrics.traffic.responses.admissions_total, 1);
    assert_eq!(metrics.traffic.responses.releases_total, 1);
    assert_eq!(metrics.traffic.responses.release_underflows_total, 0);
    assert_eq!(metrics.traffic.responses.global_limit_rejections_total, 0);
    assert_eq!(metrics.traffic.responses.lane_limit_rejections_total, 1);
    assert_eq!(metrics.traffic.compact.admissions_total, 0);
    assert_eq!(metrics.traffic.compact.releases_total, 0);
    assert_eq!(metrics.traffic.compact.global_limit_rejections_total, 1);
    assert_eq!(metrics.traffic.compact.lane_limit_rejections_total, 0);
    assert_eq!(metrics.active_request_release_underflows_total, 0);
    assert_eq!(metrics.profile_inflight_admissions_total, 0);
    assert_eq!(metrics.profile_inflight_releases_total, 0);
    assert_eq!(metrics.profile_inflight_release_underflows_total, 0);
}

#[test]
fn runtime_broker_metrics_snapshot_records_runtime_state_lock_wait() {
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    write_auth_json(&main_home.join("auth.json"), "main-account");

    let runtime = RuntimeRotationState {
        paths: AppPaths {
            root: temp_dir.path.join("prodex"),
            state_file: temp_dir.path.join("prodex/state.json"),
            managed_profiles_root: temp_dir.path.join("prodex/profiles"),
            shared_codex_root: temp_dir.path.join("shared"),
            legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
        },
        state: AppState {
            active_profile: Some("main".to_string()),
            profiles: BTreeMap::from([(
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home,
                    managed: true,
                    email: Some("main@example.com".to_string()),
                    provider: ProfileProvider::Openai,
                },
            )]),
            last_run_selected_at: BTreeMap::new(),
            response_profile_bindings: BTreeMap::new(),
            session_profile_bindings: BTreeMap::new(),
        },
        upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
        include_code_review: false,
        current_profile: "main".to_string(),
        profile_usage_auth: BTreeMap::new(),
        turn_state_bindings: BTreeMap::new(),
        session_id_bindings: BTreeMap::new(),
        continuation_statuses: RuntimeContinuationStatuses::default(),
        profile_probe_cache: BTreeMap::new(),
        profile_usage_snapshots: BTreeMap::new(),
        profile_retry_backoff_until: BTreeMap::new(),
        profile_transport_backoff_until: BTreeMap::new(),
        profile_route_circuit_open_until: BTreeMap::new(),
        profile_inflight: BTreeMap::new(),
        profile_health: BTreeMap::new(),
    };
    let shared = runtime_rotation_proxy_shared(&temp_dir, runtime.clone(), 2);
    let isolated_shared = runtime_rotation_proxy_shared(&temp_dir, runtime, 2);
    shared.reset_runtime_state_lock_wait_metrics_for_test();
    isolated_shared.reset_runtime_state_lock_wait_metrics_for_test();

    let runtime_guard = shared.runtime.lock().expect("runtime lock should succeed");
    let worker_shared = shared.clone();
    let (started_sender, started_receiver) = mpsc::channel();
    let worker = thread::spawn(move || {
        started_sender.send(()).expect("snapshot start should send");
        runtime_broker_metrics_snapshot(
            &worker_shared,
            &RuntimeBrokerMetadata {
                broker_key: "broker".to_string(),
                listen_addr: "127.0.0.1:12345".to_string(),
                started_at: Local::now().timestamp(),
                current_profile: "main".to_string(),
                include_code_review: false,
                upstream_no_proxy: false,
                instance_id: "instance".to_string(),
                admin_token: runtime_broker_test_secret("secret"),
                prodex_version: None,
                executable_path: None,
                executable_sha256: None,
            },
        )
        .expect("broker metrics snapshot should succeed");
    });

    started_receiver
        .recv_timeout(Duration::from_secs(1))
        .expect("snapshot thread should start");
    thread::sleep(Duration::from_millis(25));
    drop(runtime_guard);
    worker.join().expect("snapshot thread should join");

    let metrics = shared.runtime_state_lock_wait_metrics();
    assert_eq!(metrics.wait_count, 1);
    assert!(
        metrics.wait_total_ns >= 1_000_000,
        "lock wait total should include contention, got {metrics:?}"
    );
    assert!(
        metrics.wait_max_ns >= 1_000_000,
        "lock wait max should include contention, got {metrics:?}"
    );
    assert_eq!(
        isolated_shared.runtime_state_lock_wait_metrics(),
        RuntimeStateLockWaitMetrics::default()
    );
}
