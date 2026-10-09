use super::helpers::*;
use super::*;
use std::io::Read;

fn ready_harness(backend: &RuntimeProxyBackend) -> RuntimeProxyProfileHarness {
    let ready = runtime_usage_snapshot(
        quota_window_ready(80, 3_600),
        quota_window_ready(80, 86_400),
    );
    RuntimeProxyProfileHarnessBuilder::new()
        .openai_profile("main", "main-account", Some("main@example.com"))
        .openai_profile("second", "second-account", Some("second@example.com"))
        .active_profile("main")
        .current_profile("main")
        .upstream_base_url(backend.base_url())
        .profile_usage_snapshot("main", ready.clone())
        .profile_usage_snapshot("second", ready)
        .build()
}

fn request(stream: bool) -> RuntimeProxyRequest {
    RuntimeProxyRequest {
        method: "POST".into(),
        path_and_query: "/backend-api/codex/responses".into(),
        headers: vec![
            ("Content-Type".into(), "application/json".into()),
            ("session-id".into(), "capacity-header-regression".into()),
        ],
        body: serde_json::to_vec(&serde_json::json!({"stream": stream, "input": []})).unwrap(),
    }
}

#[test]
fn missing_content_type_capacity_must_not_commit_as_unary_success() {
    let backend = RuntimeProxyBackend::start_with_fault_script(RuntimeProxyBackendFaultScript::new([
        RuntimeProxyBackendFaultStep::sse_overloaded(RuntimeProxyBackendFaultRoute::Responses, "main-account")
            .without_content_type(),
    ]));
    let harness = ready_harness(&backend);
    let attempt = attempt_runtime_responses_request(
        437101, &request(true), harness.shared(), "main",
        RuntimeResponsesAttemptOptions { turn_state_override: None, prompt_cache_key: None,
            hard_affinity: false, selection_attempt: 1 },
    ).unwrap();
    assert!(matches!(attempt, RuntimeResponsesAttempt::Overloaded { .. }),
        "an untyped streaming failure must reach precommit recovery, not Success");
    assert_eq!(harness.shared().lane_admission.profile_inflight_count("main"), 0,
        "failed payload retention must not retain the attempt lease");
}

#[test]
fn missing_content_type_capacity_rotates_before_stream_commit() {
    let backend = RuntimeProxyBackend::start_with_fault_script(RuntimeProxyBackendFaultScript::new([
        RuntimeProxyBackendFaultStep::sse_overloaded(RuntimeProxyBackendFaultRoute::Responses, "main-account")
            .without_content_type(),
        RuntimeProxyBackendFaultStep::sse_success(RuntimeProxyBackendFaultRoute::Responses, "second-account", "recovered-headerless"),
    ]));
    let harness = ready_harness(&backend);
    let response = proxy_runtime_responses_request(437102, &request(true), harness.shared()).unwrap();
    let RuntimeResponsesReply::Streaming(mut response) = response else {
        panic!("the capacity failure was incorrectly committed as a buffered success");
    };
    let mut body = String::new();
    response.body.read_to_string(&mut body).unwrap();
    assert!(body.contains("recovered-headerless"), "{body}");
    assert!(!body.contains("server_is_overloaded"), "{body}");
    assert_eq!(backend.responses_accounts(), ["main-account", "second-account"]);
}

#[test]
fn missing_content_type_streaming_success_keeps_upstream_header_absence() {
    let backend = RuntimeProxyBackend::start_with_fault_script(RuntimeProxyBackendFaultScript::new([
        RuntimeProxyBackendFaultStep::sse_success(RuntimeProxyBackendFaultRoute::Responses, "main-account", "untyped-success")
            .without_content_type(),
    ]));
    let harness = ready_harness(&backend);
    let response = proxy_runtime_responses_request(437103, &request(true), harness.shared()).unwrap();
    let RuntimeResponsesReply::Streaming(mut response) = response else {
        panic!("the requested stream must not become unary just because a MIME header is missing");
    };
    assert!(!response.headers.iter().any(|(key, _)| key.eq_ignore_ascii_case("content-type")));
    let mut body = String::new();
    response.body.read_to_string(&mut body).unwrap();
    assert!(body.contains("untyped-success"), "{body}");
    assert_eq!(backend.responses_accounts(), ["main-account"]);
}

#[test]
fn missing_content_type_unary_and_explicit_json_contracts_remain_buffered() {
    for (stream, headerless) in [(false, true), (true, false)] {
        let step = RuntimeProxyBackendFaultStep::success(RuntimeProxyBackendFaultRoute::Responses, "main-account");
        let step = if headerless { step.without_content_type() } else { step };
        let backend = RuntimeProxyBackend::start_with_fault_script(RuntimeProxyBackendFaultScript::new([step]));
        let harness = ready_harness(&backend);
        let response = proxy_runtime_responses_request(437104, &request(stream), harness.shared()).unwrap();
        let RuntimeResponsesReply::Buffered(parts) = response else {
            panic!("explicit JSON and non-stream requests retain unary semantics");
        };
        assert_eq!(parts.status, 200);
        let value: serde_json::Value = serde_json::from_slice(&parts.body).unwrap();
        assert_eq!(value["id"], "scripted-success");
    }
}

#[test]
fn missing_content_type_capacity_in_same_turn_retries_the_bound_profile() {
    let backend = RuntimeProxyBackend::start_with_fault_script(RuntimeProxyBackendFaultScript::new([
        RuntimeProxyBackendFaultStep::sse_success(RuntimeProxyBackendFaultRoute::Responses, "main-account", "first-sample")
            .with_response_turn_state("sticky-turn-state").without_content_type(),
        RuntimeProxyBackendFaultStep::sse_overloaded(RuntimeProxyBackendFaultRoute::Responses, "main-account")
            .without_content_type(),
        RuntimeProxyBackendFaultStep::sse_success(RuntimeProxyBackendFaultRoute::Responses, "main-account", "recovered-same-turn")
            .without_content_type(),
    ]));
    let harness = ready_harness(&backend);
    let first = proxy_runtime_responses_request(437105, &request(true), harness.shared()).unwrap();
    let RuntimeResponsesReply::Streaming(mut first) = first else { panic!("first sample must stream"); };
    let mut first_body = String::new();
    first.body.read_to_string(&mut first_body).unwrap();
    assert!(first_body.contains("first-sample"));
    drop(first);

    let mut followup = request(true);
    followup.headers.push(("x-codex-turn-state".into(), "sticky-turn-state".into()));
    let response = proxy_runtime_responses_request(437106, &followup, harness.shared()).unwrap();
    let RuntimeResponsesReply::Streaming(mut response) = response else { panic!("follow-up must stream"); };
    let mut body = String::new();
    response.body.read_to_string(&mut body).unwrap();
    assert!(body.contains("recovered-same-turn"), "transient capacity must retry the pinned owner: {body}");
    assert!(!body.contains("server_is_overloaded"), "{body}");
    assert_eq!(backend.responses_accounts(), ["main-account", "main-account", "main-account"],
        "turn-state affinity must never be replayed on a different account");
}

#[test]
fn missing_content_type_capacity_after_output_is_never_replayed() {
    let backend = RuntimeProxyBackend::start_with_fault_script(RuntimeProxyBackendFaultScript::new([
        RuntimeProxyBackendFaultStep::sse_overloaded(RuntimeProxyBackendFaultRoute::Responses, "main-account")
            .after_visible_output().without_content_type(),
    ]));
    let harness = ready_harness(&backend);
    let response = proxy_runtime_responses_request(437107, &request(true), harness.shared()).unwrap();
    let RuntimeResponsesReply::Streaming(mut response) = response else { panic!("visible output commits a stream"); };
    let mut body = String::new();
    response.body.read_to_string(&mut body).unwrap();
    assert!(body.contains("visible-before-error"), "{body}");
    assert!(body.contains("server_is_overloaded"), "an error after output remains the upstream stream: {body}");
    assert_eq!(backend.responses_accounts(), ["main-account"], "committed output must not be duplicated on a retry");
}

#[test]
fn missing_content_type_persistent_capacity_has_bounded_same_owner_retries() {
    let backend = RuntimeProxyBackend::start_with_fault_script(RuntimeProxyBackendFaultScript::new(
        (0..6).map(|_| RuntimeProxyBackendFaultStep::sse_overloaded(RuntimeProxyBackendFaultRoute::Responses, "main-account")
            .without_content_type()),
    ));
    let harness = ready_harness(&backend);
    let attempt = attempt_runtime_responses_request(
        437108, &request(true), harness.shared(), "main",
        RuntimeResponsesAttemptOptions { turn_state_override: None, prompt_cache_key: None,
            hard_affinity: true, selection_attempt: 1 },
    ).unwrap();
    let RuntimeResponsesAttempt::Overloaded { response, .. } = attempt else { panic!("retry exhaustion must return the original overload class"); };
    let RuntimeResponsesReply::Streaming(mut response) = response else { panic!("preserve the original overload stream"); };
    let mut body = String::new();
    response.body.read_to_string(&mut body).unwrap();
    assert!(body.contains("server_is_overloaded"), "{body}");
    assert_eq!(backend.responses_accounts(), ["main-account"; 6], "one initial attempt and at most five same-owner retries");
    assert_eq!(harness.shared().lane_admission.profile_inflight_count("main"), 0);
    assert_eq!(harness.shared().lane_admission.profile_inflight_releases_total(), 6);
    drop(response);
    assert_eq!(harness.shared().lane_admission.profile_inflight_releases_total(), 6, "retained failed payload must not release twice");
}

#[test]
fn missing_content_type_capacity_preserves_retry_after_beyond_local_budget() {
    let backend = RuntimeProxyBackend::start_with_fault_script(RuntimeProxyBackendFaultScript::new([
        RuntimeProxyBackendFaultStep::sse_overloaded(RuntimeProxyBackendFaultRoute::Responses, "main-account")
            .with_sse_retry_after(300).without_content_type(),
    ]));
    let harness = ready_harness(&backend);
    let attempt = attempt_runtime_responses_request(
        437109, &request(true), harness.shared(), "main",
        RuntimeResponsesAttemptOptions { turn_state_override: None, prompt_cache_key: None,
            hard_affinity: true, selection_attempt: 1 },
    ).unwrap();
    let RuntimeResponsesAttempt::Overloaded { retry_after, .. } = attempt else { panic!("long provider retry advice must not become local success"); };
    assert_eq!(retry_after, Some(Duration::from_secs(300)));
    assert_eq!(backend.responses_accounts(), ["main-account"], "never shorten Retry-After to fit a local budget");
    assert_eq!(harness.shared().lane_admission.profile_inflight_count("main"), 0);
}
