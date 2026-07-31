//! Chat-compatible buffered response conversion.

use super::provider_core_chat_compatible_created_at;
use super::tool_calls::provider_core_chat_compatible_responses_tool_call_item;
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
    let response_id = value
        .get("id")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(fallback_response_id);
    let model = value
        .get("model")
        .and_then(serde_json::Value::as_str)
        .unwrap_or(default_model);
    let created_at = value
        .get("created")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or_else(provider_core_chat_compatible_created_at);
    let message = value
        .get("choices")
        .and_then(serde_json::Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("message"));
    let (output, tool_call_error) =
        chat_compatible_response_output(message, provider_adapter_label, &mut fallback_call_id);
    let mut response = serde_json::json!({
        "id": response_id,
        "object": "response",
        "created_at": created_at,
        "model": model,
        "output": output,
    });
    if let Some(message) = tool_call_error {
        response["status"] = serde_json::Value::String("failed".to_string());
        response["error"] = serde_json::json!({
            "code": "invalid_tool_call_arguments",
            "message": message,
        });
    }
    if let Some(usage) = value.get("usage").and_then(|usage| {
        provider_core_chat_compatible_responses_usage(usage, provider_metadata_key)
    }) {
        response["usage"] = usage;
    }
    let metadata = chat_compatible_response_metadata(value, message);
    if !metadata.is_empty() {
        response["metadata"] = serde_json::json!({ provider_metadata_key: metadata });
    }
    response
}

fn chat_compatible_response_output(
    message: Option<&serde_json::Value>,
    provider_adapter_label: &str,
    fallback_call_id: &mut impl FnMut() -> String,
) -> (Vec<serde_json::Value>, Option<String>) {
    let mut output = Vec::new();
    if let Some(content) = message
        .and_then(|message| message.get("content"))
        .and_then(serde_json::Value::as_str)
        .filter(|content| !content.is_empty())
    {
        output.push(serde_json::json!({
            "type": "message",
            "role": "assistant",
            "content": [{
                "type": "output_text",
                "text": content,
            }],
        }));
    }
    let Some(tool_calls) = message
        .and_then(|message| message.get("tool_calls"))
        .and_then(serde_json::Value::as_array)
    else {
        return (output, None);
    };
    for tool_call in tool_calls {
        match provider_core_chat_compatible_responses_tool_call_item(
            tool_call,
            provider_adapter_label,
            fallback_call_id,
        ) {
            Ok(Some(item)) => output.push(item),
            Ok(None) => {}
            Err(message) => return (output, Some(message)),
        }
    }
    (output, None)
}

fn chat_compatible_response_metadata(
    value: &serde_json::Value,
    message: Option<&serde_json::Value>,
) -> serde_json::Map<String, serde_json::Value> {
    let mut metadata = serde_json::Map::new();
    if let Some(logprobs) = value
        .get("choices")
        .and_then(serde_json::Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("logprobs"))
        .filter(|logprobs| !logprobs.is_null())
    {
        metadata.insert("logprobs".to_string(), logprobs.clone());
    }
    insert_non_empty_string_metadata(&mut metadata, message, "reasoning_content");
    insert_non_empty_string_metadata(&mut metadata, message, "refusal");
    if let Some(annotations) = message
        .and_then(|message| message.get("annotations"))
        .and_then(serde_json::Value::as_array)
        .filter(|annotations| !annotations.is_empty())
    {
        metadata.insert(
            "annotations".to_string(),
            serde_json::Value::Array(annotations.clone()),
        );
    }
    if let Some(finish_reason) = value
        .get("choices")
        .and_then(serde_json::Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("finish_reason"))
        .and_then(serde_json::Value::as_str)
    {
        metadata.insert(
            "finish_reason".to_string(),
            serde_json::Value::String(finish_reason.to_string()),
        );
    }
    insert_non_empty_value_metadata(&mut metadata, value, "system_fingerprint");
    metadata
}

fn insert_non_empty_string_metadata(
    metadata: &mut serde_json::Map<String, serde_json::Value>,
    message: Option<&serde_json::Value>,
    key: &str,
) {
    let Some(value) = message
        .and_then(|message| message.get(key))
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty())
    else {
        return;
    };
    metadata.insert(
        key.to_string(),
        serde_json::Value::String(value.to_string()),
    );
}

fn insert_non_empty_value_metadata(
    metadata: &mut serde_json::Map<String, serde_json::Value>,
    value: &serde_json::Value,
    key: &str,
) {
    let Some(value) = value
        .get(key)
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty())
    else {
        return;
    };
    metadata.insert(
        key.to_string(),
        serde_json::Value::String(value.to_string()),
    );
}
