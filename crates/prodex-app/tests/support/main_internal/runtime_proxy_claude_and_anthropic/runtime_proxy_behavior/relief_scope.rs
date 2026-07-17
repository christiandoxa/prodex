use super::*;

#[test]
fn runtime_proxy_responses_inflight_relief_times_out_without_relief() {
    let _budget_guard = ci_runtime_proxy_admission_wait_budget_guard(20, 20);

    let temp_dir = TestDir::isolated();
    let profile_home = temp_dir.path.join("homes/main");
    write_auth_json(&profile_home.join("auth.json"), "main-account");

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
            upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
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

    let _inflight_guard = acquire_runtime_profile_inflight_guard(&shared, "main", "responses_http")
        .expect("inflight guard should be acquired");
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/backend-api/codex/responses".to_string(),
        headers: vec![("Content-Type".to_string(), "application/json".to_string())],
        body: serde_json::json!({
            "input": "request should time out without inflight relief"
        })
        .to_string()
        .into_bytes(),
    };
    let response = proxy_runtime_responses_request(44, &request, &shared)
        .expect("responses request should return a local timeout result");

    let RuntimeResponsesReply::Buffered(parts) = response else {
        panic!("expected buffered responses reply");
    };
    assert_eq!(
        parts.status, 503,
        "unexpected status without inflight relief"
    );
    let body: serde_json::Value =
        serde_json::from_slice(&parts.body).expect("response body should parse");
    assert_eq!(
        body.get("error")
            .and_then(|error| error.get("code"))
            .and_then(serde_json::Value::as_str),
        Some("service_unavailable")
    );

    let log = fs::read_to_string(&shared.log_path).expect("runtime log should be readable");
    assert!(
        log.contains("inflight_wait_started route=responses"),
        "responses inflight timeout should log wait start"
    );
    assert!(
        log.contains("inflight_wait_finished route=responses"),
        "responses inflight timeout should log wait finish"
    );
    assert!(
        log.contains("useful=false"),
        "timeout without relief should log useful=false"
    );
    assert!(
        log.contains("wake_source=timeout"),
        "timeout without relief should log wake_source=timeout"
    );
}

#[test]
fn runtime_proxy_wait_scopes_to_session_owner_relief() {
    let _budget_guard = ci_runtime_proxy_admission_wait_budget_guard(40, 250);

    let temp_dir = TestDir::isolated();
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");

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
                active_profile: Some("second".to_string()),
                profiles: BTreeMap::from([
                    (
                        "main".to_string(),
                        ProfileEntry {
                            codex_home: main_home,
                            managed: true,
                            email: Some("main@example.com".to_string()),
                            provider: ProfileProvider::Openai,
                        },
                    ),
                    (
                        "second".to_string(),
                        ProfileEntry {
                            codex_home: second_home,
                            managed: true,
                            email: Some("second@example.com".to_string()),
                            provider: ProfileProvider::Openai,
                        },
                    ),
                ]),
                last_run_selected_at: BTreeMap::new(),
                response_profile_bindings: BTreeMap::new(),
                session_profile_bindings: BTreeMap::from([(
                    "sess-main".to_string(),
                    ResponseProfileBinding {
                        profile_name: "main".to_string(),
                        bound_at: Local::now().timestamp(),
                    },
                )]),
            },
            upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
            include_code_review: false,
            current_profile: "second".to_string(),
            profile_usage_auth: BTreeMap::new(),
            turn_state_bindings: BTreeMap::new(),
            session_id_bindings: BTreeMap::new(),
            continuation_statuses: RuntimeContinuationStatuses::default(),
            profile_probe_cache: BTreeMap::from([
                (
                    "main".to_string(),
                    RuntimeProfileProbeCacheEntry {
                        checked_at: Local::now().timestamp(),
                        auth: AuthSummary {
                            label: "chatgpt".to_string(),
                            quota_compatible: true,
                        },
                        result: Ok(usage.clone()),
                    },
                ),
                (
                    "second".to_string(),
                    RuntimeProfileProbeCacheEntry {
                        checked_at: Local::now().timestamp(),
                        auth: AuthSummary {
                            label: "chatgpt".to_string(),
                            quota_compatible: true,
                        },
                        result: Ok(usage.clone()),
                    },
                ),
            ]),
            profile_usage_snapshots: BTreeMap::from([
                ("main".to_string(), snapshot),
                (
                    "second".to_string(),
                    runtime_profile_usage_snapshot_from_usage(&usage),
                ),
            ]),
            profile_retry_backoff_until: BTreeMap::new(),
            profile_transport_backoff_until: BTreeMap::new(),
            profile_route_circuit_open_until: BTreeMap::new(),
            profile_backoff_updated_at: BTreeMap::new(),
            profile_health: BTreeMap::new(),
        },
        usize::MAX,
    );

    let main_inflight = acquire_runtime_profile_inflight_guard(&shared, "main", "responses_http")
        .expect("main inflight guard should be acquired");
    let second_inflight =
        acquire_runtime_profile_inflight_guard(&shared, "second", "responses_http")
            .expect("second inflight guard should be acquired");
    let release = thread::spawn(move || {
        thread::sleep(Duration::from_millis(20));
        drop(second_inflight);
    });

    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/backend-api/codex/responses".to_string(),
        headers: vec![
            ("Content-Type".to_string(), "application/json".to_string()),
            ("session_id".to_string(), "sess-main".to_string()),
        ],
        body: serde_json::json!({
            "input": "request should only count owner relief as useful"
        })
        .to_string()
        .into_bytes(),
    };
    let excluded_profiles = BTreeSet::new();
    assert!(
        !runtime_proxy_maybe_wait_for_interactive_inflight_relief(RuntimeInflightReliefWait {
            request_id: 45,
            request: &request,
            shared: &shared,
            excluded_profiles: &excluded_profiles,
            route_kind: RuntimeRouteKind::Responses,
            selection_started_at: Instant::now(),
            continuation: true,
            wait_affinity_owner: Some("main"),
        },)
        .expect("owner-scoped wait should complete"),
        "non-owner release should not count as useful relief"
    );

    release
        .join()
        .expect("non-owner release thread should join");
    drop(main_inflight);

    let log = fs::read_to_string(&shared.log_path).expect("runtime log should be readable");
    assert!(
        log.contains("inflight_wait_finished route=responses"),
        "owner-scoped wait should log completion"
    );
    assert!(
        log.contains("useful=false"),
        "non-owner release should not be logged as useful relief"
    );
    assert!(
        log.contains("wake_source=inflight_release"),
        "non-owner release should still be logged as an inflight release wake"
    );
}
