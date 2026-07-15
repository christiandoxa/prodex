//! Kiro request ownership characterization tests.

use super::*;
use serde_json::{Value, json};

#[test]
fn kiro_provider_core_preserves_chat_control_failure_matrix() {
    let defaults = kiro_provider_core_chat_completions_request_body(
        &serde_json::to_vec(&json!({
            "messages": [{"role": "user", "content": "hello"}],
            "stop": [],
            "temperature": 1,
            "top_p": 1,
            "presence_penalty": 0,
            "frequency_penalty": 0,
            "parallel_tool_calls": true,
            "max_output_tokens": 64,
            "max_tokens": 32,
            "max_completion_tokens": 16
        }))
        .unwrap(),
    )
    .unwrap();
    let defaults: Value = serde_json::from_slice(&defaults).unwrap();
    for field in [
        "stop",
        "temperature",
        "top_p",
        "presence_penalty",
        "frequency_penalty",
        "parallel_tool_calls",
        "max_output_tokens",
        "max_tokens",
        "max_completion_tokens",
    ] {
        assert!(defaults.get(field).is_none(), "{field}");
    }

    for (field, value, code) in [
        ("stop", json!(["END"]), "unsupported_stop"),
        ("temperature", json!(0.5), "unsupported_temperature"),
        ("top_p", json!(0.5), "unsupported_top_p"),
        ("presence_penalty", json!(1), "unsupported_presence_penalty"),
        (
            "frequency_penalty",
            json!(1),
            "unsupported_frequency_penalty",
        ),
        ("seed", json!(7), "unsupported_seed"),
        (
            "parallel_tool_calls",
            json!(false),
            "unsupported_parallel_tool_calls",
        ),
        ("max_tokens", json!(0), "unsupported_token_limit"),
    ] {
        let mut request = json!({"messages": [{"role": "user", "content": "hello"}]});
        request[field] = value;
        let error = kiro_provider_core_chat_completions_request_body(
            &serde_json::to_vec(&request).unwrap(),
        )
        .unwrap_err();
        assert_eq!(error.code, code, "{field}");
    }
}

#[test]
fn kiro_provider_core_maps_chat_messages_and_legacy_functions() {
    let tool = kiro_provider_core_responses_items_from_chat_message(&json!({
        "role": "tool",
        "tool_call_id": "call_1",
        "content": [{"text": "ok"}],
    }));
    assert_eq!(
        tool,
        vec![json!({
            "type": "function_call_output",
            "call_id": "call_1",
            "output": "ok",
        })]
    );

    let assistant = kiro_provider_core_responses_items_from_chat_message(&json!({
        "role": "assistant",
        "function_call": {"name": "read_file", "arguments": "{}"}
    }));
    assert_eq!(assistant[0]["type"], "function_call");
    assert_eq!(assistant[0]["call_id"], "read_file");

    assert_eq!(
        kiro_provider_core_tool_from_legacy_chat_function(&json!({
            "name": "read_file",
            "description": " Read a file ",
            "parameters": {"type": "object"}
        })),
        Some(json!({
            "type": "function",
            "function": {
                "name": "read_file",
                "description": "Read a file",
                "parameters": {"type": "object"}
            }
        }))
    );
    assert_eq!(
        kiro_provider_core_tool_choice_from_legacy_chat_function_call(&json!({
            "name": "read_file"
        })),
        Some(json!({"type": "function", "function": {"name": "read_file"}}))
    );
    assert_eq!(
        kiro_provider_core_tool_choice_from_legacy_chat_function_call(&json!("auto")),
        Some(json!("auto"))
    );
}
