use super::*;

#[test]
fn runtime_proxy_http_memory_consolidation_metadata_survives_precommit_rotation() {
    let fixture = start_runtime_continuation_fixture(
        RuntimeProxyBackend::start_http_usage_limit_message(),
        "main",
        &["main", "second"],
        &[],
        Vec::new(),
    );
    let turn_metadata = serde_json::json!({
        "request_kind": "memory",
        "sandbox_mode": "read-only",
        "thread_source": "memory_consolidation",
        "future_metadata": {"preserve": true},
    })
    .to_string();
    let body = serde_json::json!({
        "model": "gpt-5.6-luna",
        "input": [{
            "type": "message",
            "role": "developer",
            "content": [{"type": "input_text", "text": "Consolidate detached memory."}],
        }],
        "client_metadata": {
            "x-codex-turn-metadata": turn_metadata,
            "request_origin": "detached_memory",
            "future_client_field": {"preserve": true},
        },
        "future_body_field": ["preserve", 1491],
    });

    let response = fixture.post_json_with_headers(
        "backend-api/codex/responses",
        &[runtime_continuation_header(
            "x-codex-turn-metadata",
            turn_metadata.clone(),
        )],
        body.clone(),
    );

    assert_eq!(response.status().as_u16(), 200);
    assert!(
        response
            .text()
            .expect("responses body should decode")
            .contains("\"id\":\"resp-second\"")
    );
    assert_eq!(
        fixture.backend.responses_accounts(),
        ["main-account", "second-account"]
    );
    assert_eq!(
        fixture
            .backend
            .responses_headers()
            .iter()
            .map(|headers| headers.get("x-codex-turn-metadata").map(String::as_str))
            .collect::<Vec<_>>(),
        [Some(turn_metadata.as_str()), Some(turn_metadata.as_str())]
    );
    assert_eq!(
        fixture
            .backend
            .responses_bodies()
            .iter()
            .map(|body| serde_json::from_str::<serde_json::Value>(body).unwrap())
            .collect::<Vec<_>>(),
        [body.clone(), body]
    );
}
