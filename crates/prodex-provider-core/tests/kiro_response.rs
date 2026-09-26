//! Kiro response translation regression tests.

use prodex_provider_core::{
    ProviderEndpoint, ProviderId, ProviderTransformInput, TransformStatus,
    kiro_provider_core_chat_completion_finish_reason_from_response,
    kiro_provider_core_chat_completion_value_from_response,
    kiro_provider_core_response_has_tool_calls, provider_implementation_registry,
};
use serde_json::{Value, json};

#[test]
fn kiro_provider_core_maps_responses_value_to_chat_completion() {
    let response = json!({
        "id": "resp_kiro_1",
        "created_at": 123,
        "model": "claude-sonnet-4",
        "output": [
            {
                "type": "message",
                "content": [{"type": "output_text", "text": "hello"}]
            },
            {
                "type": "function_call",
                "call_id": "call_1",
                "name": "run",
                "arguments": "{}"
            }
        ],
        "metadata": {"kiro": {"reasoning_content": "thoughts"}}
    });
    let transformed = provider_implementation_registry()
        .get(ProviderId::Kiro)
        .expect("Kiro registration")
        .translator()
        .transform_response(ProviderTransformInput::new(
            ProviderEndpoint::ChatCompletions,
            serde_json::to_vec(&response).unwrap(),
        ));
    let body = transformed
        .body
        .expect("Kiro chat response should translate");
    let completion: Value = serde_json::from_slice(&body).expect("translated JSON is valid");

    assert_eq!(
        completion,
        json!({
            "id": "chatcmpl_resp_kiro_1",
            "object": "chat.completion",
            "created": 123,
            "model": "claude-sonnet-4",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": "hello",
                    "tool_calls": [{
                        "id": "call_1",
                        "type": "function",
                        "function": {"name": "run", "arguments": "{}"},
                    }],
                    "reasoning_content": "thoughts",
                },
                "finish_reason": "tool_calls",
            }],
            "metadata": {"kiro": {"reasoning_content": "thoughts"}},
        })
    );
}

#[test]
fn kiro_provider_core_ignores_non_string_reasoning_content() {
    assert_eq!(
        kiro_provider_core_chat_completion_value_from_response(
            &json!({"metadata": {"kiro": {"reasoning_content": 42}}}),
            0,
        ),
        json!({
            "id": "chatcmpl_kiro_0",
            "object": "chat.completion",
            "created": 0,
            "model": "kiro-cli",
            "choices": [{
                "index": 0,
                "message": {"role": "assistant", "content": ""},
                "finish_reason": "stop",
            }],
            "metadata": {"kiro": {"reasoning_content": 42}},
        })
    );
}

#[test]
fn kiro_provider_core_uses_only_the_first_output_message() {
    let response = json!({
        "output": [
            {"type": "message", "content": []},
            {"type": "message", "content": [{"text": "later"}]},
        ]
    });

    assert_eq!(
        kiro_provider_core_chat_completion_value_from_response(&response, 0)["choices"][0]["message"]
            ["content"],
        ""
    );
}

#[test]
fn kiro_provider_core_maps_failed_response_to_chat_refusal() {
    assert_eq!(
        kiro_provider_core_chat_completion_value_from_response(
            &json!({"status": "failed", "error": null}),
            9,
        ),
        json!({
            "id": "chatcmpl_kiro_9",
            "object": "chat.completion",
            "created": 0,
            "model": "kiro-cli",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": "",
                    "refusal": "Kiro request failed",
                },
                "finish_reason": "stop",
            }],
        })
    );
    assert_eq!(
        kiro_provider_core_chat_completion_value_from_response(
            &json!({
                "id": "resp_error",
                "status": "failed",
                "error": {"message": {"code": "failure"}},
            }),
            10,
        ),
        json!({
            "id": "chatcmpl_resp_error",
            "object": "chat.completion",
            "created": 0,
            "model": "kiro-cli",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": "",
                    "refusal": {"code": "failure"},
                },
                "finish_reason": "stop",
            }],
        })
    );
}

#[test]
fn kiro_provider_core_maps_chat_response_above_request_limit() {
    let text = "x".repeat(5 * 1024 * 1024);
    let response = json!({
        "id": "resp_large",
        "output": [{"type": "message", "content": [{"text": text}]}],
    });
    let completion = kiro_provider_core_chat_completion_value_from_response(&response, 0);

    assert_eq!(completion["id"], "chatcmpl_resp_large");
    assert_eq!(
        completion["choices"][0]["message"]["content"]
            .as_str()
            .unwrap()
            .len(),
        5 * 1024 * 1024
    );
}

#[test]
fn kiro_provider_core_rejects_chat_response_above_safe_limit_before_parsing() {
    let oversized = vec![b' '; prodex_mojo_core::rich::KIRO_RESPONSE_MAX_BYTES + 1];
    let result = provider_implementation_registry()
        .get(ProviderId::Kiro)
        .expect("Kiro registration")
        .translator()
        .transform_response(ProviderTransformInput::new(
            ProviderEndpoint::ChatCompletions,
            oversized,
        ));

    assert!(matches!(result.status(), TransformStatus::Rejected { .. }));
    assert_eq!(result.metadata["error_code"], "response_too_large");
}

#[test]
fn kiro_provider_core_detects_response_tool_calls() {
    let tool_response = json!({
        "output": [
            {"type": "message"},
            {"type": "function_call", "call_id": "call_1"},
        ]
    });
    assert!(kiro_provider_core_response_has_tool_calls(&tool_response));
    assert_eq!(
        kiro_provider_core_chat_completion_finish_reason_from_response(&tool_response),
        "tool_calls"
    );

    let length_response = json!({
        "output": [{"type": "message"}],
        "incomplete_details": {"reason": "max_output_tokens"},
    });
    assert_eq!(
        kiro_provider_core_chat_completion_finish_reason_from_response(&length_response),
        "length"
    );
    assert!(!kiro_provider_core_response_has_tool_calls(&json!({
        "output": [{"type": "message"}]
    })));
    assert_eq!(
        kiro_provider_core_chat_completion_finish_reason_from_response(&json!({})),
        "stop"
    );
    assert!(!kiro_provider_core_response_has_tool_calls(&json!({})));
}
