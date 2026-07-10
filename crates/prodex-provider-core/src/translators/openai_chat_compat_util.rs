//! Shared JSON/text/tool helpers for the OpenAI chat-compatible bridge.

use crate::translators::tool_args::{prefix_command_with_rtk, wrap_json_string_arg_with};

use super::{Value, json};

pub(super) fn message_content_to_output_content(content: Option<&Value>) -> Vec<Value> {
    match content {
        Some(Value::String(text)) if !text.is_empty() => {
            vec![json!({"type": "output_text", "text": text})]
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
                    Some(json!({"type": "output_text", "text": text}))
                }
            })
            .collect(),
        Some(other) => value_to_text(other)
            .filter(|text| !text.is_empty())
            .map(|text| vec![json!({"type": "output_text", "text": text})])
            .unwrap_or_default(),
        None => Vec::new(),
    }
}

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

pub(super) fn chat_usage_to_responses_usage(usage: Option<&Value>) -> Option<Value> {
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

pub(super) fn stringify_arguments(value: &Value) -> String {
    value
        .as_str()
        .map(str::to_string)
        .unwrap_or_else(|| value.to_string())
}

pub(super) fn split_flat_namespace_tool_name(name: &str) -> (Option<String>, String) {
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

pub(super) fn rtk_wrapped_tool_arguments(name: &str, arguments: &str) -> String {
    if name != "functions.exec_command" && name != "exec_command" {
        return arguments.to_string();
    }
    wrap_json_string_arg_with(arguments, &["cmd"], prefix_command_with_rtk)
}
