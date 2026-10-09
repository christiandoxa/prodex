use super::super::{
    deepseek_provider_core_chat_assistant_messages_from_response_value,
    deepseek_provider_core_responses_content_text,
    deepseek_provider_core_validate_supported_input_item,
};
use prodex_mojo_core::rich::{
    DEEPSEEK_LARGE_RESPONSE_KERNEL_MAX_BYTES, DeepSeekKernelInput, DeepSeekKernelOperation,
    DeepSeekRequestPolicyOperation, deepseek_kernel, deepseek_request_policy,
};
use serde_json::{Value, json};

fn kernel_value(input: DeepSeekKernelInput<'_>) -> Value {
    serde_json::from_slice(&deepseek_kernel(input).expect("DeepSeek ABI result"))
        .expect("DeepSeek ABI JSON")
}

#[test]
fn direct_abi_matrix_covers_role_content_history_and_response_shapes() {
    for (role, expected) in [
        ("assistant", "assistant"),
        ("developer", "system"),
        ("system", "system"),
        ("tool", "tool"),
        ("critic", "user"),
    ] {
        let mut input = DeepSeekKernelInput::new(DeepSeekKernelOperation::ChatRole);
        input.role = Some(role);
        assert_eq!(kernel_value(input), json!(expected));
    }

    let mut content = DeepSeekKernelInput::new(DeepSeekKernelOperation::ResponsesContentText);
    content.input = Some(r#"["a",{"input_text":"b"},{"output_text":"c"},{"ignored":true}]"#);
    assert_eq!(kernel_value(content), json!("a\nb\nc"));

    let history = serde_json::to_string(&json!([
        {"role":"system","content":"rules"},
        {"role":"assistant","content":"already","tool_calls":[{"id":"call_1"}]},
        {"role":"tool","tool_call_id":"call_2","content":"ok"},
        {"role":"developer","content":"dev"}
    ]))
    .unwrap();
    let mut summary = DeepSeekKernelInput::new(DeepSeekKernelOperation::ResponsesHistorySummary);
    summary.messages = Some(&history);
    assert_eq!(
        kernel_value(summary),
        json!({
            "tool_call_ids": ["call_1"],
            "tool_output_call_ids": ["call_2"],
            "signatures": [
                {"role":"system","content":"rules"},
                {"role":"assistant","content":"already"},
                {"role":"tool","content":"ok"},
                {"role":"system","content":"dev"}
            ],
            "system_messages": ["rules"]
        })
    );

    let response = json!({
        "choices": [{"message": {
            "content": "",
            "reasoning_content": "think",
            "tool_calls": [{"id":"call_1","type":"function","function":{"name":"lookup","arguments":"{}"}}]
        }}]
    });
    assert_eq!(
        deepseek_provider_core_chat_assistant_messages_from_response_value(&response),
        vec![json!({
            "role":"assistant",
            "content":"",
            "reasoning_content":"think",
            "tool_calls":[{"id":"call_1","type":"function","function":{"name":"lookup","arguments":"{}"}}]
        })]
    );
    assert_eq!(
        deepseek_provider_core_responses_content_text(Some(&json!({"x":"雪"}))),
        r#"{"x":"雪"}"#
    );
}

#[test]
fn direct_abi_matrix_covers_input_validation_and_policy_precedence() {
    for (item, gemini_compat, expected) in [
        (
            json!({"type":"message","role":"critic","content":"x"}),
            false,
            "DeepSeek message role `critic` is not supported by this Responses adapter",
        ),
        (
            json!({"type":"message","content":[{"type":"input_image"}]}),
            false,
            "DeepSeek text-only adapter does not support message content part type `input_image`",
        ),
        (
            json!({"type":"function_call","call_id":"call_1"}),
            false,
            "DeepSeek input tool call items require a function name",
        ),
        (
            json!({"type":"function_call_output","call_id":"call_1"}),
            false,
            "DeepSeek input tool output items require output content",
        ),
    ] {
        assert_eq!(
            deepseek_provider_core_validate_supported_input_item(&item, gemini_compat, "DeepSeek")
                .unwrap_err(),
            expected
        );
    }

    for (operation, input, flag, tag) in [
        (
            DeepSeekRequestPolicyOperation::ReasoningShape,
            json!({"reasoning": []}),
            false,
            1,
        ),
        (
            DeepSeekRequestPolicyOperation::ResponseFormatShape,
            json!({"response_format":{"type":"xml"}}),
            false,
            4,
        ),
        (
            DeepSeekRequestPolicyOperation::Stop,
            json!({"stop":["a",1]}),
            false,
            3,
        ),
        (
            DeepSeekRequestPolicyOperation::TopLogprobs,
            json!({"top_logprobs":21,"logprobs":true}),
            false,
            2,
        ),
        (
            DeepSeekRequestPolicyOperation::PrimitiveCore,
            json!({"max_tokens":0}),
            false,
            4,
        ),
    ] {
        let source = serde_json::to_string(&input).unwrap();
        assert_eq!(
            deepseek_request_policy(operation, &source, flag, 0)
                .expect("DeepSeek policy ABI")
                .tag,
            tag
        );
    }
}

#[test]
fn direct_abi_rejects_malformed_and_oversized_json() {
    let mut malformed = DeepSeekKernelInput::new(DeepSeekKernelOperation::ResponsesContentText);
    malformed.input = Some("{bad");
    assert!(deepseek_kernel(malformed).is_err());

    let oversized = format!(
        "\"{}\"",
        "x".repeat(DEEPSEEK_LARGE_RESPONSE_KERNEL_MAX_BYTES)
    );
    let mut input = DeepSeekKernelInput::new(DeepSeekKernelOperation::ResponsesContentText);
    input.input = Some(&oversized);
    assert!(deepseek_kernel(input).is_err());
}
