#[test]
fn runtime_proxy_http_keepalive_capacity_rotates_before_output_on_same_request() {
    let _guard = crate::acquire_test_runtime_lock();
    let backend =
        RuntimeProxyBackend::start_with_fault_script(RuntimeProxyBackendFaultScript::new([
            RuntimeProxyBackendFaultStep::sse_overloaded(
                RuntimeProxyBackendFaultRoute::Responses,
                "main-account",
            )
            .with_keepalive_prelude()
            .without_content_type(),
            RuntimeProxyBackendFaultStep::sse_success(
                RuntimeProxyBackendFaultRoute::Responses,
                "second-account",
                "resp-healthy",
            ),
        ]));
    let fixture =
        start_runtime_continuation_fixture(backend, "main", &["main", "second"], &[], Vec::new());
    let input = serde_json::json!({
        "model": "gpt-5.4",
        "stream": true,
        "session_id": "sess-heartbeat-recovery",
        "input": [{"role": "user", "content": "continue the existing work"}],
    });
    let response = fixture.post_json("backend-api/codex/responses", input.clone());
    let status = response.status().as_u16();
    let body = response.text().expect("mock SSE response should decode");
    assert_eq!(status, 200, "{body}");
    assert!(
        body.contains("resp-healthy"),
        "healthy profile was not reached: {body}"
    );
    assert!(
        !body.contains("server_is_overloaded"),
        "failed attempt leaked: {body}"
    );
    assert!(
        !body.contains("resp-held"),
        "failed attempt metadata must be discarded: {body}"
    );
    assert_eq!(
        fixture.backend.responses_accounts(),
        ["main-account", "second-account"]
    );
    let requests = fixture.backend.responses_bodies();
    assert_eq!(
        requests.len(),
        2,
        "one request per candidate, no duplicate tool turn"
    );
    for request in requests {
        let actual: serde_json::Value =
            serde_json::from_str(&request).expect("request should be JSON");
        assert_eq!(actual["input"], input["input"]);
        assert_eq!(actual["session_id"], input["session_id"]);
    }
    let log = fixture.wait_for_log(|log| log.contains("committed profile=second"));
    assert!(
        log.contains("upstream_overloaded route=responses profile=main"),
        "{log}"
    );
    assert!(
        !log.contains("transport=http committed profile=main"),
        "{log}"
    );
}

#[test]
fn runtime_proxy_http_keepalive_after_real_output_never_rotates_or_replays() {
    let _guard = crate::acquire_test_runtime_lock();
    let backend =
        RuntimeProxyBackend::start_with_fault_script(RuntimeProxyBackendFaultScript::new([
            RuntimeProxyBackendFaultStep::sse_overloaded(
                RuntimeProxyBackendFaultRoute::Responses,
                "main-account",
            )
            .with_keepalive_prelude()
            .after_visible_output(),
            RuntimeProxyBackendFaultStep::sse_success(
                RuntimeProxyBackendFaultRoute::Responses,
                "second-account",
                "must-not-replay",
            ),
        ]));
    let fixture =
        start_runtime_continuation_fixture(backend, "main", &["main", "second"], &[], Vec::new());
    let response = fixture.post_json(
        "backend-api/codex/responses",
        serde_json::json!({
            "model": "gpt-5.4", "stream": true,
            "input": [{"role": "user", "content": "preserve output already sent"}],
        }),
    );
    assert_eq!(response.status().as_u16(), 200);
    let body = response.text().expect("mock SSE response should decode");
    assert!(body.contains("visible-before-error"), "{body}");
    assert!(
        body.contains("server_is_overloaded"),
        "real post-commit failure must pass through: {body}"
    );
    assert!(!body.contains("must-not-replay"), "{body}");
    assert_eq!(fixture.backend.responses_accounts(), ["main-account"]);
}
