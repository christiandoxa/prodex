use super::*;

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
        frames
            .iter()
            .all(|frame| !frame.contains("\"type\":\"response.created\"")),
        "response.created should stay buffered when pre-output continuation failure is retryable: {frames:?}"
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
    let _test_guard = crate::acquire_test_runtime_lock();
    let _stream_idle_timeout_guard = ci_runtime_proxy_timeout_guard(
        "PRODEX_RUNTIME_PROXY_STREAM_IDLE_TIMEOUT_MS",
        2_000,
        5_000,
    );

    let temp_dir = TestDir::isolated();
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
        body.contains("previous_response_not_found")
            || body.contains("\"code\":\"stale_continuation\"")
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
            && !continuations
                .turn_state_bindings
                .contains_key("turn-second")
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
        !continuations
            .turn_state_bindings
            .contains_key("turn-second"),
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

    let temp_dir = TestDir::isolated();
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
        |log| log.contains("stale_continuation reason=previous_response_not_found_locked_affinity"),
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
            && !continuations
                .turn_state_bindings
                .contains_key("turn-second")
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
        !continuations
            .turn_state_bindings
            .contains_key("turn-second"),
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
fn runtime_proxy_http_stream_keeps_selected_profile_after_commit_failure() {
    let _test_guard = crate::acquire_test_runtime_lock();
    let _stream_idle_timeout_guard = ci_runtime_proxy_timeout_guard(
        "PRODEX_RUNTIME_PROXY_STREAM_IDLE_TIMEOUT_MS",
        2_000,
        5_000,
    );

    let temp_dir = TestDir::isolated();
    let backend = RuntimeProxyBackend::start_http_delayed_quota_after_output_item_added();
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");

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
                bound_at: Local::now().timestamp(),
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
                "model": "gpt-5-codex",
                "previous_response_id": "resp-main",
                "input": [{
                    "type": "message",
                    "role": "user",
                    "content": [{
                        "type": "input_text",
                        "text": "fresh stream",
                    }],
                }],
            })
            .to_string(),
        )
        .send()
        .expect("responses request should succeed");

    assert_eq!(response.status().as_u16(), 200);
    let body = read_runtime_http_stream_until(response, |body| {
        body.contains("response.output_item.added") && body.contains("response.failed")
    });
    assert!(
        body.contains("\"type\":\"response.output_item.added\""),
        "stream should commit visible output before the failure: {body}"
    );
    assert!(
        body.contains("You've hit your usage limit"),
        "post-commit quota-shaped failure should be forwarded from the pinned profile: {body}"
    );
    assert_eq!(
        backend.responses_accounts(),
        vec!["main-account".to_string()],
        "selected profile must stay pinned after stream commit; no mid-stream rotation to the eligible second profile"
    );
}

#[test]
fn runtime_proxy_http_quota_does_not_fresh_fallback_tool_output_only_requests() {
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
