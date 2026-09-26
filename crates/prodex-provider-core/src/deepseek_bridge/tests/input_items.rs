use super::{
    deepseek_provider_core_messages_from_responses_request,
    deepseek_provider_core_validate_supported_input_item,
};

#[test]
fn deepseek_provider_core_validates_supported_input_items() {
    for item in [
        serde_json::json!({"type": "message", "role": "developer", "content": [{"type": "input_text", "text": "hi"}]}),
        serde_json::json!({"type": "function_call", "call_id": "call_1", "function": {"name": "lookup"}}),
        serde_json::json!({"type": "custom_tool_call_output", "call_id": "call_1", "output": "ok"}),
        serde_json::json!({"type": "local_shell_call", "call_id": "call_2", "action": {"command": ["echo", "hi"]}}),
        serde_json::json!({"type": "message", "content": [{"type": "input_image"}]}),
    ] {
        deepseek_provider_core_validate_supported_input_item(&item, true, "DeepSeek").unwrap();
    }
}

#[test]
fn deepseek_provider_core_rejects_unsupported_input_items() {
    assert!(
        deepseek_provider_core_validate_supported_input_item(
            &serde_json::json!(true),
            false,
            "DeepSeek",
        )
        .unwrap_err()
        .contains("DeepSeek input items must be objects")
    );
    assert!(
        deepseek_provider_core_validate_supported_input_item(
            &serde_json::json!({"type": "message", "role": "critic", "content": "no"}),
            false,
            "DeepSeek",
        )
        .unwrap_err()
        .contains("DeepSeek message role `critic` is not supported")
    );
    assert!(
        deepseek_provider_core_validate_supported_input_item(
            &serde_json::json!({"type": "function_call", "call_id": "call_1"}),
            false,
            "DeepSeek",
        )
        .unwrap_err()
        .contains("DeepSeek input tool call items require a function name")
    );
    assert!(
        deepseek_provider_core_validate_supported_input_item(
            &serde_json::json!({"type": "function_call_output", "call_id": "call_1"}),
            false,
            "DeepSeek",
        )
        .unwrap_err()
        .contains("DeepSeek input tool output items require output content")
    );
    assert!(
        deepseek_provider_core_validate_supported_input_item(
            &serde_json::json!({"type": "message", "content": [{"type": "input_image"}]}),
            false,
            "DeepSeek",
        )
        .unwrap_err()
        .contains(
            "DeepSeek text-only adapter does not support message content part type `input_image`"
        )
    );
    assert!(
        deepseek_provider_core_validate_supported_input_item(
            &serde_json::json!({"type": "message", "content": [{"type": "input_text"}]}),
            false,
            "DeepSeek",
        )
        .unwrap_err()
        .contains("DeepSeek input_text content parts require a text field")
    );
}

#[test]
fn deepseek_provider_core_shapes_unicode_content_arrays_in_order() {
    let shape = |content: serde_json::Value, gemini_compat: bool| {
        let value = serde_json::json!({
            "input": [{"type": "message", "role": "user", "content": content}]
        });
        deepseek_provider_core_messages_from_responses_request(
            &value,
            &[],
            gemini_compat,
            "DeepSeek",
        )
        .unwrap()
        .map(|messages| {
            messages
                .into_iter()
                .map(|message| message["content"].as_str().unwrap().to_string())
                .collect::<Vec<_>>()
        })
    };

    assert_eq!(
        shape(
            serde_json::json!(["α", {"input_text": "β"}, {"output_text": "🔥"}]),
            false,
        ),
        Some(vec!["α\nβ\n🔥".to_string()])
    );
    assert_eq!(
        shape(
            serde_json::json!(["quote: \"", "slash: \\", "line:\n\t\r\0", "é🔥"]),
            false,
        ),
        Some(vec!["quote: \"\nslash: \\\nline:\n\t\r\0\né🔥".to_string()])
    );
    assert_eq!(
        shape(
            serde_json::json!([
                null,
                7,
                true,
                {"type": "input_image", "text": 7, "input_text": "fallback", "output_text": "ignored"},
                {"type": "input_image", "input_text": null, "output_text": "after-null"},
                {"type": "input_image", "text": "", "output_text": "ignored-empty"}
            ]),
            true,
        ),
        Some(vec!["fallback\nafter-null\n".to_string()])
    );
    assert_eq!(shape(serde_json::json!([]), false), None);

    let large = "🔥".repeat(16_384);
    let shaped = shape(
        serde_json::json!(["prefix", {"text": large}, "suffix"]),
        false,
    )
    .unwrap()
    .remove(0);
    assert!(
        shaped == format!("prefix\n{large}\nsuffix"),
        "large content differed; got {} bytes",
        shaped.len()
    );
}

#[test]
fn deepseek_provider_core_rejects_malformed_mojo_input_item_json() {
    let mut input = prodex_mojo_core::rich::DeepSeekKernelInput::new(
        prodex_mojo_core::rich::DeepSeekKernelOperation::RawBridgeInputItem,
    );
    input.item = Some(r#"{"type":"message","content":[}"#);

    assert!(prodex_mojo_core::rich::deepseek_kernel(input).is_err());
}
