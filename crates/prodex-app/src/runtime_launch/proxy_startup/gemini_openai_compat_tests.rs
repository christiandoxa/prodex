use super::chat_compatible_rewrite::{
    RuntimeChatCompatibleConversationStore, RuntimeChatCompatibleSseReader,
    RuntimeDeepSeekRewriteOptions, runtime_provider_chat_compatible_request_body,
};
use super::provider_bridge::RuntimeProviderBridgeKind;
use prodex_cli::SUPER_DEEPSEEK_DEFAULT_MODEL;
use prodex_provider_core::provider_core_chat_compatible_responses_value_from_chat_value;
use std::io::Read;

fn conversation_store() -> RuntimeChatCompatibleConversationStore {
    RuntimeChatCompatibleConversationStore::default()
}

fn runtime_deepseek_responses_value_from_chat_value(
    value: &serde_json::Value,
    request_id: u64,
) -> serde_json::Value {
    provider_core_chat_compatible_responses_value_from_chat_value(
        value,
        request_id,
        "deepseek",
        RuntimeProviderBridgeKind::DeepSeek.chat_compatible_adapter_label(),
        SUPER_DEEPSEEK_DEFAULT_MODEL,
        "resp_deepseek",
    )
}

#[test]
fn gemini_openai_chat_request_preserves_documented_reasoning_effort() {
    let request = serde_json::json!({
        "model": "gemini-3.5-flash",
        "stream": true,
        "input": "hello",
        "reasoning": {"effort": "medium"}
    });

    let translated = runtime_provider_chat_compatible_request_body(
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

    let translated = runtime_provider_chat_compatible_request_body(
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

#[test]
fn gemini_openai_chat_response_reports_gemini_specific_tool_call_errors() {
    let response = serde_json::json!({
        "id": "chatcmpl_bad_args",
        "model": "gemini-3.5-flash",
        "choices": [{
            "message": {
                "role": "assistant",
                "tool_calls": [{
                    "id": "call_shell_1",
                    "type": "function",
                    "function": {
                        "name": "shell",
                        "arguments": "{\"cmd\":"
                    }
                }]
            }
        }]
    });

    let translated = provider_core_chat_compatible_responses_value_from_chat_value(
        &response,
        48,
        "gemini",
        RuntimeProviderBridgeKind::Gemini.chat_compatible_adapter_label(),
        "gemini-3.5-flash",
        "resp_deepseek",
    );

    assert_eq!(translated["status"], "failed");
    assert_eq!(translated["error"]["code"], "invalid_tool_call_arguments");
    assert!(
        translated["error"]["message"]
            .as_str()
            .unwrap()
            .contains("Gemini OpenAI-compatible returned malformed JSON arguments")
    );
}

#[test]
fn gemini_openai_chat_response_uses_gemini_metadata_bucket() {
    let response = serde_json::json!({
        "id": "chatcmpl_meta",
        "model": "gemini-3.5-flash",
        "choices": [{
            "message": {
                "role": "assistant",
                "content": "Done.",
                "reasoning_content": "Need to inspect the tool output."
            },
            "finish_reason": "stop"
        }]
    });

    let translated = provider_core_chat_compatible_responses_value_from_chat_value(
        &response,
        49,
        "gemini",
        RuntimeProviderBridgeKind::Gemini.chat_compatible_adapter_label(),
        "gemini-3.5-flash",
        "resp_deepseek",
    );

    assert_eq!(
        translated["metadata"]["gemini"]["reasoning_content"],
        "Need to inspect the tool output."
    );
    assert_eq!(translated["metadata"]["gemini"]["finish_reason"], "stop");
    assert!(translated["metadata"].get("deepseek").is_none());
}

#[test]
fn gemini_openai_chat_sse_completed_response_uses_gemini_metadata_bucket() {
    let stream = concat!(
        "data: {\"id\":\"chatcmpl_meta\",\"model\":\"gemini-3.5-flash\",\"choices\":[{\"delta\":{\"reasoning_content\":\"Need tool output.\",\"content\":\"Done.\"},\"finish_reason\":\"stop\"}]}\n\n",
        "data: [DONE]\n\n",
    );
    let mut reader = RuntimeChatCompatibleSseReader::new_with_provider_and_observer(
        std::io::Cursor::new(stream.as_bytes()),
        RuntimeProviderBridgeKind::Gemini,
        50,
        Vec::new(),
        None,
        conversation_store(),
        None,
    );
    let mut output = String::new();
    reader.read_to_string(&mut output).unwrap();

    assert!(output.contains("\"metadata\":{\"gemini\":{"));
    assert!(output.contains("\"reasoning_content\":\"Need tool output.\""));
    assert!(output.contains("\"finish_reason\":\"stop\""));
    assert!(!output.contains("\"metadata\":{\"deepseek\":{"));
}

#[test]
fn gemini_openai_chat_request_uses_gemini_metadata_bucket_for_degraded_response_format() {
    let request = serde_json::json!({
        "model": "gemini-3.5-flash",
        "input": "hello",
        "response_format": {
            "type": "json_schema",
            "json_schema": {
                "name": "answer",
                "schema": {"type": "object"}
            }
        }
    });

    let translated = runtime_provider_chat_compatible_request_body(
        &serde_json::to_vec(&request).unwrap(),
        &conversation_store(),
        RuntimeProviderBridgeKind::Gemini,
        "gemini-3.5-flash",
        true,
        RuntimeDeepSeekRewriteOptions::default(),
    )
    .expect("Gemini OpenAI-compatible request should translate");

    let metadata = translated
        .response_metadata
        .expect("degraded response_format should be recorded");
    assert_eq!(
        metadata["gemini"]["degraded_response_format"]["from"],
        "json_schema"
    );
    assert_eq!(
        metadata["gemini"]["degraded_response_format"]["to"],
        "json_object"
    );
    assert!(metadata.get("deepseek").is_none());
}

#[test]
fn gemini_openai_chat_request_uses_gemini_metadata_bucket_for_omitted_tool_choice() {
    let request = serde_json::json!({
        "model": "gemini-3.5-flash",
        "input": "read recent commits",
        "tools": [{
            "type": "function",
            "name": "shell",
            "parameters": {"type": "object"}
        }],
        "tool_choice": {
            "type": "function",
            "name": "shell"
        },
        "reasoning": {"effort": "high"}
    });

    let translated = runtime_provider_chat_compatible_request_body(
        &serde_json::to_vec(&request).unwrap(),
        &conversation_store(),
        RuntimeProviderBridgeKind::Gemini,
        "gemini-3.5-flash",
        true,
        RuntimeDeepSeekRewriteOptions::default(),
    )
    .expect("Gemini OpenAI-compatible request should translate");
    let body: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();

    assert!(body.get("tool_choice").is_none());
    let metadata = translated
        .response_metadata
        .expect("tool_choice omission should be recorded");
    assert_eq!(
        metadata["gemini"]["omitted_tool_choice"]["from"]["name"],
        "shell"
    );
    assert!(
        metadata["gemini"]["omitted_tool_choice"]["reason"]
            .as_str()
            .unwrap()
            .contains("Gemini OpenAI-compatible thinking mode")
    );
    assert!(metadata.get("deepseek").is_none());
}

#[test]
fn gemini_openai_chat_request_reports_gemini_specific_model_validation_errors() {
    let request = serde_json::json!({
        "model": 123,
        "input": "hello"
    });

    let error = runtime_provider_chat_compatible_request_body(
        &serde_json::to_vec(&request).unwrap(),
        &conversation_store(),
        RuntimeProviderBridgeKind::Gemini,
        "gemini-3.5-flash",
        true,
        RuntimeDeepSeekRewriteOptions::default(),
    )
    .expect_err("invalid Gemini model should fail");

    assert!(
        error
            .to_string()
            .contains("Gemini OpenAI-compatible model must be a string")
    );
}

#[test]
fn gemini_openai_chat_request_reports_gemini_specific_reasoning_validation_errors() {
    let request = serde_json::json!({
        "model": "gemini-3.5-flash",
        "input": "hello",
        "reasoning": true
    });

    let error = runtime_provider_chat_compatible_request_body(
        &serde_json::to_vec(&request).unwrap(),
        &conversation_store(),
        RuntimeProviderBridgeKind::Gemini,
        "gemini-3.5-flash",
        true,
        RuntimeDeepSeekRewriteOptions::default(),
    )
    .expect_err("invalid Gemini reasoning should fail");

    assert!(
        error
            .to_string()
            .contains("Gemini OpenAI-compatible reasoning must be an object")
    );
}

#[test]
fn gemini_openai_chat_request_reports_gemini_specific_unsupported_reasoning_effort() {
    let request = serde_json::json!({
        "model": "gemini-3.5-flash",
        "input": "hello",
        "reasoning": {"effort": "weird"}
    });

    let error = runtime_provider_chat_compatible_request_body(
        &serde_json::to_vec(&request).unwrap(),
        &conversation_store(),
        RuntimeProviderBridgeKind::Gemini,
        "gemini-3.5-flash",
        true,
        RuntimeDeepSeekRewriteOptions::default(),
    )
    .expect_err("unsupported Gemini reasoning effort should fail");

    assert!(
        error
            .to_string()
            .contains("Gemini OpenAI-compatible reasoning effort is not supported")
    );
}

#[test]
fn gemini_openai_chat_request_reports_gemini_specific_response_format_errors() {
    let request = serde_json::json!({
        "model": "gemini-3.5-flash",
        "input": "hello",
        "response_format": {}
    });

    let error = runtime_provider_chat_compatible_request_body(
        &serde_json::to_vec(&request).unwrap(),
        &conversation_store(),
        RuntimeProviderBridgeKind::Gemini,
        "gemini-3.5-flash",
        true,
        RuntimeDeepSeekRewriteOptions::default(),
    )
    .expect_err("invalid Gemini response_format should fail");

    assert!(
        error
            .to_string()
            .contains("Gemini OpenAI-compatible response_format must include a type")
    );
}

#[test]
fn gemini_openai_chat_request_reports_gemini_specific_user_id_errors() {
    let request = serde_json::json!({
        "model": "gemini-3.5-flash",
        "input": "hello",
        "user_id": 7
    });

    let error = runtime_provider_chat_compatible_request_body(
        &serde_json::to_vec(&request).unwrap(),
        &conversation_store(),
        RuntimeProviderBridgeKind::Gemini,
        "gemini-3.5-flash",
        true,
        RuntimeDeepSeekRewriteOptions::default(),
    )
    .expect_err("invalid Gemini user_id should fail");

    assert!(
        error
            .to_string()
            .contains("Gemini OpenAI-compatible user_id must be a string")
    );
}

#[test]
fn gemini_openai_chat_request_reports_gemini_specific_tool_validation_errors() {
    let request = serde_json::json!({
        "model": "gemini-3.5-flash",
        "input": "hello",
        "tools": "not-an-array"
    });

    let error = runtime_provider_chat_compatible_request_body(
        &serde_json::to_vec(&request).unwrap(),
        &conversation_store(),
        RuntimeProviderBridgeKind::Gemini,
        "gemini-3.5-flash",
        true,
        RuntimeDeepSeekRewriteOptions::default(),
    )
    .expect_err("invalid Gemini tools should fail");

    assert!(
        error
            .to_string()
            .contains("Gemini OpenAI-compatible tools must be an array")
    );
}

#[test]
fn gemini_openai_chat_request_reports_gemini_specific_nested_namespace_tool_errors() {
    let request = serde_json::json!({
        "model": "gemini-3.5-flash",
        "input": "hello",
        "tools": [{
            "type": "namespace",
            "tools": []
        }]
    });

    let error = runtime_provider_chat_compatible_request_body(
        &serde_json::to_vec(&request).unwrap(),
        &conversation_store(),
        RuntimeProviderBridgeKind::Gemini,
        "gemini-3.5-flash",
        true,
        RuntimeDeepSeekRewriteOptions::default(),
    )
    .expect_err("invalid Gemini namespace tool should fail");

    assert!(
        error
            .to_string()
            .contains("Gemini OpenAI-compatible namespace tools require a name")
    );
}

#[test]
fn gemini_openai_chat_request_reports_gemini_specific_input_item_errors() {
    let request = serde_json::json!({
        "model": "gemini-3.5-flash",
        "input": [true]
    });

    let error = runtime_provider_chat_compatible_request_body(
        &serde_json::to_vec(&request).unwrap(),
        &conversation_store(),
        RuntimeProviderBridgeKind::Gemini,
        "gemini-3.5-flash",
        true,
        RuntimeDeepSeekRewriteOptions::default(),
    )
    .expect_err("invalid Gemini input item should fail");

    assert!(
        error
            .to_string()
            .contains("Gemini OpenAI-compatible input items must be objects")
    );
}
