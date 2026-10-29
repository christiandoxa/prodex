use super::*;

#[test]
fn runtime_proxy_guardian_http_prefers_parent_response_profile() {
    let _test_guard = crate::acquire_test_runtime_lock();
    let fixture = start_runtime_continuation_fixture(
        RuntimeProxyBackend::start(), "main", &["main", "second"],
        &[("resp-parent", "second")], Vec::new(),
    );
    let response = fixture.post_json_with_headers(
        "backend-api/codex/responses",
        &[runtime_continuation_header("x-codex-guardian", "reviewer")],
        serde_json::json!({"input": [], "client_metadata": {"parent_response_id": "resp-parent"}}),
    );
    let _ = response.text().expect("response body");
    assert_eq!(fixture.backend.responses_accounts(), ["second-account"]);
}

#[test]
fn runtime_proxy_guardian_http_quota_does_not_rotate_away_from_parent() {
    let _test_guard = crate::acquire_test_runtime_lock();
    let backend = RuntimeProxyBackend::start_with_fault_script(RuntimeProxyBackendFaultScript::new([
        RuntimeProxyBackendFaultStep::explicit_quota_429(RuntimeProxyBackendFaultRoute::Responses, "second-account"),
        RuntimeProxyBackendFaultStep::success(RuntimeProxyBackendFaultRoute::Responses, "main-account"),
    ]));
    let fixture = start_runtime_continuation_fixture(backend, "main", &["main", "second"], &[("resp-parent", "second")], Vec::new());
    let response = fixture.post_json_with_headers(
        "backend-api/codex/responses",
        &[runtime_continuation_header("x-codex-guardian", "reviewer")],
        serde_json::json!({"input": [], "client_metadata": {"parent_response_id": "resp-parent"}}),
    );
    assert_eq!(response.status().as_u16(), 429);
    assert_eq!(fixture.backend.responses_accounts(), ["second-account"]);
}

#[test]
fn runtime_proxy_guardian_http_previous_response_keeps_its_owner() {
    let _test_guard = crate::acquire_test_runtime_lock();
    let fixture = start_runtime_continuation_fixture(
        RuntimeProxyBackend::start(), "main", &["main", "second"],
        &[("resp-parent", "second"), ("resp-previous", "main")], Vec::new(),
    );
    let response = fixture.post_json_with_headers(
        "backend-api/codex/responses",
        &[runtime_continuation_header("x-codex-guardian", "reviewer")],
        serde_json::json!({"previous_response_id": "resp-previous", "input": [], "client_metadata": {"parent_response_id": "resp-parent"}}),
    );
    let _ = response.text().expect("response body");
    assert_eq!(fixture.backend.responses_accounts().first().map(String::as_str), Some("main-account"));
}

#[test]
fn runtime_proxy_policy_capacity_503_is_forwarded_without_profile_retry() {
    let _test_guard = crate::acquire_test_runtime_lock();
    for (route, path) in [
        (RuntimeProxyBackendFaultRoute::Responses, "backend-api/codex/responses"),
        (RuntimeProxyBackendFaultRoute::Compact, "backend-api/codex/responses/compact"),
    ] {
        let backend = RuntimeProxyBackend::start_with_fault_script(RuntimeProxyBackendFaultScript::new([
            RuntimeProxyBackendFaultStep::policy_capacity_503(route, "main-account"),
            RuntimeProxyBackendFaultStep::success(route, "main-account"),
        ]));
        let fixture = start_runtime_continuation_fixture(
            backend,
            "main",
            &["main", "second"],
            &[],
            Vec::new(),
        );
        let response = fixture.post_json(path, serde_json::json!({"input": []}));
        assert_eq!(response.status().as_u16(), 503);
        assert_eq!(
            response.json::<serde_json::Value>().expect("policy response JSON"),
            serde_json::json!({"error": {
                "code": "policy_capacity_unavailable",
                "message": "This model is disabled."
            }})
        );
        assert_eq!(fixture.backend.responses_accounts(), ["main-account"]);
    }
}

#[test]
fn runtime_proxy_http_compact_preserves_rich_codex_01491_payload_and_session_affinity() {
    let session_id = "sess-compact-01491";
    let fixture = start_runtime_continuation_fixture(
        RuntimeProxyBackend::start(),
        "main",
        &["main", "second"],
        &[],
        vec![(runtime_compact_session_lineage_key(session_id), "second")],
    );
    let turn_metadata = serde_json::json!({
        "request_kind": "compaction",
        "session_id": session_id,
        "thread_source": "automated_review",
        "context_window_id": "ctx-compact-before",
        "future_metadata": {"preserve": true},
    })
    .to_string();
    let body = serde_json::json!({
        "model": "gpt-5.6-luna",
        "session_id": session_id,
        "context_window_id": "ctx-compact-before",
        "instructions": "Compact without changing multimodal structures.",
        "input": [
            {
                "type": "message",
                "role": "developer",
                "content": [{"type": "input_text", "text": "Client-authored developer policy"}],
                "metadata": {"client_authored": true},
                "future_message_field": "preserve",
            },
            {
                "type": "message",
                "role": "user",
                "content": [
                    {"type": "input_text", "text": "Receipt image:"},
                    {
                        "type": "input_image",
                        "image_url": "data:image/png;base64,AA==",
                        "detail": "high",
                        "future_image_field": 1491,
                    },
                    {"type": "input_text", "text": "Adjacent image label"},
                    {
                        "type": "input_audio",
                        "audio_url": "data:audio/wav;base64,AA==",
                        "format": "wav",
                    },
                    {"type": "output_text", "text": "Preserved text part"},
                ],
            },
        ],
        "client_metadata": {
            "x-codex-turn-metadata": turn_metadata,
            "context_window_id": "ctx-compact-before",
            "future_client_field": ["preserve", 1491],
        },
        "metadata": {"trace": "synthetic"},
        "future_top_level": {"preserve": true},
    });

    let response = fixture.post_json_with_headers(
        "backend-api/codex/responses/compact",
        &[
            runtime_continuation_header("session_id", session_id),
            runtime_continuation_header("x-codex-turn-metadata", turn_metadata.clone()),
        ],
        body.clone(),
    );

    assert_eq!(response.status().as_u16(), 200);
    assert_eq!(
        fixture.backend.responses_accounts(),
        ["second-account"],
        "session-scoped compact must use its owning profile"
    );
    assert_eq!(fixture.backend.responses_bodies(), [body.to_string()]);
    let headers = fixture.backend.responses_headers();
    assert_eq!(headers[0].get("session_id").map(String::as_str), Some(session_id));
    assert_eq!(
        headers[0].get("x-codex-turn-metadata").map(String::as_str),
        Some(turn_metadata.as_str())
    );
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&fixture.backend.responses_bodies()[0])
            .expect("compact body should remain JSON")["context_window_id"],
        "ctx-compact-before"
    );
}

#[test]
fn runtime_proxy_http_compact_quota_tries_every_profile_before_final_429() {
    let backend = RuntimeProxyBackend::start_with_fault_script(RuntimeProxyBackendFaultScript::new(
        ["main-account", "second-account", "third-account"].map(|account| {
            RuntimeProxyBackendFaultStep::explicit_quota_429(
                RuntimeProxyBackendFaultRoute::Compact,
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
        "backend-api/codex/responses/compact",
        serde_json::json!({"input": [], "instructions": "compact"}),
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
        "compact must exhaust every eligible account before surfacing quota: {accounts:?}"
    );
}
