use super::super::provider_tools::runtime_provider_split_flat_namespace_tool_name;
use super::{
    RuntimeDeepSeekConversationStore, RuntimeDeepSeekPendingMessages,
    RuntimeDeepSeekPendingRequest, runtime_deepseek_rtk_wrapped_tool_arguments,
};
use crate::RuntimeHeapTrimmedBufferedResponseParts;
use anyhow::{Context, Result};
use prodex_cli::SUPER_DEEPSEEK_DEFAULT_MODEL;
use std::io::Read;
use std::time::{SystemTime, UNIX_EPOCH};

pub(in crate::runtime_launch::proxy_startup) fn runtime_deepseek_chat_buffered_response_parts(
    status: u16,
    mut response: reqwest::blocking::Response,
    request_id: u64,
    conversation_messages: Vec<serde_json::Value>,
    response_metadata: Option<serde_json::Value>,
    conversations: &RuntimeDeepSeekConversationStore,
) -> Result<RuntimeHeapTrimmedBufferedResponseParts> {
    let mut body = Vec::new();
    response
        .read_to_end(&mut body)
        .context("failed to read DeepSeek chat response body")?;
    let value: serde_json::Value =
        serde_json::from_slice(&body).context("failed to parse DeepSeek chat response JSON")?;
    let mut response = runtime_deepseek_responses_value_from_chat_value(&value, request_id);
    runtime_deepseek_merge_response_metadata(&mut response, response_metadata);
    if let Some(response_id) = response.get("id").and_then(serde_json::Value::as_str) {
        runtime_deepseek_store_conversation(
            conversations,
            response_id,
            conversation_messages,
            runtime_deepseek_chat_assistant_messages_from_response_value(&value),
        );
    }
    let body = serde_json::to_vec(&response).context("failed to serialize Responses JSON")?;
    Ok(RuntimeHeapTrimmedBufferedResponseParts {
        status,
        headers: vec![(
            "content-type".to_string(),
            b"application/json; charset=utf-8".to_vec(),
        )],
        body: body.into(),
    })
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_deepseek_take_pending_messages(
    pending: &RuntimeDeepSeekPendingMessages,
    request_id: u64,
) -> RuntimeDeepSeekPendingRequest {
    pending
        .lock()
        .ok()
        .and_then(|mut pending| pending.remove(&request_id))
        .unwrap_or_default()
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_deepseek_store_conversation(
    conversations: &RuntimeDeepSeekConversationStore,
    response_id: &str,
    mut messages: Vec<serde_json::Value>,
    assistant_messages: Vec<serde_json::Value>,
) {
    if response_id.trim().is_empty() {
        return;
    }
    messages.extend(
        assistant_messages
            .into_iter()
            .map(runtime_deepseek_normalize_assistant_tool_call_content),
    );
    if let Ok(mut conversations) = conversations.lock() {
        conversations.insert(response_id.to_string(), messages);
    }
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_deepseek_merge_response_metadata(
    response: &mut serde_json::Value,
    metadata: Option<serde_json::Value>,
) {
    let Some(metadata) = metadata else {
        return;
    };
    let Some(metadata_object) = metadata.as_object() else {
        return;
    };
    let response_metadata = response
        .as_object_mut()
        .map(|object| {
            object
                .entry("metadata".to_string())
                .or_insert_with(|| serde_json::json!({}))
        })
        .and_then(serde_json::Value::as_object_mut);
    let Some(response_metadata) = response_metadata else {
        return;
    };
    for (key, value) in metadata_object {
        match (response_metadata.get_mut(key), value.as_object()) {
            (Some(existing), Some(next)) => {
                if let Some(existing) = existing.as_object_mut() {
                    for (nested_key, nested_value) in next {
                        existing.insert(nested_key.clone(), nested_value.clone());
                    }
                }
            }
            _ => {
                response_metadata.insert(key.clone(), value.clone());
            }
        }
    }
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_deepseek_normalize_assistant_tool_call_content(
    mut message: serde_json::Value,
) -> serde_json::Value {
    let is_assistant = message.get("role").and_then(serde_json::Value::as_str) == Some("assistant");
    let has_tool_calls = message
        .get("tool_calls")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|tool_calls| !tool_calls.is_empty());
    if is_assistant
        && has_tool_calls
        && message
            .get("content")
            .is_none_or(serde_json::Value::is_null)
    {
        message["content"] = serde_json::Value::String(String::new());
    }
    message
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_deepseek_chat_assistant_messages_from_response_value(
    value: &serde_json::Value,
) -> Vec<serde_json::Value> {
    let Some(message) = value
        .get("choices")
        .and_then(serde_json::Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("message"))
    else {
        return Vec::new();
    };
    let content = message
        .get("content")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    let reasoning_content = message
        .get("reasoning_content")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    let tool_calls = message.get("tool_calls").cloned();
    if content.is_empty() && reasoning_content.is_empty() && tool_calls.is_none() {
        return Vec::new();
    }
    let has_tool_calls = tool_calls.is_some();
    let mut assistant = serde_json::json!({
        "role": "assistant",
        "content": if content.is_empty() {
            if has_tool_calls {
                serde_json::Value::String(String::new())
            } else {
                serde_json::Value::Null
            }
        } else {
            serde_json::Value::String(content.to_string())
        },
    });
    if !reasoning_content.is_empty() {
        assistant["reasoning_content"] = serde_json::Value::String(reasoning_content.to_string());
    }
    if let Some(tool_calls) = tool_calls {
        assistant["tool_calls"] = runtime_deepseek_rtk_wrapped_chat_tool_calls(tool_calls);
    }
    vec![assistant]
}

fn runtime_deepseek_rtk_wrapped_chat_tool_calls(
    mut tool_calls: serde_json::Value,
) -> serde_json::Value {
    let Some(tool_calls_array) = tool_calls.as_array_mut() else {
        return tool_calls;
    };
    for tool_call in tool_calls_array {
        let Some(function) = tool_call
            .get_mut("function")
            .and_then(serde_json::Value::as_object_mut)
        else {
            continue;
        };
        let name = function
            .get("name")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("tool_call")
            .to_string();
        let Some(arguments) = function
            .get("arguments")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string)
        else {
            continue;
        };
        function.insert(
            "arguments".to_string(),
            serde_json::Value::String(runtime_deepseek_rtk_wrapped_tool_arguments(
                &name, &arguments,
            )),
        );
    }
    tool_calls
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_deepseek_responses_value_from_chat_value(
    value: &serde_json::Value,
    request_id: u64,
) -> serde_json::Value {
    let response_id = value
        .get("id")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| format!("resp_deepseek_{request_id}"));
    let model = value
        .get("model")
        .and_then(serde_json::Value::as_str)
        .unwrap_or(SUPER_DEEPSEEK_DEFAULT_MODEL);
    let message = value
        .get("choices")
        .and_then(serde_json::Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("message"));
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
    if let Some(tool_calls) = message
        .and_then(|message| message.get("tool_calls"))
        .and_then(serde_json::Value::as_array)
    {
        for tool_call in tool_calls {
            if let Some(item) = runtime_deepseek_responses_tool_call_item(tool_call) {
                output.push(item);
            }
        }
    }
    let mut response = serde_json::json!({
        "id": response_id,
        "object": "response",
        "created_at": runtime_deepseek_created_at(),
        "model": model,
        "output": output,
    });
    if let Some(usage) = value
        .get("usage")
        .and_then(runtime_deepseek_responses_usage)
    {
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
    if !metadata.is_empty() {
        response["metadata"] = serde_json::json!({ "deepseek": metadata });
    }
    response
}

fn runtime_deepseek_responses_tool_call_item(
    tool_call: &serde_json::Value,
) -> Option<serde_json::Value> {
    let call_id = tool_call
        .get("id")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("call_0");
    let function = tool_call.get("function")?;
    let name = function
        .get("name")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("tool_call");
    let arguments = function
        .get("arguments")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("{}");
    if name == "tool_search" {
        let arguments = serde_json::from_str::<serde_json::Value>(arguments)
            .unwrap_or_else(|_| serde_json::json!({}));
        return Some(serde_json::json!({
            "type": "tool_search_call",
            "call_id": call_id,
            "execution": "client",
            "arguments": arguments,
        }));
    }
    let arguments = runtime_deepseek_rtk_wrapped_tool_arguments(name, arguments);
    let (namespace, name) = runtime_provider_split_flat_namespace_tool_name(name);
    let mut item = serde_json::json!({
        "type": "function_call",
        "call_id": call_id,
        "name": name,
        "arguments": arguments,
    });
    if let Some(namespace) = namespace {
        item["namespace"] = serde_json::Value::String(namespace);
    }
    if let Some(signature) = runtime_deepseek_chat_tool_call_thought_signature(tool_call) {
        item["gemini_thought_signature"] = serde_json::Value::String(signature.to_string());
    }
    Some(item)
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_deepseek_chat_tool_call_thought_signature(
    tool_call: &serde_json::Value,
) -> Option<String> {
    tool_call
        .get("extra_content")
        .and_then(|value| value.get("google"))
        .and_then(|value| value.get("thought_signature"))
        .and_then(serde_json::Value::as_str)
        .filter(|signature| !signature.trim().is_empty())
        .map(str::to_string)
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_deepseek_responses_usage(
    usage: &serde_json::Value,
) -> Option<serde_json::Value> {
    let input_tokens = usage
        .get("prompt_tokens")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let output_tokens = usage
        .get("completion_tokens")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let total_tokens = usage
        .get("total_tokens")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or_else(|| input_tokens.saturating_add(output_tokens));
    let cache_hit_tokens = usage
        .get("prompt_cache_hit_tokens")
        .and_then(serde_json::Value::as_u64);
    let cache_miss_tokens = usage
        .get("prompt_cache_miss_tokens")
        .and_then(serde_json::Value::as_u64);
    let reasoning_tokens = usage
        .get("completion_tokens_details")
        .and_then(|details| details.get("reasoning_tokens"))
        .and_then(serde_json::Value::as_u64);
    let mut response_usage = serde_json::json!({
        "input_tokens": input_tokens,
        "output_tokens": output_tokens,
        "total_tokens": total_tokens,
    });
    if let Some(cache_hit_tokens) = cache_hit_tokens {
        response_usage["input_tokens_details"] = serde_json::json!({
            "cached_tokens": cache_hit_tokens,
        });
    }
    if let Some(reasoning_tokens) = reasoning_tokens {
        response_usage["output_tokens_details"] = serde_json::json!({
            "reasoning_tokens": reasoning_tokens,
        });
    }
    if cache_hit_tokens.is_some() || cache_miss_tokens.is_some() {
        response_usage["metadata"] = serde_json::json!({
            "deepseek": {
                "prompt_cache_hit_tokens": cache_hit_tokens.unwrap_or(0),
                "prompt_cache_miss_tokens": cache_miss_tokens.unwrap_or(0),
            }
        });
    }
    Some(response_usage)
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_deepseek_created_at() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}
