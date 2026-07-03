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
    let responses_accounts = fixture.backend.responses_accounts();
    assert_eq!(
        responses_accounts.first().map(String::as_str),
        Some("fifth-account"),
        "fresh rotation should try the current profile first: {responses_accounts:?}"
    );
    assert_eq!(
        responses_accounts.last().map(String::as_str),
        Some("third-account"),
        "runtime proxy should keep rotating until the later healthy profile is tried: {responses_accounts:?}"
    );
    let mut sorted_responses_accounts = responses_accounts.clone();
    sorted_responses_accounts.sort();
    sorted_responses_accounts.dedup();
    assert_eq!(
        sorted_responses_accounts.len(),
        responses_accounts.len(),
        "fresh rotation should not retry the same usage-limit account before success: {responses_accounts:?}"
    );
    assert!(
        responses_accounts.len() >= 3,
        "fresh rotation should cross at least one usage-limit account before success: {responses_accounts:?}"
    );
    assert!(
        responses_accounts.iter().all(|account| matches!(
            account.as_str(),
            "fifth-account" | "fourth-account" | "main-account" | "second-account" | "third-account"
        )),
        "fresh rotation should stay within the fixture profile pool: {responses_accounts:?}"
    );
}

#[test]
fn runtime_proxy_http_fresh_sse_quota_after_output_item_added_rotates_before_model_output() {
    let fixture = start_runtime_continuation_fixture(
        RuntimeProxyBackend::start_http_delayed_quota_after_output_item_added(),
        "main",
        &["main", "second"],
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
                    "text": "retry before any model text is emitted",
                }],
            }],
        }),
    );

    assert_eq!(response.status().as_u16(), 200);
    let body = read_runtime_http_stream_until(response, |body| {
        body.contains("\"id\":\"resp-second\"") || body.contains("usage limit")
    });
    assert!(
        body.contains("\"id\":\"resp-second\""),
        "later healthy profile should complete the SSE request: {body}"
    );
    assert!(
        !body.contains("usage limit"),
        "quota failure after response.output_item.added must stay pre-commit and not leak: {body}"
    );

    let responses_accounts = fixture.backend.responses_accounts();
    assert_eq!(
        responses_accounts,
        vec!["main-account".to_string(), "second-account".to_string()],
        "fresh SSE request should retry on the ready profile after pre-output quota failure"
    );

    let log = fixture.wait_for_log(|log| {
        log.contains("sse_quota_blocked profile=main") && log.contains("committed profile=second")
    });
    assert!(
        log.contains("sse_quota_blocked profile=main")
            && log.contains("transport=http committed profile=second"),
        "delayed quota before output text should be classified pre-commit and rotate: {log}"
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
