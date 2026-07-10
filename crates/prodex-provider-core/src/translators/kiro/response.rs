//! Kiro provider response compatibility helpers.

use super::stream::kiro_provider_core_stream_content_text;
use serde_json::{Value, json};

pub fn kiro_provider_core_chat_completion_value_from_response(
    response: &Value,
    request_id: u64,
) -> Value {
    let id = response
        .get("id")
        .and_then(Value::as_str)
        .map(|id| format!("chatcmpl_{id}"))
        .unwrap_or_else(|| format!("chatcmpl_kiro_{request_id}"));
    let created = response
        .get("created_at")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let model = response
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or("kiro-cli");
    let output = response
        .get("output")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();
    let assistant_text = output
        .iter()
        .find(|item| item.get("type").and_then(Value::as_str) == Some("message"))
        .and_then(|item| item.get("content"))
        .and_then(Value::as_array)
        .and_then(|content| content.first())
        .and_then(|item| item.get("text"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    let tool_calls = output
        .iter()
        .filter(|item| item.get("type").and_then(Value::as_str) == Some("function_call"))
        .map(|item| {
            json!({
                "id": item.get("call_id").and_then(Value::as_str).unwrap_or("call_kiro"),
                "type": "function",
                "function": {
                    "name": item.get("name").and_then(Value::as_str).unwrap_or("tool_call"),
                    "arguments": item.get("arguments").and_then(Value::as_str).unwrap_or("{}"),
                }
            })
        })
        .collect::<Vec<_>>();
    let has_tool_calls = !tool_calls.is_empty();
    let mut message = json!({
        "role": "assistant",
        "content": if assistant_text.is_empty() && has_tool_calls {
            Value::Null
        } else {
            Value::String(assistant_text.to_string())
        },
    });
    if has_tool_calls {
        message["tool_calls"] = Value::Array(tool_calls);
    }
    if let Some(reasoning_content) = response
        .get("metadata")
        .and_then(|metadata| metadata.get("kiro"))
        .and_then(|kiro| kiro.get("reasoning_content"))
        .and_then(Value::as_str)
        .filter(|reasoning| !reasoning.is_empty())
    {
        message["reasoning_content"] = Value::String(reasoning_content.to_string());
    }
    let finish_reason = kiro_provider_core_chat_completion_finish_reason(response, has_tool_calls);
    let mut choice = json!({
        "index": 0,
        "message": message,
        "finish_reason": finish_reason,
    });
    if response.get("status").and_then(Value::as_str) == Some("failed")
        && let Some(error) = response.get("error")
    {
        choice["message"]["refusal"] = error
            .get("message")
            .cloned()
            .unwrap_or(Value::String("Kiro request failed".to_string()));
    }
    let mut completion = json!({
        "id": id,
        "object": "chat.completion",
        "created": created,
        "model": model,
        "choices": [choice],
    });
    if let Some(requested_model) = response.get("requested_model") {
        completion["requested_model"] = requested_model.clone();
    }
    if let Some(metadata) = response.get("metadata") {
        completion["metadata"] = metadata.clone();
    }
    completion
}

pub fn kiro_provider_core_apply_response_runtime_metadata(
    response: &mut Value,
    profile_name: &str,
    requested_model: Option<&str>,
    created_at: Option<u64>,
) {
    if let Some(created_at) = created_at {
        response["created_at"] = Value::from(created_at);
    }
    response["metadata"]["kiro"]["profile_name"] = Value::String(profile_name.to_string());
    if let Some(model) = requested_model.filter(|model| !model.is_empty()) {
        response["requested_model"] = Value::String(model.to_string());
    }
}

pub fn kiro_provider_core_response_has_tool_calls(response: &Value) -> bool {
    response
        .get("output")
        .and_then(Value::as_array)
        .is_some_and(|items| {
            items
                .iter()
                .any(|item| item.get("type").and_then(Value::as_str) == Some("function_call"))
        })
}

pub fn kiro_provider_core_model_list_value(model_catalog: &[Value]) -> Value {
    json!({
        "object": "list",
        "data": model_catalog,
    })
}

pub fn kiro_provider_core_model_value_or_not_found(
    model_catalog: &[Value],
    model_id: &str,
) -> (u16, Value) {
    if let Some(model) = model_catalog.iter().find(|model| {
        model
            .get("id")
            .and_then(Value::as_str)
            .is_some_and(|id| id.eq_ignore_ascii_case(model_id))
    }) {
        return (200, model.clone());
    }
    (
        404,
        json!({
            "error": {
                "message": format!("model '{model_id}' is not available for kiro"),
                "type": "invalid_request_error",
                "code": "model_not_found",
            }
        }),
    )
}

pub fn kiro_provider_core_invalid_request_error_value(message: &str, code: &str) -> Value {
    json!({
        "error": {
            "message": message,
            "type": "invalid_request_error",
            "code": code,
        }
    })
}

pub fn kiro_provider_core_unsupported_path_error_value(path: &str) -> Value {
    kiro_provider_core_invalid_request_error_value(
        &format!("Kiro provider does not support {path} yet"),
        "unsupported_path",
    )
}

pub fn kiro_provider_core_chat_completion_finish_reason(
    response: &Value,
    has_tool_calls: bool,
) -> &'static str {
    if has_tool_calls {
        return "tool_calls";
    }
    if response
        .get("incomplete_details")
        .and_then(|details| details.get("reason"))
        .and_then(Value::as_str)
        == Some("max_output_tokens")
    {
        return "length";
    }
    "stop"
}

pub fn kiro_provider_core_chat_completion_finish_reason_from_response(
    response: &Value,
) -> &'static str {
    kiro_provider_core_chat_completion_finish_reason(
        response,
        kiro_provider_core_response_has_tool_calls(response),
    )
}

pub fn kiro_provider_core_anthropic_message_value_from_response(
    response: &Value,
    requested_model: &str,
) -> Value {
    let output = response
        .get("output")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();
    let text = output
        .iter()
        .find(|item| item.get("type").and_then(Value::as_str) == Some("message"))
        .and_then(|item| item.get("content"))
        .and_then(kiro_provider_core_stream_content_text)
        .unwrap_or_default();
    let tool_use_blocks = output
        .iter()
        .filter(|item| item.get("type").and_then(Value::as_str) == Some("function_call"))
        .map(kiro_provider_core_anthropic_tool_use_block)
        .collect::<Vec<_>>();
    let mut content = tool_use_blocks;
    if !text.is_empty() {
        content.push(json!({
            "type": "text",
            "text": text,
        }));
    }
    let usage = response.get("usage").cloned().unwrap_or_else(|| json!({}));
    let stop_reason = if content
        .iter()
        .any(|block| block.get("type").and_then(Value::as_str) == Some("tool_use"))
    {
        "tool_use"
    } else {
        kiro_provider_core_anthropic_stop_reason(response)
    };
    json!({
        "id": response.get("id").cloned().unwrap_or_else(|| Value::String("msg_kiro".to_string())),
        "type": "message",
        "role": "assistant",
        "model": requested_model,
        "content": content,
        "stop_reason": stop_reason,
        "stop_sequence": Value::Null,
        "usage": {
            "input_tokens": usage.get("input_tokens").cloned().unwrap_or(Value::from(0)),
            "output_tokens": usage.get("output_tokens").cloned().unwrap_or(Value::from(0)),
        }
    })
}

fn kiro_provider_core_anthropic_stop_reason(response: &Value) -> &'static str {
    response
        .get("metadata")
        .and_then(|metadata| metadata.get("kiro"))
        .and_then(|kiro| kiro.get("stop_reason"))
        .and_then(Value::as_str)
        .map(|reason| match reason {
            "max_output_tokens" => "max_tokens",
            "tool_use" => "tool_use",
            _ => "end_turn",
        })
        .unwrap_or("end_turn")
}

fn kiro_provider_core_anthropic_tool_use_block(item: &Value) -> Value {
    let input = item
        .get("arguments")
        .and_then(Value::as_str)
        .and_then(|arguments| serde_json::from_str::<Value>(arguments).ok())
        .unwrap_or_else(|| json!({}));
    json!({
        "type": "tool_use",
        "id": item.get("call_id").cloned().unwrap_or_else(|| Value::String("call_kiro".to_string())),
        "name": item.get("name").cloned().unwrap_or_else(|| Value::String("tool_call".to_string())),
        "input": input,
    })
}
