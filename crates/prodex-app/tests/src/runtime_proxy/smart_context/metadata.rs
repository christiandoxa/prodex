use super::*;

#[test]
fn smart_context_rewrite_preserves_memory_consolidation_client_metadata() {
    let shared = smart_context_test_shared("memory-consolidation-metadata");
    register_runtime_smart_context_proxy_state(&shared, true, None, None);
    smart_context_observe_minimal_budget(&shared);
    let output = std::iter::once("panic: preserve metadata at src/memory.rs:42:7".to_string())
        .chain((0..500).map(|index| format!("line {index}: repeated memory output")))
        .collect::<Vec<_>>()
        .join("\n");
    let turn_metadata = serde_json::json!({
        "request_kind": "memory",
        "sandbox_mode": "read-only",
        "thread_source": "memory_consolidation",
        "future_metadata": {"preserve": true},
    })
    .to_string();
    let client_metadata = serde_json::json!({
        "x-codex-turn-metadata": turn_metadata,
        "future_client_field": ["preserve", 1491],
    });
    let mut request = smart_context_test_request(serde_json::json!({
        "input": [
            {
                "type": "message",
                "role": "developer",
                "content": [{"type": "input_text", "text": "Client-authored memory policy"}],
            },
            {"type": "function_call_output", "call_id": "call_memory_1", "output": output},
            {"type": "function_call_output", "call_id": "call_memory_2", "output": output},
        ],
        "client_metadata": client_metadata,
        "metadata": {"future_body_metadata": true},
        "future_top_level": {"preserve": true},
    }));
    request
        .headers
        .push(("x-codex-turn-metadata".to_string(), turn_metadata.clone()));

    let rewritten = prepare_runtime_smart_context_http_body(
        1491,
        &request,
        &shared,
        RuntimeRouteKind::Responses,
    )
    .expect("smart context prepare");

    let Cow::Owned(body) = rewritten else {
        panic!("expected smart-context rewrite");
    };
    let value = serde_json::from_slice::<serde_json::Value>(&body).unwrap();
    assert_eq!(value["client_metadata"], client_metadata);
    assert_eq!(
        value["metadata"],
        serde_json::json!({"future_body_metadata": true})
    );
    assert_eq!(
        value["future_top_level"],
        serde_json::json!({"preserve": true})
    );
    assert_eq!(
        request.headers,
        [("x-codex-turn-metadata".to_string(), turn_metadata)]
    );
}
