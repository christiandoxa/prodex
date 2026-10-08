use super::helpers::*;
use super::*;

#[test]
fn responses_hard_affinity_quota_block_requests_full_context_replay_then_rotates() {
    let backend =
        RuntimeProxyBackend::start_with_fault_script(RuntimeProxyBackendFaultScript::new([
            RuntimeProxyBackendFaultStep::explicit_quota_429(
                RuntimeProxyBackendFaultRoute::Responses,
                "main-account",
            ),
        ]));
    let ready = runtime_usage_snapshot(
        quota_window_ready(80, 3_600),
        quota_window_ready(80, 86_400),
    );
    let harness = RuntimeProxyProfileHarnessBuilder::new()
        .openai_profile("main", "main-account", Some("main@example.com"))
        .openai_profile("second", "second-account", Some("second@example.com"))
        .active_profile("main")
        .current_profile("main")
        .upstream_base_url(backend.base_url())
        .profile_usage_snapshot("main", ready.clone())
        .profile_usage_snapshot("second", ready)
        .build();
    {
        let now = Local::now().timestamp();
        let mut runtime = harness.shared().runtime.lock().expect("runtime lock");
        runtime.state.response_profile_bindings.insert(
            "resp-main".to_string(),
            ResponseProfileBinding {
                binding_identity: None,
                profile_name: "main".to_string(),
                bound_at: now,
            },
        );
        assert!(runtime_mark_continuation_status_verified(
            &mut runtime.continuation_statuses,
            RuntimeContinuationBindingKind::Response,
            "resp-main",
            now,
            Some(RuntimeRouteKind::Responses),
        ));
    }

    let continuation = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/backend-api/codex/responses".to_string(),
        headers: vec![("Content-Type".to_string(), "application/json".to_string())],
        body: br#"{"session_id":"sess-main","previous_response_id":"resp-main","input":[{"type":"function_call_output","call_id":"call_123","output":"done"}]}"#.to_vec(),
    };
    assert_eq!(
        runtime_request_previous_response_fresh_fallback_shape(&continuation),
        Some(RuntimePreviousResponseFreshFallbackShape::ToolOutputOnly),
        "regression must exercise a context-dependent continuation that cannot rotate directly",
    );
    let retry = proxy_runtime_responses_request(91, &continuation, harness.shared())
        .expect("quota-blocked hard affinity should request a full-context replay");
    let RuntimeResponsesReply::Buffered(parts) = retry else {
        panic!("quota-blocked hard affinity should return a buffered replay signal");
    };
    let status = parts.status;
    let body = String::from_utf8(parts.body.into_vec()).expect("replay signal should decode");
    assert_eq!(status, 400, "{body}");
    assert!(body.contains("previous_response_not_found"), "{body}");
    assert!(!body.contains("insufficient_quota"), "{body}");
    assert!(!body.contains("service_unavailable"), "{body}");
    assert_eq!(
        backend.responses_accounts(),
        vec!["main-account".to_string()]
    );
    let first_log = read_runtime_proxy_test_log(&harness.shared().log_path);
    assert!(first_log.contains("quota_blocked"), "{first_log}");
    assert!(
        !harness
            .shared()
            .runtime
            .lock()
            .expect("runtime lock")
            .state
            .response_profile_bindings
            .contains_key("resp-main"),
        "full-context retry signal must release the exhausted response owner"
    );

    let replay = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/backend-api/codex/responses".to_string(),
        headers: vec![("Content-Type".to_string(), "application/json".to_string())],
        body: br#"{"session_id":"sess-main","input":[{"role":"user","content":"original prompt"},{"role":"assistant","content":"previous answer"},{"role":"user","content":"continue"}]}"#.to_vec(),
    };
    let response = proxy_runtime_responses_request(92, &replay, harness.shared())
        .expect("full-context replay should rotate to the healthy profile");
    let (status, body) = match response {
        RuntimeResponsesReply::Buffered(parts) => (
            parts.status,
            String::from_utf8(parts.body.into_vec()).expect("response body should decode"),
        ),
        RuntimeResponsesReply::Streaming(mut response) => {
            let mut body = String::new();
            response
                .body
                .read_to_string(&mut body)
                .expect("streaming response should decode");
            (response.status, body)
        }
    };
    assert_eq!(status, 200, "{body}");
    assert_eq!(
        backend.responses_accounts(),
        vec!["main-account".to_string(), "second-account".to_string()]
    );
}


#[test]
fn responses_hard_affinity_quota_without_session_still_requests_full_context_replay() {
    let backend =
        RuntimeProxyBackend::start_with_fault_script(RuntimeProxyBackendFaultScript::new([
            RuntimeProxyBackendFaultStep::explicit_quota_429(
                RuntimeProxyBackendFaultRoute::Responses,
                "main-account",
            ),
        ]));
    let ready = runtime_usage_snapshot(
        quota_window_ready(80, 3_600),
        quota_window_ready(80, 86_400),
    );
    let harness = RuntimeProxyProfileHarnessBuilder::new()
        .openai_profile("main", "main-account", Some("main@example.com"))
        .openai_profile("second", "second-account", Some("second@example.com"))
        .active_profile("main")
        .current_profile("main")
        .upstream_base_url(backend.base_url())
        .profile_usage_snapshot("main", ready.clone())
        .profile_usage_snapshot("second", ready)
        .build();
    bind_hard_affinity_response(&harness, "resp-no-session");

    let continuation = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/backend-api/codex/responses".to_string(),
        headers: vec![("Content-Type".to_string(), "application/json".to_string())],
        body: br#"{"previous_response_id":"resp-no-session","input":[{"type":"function_call_output","call_id":"call_123","output":"done"}]}"#.to_vec(),
    };
    let retry = proxy_runtime_responses_request(93, &continuation, harness.shared())
        .expect("session-less hard-affinity quota block should request full-context replay");
    let RuntimeResponsesReply::Buffered(parts) = retry else {
        panic!("session-less quota recovery should return a buffered replay signal");
    };
    let status = parts.status;
    let body = String::from_utf8(parts.body.into_vec()).expect("replay signal should decode");
    assert_eq!(status, 400, "{body}");
    assert!(body.contains("previous_response_not_found"), "{body}");
    assert!(!body.contains("insufficient_quota"), "{body}");
    assert!(!body.contains("service_unavailable"), "{body}");

    let replay = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/backend-api/codex/responses".to_string(),
        headers: vec![("Content-Type".to_string(), "application/json".to_string())],
        body: br#"{"input":[{"role":"user","content":"original prompt"},{"role":"assistant","content":"previous answer"},{"role":"user","content":"continue"}]}"#.to_vec(),
    };
    let response = proxy_runtime_responses_request(94, &replay, harness.shared())
        .expect("session-less full-context replay should rotate to the healthy profile");
    let status = match response {
        RuntimeResponsesReply::Buffered(parts) => parts.status,
        RuntimeResponsesReply::Streaming(response) => response.status,
    };
    assert_eq!(status, 200);
    assert_eq!(
        backend.responses_accounts(),
        vec!["main-account".to_string(), "second-account".to_string()]
    );
}

fn bind_hard_affinity_response(harness: &RuntimeProxyProfileHarness, response_id: &str) {
    let now = Local::now().timestamp();
    let mut runtime = harness.shared().runtime.lock().expect("runtime lock");
    runtime.state.response_profile_bindings.insert(
        response_id.to_string(),
        ResponseProfileBinding {
            binding_identity: None,
            profile_name: "main".to_string(),
            bound_at: now,
        },
    );
    assert!(runtime_mark_continuation_status_verified(
        &mut runtime.continuation_statuses,
        RuntimeContinuationBindingKind::Response,
        response_id,
        now,
        Some(RuntimeRouteKind::Responses),
    ));
}

fn bind_hard_affinity_turn_state(harness: &RuntimeProxyProfileHarness, turn_state: &str) {
    let now = Local::now().timestamp();
    let mut runtime = harness.shared().runtime.lock().expect("runtime lock");
    runtime.turn_state_bindings.insert(
        turn_state.to_string(),
        ResponseProfileBinding {
            binding_identity: None,
            profile_name: "main".to_string(),
            bound_at: now,
        },
    );
    assert!(runtime_mark_continuation_status_verified(
        &mut runtime.continuation_statuses,
        RuntimeContinuationBindingKind::TurnState,
        turn_state,
        now,
        Some(RuntimeRouteKind::Responses),
    ));
}

fn turn_state_request(turn_state: &str, full_history: bool) -> RuntimeProxyRequest {
    let input = if full_history {
        serde_json::json!([
            {"type":"message","role":"user","content":"compacted history"},
            {"type":"message","role":"assistant","content":"completed work"},
            {"type":"message","role":"user","content":"continue"},
        ])
    } else {
        serde_json::json!([
            {"type":"message","role":"user","content":"continue"},
        ])
    };
    RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/backend-api/codex/responses".to_string(),
        headers: vec![
            ("Content-Type".to_string(), "application/json".to_string()),
            ("x-codex-turn-state".to_string(), turn_state.to_string()),
        ],
        body: serde_json::json!({
            "model": "gpt-6-luna",
            "input": input,
            "client_metadata": {"x-codex-turn-state": turn_state},
        })
        .to_string()
        .into_bytes(),
    }
}

#[test]
fn responses_post_compaction_turn_state_usage_limit_replays_to_ready_profile_without_user_error() {
    let backend =
        RuntimeProxyBackend::start_with_fault_script(RuntimeProxyBackendFaultScript::new([
            RuntimeProxyBackendFaultStep::usage_limit_429(
                RuntimeProxyBackendFaultRoute::Responses,
                "main-account",
            ),
        ]));
    let ready = runtime_usage_snapshot(
        quota_window_ready(80, 3_600),
        quota_window_ready(80, 86_400),
    );
    let harness = RuntimeProxyProfileHarnessBuilder::new()
        .openai_profile("main", "main-account", Some("main@example.com"))
        .openai_profile("second", "second-account", Some("second@example.com"))
        .active_profile("main")
        .current_profile("main")
        .upstream_base_url(backend.base_url())
        .profile_usage_snapshot("main", ready.clone())
        .profile_usage_snapshot("second", ready)
        .build();
    let turn_state = "turn-post-compact-http";
    bind_hard_affinity_turn_state(&harness, turn_state);

    let request = turn_state_request(turn_state, true);
    assert!(
        runtime_proxy_crate::runtime_request_has_reconstructable_full_history(&request),
        "regression must exercise a replayable post-compaction full-history request"
    );
    let response = proxy_runtime_responses_request(95, &request, harness.shared())
        .expect("post-compaction quota should replay on the healthy profile");
    let (status, body) = match response {
        RuntimeResponsesReply::Buffered(parts) => (
            parts.status,
            String::from_utf8(parts.body.into_vec()).expect("response body should decode"),
        ),
        RuntimeResponsesReply::Streaming(mut response) => {
            let mut body = String::new();
            response
                .body
                .read_to_string(&mut body)
                .expect("streaming response should decode");
            (response.status, body)
        }
    };

    assert_eq!(status, 200, "{body}");
    assert!(
        !body.contains("usage_limit_reached") && !body.contains("usage limit"),
        "recoverable quota must not leak to the HTTP client: {body}"
    );
    assert_eq!(
        backend.responses_accounts(),
        vec!["main-account".to_string(), "second-account".to_string()]
    );
    let headers = backend.responses_headers();
    assert_eq!(headers.len(), 2, "{headers:?}");
    assert!(
        !headers[1].contains_key("x-codex-turn-state"),
        "rotated HTTP replay must scrub the dead sticky token: {headers:?}"
    );
    let bodies = backend.responses_bodies();
    assert_eq!(bodies.len(), 2, "{bodies:?}");
    let replay: serde_json::Value =
        serde_json::from_str(&bodies[1]).expect("replay body should be JSON");
    assert_eq!(
        replay
            .get("client_metadata")
            .and_then(|metadata| metadata.get("x-codex-turn-state")),
        None
    );
    let log = read_runtime_proxy_test_log(&harness.shared().log_path);
    assert!(
        log.contains("quota_blocked_turn_state_full_context_replay")
            && !log.contains("upstream_usage_limit_passthrough"),
        "{log}"
    );
}

#[test]
fn responses_turn_state_quota_without_full_history_stays_fail_closed() {
    let backend =
        RuntimeProxyBackend::start_with_fault_script(RuntimeProxyBackendFaultScript::new([
            RuntimeProxyBackendFaultStep::usage_limit_429(
                RuntimeProxyBackendFaultRoute::Responses,
                "main-account",
            ),
        ]));
    let ready = runtime_usage_snapshot(
        quota_window_ready(80, 3_600),
        quota_window_ready(80, 86_400),
    );
    let harness = RuntimeProxyProfileHarnessBuilder::new()
        .openai_profile("main", "main-account", Some("main@example.com"))
        .openai_profile("second", "second-account", Some("second@example.com"))
        .active_profile("main")
        .current_profile("main")
        .upstream_base_url(backend.base_url())
        .profile_usage_snapshot("main", ready.clone())
        .profile_usage_snapshot("second", ready)
        .build();
    let turn_state = "turn-nonreplayable-http";
    bind_hard_affinity_turn_state(&harness, turn_state);

    let request = turn_state_request(turn_state, false);
    assert!(
        !runtime_proxy_crate::runtime_request_has_reconstructable_full_history(&request),
        "negative control must remain context-dependent"
    );
    let response = proxy_runtime_responses_request(96, &request, harness.shared())
        .expect("unsafe replay must return the real quota failure");
    let RuntimeResponsesReply::Buffered(parts) = response else {
        panic!("terminal quota failure should remain buffered");
    };
    let status = parts.status;
    let body = String::from_utf8(parts.body.into_vec()).expect("quota body should decode");
    assert_eq!(status, 429, "{body}");
    assert!(body.contains("usage limit"), "{body}");
    assert_eq!(backend.responses_accounts(), vec!["main-account".to_string()]);
    let log = read_runtime_proxy_test_log(&harness.shared().log_path);
    assert!(log.contains("upstream_usage_limit_passthrough"), "{log}");
}


fn hard_affinity_continuation_request(response_id: &str) -> RuntimeProxyRequest {
    RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/backend-api/codex/responses".to_string(),
        headers: vec![("Content-Type".to_string(), "application/json".to_string())],
        body: format!(
            r#"{{"session_id":"sess-main","previous_response_id":"{response_id}","input":[{{"type":"function_call_output","call_id":"call_123","output":"done"}}]}}"#
        )
        .into_bytes(),
    }
}

fn full_context_replay_request() -> RuntimeProxyRequest {
    RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/backend-api/codex/responses".to_string(),
        headers: vec![("Content-Type".to_string(), "application/json".to_string())],
        body: br#"{"session_id":"sess-main","input":[{"role":"user","content":"original prompt"},{"role":"assistant","content":"previous answer"},{"role":"user","content":"continue"}]}"#.to_vec(),
    }
}

fn run_retryable_hard_affinity_replay_case(fault: RuntimeProxyBackendFaultStep, marker: &str) {
    run_retryable_hard_affinity_replay_case_after_delay(fault, marker, std::time::Duration::ZERO);
}

fn run_retryable_hard_affinity_replay_case_after_delay(
    fault: RuntimeProxyBackendFaultStep,
    marker: &str,
    client_replay_delay: std::time::Duration,
) {
    let backend =
        RuntimeProxyBackend::start_with_fault_script(RuntimeProxyBackendFaultScript::new([fault]));
    let ready = runtime_usage_snapshot(
        quota_window_ready(80, 3_600),
        quota_window_ready(80, 86_400),
    );
    let harness = RuntimeProxyProfileHarnessBuilder::new()
        .openai_profile("main", "main-account", Some("main@example.com"))
        .openai_profile("second", "second-account", Some("second@example.com"))
        .active_profile("main")
        .current_profile("main")
        .upstream_base_url(backend.base_url())
        .profile_usage_snapshot("main", ready.clone())
        .profile_usage_snapshot("second", ready)
        .build();
    bind_hard_affinity_response(&harness, "resp-main");

    let continuation = hard_affinity_continuation_request("resp-main");
    let retry = proxy_runtime_responses_request(101, &continuation, harness.shared())
        .expect("retryable hard-affinity failure should request full-context replay");
    let RuntimeResponsesReply::Buffered(parts) = retry else {
        panic!("retryable hard-affinity failure should return a buffered replay signal");
    };
    let status = parts.status;
    let body = String::from_utf8(parts.body.into_vec()).expect("replay signal should decode");
    assert_eq!(status, 400, "{body}");
    assert!(body.contains("previous_response_not_found"), "{body}");
    assert!(!body.contains("service_unavailable"), "{body}");
    assert!(!body.contains("rate_limit_exceeded"), "{body}");
    let log = read_runtime_proxy_test_log(&harness.shared().log_path);
    assert!(log.contains(marker), "missing {marker}: {log}");

    assert_eq!(
        backend.responses_accounts(),
        ["main-account"],
        "the failed hard-affinity request must signal replay before trying another profile: {log}",
    );
    std::thread::sleep(client_replay_delay);

    let response =
        proxy_runtime_responses_request(102, &full_context_replay_request(), harness.shared())
            .expect("full-context replay should rotate to the healthy profile");
    let status = match response {
        RuntimeResponsesReply::Buffered(parts) => parts.status,
        RuntimeResponsesReply::Streaming(response) => response.status,
    };
    assert_eq!(status, 200);
    assert_eq!(
        backend.responses_accounts(),
        vec!["main-account".to_string(), "second-account".to_string()]
    );
}

#[test]
fn responses_hard_affinity_overload_requests_full_context_replay_then_rotates() {
    run_retryable_hard_affinity_replay_case(
        RuntimeProxyBackendFaultStep::overloaded_503(
            RuntimeProxyBackendFaultRoute::Responses,
            "main-account",
        ),
        "upstream_overload_full_context_retry_signal",
    );
}

#[test]
fn responses_hard_affinity_auth_failure_requests_full_context_replay_then_rotates() {
    run_retryable_hard_affinity_replay_case(
        RuntimeProxyBackendFaultStep::unauthorized(
            RuntimeProxyBackendFaultRoute::Responses,
            "main-account",
        ),
        "auth_failed_full_context_retry_signal",
    );
}

#[test]
fn responses_hard_affinity_rate_limit_requests_full_context_replay_then_rotates() {
    // A one-second Retry-After can expire while a loaded Windows runner writes
    // the replay signal. Keep the server-declared hold active across an explicit
    // slow-client delay; otherwise retrying the original profile is valid.
    run_retryable_hard_affinity_replay_case_after_delay(
        RuntimeProxyBackendFaultStep::rate_limited_429_for_delay(
            RuntimeProxyBackendFaultRoute::Responses,
            "main-account",
            30,
        ),
        "rate_limit_full_context_retry_signal",
        std::time::Duration::from_millis(2_200),
    );
}

#[test]
fn responses_hard_affinity_generic_429_requests_full_context_replay_then_rotates() {
    run_retryable_hard_affinity_replay_case(
        RuntimeProxyBackendFaultStep::plain_429(
            RuntimeProxyBackendFaultRoute::Responses,
            "main-account",
        ),
        "rate_limit_full_context_retry_signal",
    );
}

#[test]
fn responses_hard_affinity_quota_without_fallback_remains_terminal() {
    let backend =
        RuntimeProxyBackend::start_with_fault_script(RuntimeProxyBackendFaultScript::new([
            RuntimeProxyBackendFaultStep::explicit_quota_429(
                RuntimeProxyBackendFaultRoute::Responses,
                "main-account",
            ),
        ]));
    let ready = runtime_usage_snapshot(
        quota_window_ready(80, 3_600),
        quota_window_ready(80, 86_400),
    );
    let harness = RuntimeProxyProfileHarnessBuilder::single_openai_profile(
        "main",
        "main-account",
        "main@example.com",
    )
    .upstream_base_url(backend.base_url())
    .profile_usage_snapshot("main", ready)
    .build();
    bind_hard_affinity_response(&harness, "resp-main");

    let response = proxy_runtime_responses_request(
        103,
        &hard_affinity_continuation_request("resp-main"),
        harness.shared(),
    )
    .expect("fully exhausted pool should return the real upstream quota failure");
    let RuntimeResponsesReply::Buffered(parts) = response else {
        panic!("terminal quota failure should remain buffered");
    };
    let status = parts.status;
    let body = String::from_utf8(parts.body.into_vec()).expect("quota body should decode");
    assert_eq!(status, 429, "{body}");
    assert!(body.contains("insufficient_quota"), "{body}");
    assert!(!body.contains("previous_response_not_found"), "{body}");
    assert!(
        harness
            .shared()
            .runtime
            .lock()
            .expect("runtime lock")
            .state
            .response_profile_bindings
            .contains_key("resp-main"),
        "terminal quota failure must not pretend a replay target exists"
    );
}

fn bind_standard_session(harness: &RuntimeProxyProfileHarness, session_id: &str) {
    let now = Local::now().timestamp();
    let binding = ResponseProfileBinding {
        binding_identity: None,
        profile_name: "main".to_string(),
        bound_at: now,
    };
    let mut runtime = harness.shared().runtime.lock().expect("runtime lock");
    runtime
        .session_id_bindings
        .insert(session_id.to_string(), binding.clone());
    runtime
        .state
        .session_profile_bindings
        .insert(session_id.to_string(), binding);
    assert!(runtime_mark_continuation_status_verified(
        &mut runtime.continuation_statuses,
        RuntimeContinuationBindingKind::SessionId,
        session_id,
        now,
        Some(RuntimeRouteKind::Standard),
    ));
}

#[test]
fn standard_bound_session_rate_limit_rotates_to_ready_profile() {
    let backend =
        RuntimeProxyBackend::start_with_fault_script(RuntimeProxyBackendFaultScript::new([
            RuntimeProxyBackendFaultStep::rate_limited_429(
                RuntimeProxyBackendFaultRoute::Status,
                "main-account",
            ),
        ]));
    let ready = runtime_usage_snapshot(
        quota_window_ready(80, 3_600),
        quota_window_ready(80, 86_400),
    );
    let harness = RuntimeProxyProfileHarnessBuilder::new()
        .openai_profile("main", "main-account", Some("main@example.com"))
        .openai_profile("second", "second-account", Some("second@example.com"))
        .active_profile("main")
        .current_profile("main")
        .upstream_base_url(backend.base_url())
        .profile_usage_snapshot("main", ready.clone())
        .profile_usage_snapshot("second", ready)
        .build();
    bind_standard_session(&harness, "sess-standard");
    let request = RuntimeProxyRequest {
        method: "GET".to_string(),
        path_and_query: "/backend-api/status".to_string(),
        headers: vec![("session_id".to_string(), "sess-standard".to_string())],
        body: Vec::new(),
    };

    let response = proxy_runtime_standard_request(111, &request, harness.shared())
        .expect("session-bound rate limit should rotate to a healthy profile");
    let (status, body) = tiny_http_response_status_and_body(response);
    assert_eq!(status, 200, "{body}");
    assert!(body.contains("second-account"), "{body}");
    assert_eq!(
        backend.responses_accounts(),
        vec!["main-account".to_string(), "second-account".to_string()]
    );
    let runtime = harness.shared().runtime.lock().expect("runtime lock");
    assert_eq!(
        runtime
            .state
            .session_profile_bindings
            .get("sess-standard")
            .map(|binding| binding.profile_name.as_str()),
        Some("second")
    );
}

fn run_standard_bound_session_retryable_case(fault: RuntimeProxyBackendFaultStep) {
    let backend =
        RuntimeProxyBackend::start_with_fault_script(RuntimeProxyBackendFaultScript::new([fault]));
    let ready = runtime_usage_snapshot(
        quota_window_ready(80, 3_600),
        quota_window_ready(80, 86_400),
    );
    let harness = RuntimeProxyProfileHarnessBuilder::new()
        .openai_profile("main", "main-account", Some("main@example.com"))
        .openai_profile("second", "second-account", Some("second@example.com"))
        .active_profile("main")
        .current_profile("main")
        .upstream_base_url(backend.base_url())
        .profile_usage_snapshot("main", ready.clone())
        .profile_usage_snapshot("second", ready)
        .build();
    bind_standard_session(&harness, "sess-standard-retry");
    let request = RuntimeProxyRequest {
        method: "GET".to_string(),
        path_and_query: "/backend-api/status".to_string(),
        headers: vec![("session_id".to_string(), "sess-standard-retry".to_string())],
        body: Vec::new(),
    };

    let response = proxy_runtime_standard_request(112, &request, harness.shared())
        .expect("retryable session failure should rotate to a healthy profile");
    let (status, body) = tiny_http_response_status_and_body(response);
    assert_eq!(status, 200, "{body}");
    assert!(body.contains("second-account"), "{body}");
    assert_eq!(
        backend.responses_accounts(),
        vec!["main-account".to_string(), "second-account".to_string()]
    );
    let runtime = harness.shared().runtime.lock().expect("runtime lock");
    assert_eq!(
        runtime
            .state
            .session_profile_bindings
            .get("sess-standard-retry")
            .map(|binding| binding.profile_name.as_str()),
        Some("second")
    );
}

#[test]
fn standard_bound_session_overload_rotates_to_ready_profile() {
    run_standard_bound_session_retryable_case(RuntimeProxyBackendFaultStep::overloaded_503(
        RuntimeProxyBackendFaultRoute::Status,
        "main-account",
    ));
}

#[test]
fn standard_bound_session_profile_unavailable_rotates_to_ready_profile() {
    run_standard_bound_session_retryable_case(RuntimeProxyBackendFaultStep::profile_unavailable(
        RuntimeProxyBackendFaultRoute::Status,
        "main-account",
    ));
}
