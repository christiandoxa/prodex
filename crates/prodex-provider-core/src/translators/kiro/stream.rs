//! Kiro provider stream compatibility helpers.

use serde_json::{Value, json};

pub fn kiro_provider_core_chat_completion_chunk(
    chat_completion_id: &str,
    model: Option<&str>,
    delta: Value,
    finish_reason: Option<&str>,
) -> Result<Vec<u8>, serde_json::Error> {
    let mut chunk = json!({
        "id": chat_completion_id,
        "object": "chat.completion.chunk",
        "choices": [{
            "index": 0,
            "delta": delta,
        }],
    });
    if let Some(model) = model.filter(|value| !value.is_empty()) {
        chunk["model"] = Value::String(model.to_string());
    }
    if let Some(finish_reason) = finish_reason {
        chunk["choices"][0]["finish_reason"] = Value::String(finish_reason.to_string());
    }
    Ok(format!("data: {}\n\n", serde_json::to_string(&chunk)?).into_bytes())
}

pub fn kiro_provider_core_chat_completion_role_delta() -> Value {
    json!({"role": "assistant"})
}

pub fn kiro_provider_core_chat_completion_empty_delta() -> Value {
    json!({})
}

pub fn kiro_provider_core_chat_completion_text_delta(text: &str, include_role: bool) -> Value {
    if include_role {
        json!({"role": "assistant", "content": text})
    } else {
        json!({"content": text})
    }
}

pub fn kiro_provider_core_chat_completion_tool_call_delta(
    tool_call_id: &str,
    name: &str,
    arguments: &str,
    include_role: bool,
) -> Value {
    let tool_call = json!({
        "index": 0,
        "id": tool_call_id,
        "type": "function",
        "function": {
            "name": name,
            "arguments": arguments,
        }
    });
    if include_role {
        json!({"role": "assistant", "tool_calls": [tool_call]})
    } else {
        json!({"tool_calls": [tool_call]})
    }
}

pub fn kiro_provider_core_output_text_delta_event(
    sequence_number: u64,
    created_at: u64,
    response_id: &str,
    delta: &str,
) -> Value {
    json!({
        "type": "response.output_text.delta",
        "sequence_number": sequence_number,
        "created_at": created_at,
        "response_id": response_id,
        "delta": delta,
    })
}

pub fn kiro_provider_core_response_created_event(
    sequence_number: u64,
    created_at: u64,
    response_id: &str,
) -> Value {
    json!({
        "type": "response.created",
        "sequence_number": sequence_number,
        "created_at": created_at,
        "response": {"id": response_id},
    })
}

pub fn kiro_provider_core_output_item_added_event(sequence_number: u64, item: &Value) -> Value {
    json!({
        "type": "response.output_item.added",
        "sequence_number": sequence_number,
        "item": item,
    })
}

pub fn kiro_provider_core_output_item_done_event(
    sequence_number: u64,
    response_id: &str,
    item: &Value,
) -> Value {
    json!({
        "type": "response.output_item.done",
        "sequence_number": sequence_number,
        "item": item,
        "response_id": response_id,
    })
}

pub fn kiro_provider_core_response_completed_event(
    sequence_number: u64,
    created_at: u64,
    response: &Value,
) -> Value {
    json!({
        "type": "response.completed",
        "sequence_number": sequence_number,
        "created_at": created_at,
        "response": response,
    })
}

pub fn kiro_provider_core_tool_call_arguments_delta_chat_value(
    tool_call_id: &str,
    arguments: &str,
) -> Value {
    json!({
        "choices": [{
            "delta": {
                "tool_calls": [{
                    "id": tool_call_id,
                    "function": {
                        "arguments": arguments,
                    }
                }]
            }
        }]
    })
}

pub fn kiro_provider_core_stream_content_text(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => (!text.is_empty()).then(|| text.clone()),
        Value::Array(items) => {
            let mut text = String::new();
            for item in items {
                if let Some(chunk) = kiro_provider_core_stream_content_text(item) {
                    text.push_str(&chunk);
                }
            }
            (!text.is_empty()).then_some(text)
        }
        Value::Object(object) => {
            if let Some(text) = object.get("text").and_then(Value::as_str) {
                return (!text.is_empty()).then(|| text.to_string());
            }
            object
                .get("content")
                .and_then(kiro_provider_core_stream_content_text)
        }
        _ => None,
    }
}

pub fn kiro_provider_core_stream_tool_call_item(
    tool_call_id: &str,
    title: Option<&str>,
    status: Option<&str>,
    kind: Option<&str>,
    raw_input: Option<&Value>,
) -> Value {
    let name = kiro_provider_core_stream_tool_name(title, kind);
    let arguments = kiro_provider_core_stream_tool_arguments(raw_input);
    json!({
        "type": "function_call",
        "call_id": tool_call_id,
        "name": name,
        "namespace": "kiro",
        "arguments": arguments,
        "metadata": {
            "kiro": {
                "title": title.unwrap_or("tool_call"),
                "status": status,
                "kind": kind,
            }
        }
    })
}

#[allow(clippy::too_many_arguments)]
pub fn kiro_provider_core_acp_responses_tool_call_item(
    tool_call_id: &str,
    title: Option<&str>,
    status: Option<&str>,
    kind: Option<&str>,
    raw_input: Option<&Value>,
    raw_output: Option<&Value>,
    content: Option<&[Value]>,
    locations: Option<&[Value]>,
) -> Value {
    json!({
        "type": "function_call",
        "call_id": tool_call_id,
        "name": kiro_provider_core_stream_tool_name(title, kind),
        "namespace": "kiro",
        "arguments": kiro_provider_core_stream_tool_arguments(raw_input),
        "metadata": {
            "kiro": {
                "title": title.unwrap_or("tool_call"),
                "status": status,
                "kind": kind,
                "raw_output": raw_output,
                "content": content,
                "locations": locations,
            }
        }
    })
}

pub fn kiro_provider_core_acp_chat_tool_call_item(
    tool_call_id: &str,
    title: Option<&str>,
    kind: Option<&str>,
    raw_input: Option<&Value>,
) -> Value {
    json!({
        "id": tool_call_id,
        "type": "function",
        "function": {
            "name": kiro_provider_core_stream_tool_name(title, kind),
            "arguments": kiro_provider_core_stream_tool_arguments(raw_input),
        },
    })
}

pub fn kiro_provider_core_acp_usage_update_json(
    used: u64,
    size: u64,
    cost: Option<(f64, &str)>,
) -> Value {
    json!({
        "used": used,
        "size": size,
        "remaining": size.saturating_sub(used),
        "cost": cost.map(|(amount, currency)| json!({
            "amount": amount,
            "currency": currency,
        })),
    })
}

pub fn kiro_provider_core_stream_tool_arguments(raw_input: Option<&Value>) -> String {
    raw_input
        .and_then(|value| serde_json::to_string(value).ok())
        .unwrap_or_else(|| "{}".to_string())
}

fn kiro_provider_core_stream_tool_name(title: Option<&str>, kind: Option<&str>) -> String {
    let candidate = title
        .filter(|title| !title.trim().is_empty())
        .or(kind)
        .unwrap_or("tool_call");
    let mut normalized = String::new();
    let mut last_was_separator = false;
    for ch in candidate.chars().flat_map(char::to_lowercase) {
        if ch.is_ascii_alphanumeric() {
            normalized.push(ch);
            last_was_separator = false;
        } else if !last_was_separator && !normalized.is_empty() {
            normalized.push('_');
            last_was_separator = true;
        }
    }
    normalized.trim_matches('_').to_string()
}
