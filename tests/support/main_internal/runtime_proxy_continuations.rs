use super::*;

#[test]
fn runtime_proxy_websocket_empty_session_previous_response_does_not_fresh_fallback() {
    let _test_guard = crate::acquire_test_runtime_lock();
    let (_connect_timeout_guard, _progress_timeout_guard) =
        ci_runtime_proxy_websocket_timeout_guards();

    let fixture = start_runtime_continuation_fixture(
        RuntimeProxyBackend::start_websocket_previous_response_missing_without_turn_state(),
        "second",
        &["second"],
        &[("resp-second", "second")],
        Vec::new(),
    );
    let mut socket = fixture.connect_websocket("backend-api/codex/realtime?call_id=call-123");
    send_runtime_websocket_json(
        &mut socket,
        serde_json::json!({
            "previous_response_id": "resp-second",
            "session_id": "sess-replayable",
            "input": [],
        }),
    );

    let response_message = read_runtime_websocket_text(&mut socket);
    let _ = socket.close(None);

    assert!(
        response_message.contains("\"code\":\"stale_continuation\""),
        "empty session-scoped previous_response continuation should fail stale instead of replaying fresh: {response_message}"
    );

    let websocket_requests = fixture.backend.websocket_requests();
    assert_eq!(
        websocket_requests.len(),
        1,
        "backend should observe only the original continuation"
    );

    let first_request = &websocket_requests[0];
    for (field, value) in [
        ("previous_response_id", "resp-second"),
        ("session_id", "sess-replayable"),
    ] {
        assert_request_json_field(
            first_request,
            field,
            value,
            "empty session-scoped continuation should preserve original context",
        );
    }

    let log = fixture.wait_for_log(|log| {
        log.contains("stale_continuation reason=previous_response_not_found_locked_affinity")
    });
    assert!(
        !log.contains("previous_response_fresh_fallback reason="),
        "empty session-scoped continuations must not drop previous_response_id: {log}"
    );
    assert!(
        log.contains("stale_continuation reason=previous_response_not_found_locked_affinity"),
        "runtime log should show guarded stale-continuation behavior: {log}"
    );
}

#[test]
fn runtime_proxy_websocket_tool_output_with_session_does_not_fresh_fallback() {
    let _test_guard = crate::acquire_test_runtime_lock();
    let (_connect_timeout_guard, _progress_timeout_guard) =
        ci_runtime_proxy_websocket_timeout_guards();

    let fixture = start_runtime_continuation_fixture(
        RuntimeProxyBackend::start_websocket_previous_response_missing_without_turn_state(),
        "second",
        &["second"],
        &[("resp-second", "second")],
        Vec::new(),
    );
    let mut socket = fixture.connect_websocket("backend-api/codex/realtime?call_id=call-123");
    send_runtime_websocket_json(
        &mut socket,
        serde_json::json!({
            "previous_response_id": "resp-second",
            "session_id": "sess-replayable",
            "input": [{
                "type": "function_call_output",
                "call_id": "call_h7GvfUPAvb95drykPBrTw65i",
                "output": "ok"
            }],
        }),
    );

    let response_message = read_runtime_websocket_text(&mut socket);
    let _ = socket.close(None);

    assert!(
        response_message.contains("\"code\":\"stale_continuation\""),
        "tool-output continuation should fail as stale instead of replaying fresh: {response_message}"
    );
    assert!(
        !response_message.contains("No tool call found"),
        "proxy should not surface the fresh tool-output context error: {response_message}"
    );

    let websocket_requests = fixture.backend.websocket_requests();
    assert!(
        !websocket_requests.is_empty(),
        "backend should observe at least the original continuation"
    );
    for (field, value) in [
        ("previous_response_id", "resp-second"),
        ("session_id", "sess-replayable"),
        ("call_id", "call_h7GvfUPAvb95drykPBrTw65i"),
    ] {
        assert_all_requests_json_field(
            &websocket_requests,
            field,
            value,
            "backend should observe at least the original continuation",
            "websocket retries must preserve original continuation context",
        );
    }

    let log = fixture.wait_for_log(|log| {
        log.contains("stale_continuation reason=previous_response_not_found_locked_affinity")
    });
    assert!(
        log.contains("previous_response_not_found"),
        "runtime log should classify the broken continuation before surfacing stale: {log}"
    );
    assert!(
        log.contains("stale_continuation reason=previous_response_not_found_locked_affinity"),
        "tool outputs must stay chained instead of becoming fresh requests: {log}"
    );
    assert!(
        !log.contains(
            "previous_response_fresh_fallback reason=websocket_missing_turn_state_tool_result"
        ),
        "tool-output-only requests must not use proactive fresh replay: {log}"
    );
}

#[test]
fn runtime_proxy_websocket_tool_output_with_session_blocks_proactive_session_replay() {
    let _test_guard = crate::acquire_test_runtime_lock();
    let (_connect_timeout_guard, _progress_timeout_guard) =
        ci_runtime_proxy_websocket_timeout_guards();

    let fixture = start_runtime_continuation_fixture(
        RuntimeProxyBackend::start_websocket_owned_tool_output_needs_session_replay(),
        "second",
        &["second"],
        &[("resp-second", "second")],
        Vec::new(),
    );
    let mut socket = fixture.connect_websocket("backend-api/prodex/responses");
    send_runtime_websocket_json(
        &mut socket,
        serde_json::json!({
            "previous_response_id": "resp-second",
            "session_id": "sess-replayable",
            "input": [{
                "type": "function_call_output",
                "call_id": "call_J7U3Kdc539EyfWU4nZj9LCWQZ",
                "output": "ok"
            }],
        }),
    );

    let response_message = read_runtime_websocket_text(&mut socket);
    let _ = socket.close(None);

    assert!(
        response_message.contains("\"code\":\"stale_continuation\""),
        "tool-context failures should surface as stale continuation, not fresh replay: {response_message}"
    );
    assert!(
        !response_message.contains("No tool call found"),
        "proxy should translate upstream tool-context loss before it reaches Codex: {response_message}"
    );

    let websocket_requests = fixture.backend.websocket_requests();
    assert!(
        !websocket_requests.is_empty(),
        "backend should observe the guarded continuation"
    );
    for (field, value) in [
        ("previous_response_id", "resp-second"),
        ("session_id", "sess-replayable"),
    ] {
        assert_all_requests_json_field(
            &websocket_requests,
            field,
            value,
            "backend should observe the guarded continuation",
            "guarded attempts must preserve original continuation context",
        );
    }

    let log = fixture.wait_for_log(|log| {
        log.contains("stale_continuation reason=previous_response_not_found_locked_affinity")
    });
    assert!(
        !log.contains(
            "previous_response_fresh_fallback reason=websocket_missing_turn_state_tool_result"
        ),
        "runtime log must not show proactive fresh replay for tool outputs: {log}"
    );
    assert!(
        log.contains("stale_continuation reason=previous_response_not_found_locked_affinity"),
        "runtime log should show the guarded stale-continuation path: {log}"
    );
}

#[test]
fn runtime_proxy_http_empty_session_previous_response_does_not_fresh_fallback() {
    let temp_dir = TestDir::new();
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
    let temp_dir = TestDir::new();
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
    let temp_dir = TestDir::new();
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
    let temp_dir = TestDir::new();
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
    let temp_dir = TestDir::new();
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

#[test]
fn runtime_proxy_http_tool_output_with_session_does_not_fresh_fallback() {
    let temp_dir = TestDir::new();
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
    let temp_dir = TestDir::new();
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
    let temp_dir = TestDir::new();
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

#[test]
fn runtime_proxy_websocket_previous_response_not_found_after_prelude_surfaces_stale_continuation() {
    let _test_guard = crate::acquire_test_runtime_lock();
    let (_connect_timeout_guard, _progress_timeout_guard) =
        ci_runtime_proxy_websocket_timeout_guards();

    let fixture = start_runtime_continuation_fixture(
        RuntimeProxyBackend::start_websocket_previous_response_not_found_after_prelude(),
        "second",
        &["second"],
        &[("resp-second", "second")],
        Vec::new(),
    );
    let mut socket = fixture.connect_websocket("backend-api/prodex/responses");
    send_runtime_websocket_json(
        &mut socket,
        serde_json::json!({
            "previous_response_id": "resp-second",
            "input": [{
                "type": "message",
                "role": "user",
                "content": [{
                    "type": "input_text",
                    "text": "continue",
                }],
            }],
        }),
    );

    let (frames, response_message) = read_runtime_websocket_until(&mut socket, |text| {
        text.contains("previous_response_not_found") || text.contains("stale_continuation")
    });
    let _ = socket.close(None);

    assert!(
        response_message.contains("\"code\":\"stale_continuation\""),
        "precommit websocket continuation loss should surface stale_continuation: {response_message}"
    );
    assert!(
        !response_message.contains("previous_response_not_found"),
        "proxy should not leak raw previous_response_not_found before visible output: {response_message}"
    );
    assert!(
        !frames
            .iter()
            .any(|frame| frame.contains("\"type\":\"response.created\"")),
        "precommit hold frames should stay buffered when continuation fails before visible output: {frames:?}"
    );

    let websocket_requests = fixture.backend.websocket_requests();
    let first_request = assert_single_recorded_request(
        &websocket_requests,
        "backend should observe exactly one continuation attempt",
    );
    assert_request_json_field(
        first_request,
        "previous_response_id",
        "resp-second",
        "continuation request should preserve previous_response_id",
    );

    let log = fixture.wait_for_log(|log| {
        log.contains("stale_continuation reason=previous_response_not_found_locked_affinity")
    });
    assert!(
        log.contains("stale_continuation reason=previous_response_not_found_locked_affinity"),
        "runtime log should classify the precommit loss as stale continuation: {log}"
    );
}

#[test]
fn runtime_proxy_http_previous_response_not_found_after_commit_passes_through() {
    let temp_dir = TestDir::new();
    let backend = RuntimeProxyBackend::start_http_previous_response_not_found_after_commit();
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
                        last_verified_route: Some("websocket".to_string()),
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
            "http://{}/backend-api/codex/responses",
            proxy.listen_addr
        ))
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body(
            serde_json::json!({
                "previous_response_id": "resp-second",
                "input": [{
                    "type": "message",
                    "role": "user",
                    "content": [{
                        "type": "input_text",
                        "text": "continue",
                    }],
                }],
            })
            .to_string(),
        )
        .send()
        .expect("responses request should succeed");

    assert_eq!(response.status().as_u16(), 200);
    let body = read_runtime_http_stream_until(response, |body| {
        body.contains("previous_response_not_found") || body.contains("\"code\":\"stale_continuation\"")
    });
    assert!(
        body.contains("previous_response_not_found"),
        "post-commit HTTP continuation error should pass through raw upstream payload: {body}"
    );
    assert!(
        !body.contains("\"code\":\"stale_continuation\""),
        "post-commit HTTP continuation error must not be rewritten after commit: {body}"
    );

    let continuations = wait_for_runtime_continuations(&paths, |continuations| {
        continuations
            .statuses
            .response
            .get("resp-second")
            .is_some_and(|status| status.state == RuntimeContinuationBindingLifecycle::Dead)
            && continuations
                .statuses
                .response
                .get("resp-second-next")
                .is_some_and(|status| status.state == RuntimeContinuationBindingLifecycle::Dead)
            && !continuations
                .response_profile_bindings
                .contains_key("resp-second")
            && !continuations
                .response_profile_bindings
                .contains_key("resp-second-next")
            && !continuations.turn_state_bindings.contains_key("turn-second")
            && continuations
                .statuses
                .turn_state
                .get("turn-second")
                .is_some_and(|status| status.state == RuntimeContinuationBindingLifecycle::Dead)
    });
    assert!(
        continuations
            .statuses
            .response
            .get("resp-second")
            .is_some_and(|status| status.state == RuntimeContinuationBindingLifecycle::Dead),
        "upstream-confirmed dead previous_response_id should be tombstoned"
    );
    assert!(
        continuations
            .statuses
            .response
            .get("resp-second-next")
            .is_some_and(|status| status.state == RuntimeContinuationBindingLifecycle::Dead),
        "response id emitted before the committed failure should be cleared back out"
    );
    assert!(
        !continuations.turn_state_bindings.contains_key("turn-second"),
        "turn_state affinity derived from a dead committed chain should be cleared"
    );
    assert!(
        continuations
            .statuses
            .turn_state
            .get("turn-second")
            .is_some_and(|status| status.state == RuntimeContinuationBindingLifecycle::Dead),
        "dead committed chain should tombstone the related turn_state"
    );
}

#[test]
fn runtime_proxy_websocket_previous_response_not_found_after_commit_surfaces_stale_continuation() {
    let _test_guard = crate::acquire_test_runtime_lock();
    let (_connect_timeout_guard, _progress_timeout_guard) =
        ci_runtime_proxy_websocket_timeout_guards();

    let temp_dir = TestDir::new();
    let backend = RuntimeProxyBackend::start_websocket_previous_response_not_found_after_commit();
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

    let (mut socket, _response) = ws_connect(format!(
        "ws://{}/backend-api/prodex/responses",
        proxy.listen_addr
    ))
    .expect("websocket client should connect");
    set_test_websocket_io_timeout(&mut socket, ci_timing_upper_bound_ms(1_000, 3_000));

    socket
        .send(WsMessage::Text(
            serde_json::json!({
                "previous_response_id": "resp-second",
                "input": [{
                    "type": "message",
                    "role": "user",
                    "content": [{
                        "type": "input_text",
                        "text": "continue",
                    }],
                }],
            })
            .to_string()
            .into(),
        ))
        .expect("websocket request should send");

    let mut frames = Vec::new();
    let error_message = loop {
        match socket.read().expect("websocket response should read") {
            WsMessage::Text(text) => {
                let text = text.to_string();
                let is_error = text.contains("previous_response_not_found")
                    || text.contains("stale_continuation");
                frames.push(text.clone());
                if is_error {
                    break text;
                }
            }
            WsMessage::Ping(payload) => socket
                .send(WsMessage::Pong(payload))
                .expect("websocket pong should send"),
            WsMessage::Pong(_) | WsMessage::Frame(_) => {}
            other => panic!("unexpected websocket response: {other:?}"),
        }
    };
    let _ = socket.close(None);

    assert!(
        frames
            .iter()
            .any(|frame| frame.contains("\"type\":\"response.output_text.delta\"")),
        "client should see committed model output before the later continuation error: {frames:?}"
    );
    assert!(
        error_message.contains("\"code\":\"stale_continuation\""),
        "post-commit websocket continuation error should surface stale_continuation: {error_message}"
    );
    assert!(
        !error_message.contains("previous_response_not_found"),
        "proxy should not leak raw previous_response_not_found after a committed websocket chain dies: {error_message}"
    );

    let log_tail = wait_for_runtime_log_tail_until(
        || fs::read(&proxy.log_path).ok(),
        |log| {
            log.contains("stale_continuation reason=previous_response_not_found_locked_affinity")
        },
        500,
        2_000,
        20,
    );
    let log_tail = String::from_utf8_lossy(&log_tail);
    assert!(
        log_tail.contains("stale_continuation reason=previous_response_not_found_locked_affinity"),
        "runtime log should classify the committed websocket loss as stale continuation: {log_tail}"
    );

    let continuations = wait_for_runtime_continuations(&paths, |continuations| {
        continuations
            .statuses
            .response
            .get("resp-second")
            .is_some_and(|status| status.state == RuntimeContinuationBindingLifecycle::Dead)
            && continuations
                .statuses
                .response
                .get("resp-second-next")
                .is_some_and(|status| status.state == RuntimeContinuationBindingLifecycle::Dead)
            && !continuations
                .response_profile_bindings
                .contains_key("resp-second")
            && !continuations
                .response_profile_bindings
                .contains_key("resp-second-next")
            && !continuations.turn_state_bindings.contains_key("turn-second")
            && continuations
                .statuses
                .turn_state
                .get("turn-second")
                .is_some_and(|status| status.state == RuntimeContinuationBindingLifecycle::Dead)
    });
    assert!(
        continuations
            .statuses
            .response
            .get("resp-second")
            .is_some_and(|status| status.state == RuntimeContinuationBindingLifecycle::Dead),
        "upstream-confirmed dead previous_response_id should be tombstoned"
    );
    assert!(
        continuations
            .statuses
            .response
            .get("resp-second-next")
            .is_some_and(|status| status.state == RuntimeContinuationBindingLifecycle::Dead),
        "response id emitted before the committed websocket failure should be cleared back out"
    );
    assert!(
        !continuations.turn_state_bindings.contains_key("turn-second"),
        "turn_state affinity derived from a dead committed websocket chain should be cleared"
    );
    assert!(
        continuations
            .statuses
            .turn_state
            .get("turn-second")
            .is_some_and(|status| status.state == RuntimeContinuationBindingLifecycle::Dead),
        "dead committed websocket chain should tombstone the related turn_state"
    );
}

#[test]
fn runtime_proxy_http_quota_does_not_fresh_fallback_tool_output_only_requests() {
    let temp_dir = TestDir::new();
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
                "input": [{
                    "type": "custom_tool_call_output",
                    "call_id": "call_custom_123",
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
        "quota failure should pass through instead of degrading into a fresh tool-output retry: {body}"
    );
    assert!(
        !body.contains("No tool call found"),
        "non-replayable tool output should never be retried as a fresh request: {body}"
    );

    let responses_bodies = backend.responses_bodies();
    assert_eq!(
        responses_bodies.len(),
        1,
        "proxy should not send a second fresh retry for tool-output-only payloads: {responses_bodies:?}"
    );
    assert!(
        responses_bodies[0].contains("\"previous_response_id\":\"resp-main\""),
        "original upstream request should preserve previous_response affinity: {}",
        responses_bodies[0]
    );

    let log_tail = wait_for_runtime_log_tail_until(
        || fs::read(&proxy.log_path).ok(),
        |log| log.contains("quota_blocked_affinity_released") || log.contains("insufficient_quota"),
        2_000,
        5_000,
        20,
    );
    let log = String::from_utf8_lossy(&log_tail);
    assert!(
        !log.contains("previous_response_fresh_fallback reason=quota_blocked"),
        "quota-blocked tool-output-only path should not drop previous_response_id: {log}"
    );
}
