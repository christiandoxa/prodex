//! Gemini GenerateContent response conversion into chat-compatible assistant messages.

use serde_json::{Value, json};

use super::gemini_function_call_id;
use super::response_media::{gemini_media_content_item_from_part, gemini_text_from_special_part};
use super::response_metadata::gemini_response_metadata;
use super::response_tool_calls::gemini_chat_assistant_tool_call_item_with_call_id;

pub(crate) fn gemini_chat_assistant_messages_from_generate_value(
    value: &Value,
    request_id: u64,
    mut blocked_tool_call_message: impl FnMut(&str, &Value) -> Option<String>,
) -> Vec<Value> {
    let Some(parts) = value
        .get("candidates")
        .and_then(Value::as_array)
        .and_then(|candidates| candidates.first())
        .and_then(|candidate| candidate.get("content"))
        .and_then(|content| content.get("parts"))
        .and_then(Value::as_array)
    else {
        return Vec::new();
    };
    let mut text = String::new();
    let mut reasoning_content = String::new();
    let mut gemini_content = Vec::new();
    let mut native_parts = Vec::new();
    let mut tool_calls = Vec::new();
    let suppress_visible_text = parts.iter().any(|part| part.get("functionCall").is_some());
    for (index, part) in parts.iter().enumerate() {
        gemini_append_chat_response_text(
            part,
            suppress_visible_text,
            &mut text,
            &mut reasoning_content,
        );
        gemini_append_chat_response_media(part, &mut gemini_content, &mut native_parts);
        gemini_append_chat_response_tool_call(
            part,
            request_id,
            index,
            &mut text,
            &mut tool_calls,
            &mut blocked_tool_call_message,
        );
    }
    gemini_chat_assistant_message(
        value,
        text,
        reasoning_content,
        gemini_content,
        native_parts,
        tool_calls,
    )
    .into_iter()
    .collect()
}

fn gemini_append_chat_response_text(
    part: &Value,
    suppress_visible_text: bool,
    text: &mut String,
    reasoning_content: &mut String,
) {
    if let Some(part_text) = part.get("text").and_then(Value::as_str)
        && part
            .get("thought")
            .and_then(Value::as_bool)
            .unwrap_or(false)
    {
        reasoning_content.push_str(part_text);
    } else if !suppress_visible_text
        && let Some(part_text) =
            crate::gemini_bridge::gemini_provider_core_visible_text_from_part(part)
    {
        text.push_str(&part_text);
    }
    if let Some(part_text) = gemini_text_from_special_part(part) {
        if !text.is_empty() {
            text.push('\n');
        }
        text.push_str(&part_text);
    }
}

fn gemini_append_chat_response_media(
    part: &Value,
    gemini_content: &mut Vec<Value>,
    native_parts: &mut Vec<Value>,
) {
    if let Some(content_item) = gemini_media_content_item_from_part(part) {
        gemini_content.push(content_item);
        native_parts.push(part.clone());
    }
    if part.get("videoMetadata").is_some() && !native_parts.contains(part) {
        native_parts.push(part.clone());
    }
}

fn gemini_append_chat_response_tool_call(
    part: &Value,
    request_id: u64,
    index: usize,
    text: &mut String,
    tool_calls: &mut Vec<Value>,
    blocked_tool_call_message: &mut impl FnMut(&str, &Value) -> Option<String>,
) {
    let Some(function_call) = part.get("functionCall") else {
        return;
    };
    let name = function_call
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("tool_call");
    let call_id = gemini_function_call_id(function_call, request_id, index);
    let args = function_call
        .get("args")
        .cloned()
        .unwrap_or_else(|| json!({}));
    if let Some(blocked) = blocked_tool_call_message(name, &args) {
        if !text.is_empty() {
            text.push('\n');
        }
        text.push_str(&blocked);
        return;
    }
    tool_calls.push(gemini_chat_assistant_tool_call_item_with_call_id(
        part,
        function_call,
        Some(&call_id),
    ));
}

fn gemini_chat_assistant_message(
    value: &Value,
    text: String,
    reasoning_content: String,
    gemini_content: Vec<Value>,
    native_parts: Vec<Value>,
    tool_calls: Vec<Value>,
) -> Option<Value> {
    if text.is_empty()
        && reasoning_content.is_empty()
        && gemini_content.is_empty()
        && tool_calls.is_empty()
    {
        return None;
    }
    let mut assistant = json!({
        "role": "assistant",
        "content": gemini_chat_assistant_content(&text, &tool_calls),
    });
    if !reasoning_content.is_empty() {
        assistant["reasoning_content"] = Value::String(reasoning_content);
    }
    if !gemini_content.is_empty() {
        assistant["gemini_media_content"] = Value::Array(gemini_content);
    }
    if !native_parts.is_empty() {
        assistant["gemini_native_parts"] = Value::Array(native_parts);
    }
    if !tool_calls.is_empty() {
        assistant["tool_calls"] = Value::Array(tool_calls);
    }
    if let Some(metadata) = gemini_response_metadata(value) {
        assistant["gemini_metadata"] = metadata;
    }
    Some(assistant)
}

fn gemini_chat_assistant_content(text: &str, tool_calls: &[Value]) -> Value {
    if !text.is_empty() {
        Value::String(text.to_string())
    } else if tool_calls.is_empty() {
        Value::Null
    } else {
        Value::String(String::new())
    }
}
