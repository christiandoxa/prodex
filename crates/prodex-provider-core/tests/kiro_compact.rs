use prodex_provider_core::{
    kiro_provider_core_compact_summary_from_response,
    kiro_provider_core_semantic_compact_instructions,
    kiro_provider_core_semantic_compact_request_body,
};
use serde_json::{Value, json};

#[test]
fn kiro_compact_request_uses_mojo_policy_and_preserves_allowed_fields() {
    let body = serde_json::to_vec(&json!({
        "zeta": {"keep": true},
        "stream": true,
        "store": true,
        "include": ["reasoning.encrypted_content"],
        "previous_response_id": "resp_old",
        "prompt_cache_key": "cache",
        "text": {"format": {"type": "json_schema"}},
        "tool_choice": "auto",
        "tools": [{"type": "function", "name": "shell"}],
        "input": [
            {"type": "message", "role": "user", "content": "summarize"},
            42,
            null
        ]
    }))
    .unwrap();

    let rewritten: Value =
        serde_json::from_slice(&kiro_provider_core_semantic_compact_request_body(&body).unwrap())
            .unwrap();
    assert_eq!(rewritten["zeta"]["keep"], true);
    assert_eq!(rewritten["stream"], false);
    assert_eq!(rewritten["store"], false);
    for key in [
        "include",
        "previous_response_id",
        "prompt_cache_key",
        "text",
        "tool_choice",
        "tools",
    ] {
        assert!(rewritten.get(key).is_none(), "{key} should be removed");
    }
    assert_eq!(rewritten["input"].as_array().unwrap().len(), 4);
    assert_eq!(rewritten["input"][1], 42);
    assert!(rewritten["input"][2].is_null());
    assert_eq!(
        rewritten["input"][3]["content"][0]["text"],
        "Compact the supplied coding-agent transcript into one durable continuation summary. Preserve the user's goals, repository instructions, decisions, files changed, exact identifiers, commands and test results, unresolved failures, current worktree state, and the next concrete steps. Remove redundant narration and obsolete intermediate reasoning. Do not call tools. Return only the continuation summary, with no preamble or completion claim."
    );
    assert_eq!(
        kiro_provider_core_semantic_compact_instructions(),
        rewritten["input"][3]["content"][0]["text"]
    );

    let defaults: Value = serde_json::from_slice(
        &kiro_provider_core_semantic_compact_request_body(br#"{"input":[]}"#).unwrap(),
    )
    .unwrap();
    assert_eq!(defaults["stream"], false);
    assert_eq!(defaults["store"], false);

    let escaped_keys: Value = serde_json::from_slice(
        &kiro_provider_core_semantic_compact_request_body(
            br#"{"in\u0070ut":[],"str\u0065am":true}"#,
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(escaped_keys["stream"], false);
    assert_eq!(escaped_keys["input"].as_array().unwrap().len(), 1);
}

#[test]
fn kiro_compact_request_rejects_invalid_json_and_schema() {
    assert!(
        kiro_provider_core_semantic_compact_request_body(b"not json")
            .unwrap_err()
            .starts_with("failed to parse Kiro compact request JSON:")
    );
    for invalid in [br#"{"input":[x]}"#.as_slice(), br#"{"input":[]} trailing"#] {
        assert!(
            kiro_provider_core_semantic_compact_request_body(invalid)
                .unwrap_err()
                .starts_with("failed to parse Kiro compact request JSON:")
        );
    }
    assert_eq!(
        kiro_provider_core_semantic_compact_request_body(b"[]").unwrap_err(),
        "Kiro compact request must be a JSON object"
    );
    assert_eq!(
        kiro_provider_core_semantic_compact_request_body(br#"{"input":{}}"#).unwrap_err(),
        "Kiro compact request must contain an input array"
    );
}

#[test]
fn kiro_compact_summary_uses_first_message_and_string_text_only() {
    let response = json!({
        "output": [
            {"type": "reasoning", "summary": [{"text": "ignore"}]},
            {"type": "message", "content": [
                {"type": "output_text", "text": 7},
                {"type": "output_text", "text": "  Keep this\u{2003} "}
            ]},
            {"type": "message", "content": [{"text": "later message"}]}
        ]
    });

    assert_eq!(
        kiro_provider_core_compact_summary_from_response(&response).unwrap(),
        "Keep this"
    );
    assert_eq!(
        kiro_provider_core_compact_summary_from_response(&json!({"output": []})).unwrap_err(),
        "Kiro compact response returned no summary text"
    );
    assert_eq!(
        kiro_provider_core_compact_summary_from_response(&json!({})).unwrap_err(),
        "Kiro compact response is missing output"
    );
    assert_eq!(
        kiro_provider_core_compact_summary_from_response(&json!({
            "output": [{
                "type": "message",
                "content": [{"text": "  "}, {"text": "later text"}]
            }]
        }))
        .unwrap_err(),
        "Kiro compact response returned no summary text"
    );
}
