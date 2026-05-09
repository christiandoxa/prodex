use super::*;

#[test]
fn runtime_proxy_http_tool_output_with_session_does_not_fresh_fallback() {
    let temp_dir = TestDir::isolated();
    let backend = RuntimeProxyBackend::start_http_quota_then_tool_output_fresh_fallback_error();
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let now = Local::now().timestamp();
    let state = AppState {
        active_profile: Some("main".to_string()),
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
        response_profile_bindings: BTreeMap::from([(
            "resp-main".to_string(),
            ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: now,
            },
        )]),
        session_profile_bindings: BTreeMap::new(),
    };
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    state.save(&paths).expect("failed to save initial state");

    let proxy = start_runtime_rotation_proxy(&paths, &state, "main", backend.base_url(), false)
        .expect("runtime proxy should start");
    let response = Client::builder()
        .build()
        .expect("client")
        .post(format!(
            "http://{}/backend-api/codex/responses",
            proxy.listen_addr
        ))
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body(
            serde_json::json!({
                "previous_response_id": "resp-main",
                "session_id": "sess-replayable",
                "input": [{
                    "type": "function_call_output",
                    "call_id": "call_tk0AjVbCh1EZCS0XTVva002N",
                    "output": "ok",
                }],
            })
            .to_string(),
        )
        .send()
        .expect("responses request should succeed");

    assert_eq!(response.status().as_u16(), 200);
    let body = response.text().expect("responses body should decode");
    assert!(
        body.contains("insufficient_quota"),
        "quota failure should pass through instead of degrading into fresh tool-output retry: {body}"
    );
    assert!(
        !body.contains("No tool call found"),
        "proxy should not create a fresh tool-output request that loses call context: {body}"
    );

    let responses_accounts = backend.responses_accounts();
    assert_eq!(
        responses_accounts,
        vec!["main-account".to_string()],
        "tool-output continuations must not rotate away from the owning profile: {responses_accounts:?}"
    );

    let responses_bodies = backend.responses_bodies();
    assert_eq!(
        responses_bodies.len(),
        1,
        "backend should observe only the original continuation: {responses_bodies:?}"
    );
    assert!(
        responses_bodies[0].contains("\"previous_response_id\":\"resp-main\""),
        "first request should preserve previous_response_id: {}",
        responses_bodies[0]
    );
    assert!(
        responses_bodies[0].contains("\"session_id\":\"sess-replayable\""),
        "original request should preserve session_id: {}",
        responses_bodies[0]
    );

    let log_tail = wait_for_runtime_log_tail_until(
        || fs::read(&proxy.log_path).ok(),
        |log| log.contains("upstream_usage_limit_passthrough") || log.contains("quota_blocked"),
        2_000,
        5_000,
        20,
    );
    let log = String::from_utf8_lossy(&log_tail);
    assert!(
        !log.contains("previous_response_fresh_fallback reason=quota_blocked"),
        "quota-blocked tool-output path must not drop previous_response_id: {log}"
    );
}

#[test]
fn runtime_proxy_http_tool_output_with_session_surfaces_stale_continuation_without_fresh_retry() {
    let temp_dir = TestDir::isolated();
    let backend = RuntimeProxyBackend::start_http_previous_response_tool_context_missing();
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let now = Local::now().timestamp();
    let state = AppState {
        active_profile: Some("second".to_string()),
        profiles: BTreeMap::from([(
            "second".to_string(),
            ProfileEntry {
                codex_home: second_home,
                managed: true,
                email: Some("second@example.com".to_string()),
                provider: ProfileProvider::Openai,
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::from([(
            "resp-second".to_string(),
            ResponseProfileBinding {
                profile_name: "second".to_string(),
                bound_at: now,
            },
        )]),
        session_profile_bindings: BTreeMap::new(),
    };
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    state.save(&paths).expect("failed to save initial state");

    let proxy = start_runtime_rotation_proxy(&paths, &state, "second", backend.base_url(), false)
        .expect("runtime proxy should start");
    let response = Client::builder()
        .build()
        .expect("client")
        .post(format!(
            "http://{}/backend-api/codex/responses",
            proxy.listen_addr
        ))
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body(
            serde_json::json!({
                "previous_response_id": "resp-second",
                "session_id": "sess-replayable",
                "input": [{
                    "type": "function_call_output",
                    "call_id": "call_J7U3Kdc539EyfWU4nZj9LCWQZ",
                    "output": "ok",
                }],
            })
            .to_string(),
        )
        .send()
        .expect("responses request should succeed");

    assert_eq!(response.status().as_u16(), 409);
    let body = response.text().expect("responses body should decode");
    assert!(
        body.contains("\"code\":\"stale_continuation\""),
        "HTTP should translate tool-context loss into stale_continuation: {body}"
    );
    assert!(
        !body.contains("No tool call found"),
        "HTTP should not surface the upstream tool-context string after classification: {body}"
    );

    let responses_bodies = backend.responses_bodies();
    assert_eq!(
        responses_bodies.len(),
        1,
        "backend should not observe a second fresh tool-output replay"
    );
    assert!(
        responses_bodies[0].contains("\"previous_response_id\":\"resp-second\""),
        "first request should preserve previous_response_id: {}",
        responses_bodies[0]
    );
    assert!(
        responses_bodies[0].contains("\"session_id\":\"sess-replayable\""),
        "original request should preserve session_id: {}",
        responses_bodies[0]
    );

    let log_tail = wait_for_runtime_log_tail_until(
        || fs::read(&proxy.log_path).ok(),
        |log| log.contains("previous_response_not_found"),
        2_000,
        5_000,
        20,
    );
    let log = String::from_utf8_lossy(&log_tail);
    assert!(
        log.contains("previous_response_not_found"),
        "runtime log should classify upstream tool-context loss as a continuation miss: {log}"
    );
    assert!(
        !log.contains("previous_response_fresh_fallback reason=previous_response_not_found"),
        "tool-output-only context misses must not trigger fresh replay: {log}"
    );
}

#[test]
fn runtime_proxy_http_compact_previous_response_not_found_surfaces_stale_continuation() {
    let temp_dir = TestDir::isolated();
    let backend = RuntimeProxyBackend::start_http_compact_previous_response_not_found();
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let now = Local::now().timestamp();
    let state = AppState {
        active_profile: Some("second".to_string()),
        profiles: BTreeMap::from([(
            "second".to_string(),
            ProfileEntry {
                codex_home: second_home,
                managed: true,
                email: Some("second@example.com".to_string()),
                provider: ProfileProvider::Openai,
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::from([(
            runtime_compact_session_lineage_key("sess-compact"),
            ResponseProfileBinding {
                profile_name: "second".to_string(),
                bound_at: now,
            },
        )]),
    };
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    state.save(&paths).expect("failed to save initial state");
    save_runtime_continuations(
        &paths,
        &RuntimeContinuationStore {
            turn_state_bindings: BTreeMap::from([(
                "turn-second".to_string(),
                ResponseProfileBinding {
                    profile_name: "second".to_string(),
                    bound_at: now,
                },
            )]),
            statuses: RuntimeContinuationStatuses {
                turn_state: BTreeMap::from([(
                    "turn-second".to_string(),
                    RuntimeContinuationBindingStatus {
                        state: RuntimeContinuationBindingLifecycle::Verified,
                        confidence: 1,
                        last_touched_at: Some(now),
                        last_verified_at: Some(now),
                        last_verified_route: Some("responses".to_string()),
                        last_not_found_at: None,
                        not_found_streak: 0,
                        success_count: 1,
                        failure_count: 0,
                    },
                )]),
                ..RuntimeContinuationStatuses::default()
            },
            ..RuntimeContinuationStore::default()
        },
    )
    .expect("failed to save initial continuations");

    let proxy = start_runtime_rotation_proxy(&paths, &state, "second", backend.base_url(), false)
        .expect("runtime proxy should start");
    let response = Client::builder()
        .build()
        .expect("client")
        .post(format!(
            "http://{}/backend-api/codex/responses/compact",
            proxy.listen_addr
        ))
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .header("x-openai-subagent", "compact")
        .body(
            serde_json::json!({
                "previous_response_id": "resp-compact-missing",
                "session_id": "sess-compact",
                "input": [],
                "instructions": "compact",
            })
            .to_string(),
        )
        .send()
        .expect("compact request should succeed");

    assert_eq!(response.status().as_u16(), 409);
    let body = response.text().expect("compact body should decode");
    assert!(
        body.contains("\"code\":\"stale_continuation\""),
        "compact previous_response miss should surface stale_continuation: {body}"
    );
    assert!(
        !body.contains("previous_response_not_found"),
        "compact path should not leak raw previous_response_not_found: {body}"
    );

    let responses_bodies = backend.responses_bodies();
    assert_eq!(
        responses_bodies.len(),
        1,
        "compact path should not retry after a stale continuation: {responses_bodies:?}"
    );
    assert!(
        responses_bodies[0].contains("\"previous_response_id\":\"resp-compact-missing\""),
        "compact request should preserve the original previous_response_id: {}",
        responses_bodies[0]
    );
}
