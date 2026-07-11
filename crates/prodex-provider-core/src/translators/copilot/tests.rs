//! Copilot provider-core characterization tests.

use super::{
    copilot_provider_core_request_body_with_canonical_model,
    copilot_provider_core_request_body_without_encrypted_content,
    copilot_provider_core_request_has_agent_input, copilot_provider_core_request_has_vision_input,
    copilot_provider_core_response_id_from_value,
};
use serde_json::{Value, json};

#[test]
fn copilot_provider_core_extracts_response_id_shapes() {
    assert_eq!(
        copilot_provider_core_response_id_from_value(&json!({
            "response": {"id": " resp_nested "}
        }))
        .as_deref(),
        Some("resp_nested")
    );
    assert_eq!(
        copilot_provider_core_response_id_from_value(&json!({"id": "resp_top"})).as_deref(),
        Some("resp_top")
    );
    assert_eq!(
        copilot_provider_core_response_id_from_value(&json!({"response_id": "resp_event"}))
            .as_deref(),
        Some("resp_event")
    );
    assert_eq!(
        copilot_provider_core_response_id_from_value(&json!({"id": "   "})),
        None
    );
}

#[test]
fn copilot_provider_core_rewrites_request_to_canonical_model() {
    let rewritten = copilot_provider_core_request_body_with_canonical_model(
        br#"{"model":"codex","messages":[{"role":"user","content":"hi"}]}"#,
    );
    let value: Value = serde_json::from_slice(&rewritten).unwrap();

    assert_eq!(value["model"], "gpt-5.3-codex");
}

#[test]
fn copilot_provider_core_strips_encrypted_content_only() {
    let body = serde_json::to_vec(&json!({
        "messages": [{
            "role": "assistant",
            "content": [{"type": "text", "text": "keep", "encrypted_content": "qAAA.VDK="}]
        }],
        "reasoning": {"encrypted_content": "qBBB.VDK=", "summary": "keep"}
    }))
    .unwrap();

    let (stripped, changed) = copilot_provider_core_request_body_without_encrypted_content(&body);
    let value: Value = serde_json::from_slice(&stripped).unwrap();

    assert!(changed);
    assert!(!value.to_string().contains("encrypted_content"));
    assert_eq!(value["messages"][0]["content"][0]["text"], "keep");
    assert_eq!(value["reasoning"]["summary"], "keep");
}

#[test]
fn copilot_provider_core_detects_agent_and_vision_input() {
    let agent = serde_json::to_vec(&json!({
        "messages": [{"role": "assistant", "content": "hello"}]
    }))
    .unwrap();
    let user = serde_json::to_vec(&json!({
        "messages": [{"role": "user", "content": "hello"}]
    }))
    .unwrap();
    let vision = serde_json::to_vec(&json!({
        "input": [{"content": [{"type": "input_image"}]}]
    }))
    .unwrap();

    assert!(copilot_provider_core_request_has_agent_input(&agent));
    assert!(!copilot_provider_core_request_has_agent_input(&user));
    assert!(copilot_provider_core_request_has_vision_input(&vision));
}
