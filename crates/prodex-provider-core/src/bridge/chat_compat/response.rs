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
    let mut output = Vec::new();
    let mut tool_call_error = None;
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
    if let Some(tool_calls) = message
        .and_then(|message| message.get("tool_calls"))
        .and_then(serde_json::Value::as_array)
    {
        for tool_call in tool_calls {
            match provider_core_chat_compatible_responses_tool_call_item(
                tool_call,
                provider_adapter_label,
                &mut fallback_call_id,
            ) {
                Ok(Some(item)) => output.push(item),
                Ok(None) => {}
                Err(message) => {
                    tool_call_error = Some(message);
                    break;
                }
            }
        }
    }
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
    if let Some(reasoning_content) = message
        .and_then(|message| message.get("reasoning_content"))
        .and_then(serde_json::Value::as_str)
        .filter(|reasoning_content| !reasoning_content.is_empty())
    {
        metadata.insert(
            "reasoning_content".to_string(),
            serde_json::Value::String(reasoning_content.to_string()),
        );
    }
    if let Some(refusal) = message
        .and_then(|message| message.get("refusal"))
        .and_then(serde_json::Value::as_str)
        .filter(|refusal| !refusal.is_empty())
    {
        metadata.insert(
            "refusal".to_string(),
            serde_json::Value::String(refusal.to_string()),
        );
    }
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
    if let Some(system_fingerprint) = value
        .get("system_fingerprint")
        .and_then(serde_json::Value::as_str)
        .filter(|system_fingerprint| !system_fingerprint.is_empty())
    {
        metadata.insert(
            "system_fingerprint".to_string(),
            serde_json::Value::String(system_fingerprint.to_string()),
        );
    }
    if !metadata.is_empty() {
        response["metadata"] = serde_json::json!({ provider_metadata_key: metadata });
    }
    response
}
