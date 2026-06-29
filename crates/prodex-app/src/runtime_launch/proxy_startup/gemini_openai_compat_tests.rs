use super::deepseek_rewrite::{
    RuntimeDeepSeekConversationStore, RuntimeDeepSeekRewriteOptions,
    runtime_chat_compatible_request_body, runtime_deepseek_responses_value_from_chat_value,
};
use super::provider_bridge::RuntimeProviderBridgeKind;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

fn conversation_store() -> RuntimeDeepSeekConversationStore {
    Arc::new(Mutex::new(BTreeMap::new()))
}

#[test]
fn gemini_openai_chat_request_preserves_documented_reasoning_effort() {
    let request = serde_json::json!({
        "model": "gemini-3.5-flash",
        "stream": true,
        "input": "hello",
        "reasoning": {"effort": "medium"}
    });

    let translated = runtime_chat_compatible_request_body(
        &serde_json::to_vec(&request).unwrap(),
        &conversation_store(),
        RuntimeProviderBridgeKind::Gemini,
        "gemini-3.5-flash",
        true,
        RuntimeDeepSeekRewriteOptions::default(),
    )
    .expect("Gemini OpenAI-compatible request should translate");
    let body: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();

    assert_eq!(body["reasoning_effort"], "medium");
    assert!(body.get("thinking").is_none());
    assert_eq!(body["stream"], true);
}

#[test]
fn gemini_openai_chat_request_maps_thought_signature_to_extra_content() {
    let request = serde_json::json!({
        "model": "gemini-3.5-flash",
        "input": [{
            "type": "function_call",
            "call_id": "call_1",
            "name": "shell",
            "arguments": "{\"cmd\":\"pwd\"}",
            "gemini_thought_signature": "signature-a"
        }, {
            "type": "function_call_output",
            "call_id": "call_1",
            "output": "ok"
        }]
    });

    let translated = runtime_chat_compatible_request_body(
        &serde_json::to_vec(&request).unwrap(),
        &conversation_store(),
        RuntimeProviderBridgeKind::Gemini,
        "gemini-3.5-flash",
        true,
        RuntimeDeepSeekRewriteOptions::default(),
    )
    .unwrap();
    let body: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();

    assert_eq!(
        body["messages"][0]["tool_calls"][0]["extra_content"]["google"]["thought_signature"],
        "signature-a"
    );
    assert!(
        body["messages"][0]["tool_calls"][0]
            .get("gemini_thought_signature")
            .is_none()
    );
}

#[test]
fn gemini_openai_chat_response_exposes_thought_signature_to_codex() {
    let response = serde_json::json!({
        "id": "chatcmpl_1",
        "model": "gemini-3.5-flash",
        "choices": [{
            "message": {
                "role": "assistant",
                "tool_calls": [{
                    "id": "call_1",
                    "type": "function",
                    "extra_content": {
                        "google": {
                            "thought_signature": "signature-a"
                        }
                    },
                    "function": {
                        "name": "shell",
                        "arguments": "{\"cmd\":\"pwd\"}"
                    }
                }]
            }
        }]
    });

    let responses = runtime_deepseek_responses_value_from_chat_value(&response, 1);

    assert_eq!(
        responses["output"][0]["gemini_thought_signature"],
        "signature-a"
    );
}
