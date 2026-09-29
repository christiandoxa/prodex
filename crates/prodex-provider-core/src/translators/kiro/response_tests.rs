use super::super::*;
use serde_json::{Value, json};

#[test]
fn kiro_response_chat_completion_preserves_malformed_fields_and_error_values() {
    assert_eq!(
        kiro_provider_core_chat_completion_value_from_response(&Value::Null, 7),
        json!({
            "id": "chatcmpl_kiro_7",
            "object": "chat.completion",
            "created": 0,
            "model": "kiro-cli",
            "choices": [{
                "index": 0,
                "message": {"role": "assistant", "content": ""},
                "finish_reason": "stop",
            }],
        })
    );

    assert_eq!(
        kiro_provider_core_chat_completion_value_from_response(
            &json!({
                "id": 42,
                "created_at": "later",
                "model": [],
                "output": {},
                "status": "failed",
                "error": null,
                "requested_model": {"raw": true},
                "metadata": null,
            }),
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
            "requested_model": {"raw": true},
            "metadata": null,
        })
    );
}

#[test]
fn kiro_response_chat_completion_keeps_output_order_unicode_and_large_text() {
    let response = json!({
        "id": "界\"",
        "created_at": 12,
        "model": "kiro-模型",
        "output": [
            null,
            {"type": "function_call", "name": "first", "arguments": "{}"},
            {"type": "message", "content": [{"text": "first 🐈"}, {"text": "ignored"}]},
            {"type": "function_call", "call_id": 7, "name": null, "arguments": false},
        ],
        "status": "failed",
        "error": {"message": {"detail": "bad 🐈"}},
    });
    assert_eq!(
        kiro_provider_core_chat_completion_value_from_response(&response, 1),
        json!({
            "id": "chatcmpl_界\"",
            "object": "chat.completion",
            "created": 12,
            "model": "kiro-模型",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": "first 🐈",
                    "tool_calls": [
                        {
                            "id": "call_kiro",
                            "type": "function",
                            "function": {"name": "first", "arguments": "{}"},
                        },
                        {
                            "id": "call_kiro",
                            "type": "function",
                            "function": {"name": "tool_call", "arguments": "{}"},
                        },
                    ],
                    "refusal": {"detail": "bad 🐈"},
                },
                "finish_reason": "tool_calls",
            }],
        })
    );

    let large_text = "x".repeat(512 * 1024);
    let response = json!({
        "output": [{"type": "message", "content": [{"text": large_text}]}]
    });
    let expected = json!({
        "id": "chatcmpl_kiro_0",
        "object": "chat.completion",
        "created": 0,
        "model": "kiro-cli",
        "choices": [{
            "index": 0,
            "message": {"role": "assistant", "content": large_text},
            "finish_reason": "stop",
        }],
    });
    assert_eq!(
        kiro_provider_core_chat_completion_value_from_response(&response, 0),
        expected
    );
}

#[test]
fn kiro_response_chat_completion_handles_eight_mib_text() {
    let large_text = "x".repeat(8 * 1024 * 1024);
    let response = json!({
        "output": [{"type": "message", "content": [{"text": large_text.clone()}]}]
    });
    let expected = json!({
        "id": "chatcmpl_kiro_0",
        "object": "chat.completion",
        "created": 0,
        "model": "kiro-cli",
        "choices": [{
            "index": 0,
            "message": {"role": "assistant", "content": large_text},
            "finish_reason": "stop",
        }],
    });

    assert_eq!(
        kiro_provider_core_chat_completion_value_from_response(&response, 0),
        expected
    );
}

#[test]
fn kiro_response_anthropic_handles_eight_mib_text() {
    let large_text = "x".repeat(8 * 1024 * 1024);
    let response = json!({
        "id": "resp_kiro",
        "output": [{"type": "message", "content": [{"text": large_text.clone()}]}],
        "usage": {"input_tokens": 1, "output_tokens": 2},
    });

    let message = kiro_provider_core_anthropic_message_value_from_response(&response, "kiro");
    assert_eq!(message["id"], "resp_kiro");
    assert_eq!(
        message["content"],
        json!([{"type": "text", "text": large_text}])
    );
    assert_eq!(
        message["usage"],
        json!({"input_tokens": 1, "output_tokens": 2})
    );
}

#[test]
fn kiro_response_mojo_wrappers_reject_over_limit_inputs() {
    let oversized = " ".repeat(prodex_mojo_core::rich::KIRO_RESPONSE_MAX_BYTES + 1);
    assert_eq!(
        prodex_mojo_core::rich::kiro_rewrite_chat_response_json(&oversized, 0),
        Err(prodex_mojo_core::MojoError::InvalidInput)
    );

    let mut input = prodex_mojo_core::rich::KiroKernelInput::new(
        prodex_mojo_core::rich::KiroKernelOperation::StreamContentText,
    );
    input.input = Some(&oversized);
    assert_eq!(
        prodex_mojo_core::rich::kiro_kernel(input),
        Err(prodex_mojo_core::MojoError::InvalidInput)
    );

    let mut request = prodex_mojo_core::rich::KiroKernelInput::new(
        prodex_mojo_core::rich::KiroKernelOperation::RequestBody,
    );
    request.input = Some(&oversized);
    assert_eq!(
        prodex_mojo_core::rich::kiro_kernel(request),
        Err(prodex_mojo_core::MojoError::InvalidInput)
    );
}

#[test]
fn kiro_response_model_shapes_keep_catalog_order_and_statuses() {
    let catalog = vec![
        json!({"id": "Claude", "order": 1}),
        Value::Null,
        json!({"id": "claude", "order": 2}),
    ];
    assert_eq!(
        kiro_provider_core_model_list_value(&catalog),
        json!({"object": "list", "data": catalog})
    );
    assert_eq!(
        kiro_provider_core_model_list_value(&[]),
        json!({"object": "list", "data": []})
    );
    assert_eq!(
        kiro_provider_core_model_value_or_not_found(&catalog, "CLAUDE"),
        (200, json!({"id": "Claude", "order": 1}))
    );
    assert_eq!(
        kiro_provider_core_model_value_or_not_found(&catalog, "missing"),
        (
            404,
            json!({"error": {
                "message": "model 'missing' is not available for kiro",
                "type": "invalid_request_error",
                "code": "model_not_found",
            }})
        )
    );
    let missing_id = "\n界'\"\\path";
    let (status, body) = kiro_provider_core_model_value_or_not_found(&catalog, missing_id);
    assert_eq!(status, 404);
    assert_eq!(
        body,
        json!({"error": {
            "message": format!("model '{missing_id}' is not available for kiro"),
            "type": "invalid_request_error",
            "code": "model_not_found",
        }})
    );

    let large_id = "界".repeat(32 * 1024);
    let (status, body) = kiro_provider_core_model_value_or_not_found(&catalog, &large_id);
    assert_eq!(status, 404);
    assert_eq!(
        body["error"]["message"],
        format!("model '{large_id}' is not available for kiro")
    );

    let large_catalog: Vec<_> = (0..1024)
        .map(|index| json!({"id": format!("model-{index}"), "label": "界".repeat(128)}))
        .collect();
    assert_eq!(
        kiro_provider_core_model_list_value(&large_catalog),
        json!({"object": "list", "data": large_catalog})
    );
}

#[test]
fn kiro_response_error_shapes_keep_unicode_and_large_messages() {
    assert_eq!(
        kiro_provider_core_invalid_request_error_value("", ""),
        json!({"error": {
            "message": "",
            "type": "invalid_request_error",
            "code": "",
        }})
    );
    let message = "échec\n界\"".repeat(1024);
    let code = "bad_\"code";
    assert_eq!(
        kiro_provider_core_invalid_request_error_value(&message, code),
        json!({"error": {
            "message": message,
            "type": "invalid_request_error",
            "code": code,
        }})
    );
    let path = "/v1/界\"\nfiles";
    assert_eq!(
        kiro_provider_core_unsupported_path_error_value(path),
        json!({"error": {
            "message": format!("Kiro provider does not support {path} yet"),
            "type": "invalid_request_error",
            "code": "unsupported_path",
        }})
    );
}

#[test]
fn kiro_response_anthropic_mapping_preserves_order_and_wrong_type_values() {
    let response = json!({
        "id": {"synthetic": "response"},
        "output": [
            {"type": "function_call", "call_id": 17, "name": false, "arguments": "[1,\"é\"]"},
            {"type": "message", "content": [{"text": "done 🐈"}]},
            {"type": "function_call", "arguments": "not json"},
        ],
        "usage": {"input_tokens": "3", "output_tokens": null},
        "metadata": {"kiro": {"stop_reason": "max_output_tokens"}},
    });
    assert_eq!(
        kiro_provider_core_anthropic_message_value_from_response(&response, "kiro-模型"),
        json!({
            "id": {"synthetic": "response"},
            "type": "message",
            "role": "assistant",
            "model": "kiro-模型",
            "content": [
                {"type": "tool_use", "id": 17, "name": false, "input": [1, "é"]},
                {"type": "tool_use", "id": "call_kiro", "name": "tool_call", "input": {}},
                {"type": "text", "text": "done 🐈"},
            ],
            "stop_reason": "tool_use",
            "stop_sequence": null,
            "usage": {"input_tokens": "3", "output_tokens": null},
        })
    );

    let large_text = "界".repeat(32 * 1024);
    let response = json!({
        "output": [{"type": "message", "content": [{"text": large_text}]}]
    });
    let message = kiro_provider_core_anthropic_message_value_from_response(&response, "kiro");
    assert_eq!(
        message["content"],
        json!([{"type": "text", "text": large_text}])
    );
}

#[test]
fn kiro_response_finish_reason_handles_missing_and_wrong_type_details() {
    for response in [
        Value::Null,
        json!({"incomplete_details": null}),
        json!({"incomplete_details": []}),
        json!({"incomplete_details": {"reason": 7}}),
        json!({"incomplete_details": {"reason": "max_tokens"}}),
    ] {
        assert_eq!(
            kiro_provider_core_chat_completion_finish_reason(&response, false),
            "stop"
        );
    }
    assert_eq!(
        kiro_provider_core_chat_completion_finish_reason(
            &json!({"incomplete_details": {"reason": "max_output_tokens"}}),
            false,
        ),
        "length"
    );
    assert_eq!(
        kiro_provider_core_chat_completion_finish_reason(
            &json!({"incomplete_details": {"reason": "max_output_tokens"}}),
            true,
        ),
        "tool_calls"
    );
    assert_eq!(
        kiro_provider_core_chat_completion_finish_reason(
            &json!({"incomplete_details": {"reason": "界".repeat(32 * 1024)}}),
            false,
        ),
        "stop"
    );
}
