#[test]
fn runtime_proxy_http_precommit_transport_rotates_fresh_request() {
    let fixture = start_runtime_continuation_fixture(
        RuntimeProxyBackend::start_http_reset_before_first_byte(),
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
                "content": "continue after the first upstream disconnects",
            }],
        }),
    );

    assert_eq!(response.status().as_u16(), 200);
    let body = response.text().expect("responses body should decode");
    assert!(body.contains("\"id\":\"resp-second\""), "{body}");
    assert_eq!(
        fixture.backend.responses_accounts(),
        vec!["main-account".to_string(), "second-account".to_string()]
    );
    let log = fixture.wait_for_log(|log| {
        log.contains("responses_transport_failure profile=main")
            && log.contains("committed profile=second")
    });
    assert!(log.contains("hard_affinity=false"), "{log}");
}

#[test]
fn runtime_proxy_http_precommit_transport_rotates_fresh_sse_close() {
    let fixture = start_runtime_continuation_fixture(
        RuntimeProxyBackend::start_http_reset_after_headers(),
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
                "content": "continue after the first SSE closes before output",
            }],
        }),
    );

    assert_eq!(response.status().as_u16(), 200);
    let body = response.text().expect("responses body should decode");
    assert!(body.contains("\"id\":\"resp-second\""), "{body}");
    assert_eq!(
        fixture.backend.responses_accounts(),
        vec!["main-account".to_string(), "second-account".to_string()]
    );
    let log = fixture.wait_for_log(|log| {
        log.contains("responses_transport_failure profile=main")
            && log.contains("stage=responses_sse_lookahead")
            && log.contains("committed profile=second")
    });
    assert!(log.contains("hard_affinity=false"), "{log}");
}

#[test]
fn runtime_proxy_http_precommit_transport_returns_503_after_pool_exhaustion() {
    let fixture = start_runtime_continuation_fixture(
        RuntimeProxyBackend::start_http_reset_before_first_byte(),
        "main",
        &["main"],
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
                "content": "fail only after every usable profile is tried",
            }],
        }),
    );

    assert_eq!(response.status().as_u16(), 503);
    let body = response.text().expect("error body should decode");
    assert!(body.contains("service_unavailable"), "{body}");
    assert_eq!(
        fixture.backend.responses_accounts(),
        vec!["main-account".to_string()]
    );
}

#[test]
fn runtime_proxy_http_precommit_transport_tries_every_profile_before_503() {
    let fixture = start_runtime_continuation_fixture(
        RuntimeProxyBackend::start_http_reset_before_first_byte_all(),
        "main",
        &["main", "second", "third"],
        &[],
        Vec::new(),
    );

    let response = fixture.post_json(
        "backend-api/codex/responses",
        serde_json::json!({
            "model": "gpt-5.4",
            "input": [{"role": "user", "content": "try every account before failing"}],
        }),
    );

    assert_eq!(response.status().as_u16(), 503);
    assert!(
        response
            .text()
            .expect("error body should decode")
            .contains("service_unavailable")
    );
    let accounts = fixture.backend.responses_accounts();
    let mut sorted = accounts.clone();
    sorted.sort();
    assert_eq!(
        sorted,
        ["main-account", "second-account", "third-account"],
        "every eligible account must receive exactly one pre-commit attempt: {accounts:?}"
    );
}

#[test]
fn runtime_proxy_http_waits_for_transient_profiles_then_starts_a_new_sweep() {
    let backend = RuntimeProxyBackend::start_with_fault_script(
        RuntimeProxyBackendFaultScript::new([
            RuntimeProxyBackendFaultStep::overloaded_503(
                RuntimeProxyBackendFaultRoute::Responses,
                "main-account",
            ),
            RuntimeProxyBackendFaultStep::overloaded_503(
                RuntimeProxyBackendFaultRoute::Responses,
                "second-account",
            ),
            RuntimeProxyBackendFaultStep::success(
                RuntimeProxyBackendFaultRoute::Responses,
                "main-account",
            ),
        ]),
    );
    let fixture = start_runtime_continuation_fixture(
        backend,
        "main",
        &["main", "second"],
        &[],
        Vec::new(),
    );

    let response = fixture.post_json(
        "backend-api/codex/responses",
        serde_json::json!({
            "model": "gpt-5.4",
            "input": [{"role": "user", "content": "recover after transient overloads"}],
        }),
    );

    let response_status = response.status().as_u16();
    let response_body = response.text().expect("response body should decode");
    let debug_log = fixture.wait_for_log(|log| log.contains("request="));
    assert_eq!(response_status, 200, "body={response_body} log={debug_log}");
    assert!(
        response_body.contains("\"id\":\"scripted-success\""),
        "body={response_body} log={debug_log}"
    );
    assert_eq!(
        fixture.backend.responses_accounts(),
        vec![
            "main-account".to_string(),
            "second-account".to_string(),
            "main-account".to_string(),
        ]
    );
    let log = fixture.wait_for_log(|log| {
        log.contains("rotation_waiting_for_recovery")
            && log.contains("rotation_sweep_start")
            && log.contains("committed profile=main")
    });
    assert!(log.contains("sweep=1"), "{log}");
}

#[test]
fn runtime_proxy_http_quota_tries_every_profile_before_final_429() {
    let backend = RuntimeProxyBackend::start_with_fault_script(RuntimeProxyBackendFaultScript::new(
        ["main-account", "second-account", "third-account"].map(|account| {
            RuntimeProxyBackendFaultStep::explicit_quota_429(
                RuntimeProxyBackendFaultRoute::Responses,
                account,
            )
        }),
    ));
    let fixture = start_runtime_continuation_fixture(
        backend,
        "main",
        &["main", "second", "third"],
        &[],
        Vec::new(),
    );

    let response = fixture.post_json(
        "backend-api/codex/responses",
        serde_json::json!({
            "model": "gpt-5.4",
            "input": [{"role": "user", "content": "drain the eligible quota pool"}],
        }),
    );

    assert_eq!(response.status().as_u16(), 429);
    let body = response.text().expect("quota body should decode");
    assert!(body.contains("insufficient_quota"), "{body}");
    assert!(!body.contains("service_unavailable"), "{body}");
    let accounts = fixture.backend.responses_accounts();
    let mut sorted = accounts.clone();
    sorted.sort();
    assert_eq!(
        sorted,
        ["main-account", "second-account", "third-account"],
        "every eligible account must be exhausted before quota is surfaced: {accounts:?}"
    );
}

#[test]
fn runtime_proxy_http_precommit_transport_keeps_previous_response_owner() {
    let fixture = start_runtime_continuation_fixture(
        RuntimeProxyBackend::start_http_reset_before_first_byte(),
        "main",
        &["main", "second"],
        &[("resp-main", "main")],
        Vec::new(),
    );

    let response = fixture.post_json(
        "backend-api/codex/responses",
        serde_json::json!({
            "previous_response_id": "resp-main",
            "input": [{
                "type": "message",
                "role": "user",
                "content": "preserve the owning response chain",
            }],
        }),
    );

    assert_eq!(response.status().as_u16(), 503);
    let body = response.text().expect("error body should decode");
    assert!(body.contains("service_unavailable"), "{body}");
    assert_eq!(
        fixture.backend.responses_accounts(),
        vec!["main-account".to_string()],
        "hard previous-response affinity must not rotate to another profile"
    );
    let log = fixture.wait_for_log(|log| {
        log.contains("responses_transport_failure profile=main")
            && log.contains("hard_affinity=true")
    });
    assert!(log.contains("hard_affinity=true"), "{log}");
}

#[test]
fn runtime_proxy_http_precommit_transport_keeps_unbound_turn_state_on_current_profile() {
    let fixture = start_runtime_continuation_fixture(
        RuntimeProxyBackend::start_http_reset_before_first_byte(),
        "main",
        &["main", "second"],
        &[],
        Vec::new(),
    );

    let response = fixture.post_json_with_headers(
        "backend-api/codex/responses",
        &[runtime_continuation_header(
            "x-codex-turn-state",
            "turn-unbound",
        )],
        serde_json::json!({"input": []}),
    );

    assert_eq!(response.status().as_u16(), 503);
    assert_eq!(
        fixture.backend.responses_accounts(),
        vec!["main-account".to_string()],
        "an opaque turn-state token must never cross account identities"
    );
}

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
    let responses_accounts = fixture.backend.responses_accounts();
    assert!(
        body.contains("\"id\":\"resp-third\""),
        "healthy later profile should complete the request: {body}; accounts={responses_accounts:?}"
    );
    assert!(
        !body.contains("usage limit") && !body.contains("service_unavailable"),
        "retryable usage-limit failures must not leak once a later profile succeeds: {body}"
    );
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
fn runtime_proxy_http_transport_backoff_rotation_rebinds_soft_session() {
    let fixture = start_runtime_continuation_fixture(
        RuntimeProxyBackend::start(),
        "main",
        &["main", "second"],
        &[],
        vec![("sess-transport-rotation".to_string(), "main")],
    )
    .restart_with_transport_backoff("main", RuntimeRouteKind::Responses);

    let response = fixture.post_json(
        "backend-api/codex/responses",
        serde_json::json!({
            "session_id": "sess-transport-rotation",
            "input": [{
                "type": "message",
                "role": "user",
                "content": "resume after the original owner entered transport backoff",
            }],
        }),
    );
    assert_eq!(response.status().as_u16(), 200);
    assert!(
        response
            .text()
            .expect("resume body should decode")
            .contains("\"id\":\"resp-second\"")
    );

    let continuations = wait_for_runtime_continuations(&fixture.paths, |continuations| {
        continuations
            .session_profile_bindings
            .get("sess-transport-rotation")
            .is_some_and(|binding| binding.profile_name == "second")
    });
    assert_eq!(
        continuations.session_profile_bindings["sess-transport-rotation"].profile_name,
        "second"
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
fn runtime_proxy_http_restart_recovers_previous_response_affinity_from_journal() {
    let now = Local::now().timestamp();
    let fixture = start_runtime_continuation_fixture(
        RuntimeProxyBackend::start(),
        "main",
        &["main", "second"],
        &[],
        Vec::new(),
    )
    .restart_with_journal_continuations(RuntimeContinuationStore {
        response_profile_bindings: BTreeMap::from([(
            "resp-second".to_string(),
            ResponseProfileBinding {
                binding_identity: None,
                profile_name: "second".to_string(),
                bound_at: now,
            },
        )]),
        ..RuntimeContinuationStore::default()
    });

    let response = fixture.post_json(
        "backend-api/codex/responses",
        serde_json::json!({
            "previous_response_id": "resp-second",
            "input": [{
                "type": "message",
                "role": "user",
                "content": [{"type": "input_text", "text": "resume after restart"}],
            }],
        }),
    );

    assert_eq!(response.status().as_u16(), 200);
    assert_eq!(
        fixture.backend.responses_accounts(),
        vec!["second-account".to_string()],
        "journal-only previous-response affinity must survive restart"
    );
}
