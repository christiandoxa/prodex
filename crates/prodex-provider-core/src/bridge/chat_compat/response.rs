//! Chat-compatible buffered response conversion.

use super::provider_core_chat_compatible_created_at;
use super::tool_calls::provider_core_chat_compatible_responses_tool_call_plan;
use super::usage::provider_core_chat_compatible_responses_usage;

pub fn provider_core_chat_compatible_responses_value_from_chat_value(
    value: &serde_json::Value,
    request_id: u64,
    provider_metadata_key: &str,
    provider_adapter_label: &str,
    default_model: &str,
    fallback_response_id_prefix: &str,
) -> serde_json::Value {
    provider_core_chat_compatible_responses_value_from_chat_value_with_fallback_ids(
        value,
        provider_metadata_key,
        provider_adapter_label,
        default_model,
        || format!("{fallback_response_id_prefix}_{request_id}"),
        || "call_0".to_string(),
    )
}

pub fn provider_core_chat_compatible_responses_value_from_chat_value_with_fallback_ids(
    value: &serde_json::Value,
    provider_metadata_key: &str,
    provider_adapter_label: &str,
    default_model: &str,
    fallback_response_id: impl FnOnce() -> String,
    mut fallback_call_id: impl FnMut() -> String,
) -> serde_json::Value {
    let fallback_response_id = value
        .get("id")
        .and_then(serde_json::Value::as_str)
        .is_none()
        .then(fallback_response_id);
    let fallback_created_at = value
        .get("created")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or_else(provider_core_chat_compatible_created_at);
    let message = value
        .get("choices")
        .and_then(serde_json::Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("message"));
    let (tool_plans, tool_call_error) =
        chat_compatible_response_tool_plans(message, provider_adapter_label, &mut fallback_call_id);
    let usage = value.get("usage").and_then(|usage| {
        provider_core_chat_compatible_responses_usage(usage, provider_metadata_key)
    });
    let metadata = chat_compatible_response_metadata(value);
    let mut context = serde_json::json!({
        "response": value,
        "fallback_response_id": fallback_response_id,
        "fallback_created_at": fallback_created_at,
        "default_model": default_model,
        "provider_metadata_key": provider_metadata_key,
        "tool_plans": tool_plans,
    });
    if let Some(message) = tool_call_error {
        context["tool_call_error"] = serde_json::Value::String(message);
    }
    if let Some(usage) = usage {
        context["usage"] = usage;
    }
    if !metadata.is_empty() {
        context["metadata"] = serde_json::Value::Object(metadata);
    }
    let mut document = crate::mojo_json::Document::default();
    document.root(&context);
    let raw = std::str::from_utf8(&document.raw).expect("Serde emits UTF-8 JSON");
    let body = prodex_mojo_core::json::transform_chat_bridge_response(&document.nodes, raw)
        .expect("compiled Mojo chat bridge response mapping failed");
    serde_json::from_slice(&body).expect("Mojo chat bridge response must be valid JSON")
}

fn chat_compatible_response_tool_plans(
    message: Option<&serde_json::Value>,
    provider_adapter_label: &str,
    fallback_call_id: &mut impl FnMut() -> String,
) -> (Vec<serde_json::Value>, Option<String>) {
    let mut plans = Vec::new();
    let Some(tool_calls) = message
        .and_then(|message| message.get("tool_calls"))
        .and_then(serde_json::Value::as_array)
    else {
        return (plans, None);
    };
    for tool_call in tool_calls {
        match provider_core_chat_compatible_responses_tool_call_plan(
            tool_call,
            provider_adapter_label,
            fallback_call_id,
        ) {
            Ok(Some(plan)) => plans.push(plan),
            Ok(None) => {}
            Err(message) => return (plans, Some(message)),
        }
    }
    (plans, None)
}

fn chat_compatible_response_metadata(
    value: &serde_json::Value,
) -> serde_json::Map<String, serde_json::Value> {
    let mut document = crate::mojo_json::Document::default();
    document.openai_chat_context(value, None);
    let raw = std::str::from_utf8(&document.raw).expect("Serde emits UTF-8 JSON");
    let output = prodex_mojo_core::json::transform_chat_bridge_metadata(&document.nodes, raw)
        .expect("compiled Mojo chat bridge metadata mapping failed");
    serde_json::from_slice(&output).expect("Mojo chat metadata must be a valid JSON object")
}

#[cfg(test)]
mod metadata_tests {
    use super::chat_compatible_response_metadata;
    use serde_json::json;

    #[test]
    fn mojo_chat_metadata_preserves_type_presence_and_first_choice() {
        let value = json!({"choices":[{"logprobs":[],"finish_reason":"","message":{
            "reasoning_content":" ","refusal":"","annotations":[false],
        }},{"logprobs":{"ignored":true}}],"system_fingerprint":"fp"});
        assert_eq!(
            serde_json::Value::Object(chat_compatible_response_metadata(&value)),
            json!({
                "logprobs":[],"finish_reason":"","reasoning_content":" ","annotations":[false],"system_fingerprint":"fp",
            })
        );
        let invalid = json!({"choices":{"message":{"reasoning_content":"ignored"}},"system_fingerprint":false});
        assert!(chat_compatible_response_metadata(&invalid).is_empty());
        assert!(chat_compatible_response_metadata(&json!(null)).is_empty());
    }
}
