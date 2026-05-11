use super::*;

#[test]
fn runtime_proxy_http_fresh_request_reaches_later_profile_after_usage_limit_chain() {
    let fixture = start_runtime_continuation_fixture(
        RuntimeProxyBackend::start_http_usage_limit_until_third(),
        "fifth",
        &["fifth", "fourth", "main", "second", "third"],
        &[],
        Vec::new(),
    );

    let response = fixture.post_json(
        "backend-api/codex/responses",
        serde_json::json!({
            "model": "gpt-5.4",
            "input": [{
                "type": "message",
                "role": "user",
                "content": [{
                    "type": "input_text",
                    "text": "continue on the next healthy account",
                }],
            }],
        }),
    );

    assert_eq!(
        response.status().as_u16(),
        200,
        "fresh requests should rotate past usage-limit accounts"
    );
    let body = response.text().expect("responses body should decode");
    assert!(
        body.contains("\"id\":\"resp-third\""),
        "healthy later profile should complete the request: {body}"
    );
    assert!(
        !body.contains("usage limit") && !body.contains("service_unavailable"),
        "retryable usage-limit failures must not leak once a later profile succeeds: {body}"
    );
    assert_eq!(
        fixture.backend.responses_accounts(),
        vec![
            "fifth-account".to_string(),
            "fourth-account".to_string(),
            "main-account".to_string(),
            "second-account".to_string(),
            "third-account".to_string(),
        ],
        "runtime proxy should keep rotating until the later healthy profile is tried"
    );
}

#[test]
fn runtime_proxy_http_resume_continuation_preserves_metadata_headers_and_affinity() {
    let fixture = start_runtime_continuation_fixture(
        RuntimeProxyBackend::start_http_previous_response_needs_turn_state(),
        "main",
        &["main", "second"],
        &[("resp-second", "second")],
        Vec::new(),
    );
    let turn_metadata = serde_json::json!({
        "source": "resume",
        "session_id": "sess-goal-resume",
    })
    .to_string();
    let response = fixture.post_json_with_headers(
        "backend-api/codex/responses",
        &[
            runtime_continuation_header("session_id", "sess-goal-resume"),
            runtime_continuation_header("x-codex-turn-state", "turn-second"),
            runtime_continuation_header("x-codex-turn-metadata", turn_metadata.clone()),
            runtime_continuation_header("x-codex-beta-features", "goals"),
            runtime_continuation_header("User-Agent", "codex-cli/0.128.0"),
        ],
        serde_json::json!({
            "previous_response_id": "resp-second",
            "session_id": "sess-goal-resume",
            "input": [{
                "type": "message",
                "role": "user",
                "content": [{
                    "type": "input_text",
                    "text": "continue goal workflow",
                }],
            }],
        }),
    );

    assert_eq!(response.status().as_u16(), 200);
    let body = response.text().expect("responses body should decode");
    assert!(
        body.contains("\"id\":\"resp-second-next\""),
        "continuation should succeed on the bound upstream profile: {body}"
    );

    let responses_accounts = fixture.backend.responses_accounts();
    assert_eq!(
        responses_accounts,
        vec!["second-account".to_string()],
        "resume continuation should stay on previous_response owner without probing current profile"
    );

    let responses_bodies = fixture.backend.responses_bodies();
    assert_eq!(
        responses_bodies.len(),
        1,
        "backend should observe exactly one continuation attempt: {responses_bodies:?}"
    );
    assert!(
        responses_bodies[0].contains("\"previous_response_id\":\"resp-second\""),
        "upstream body should preserve previous_response_id: {}",
        responses_bodies[0]
    );
    assert!(
        responses_bodies[0].contains("\"session_id\":\"sess-goal-resume\""),
        "upstream body should preserve resume session_id: {}",
        responses_bodies[0]
    );

    let responses_headers = fixture.backend.responses_headers();
    assert_eq!(
        responses_headers.len(),
        1,
        "backend should record the single upstream attempt: {responses_headers:?}"
    );
    let headers = &responses_headers[0];
    assert_eq!(
        headers.get("session_id").map(String::as_str),
        Some("sess-goal-resume")
    );
    assert_eq!(
        headers.get("x-codex-turn-state").map(String::as_str),
        Some("turn-second")
    );
    assert_eq!(
        headers.get("x-codex-turn-metadata").map(String::as_str),
        Some(turn_metadata.as_str())
    );
    assert_eq!(
        headers.get("x-codex-beta-features").map(String::as_str),
        Some("goals")
    );
    assert_eq!(
        headers.get("user-agent").map(String::as_str),
        Some("codex-cli/0.128.0")
    );
    assert_eq!(
        headers.get("chatgpt-account-id").map(String::as_str),
        Some("second-account")
    );

    let log = fixture.wait_for_log(|log| {
        log.contains("binding session_id profile=second value=sess-goal-resume")
    });
    assert!(
        log.contains("binding session_id profile=second value=sess-goal-resume"),
        "successful resume continuation should preserve session binding: {log}"
    );
}

#[test]
fn runtime_proxy_http_empty_session_previous_response_does_not_fresh_fallback() {
    let temp_dir = TestDir::isolated();
    let backend = RuntimeProxyBackend::start_http_buffered_json();
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
            "resp-missing".to_string(),
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
        .timeout(ci_timing_upper_bound_ms(1_000, 5_000))
        .build()
        .expect("client")
        .post(format!(
            "http://{}/backend-api/codex/responses",
            proxy.listen_addr
        ))
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body(
            serde_json::json!({
                "previous_response_id": "resp-missing",
                "session_id": "sess-replayable",
                "input": [],
            })
            .to_string(),
        )
        .send()
        .expect("responses request should succeed");

    assert_eq!(response.status().as_u16(), 409);
    let body = response.text().expect("responses body should decode");
    assert!(
        body.contains("\"code\":\"stale_continuation\""),
        "client should see stale_continuation instead of raw previous_response loss: {body}"
    );

    let responses_bodies = backend.responses_bodies();
    assert_eq!(
        responses_bodies.len(),
        1,
        "backend should observe only the original continuation: {responses_bodies:?}"
    );
    assert!(
        responses_bodies[0].contains("\"previous_response_id\":\"resp-missing\""),
        "request should preserve previous_response_id: {}",
        responses_bodies[0]
    );
    assert!(
        responses_bodies[0].contains("\"session_id\":\"sess-replayable\""),
        "request should preserve session_id: {}",
        responses_bodies[0]
    );

    let responses_headers = backend.responses_headers();
    assert_eq!(
        responses_headers.len(),
        1,
        "backend should record only the original attempt: {responses_headers:?}"
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
        !log.contains("previous_response_fresh_fallback reason="),
        "empty session-scoped continuations must not drop previous_response_id: {log}"
    );
}

#[test]
fn runtime_proxy_http_message_followup_previous_response_does_not_fresh_fallback() {
    let temp_dir = TestDir::isolated();
    let backend = RuntimeProxyBackend::start_http_buffered_json();
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
            "resp-missing".to_string(),
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
                "previous_response_id": "resp-missing",
                "input": [{
                    "type": "message",
                    "role": "user",
                    "content": [{
                        "type": "input_text",
                        "text": "continue the same conversation",
                    }],
                }],
            })
            .to_string(),
        )
        .send()
        .expect("responses request should succeed");

    assert_eq!(
        response.status().as_u16(),
        409,
        "message follow-up should fail stale instead of becoming a fresh success"
    );
    let body = response.text().expect("responses body should decode");
    assert!(
        body.contains("\"code\":\"stale_continuation\""),
        "client should see stale_continuation instead of raw continuity loss: {body}"
    );
    assert!(
        !body.contains("\"id\":\"resp-second\""),
        "proxy must not hide continuity failure behind fresh response success: {body}"
    );

    let responses_accounts = backend.responses_accounts();
    assert_eq!(
        responses_accounts,
        vec!["second-account".to_string()],
        "follow-up should stay on owning profile without cross-profile fallback"
    );

    let responses_bodies = backend.responses_bodies();
    assert_eq!(
        responses_bodies.len(),
        1,
        "proxy should not send a second fresh retry for plain message follow-ups: {responses_bodies:?}"
    );
    assert!(
        responses_bodies[0].contains("\"previous_response_id\":\"resp-missing\""),
        "upstream request must preserve previous_response chain: {}",
        responses_bodies[0]
    );
}

#[test]
fn runtime_proxy_http_message_followup_with_session_header_does_not_fresh_fallback() {
    let temp_dir = TestDir::isolated();
    let backend = RuntimeProxyBackend::start_http_buffered_json();
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
            "resp-missing".to_string(),
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
        .header("session_id", "sess-replayable")
        .body(
            serde_json::json!({
                "previous_response_id": "resp-missing",
                "input": [{
                    "type": "message",
                    "role": "user",
                    "content": [{
                        "type": "input_text",
                        "text": "continue the same conversation",
                    }],
                }],
            })
            .to_string(),
        )
        .send()
        .expect("responses request should succeed");

    assert_eq!(
        response.status().as_u16(),
        409,
        "session header must not make a message follow-up replayable"
    );
    let body = response.text().expect("responses body should decode");
    assert!(
        body.contains("\"code\":\"stale_continuation\""),
        "client should see stale_continuation instead of raw continuity loss: {body}"
    );
    assert!(
        !body.contains("\"id\":\"resp-second\""),
        "proxy must not replace lost context with a fresh response: {body}"
    );

    let responses_bodies = backend.responses_bodies();
    assert_eq!(
        responses_bodies.len(),
        1,
        "proxy should not send a fresh retry for session-header message follow-ups: {responses_bodies:?}"
    );
    assert!(
        responses_bodies[0].contains("\"previous_response_id\":\"resp-missing\""),
        "upstream request must preserve previous_response chain: {}",
        responses_bodies[0]
    );
}

#[test]
fn runtime_proxy_http_message_followup_with_turn_metadata_session_does_not_fresh_fallback() {
    let temp_dir = TestDir::isolated();
    let backend = RuntimeProxyBackend::start_http_buffered_json();
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
            "resp-missing".to_string(),
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

    let turn_metadata = serde_json::json!({
        "session_id": "sess-replayable"
    })
    .to_string();
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
        .header("x-codex-turn-metadata", turn_metadata.clone())
        .body(
            serde_json::json!({
                "previous_response_id": "resp-missing",
                "input": [{
                    "type": "message",
                    "role": "user",
                    "content": [{
                        "type": "input_text",
                        "text": "continue the same conversation",
                    }],
                }],
            })
            .to_string(),
        )
        .send()
        .expect("responses request should succeed");

    assert_eq!(
        response.status().as_u16(),
        409,
        "turn metadata session_id must not make a message follow-up replayable"
    );
    let body = response.text().expect("responses body should decode");
    assert!(
        body.contains("\"code\":\"stale_continuation\""),
        "client should see stale_continuation instead of raw continuity loss: {body}"
    );
    assert!(
        !body.contains("\"id\":\"resp-second\""),
        "proxy must not replace lost context with a fresh response: {body}"
    );

    let responses_accounts = backend.responses_accounts();
    assert_eq!(
        responses_accounts,
        vec!["second-account".to_string()],
        "metadata session follow-up should stay on owning profile without fallback"
    );

    let responses_bodies = backend.responses_bodies();
    assert_eq!(
        responses_bodies.len(),
        1,
        "proxy should not send a fresh retry for metadata session message follow-ups: {responses_bodies:?}"
    );
    assert!(
        responses_bodies[0].contains("\"previous_response_id\":\"resp-missing\""),
        "upstream request must preserve previous_response chain: {}",
        responses_bodies[0]
    );

    let responses_headers = backend.responses_headers();
    assert_eq!(
        responses_headers.len(),
        1,
        "backend should record the single upstream attempt: {responses_headers:?}"
    );
    assert_eq!(
        responses_headers[0]
            .get("x-codex-turn-metadata")
            .map(String::as_str),
        Some(turn_metadata.as_str()),
        "turn metadata should be preserved on the upstream continuation"
    );
}

#[test]
fn runtime_proxy_http_message_followup_with_session_quota_does_not_rotate_or_fresh_fallback() {
    let temp_dir = TestDir::isolated();
    let backend = RuntimeProxyBackend::start();
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
                    "type": "message",
                    "role": "user",
                    "content": [{
                        "type": "input_text",
                        "text": "continue after quota pressure",
                    }],
                }],
            })
            .to_string(),
        )
        .send()
        .expect("responses request should succeed");

    let status = response.status().as_u16();
    let body = response.text().expect("responses body should decode");
    assert_eq!(
        status, 200,
        "quota SSE response should pass through from the owning profile: {body}"
    );
    assert!(
        body.contains("insufficient_quota"),
        "quota failure should pass through instead of becoming a fresh response: {body}"
    );
    assert!(
        !body.contains("\"id\":\"resp-second\""),
        "proxy must not replace quota-blocked message context with a fresh response: {body}"
    );

    let responses_accounts = backend.responses_accounts();
    assert_eq!(
        responses_accounts,
        vec!["main-account".to_string()],
        "quota-blocked message follow-up should not rotate off previous_response owner"
    );

    let responses_bodies = backend.responses_bodies();
    assert_eq!(
        responses_bodies.len(),
        1,
        "proxy should not send a fresh retry for quota-blocked message follow-ups: {responses_bodies:?}"
    );
    assert!(
        responses_bodies[0].contains("\"previous_response_id\":\"resp-main\""),
        "upstream request must preserve previous_response_id under quota pressure: {}",
        responses_bodies[0]
    );
    assert!(
        responses_bodies[0].contains("\"session_id\":\"sess-replayable\""),
        "upstream request must preserve session_id under quota pressure: {}",
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
        "quota-blocked message follow-up must not drop previous_response_id: {log}"
    );
    assert!(
        !log.contains("quota_blocked_affinity_released"),
        "quota-blocked message follow-up must keep previous_response affinity: {log}"
    );
}
