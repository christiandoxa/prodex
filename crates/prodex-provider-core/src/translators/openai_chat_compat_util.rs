//! Shared JSON/text/tool helpers for the OpenAI chat-compatible bridge.

#[cfg(any(not(feature = "mojo"), test))]
use crate::translators::tool_args::{prefix_command_with_rtk, wrap_json_string_arg_with};

#[cfg(any(not(feature = "mojo"), test))]
use super::Value;
#[cfg(any(not(feature = "mojo"), test))]
use serde_json::json;

#[cfg(any(not(feature = "mojo"), test))]
pub(super) fn message_content_to_output_content_rust(content: Option<&Value>) -> Vec<Value> {
    match content {
        Some(Value::String(text)) if !text.is_empty() => {
            vec![output_text_rust(text)]
        }
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|item| {
                let text = item
                    .get("text")
                    .and_then(Value::as_str)
                    .or_else(|| item.get("content").and_then(Value::as_str))?;
                if text.is_empty() {
                    None
                } else {
                    Some(output_text_rust(text))
                }
            })
            .collect(),
        Some(other) => value_to_text(other)
            .filter(|text| !text.is_empty())
            .map(|text| vec![output_text_rust(&text)])
            .unwrap_or_default(),
        None => Vec::new(),
    }
}

#[cfg(any(not(feature = "mojo"), test))]
fn output_text_rust(text: &str) -> Value {
    json!({"type": "output_text", "text": text})
}

#[cfg(any(not(feature = "mojo"), test))]
pub(super) fn value_to_text(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.to_string()),
        Value::Array(items) => {
            let parts: Vec<&str> = items
                .iter()
                .filter_map(|item| {
                    item.get("text")
                        .and_then(Value::as_str)
                        .or_else(|| item.get("content").and_then(Value::as_str))
                })
                .filter(|text| !text.is_empty())
                .collect();
            if parts.is_empty() {
                None
            } else {
                Some(parts.join("\n"))
            }
        }
        Value::Object(obj) => obj
            .get("text")
            .and_then(Value::as_str)
            .map(str::to_string)
            .or_else(|| obj.get("content").and_then(value_to_text))
            .or_else(|| {
                obj.get("output_text")
                    .and_then(Value::as_str)
                    .map(str::to_string)
            }),
        _ => None,
    }
}

#[cfg(any(not(feature = "mojo"), test))]
pub(super) fn copy_if_present(
    source: &serde_json::Map<String, Value>,
    target: &mut serde_json::Map<String, Value>,
    keys: &[&str],
) {
    for key in keys {
        if let Some(value) = source.get(*key) {
            target.insert((*key).to_string(), value.clone());
        }
    }
}

#[cfg(any(not(feature = "mojo"), test))]
pub(super) fn copy_first_if_present(
    source: &serde_json::Map<String, Value>,
    target: &mut serde_json::Map<String, Value>,
    target_key: &str,
    source_keys: &[&str],
) {
    for key in source_keys {
        if let Some(value) = source.get(*key) {
            target.insert(target_key.to_string(), value.clone());
            return;
        }
    }
}

#[cfg(any(not(feature = "mojo"), test))]
pub(super) fn chat_usage_to_responses_usage_rust(usage: Option<&Value>) -> Option<Value> {
    let usage = usage?.as_object()?;
    let input_tokens = usage
        .get("prompt_tokens")
        .or_else(|| usage.get("input_tokens"))
        .and_then(Value::as_u64)?;
    let output_tokens = usage
        .get("completion_tokens")
        .or_else(|| usage.get("output_tokens"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let total_tokens = usage
        .get("total_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(input_tokens + output_tokens);
    Some(json!({
        "input_tokens": input_tokens,
        "output_tokens": output_tokens,
        "total_tokens": total_tokens,
    }))
}

#[cfg(any(not(feature = "mojo"), test))]
pub(super) fn chat_response_body_rust(
    response_id: &str,
    created_at: u64,
    model: &str,
    output: &[Value],
    usage: Option<&Value>,
) -> Vec<u8> {
    let mut response = json!({
        "id": response_id,
        "object": "response",
        "created_at": created_at,
        "model": model,
        "output": output,
    });
    if let Some(usage) = usage {
        response["usage"] = usage.clone();
    }
    serde_json::to_vec(&response).expect("chat compatibility response serializes")
}

#[cfg(any(not(feature = "mojo"), test))]
pub(super) fn stringify_arguments(value: &Value) -> String {
    value
        .as_str()
        .map(str::to_string)
        .unwrap_or_else(|| value.to_string())
}

#[cfg(any(not(feature = "mojo"), test))]
pub(super) fn split_flat_namespace_tool_name_rust(name: &str) -> (Option<String>, String) {
    let mut parts = name.split('.');
    let first = parts.next().unwrap_or(name).trim();
    let second = parts.next();
    match second {
        Some(rest) if !first.is_empty() => {
            let remainder = std::iter::once(rest)
                .chain(parts)
                .collect::<Vec<_>>()
                .join(".");
            if remainder.is_empty() {
                (None, name.to_string())
            } else {
                (Some(first.to_string()), remainder)
            }
        }
        _ => (None, name.to_string()),
    }
}

#[cfg(any(not(feature = "mojo"), test))]
pub(super) fn rtk_wrapped_tool_arguments_rust(name: &str, arguments: &str) -> String {
    if name != "functions.exec_command" && name != "exec_command" {
        return arguments.to_string();
    }
    wrap_json_string_arg_with(arguments, &["cmd"], prefix_command_with_rtk)
}
