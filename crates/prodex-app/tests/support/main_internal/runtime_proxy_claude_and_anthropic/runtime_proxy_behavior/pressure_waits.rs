use super::*;

#[test]
fn runtime_proxy_returns_anthropic_overloaded_error_when_interactive_capacity_is_full() {
    let _limit_guard = TestEnvVarGuard::set("PRODEX_RUNTIME_PROXY_ACTIVE_REQUEST_LIMIT", "4");
    let _lane_guard = TestEnvVarGuard::set("PRODEX_RUNTIME_PROXY_RESPONSES_ACTIVE_LIMIT", "1");
    let _budget_guard = ci_runtime_proxy_admission_wait_budget_guard(1, 1);

    let temp_dir = TestDir::isolated();
    let backend = RuntimeProxyBackend::start_http_slow_stream();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let profile_home = temp_dir.path.join("homes/main");
    write_auth_json(&profile_home.join("auth.json"), "second-account");

    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: profile_home,
                managed: true,
                email: Some("main@example.com".to_string()),
                provider: ProfileProvider::Openai,
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    state.save(&paths).expect("failed to save initial state");

    let proxy = start_runtime_rotation_proxy(&paths, &state, "main", backend.base_url(), false)
        .expect("runtime proxy should start");
    let first_url = format!("http://{}/v1/messages", proxy.listen_addr);
    let second_url = first_url.clone();
    let (first_started_tx, first_started_rx) = std::sync::mpsc::channel();
    let first = thread::spawn(move || {
        let client = Client::builder().build().expect("client");
        let response = client
            .post(first_url)
            .header("Content-Type", "application/json")
            .header("x-api-key", "dummy")
            .header("anthropic-version", "2023-06-01")
            .body(
                serde_json::json!({
                    "model": "claude-sonnet-4-6",
                    "stream": true,
                    "messages": [
                        {
                            "role": "user",
                            "content": "hold the interactive slot"
                        }
                    ]
                })
                .to_string(),
            )
            .send()
            .expect("first anthropic request should start");
        assert_eq!(
            response.status(),
            reqwest::StatusCode::OK,
            "first anthropic request should hold the interactive slot"
        );
        first_started_tx
            .send(())
            .expect("first anthropic request should signal readiness");
        thread::sleep(Duration::from_millis(250));
        let body = response
            .text()
            .expect("first anthropic stream should decode");
        assert!(body.contains("event: message_start"));
    });

    first_started_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("first anthropic request should start before the overload probe");

    let client = Client::builder().build().expect("client");
    let response = client
        .post(second_url)
        .header("Content-Type", "application/json")
        .header("x-api-key", "dummy")
        .header("anthropic-version", "2023-06-01")
        .body(
            serde_json::json!({
                "model": "claude-sonnet-4-6",
                "messages": [
                    {
                        "role": "user",
                        "content": "second request"
                    }
                ]
            })
            .to_string(),
        )
        .send()
        .expect("second anthropic request should receive an overload response");

    assert_eq!(response.status(), reqwest::StatusCode::SERVICE_UNAVAILABLE);
    let body: serde_json::Value = response.json().expect("error body should parse");
    assert_eq!(
        body.get("type").and_then(serde_json::Value::as_str),
        Some("error")
    );
    assert_eq!(
        body.get("error")
            .and_then(|error| error.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("overloaded_error")
    );
    assert!(
        body.get("error")
            .and_then(|error| error.get("message"))
            .and_then(serde_json::Value::as_str)
            .is_some_and(|message| message.contains("temporarily saturated")),
        "unexpected error body: {body}"
    );

    first.join().expect("first request should join");
}

#[test]
fn runtime_proxy_waits_for_anthropic_inflight_relief_then_succeeds() {
    let _budget_guard = ci_runtime_proxy_admission_wait_budget_guard(20, 250);

    let temp_dir = TestDir::isolated();
    let backend = RuntimeProxyBackend::start_http_buffered_json();
    let profile_home = temp_dir.path.join("homes/main");
    write_auth_json(&profile_home.join("auth.json"), "second-account");

    let usage = usage_with_main_windows(90, 3600, 90, 604_800);
    let snapshot = runtime_profile_usage_snapshot_from_usage(&usage);
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
                        codex_home: profile_home,
                        managed: true,
                        email: Some("main@example.com".to_string()),
                        provider: ProfileProvider::Openai,
                    },
                )]),
                last_run_selected_at: BTreeMap::new(),
                response_profile_bindings: BTreeMap::new(),
                session_profile_bindings: BTreeMap::new(),
            },
            upstream_base_url: backend.base_url(),
            include_code_review: false,
            current_profile: "main".to_string(),
            profile_usage_auth: BTreeMap::new(),
            turn_state_bindings: BTreeMap::new(),
            session_id_bindings: BTreeMap::new(),
            continuation_statuses: RuntimeContinuationStatuses::default(),
            profile_probe_cache: BTreeMap::from([(
                "main".to_string(),
                RuntimeProfileProbeCacheEntry {
                    checked_at: Local::now().timestamp(),
                    auth: AuthSummary {
                        label: "chatgpt".to_string(),
                        quota_compatible: true,
                    },
                    result: Ok(usage.clone()),
                },
            )]),
            profile_usage_snapshots: BTreeMap::from([("main".to_string(), snapshot)]),
            profile_retry_backoff_until: BTreeMap::new(),
            profile_transport_backoff_until: BTreeMap::new(),
            profile_route_circuit_open_until: BTreeMap::new(),
            profile_backoff_updated_at: BTreeMap::new(),
            profile_health: BTreeMap::new(),
        },
        usize::MAX,
    );

    let inflight_guard = acquire_runtime_profile_inflight_guard(&shared, "main", "responses_http")
        .expect("inflight guard should be acquired");
    let release = thread::spawn(move || {
        thread::sleep(Duration::from_millis(25));
        drop(inflight_guard);
    });

    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages".to_string(),
        headers: vec![
            ("Content-Type".to_string(), "application/json".to_string()),
            ("x-api-key".to_string(), "dummy".to_string()),
            ("anthropic-version".to_string(), "2023-06-01".to_string()),
        ],
        body: serde_json::json!({
            "model": "claude-sonnet-4-6",
            "messages": [
                {
                    "role": "user",
                    "content": "second request should wait instead of failing"
                }
            ]
        })
        .to_string()
        .into_bytes(),
    };
    let response = proxy_runtime_anthropic_messages_request(42, &request, &shared)
        .expect("anthropic request should complete after inflight relief");

    let RuntimeResponsesReply::Buffered(parts) = response else {
        panic!("expected buffered anthropic response");
    };
    assert_eq!(parts.status, 200, "unexpected status after inflight wait");
    let body: serde_json::Value =
        serde_json::from_slice(&parts.body).expect("response body should parse");
    assert_eq!(
        body.get("type").and_then(serde_json::Value::as_str),
        Some("message")
    );

    release.join().expect("release thread should join");

    let log = fs::read_to_string(&shared.log_path).expect("runtime log should be readable");
    assert!(
        log.contains("inflight_wait_started route=responses"),
        "interactive inflight wait should be logged"
    );
    assert!(
        log.contains("inflight_wait_finished route=responses"),
        "interactive inflight wait completion should be logged"
    );
    assert!(
        log.contains("useful=true"),
        "successful Anthropics inflight wait should log useful relief"
    );
    assert!(
        log.contains("wake_source=inflight_release"),
        "successful Anthropics inflight wait should log inflight_release wake source"
    );
}

#[test]
fn runtime_proxy_waits_for_responses_inflight_relief_then_succeeds() {
    let _budget_guard = ci_runtime_proxy_admission_wait_budget_guard(40, 250);

    let temp_dir = TestDir::isolated();
    let backend = RuntimeProxyBackend::start_http_buffered_json();
    let profile_home = temp_dir.path.join("homes/main");
    write_auth_json(&profile_home.join("auth.json"), "second-account");

    let usage = usage_with_main_windows(90, 3600, 90, 604_800);
    let snapshot = runtime_profile_usage_snapshot_from_usage(&usage);
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
                        codex_home: profile_home,
                        managed: true,
                        email: Some("main@example.com".to_string()),
                        provider: ProfileProvider::Openai,
                    },
                )]),
                last_run_selected_at: BTreeMap::new(),
                response_profile_bindings: BTreeMap::new(),
                session_profile_bindings: BTreeMap::new(),
            },
            upstream_base_url: backend.base_url(),
            include_code_review: false,
            current_profile: "main".to_string(),
            profile_usage_auth: BTreeMap::new(),
            turn_state_bindings: BTreeMap::new(),
            session_id_bindings: BTreeMap::new(),
            continuation_statuses: RuntimeContinuationStatuses::default(),
            profile_probe_cache: BTreeMap::from([(
                "main".to_string(),
                RuntimeProfileProbeCacheEntry {
                    checked_at: Local::now().timestamp(),
                    auth: AuthSummary {
                        label: "chatgpt".to_string(),
                        quota_compatible: true,
                    },
                    result: Ok(usage.clone()),
                },
            )]),
            profile_usage_snapshots: BTreeMap::from([("main".to_string(), snapshot)]),
            profile_retry_backoff_until: BTreeMap::new(),
            profile_transport_backoff_until: BTreeMap::new(),
            profile_route_circuit_open_until: BTreeMap::new(),
            profile_backoff_updated_at: BTreeMap::new(),
            profile_health: BTreeMap::new(),
        },
        usize::MAX,
    );

    let inflight_guard = acquire_runtime_profile_inflight_guard(&shared, "main", "responses_http")
        .expect("inflight guard should be acquired");
    let release = thread::spawn(move || {
        thread::sleep(Duration::from_millis(25));
        drop(inflight_guard);
    });

    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/backend-api/codex/responses".to_string(),
        headers: vec![("Content-Type".to_string(), "application/json".to_string())],
        body: serde_json::json!({
            "input": "second request should wait instead of failing"
        })
        .to_string()
        .into_bytes(),
    };
    let response = proxy_runtime_responses_request(43, &request, &shared)
        .expect("responses request should complete after inflight relief");

    let RuntimeResponsesReply::Buffered(parts) = response else {
        panic!("expected buffered responses reply");
    };
    assert_eq!(parts.status, 200, "unexpected status after inflight wait");
    let body: serde_json::Value =
        serde_json::from_slice(&parts.body).expect("response body should parse");
    assert_eq!(
        body.get("id").and_then(serde_json::Value::as_str),
        Some("resp-second")
    );

    release.join().expect("release thread should join");

    let log = fs::read_to_string(&shared.log_path).expect("runtime log should be readable");
    assert!(
        log.contains("inflight_wait_started route=responses"),
        "responses inflight wait should be logged"
    );
    assert!(
        log.contains("inflight_wait_finished route=responses"),
        "responses inflight wait completion should be logged"
    );
    assert!(
        log.contains("useful=true"),
        "successful responses inflight wait should log useful relief"
    );
    assert!(
        log.contains("wake_source=inflight_release"),
        "successful responses inflight wait should log inflight_release wake source"
    );
}

#[test]
fn runtime_profile_inflight_relief_wait_returns_immediately_after_prior_release() {
    let temp_dir = TestDir::isolated();
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
            state: AppState::default(),
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
            profile_backoff_updated_at: BTreeMap::new(),
            profile_health: BTreeMap::new(),
        },
        usize::MAX,
    );

    let observed_revision = runtime_profile_inflight_release_revision(&shared);
    shared.lane_admission.record_inflight_release();

    let started_at = Instant::now();
    assert!(wait_for_runtime_profile_inflight_relief_since(
        &shared,
        Duration::from_millis(100),
        observed_revision,
    ));
    assert!(
        started_at.elapsed() < ci_timing_upper_bound_ms(20, 100),
        "release-aware inflight wait should not sleep after the release was already observed"
    );
}

#[test]
fn runtime_profile_inflight_relief_wait_ignores_active_request_release_notify() {
    let temp_dir = TestDir::isolated();
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
            state: AppState::default(),
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
            profile_backoff_updated_at: BTreeMap::new(),
            profile_health: BTreeMap::new(),
        },
        usize::MAX,
    );

    let observed_revision = runtime_profile_inflight_release_revision(&shared);
    let active_guard = try_acquire_runtime_proxy_active_request_slot(
        &shared,
        "http",
        "/backend-api/codex/responses",
    )
    .expect("active request slot should be acquired");
    let release = thread::spawn(move || {
        thread::sleep(Duration::from_millis(20));
        drop(active_guard);
    });

    assert!(
        !wait_for_runtime_profile_inflight_relief_since(
            &shared,
            Duration::from_millis(100),
            observed_revision,
        ),
        "active-request release notify should not count as inflight relief"
    );

    release
        .join()
        .expect("active-request release thread should join");
    assert_eq!(
        runtime_profile_inflight_release_revision(&shared),
        observed_revision,
        "active-request release should not change inflight release revision"
    );
}

#[test]
fn runtime_probe_refresh_wait_returns_immediately_after_progress_is_observed() {
    let probe_refresh = RuntimeProbeRefreshTestGuard::new();
    let temp_dir = TestDir::isolated();
    let _shared = runtime_rotation_proxy_shared(
        &temp_dir,
        RuntimeRotationState {
            paths: AppPaths {
                root: temp_dir.path.join("prodex"),
                state_file: temp_dir.path.join("prodex/state.json"),
                managed_profiles_root: temp_dir.path.join("prodex/profiles"),
                shared_codex_root: temp_dir.path.join("shared"),
                legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
            },
            state: AppState::default(),
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
            profile_backoff_updated_at: BTreeMap::new(),
            profile_health: BTreeMap::new(),
        },
        usize::MAX,
    );

    note_runtime_probe_refresh_progress();

    let started_at = Instant::now();
    assert!(wait_for_runtime_probe_refresh_since(
        Duration::from_millis(100),
        probe_refresh.observed_revision(),
    ));
    assert!(
        started_at.elapsed() < ci_timing_upper_bound_ms(20, 100),
        "probe-refresh wait should not sleep after progress was already observed"
    );
}

#[test]
fn runtime_probe_refresh_wait_ignores_lane_release_notify() {
    let probe_refresh = RuntimeProbeRefreshTestGuard::new();
    let temp_dir = TestDir::isolated();
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
            state: AppState::default(),
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
            profile_backoff_updated_at: BTreeMap::new(),
            profile_health: BTreeMap::new(),
        },
        usize::MAX,
    );

    let active_guard = try_acquire_runtime_proxy_active_request_slot(
        &shared,
        "http",
        "/backend-api/codex/responses",
    )
    .expect("active request slot should be acquired");
    let release = thread::spawn(move || {
        thread::sleep(Duration::from_millis(20));
        drop(active_guard);
    });

    assert!(
        !wait_for_runtime_probe_refresh_since(
            Duration::from_millis(100),
            probe_refresh.observed_revision(),
        ),
        "lane-release notify should not count as probe-refresh progress"
    );

    release
        .join()
        .expect("active-request release thread should join");
    assert_eq!(
        runtime_probe_refresh_revision(),
        probe_refresh.observed_revision(),
        "lane-release notify should not change probe refresh revision"
    );
}
