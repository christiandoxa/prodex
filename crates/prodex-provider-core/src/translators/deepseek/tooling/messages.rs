//! DeepSeek request input/message conversion helpers.

#[path = "messages/chat_items.rs"]
mod chat_items;
#[path = "messages/input_tool_calls.rs"]
mod input_tool_calls;
#[path = "messages/local_shell.rs"]
mod local_shell;
#[path = "messages/thought_signature.rs"]
mod thought_signature;

use crate::deepseek_provider_core_responses_content_text;
use chat_items::{deepseek_message_content_text, deepseek_message_tool_calls};
use input_tool_calls::{
    deepseek_input_custom_tool_call_message, deepseek_input_function_call_message,
    deepseek_input_mcp_call_messages,
};
use local_shell::deepseek_input_local_shell_call_message;
use serde_json::{Value, json};
pub(crate) use thought_signature::deepseek_tool_call_thought_signature_object;

pub(crate) fn deepseek_messages_from_request(value: &Value) -> Vec<Value> {
    let mut messages = if let Some(messages) = value.get("messages").and_then(Value::as_array) {
        messages.clone()
    } else if let Some(input) = value.get("input") {
        match input {
            Value::String(text) => vec![json!({"role":"user","content": text})],
            Value::Array(items) => items
                .iter()
                .flat_map(deepseek_messages_from_input_item)
                .collect(),
            _ => vec![json!({"role":"user","content":""})],
        }
    } else {
        vec![json!({"role":"user","content":""})]
    };
    if let Some(instructions) = value
        .get("instructions")
        .and_then(Value::as_str)
        .filter(|text| !text.trim().is_empty())
    {
        messages.insert(0, json!({ "role": "system", "content": instructions }));
    }
    messages
}

fn deepseek_messages_from_input_item(item: &Value) -> Vec<Value> {
    match item.get("type").and_then(Value::as_str) {
        Some("function_call") => {
            return vec![
                deepseek_input_function_call_message(item)
                    .unwrap_or_else(|| json!({"role":"assistant","content":""})),
            ];
        }
        Some("mcp_call") => {
            return deepseek_input_mcp_call_messages(item)
                .unwrap_or_else(|| vec![json!({"role":"assistant","content":""})]);
        }
        Some("custom_tool_call") => {
            return vec![
                deepseek_input_custom_tool_call_message(item)
                    .unwrap_or_else(|| json!({"role":"assistant","content":""})),
            ];
        }
        Some("local_shell_call") => {
            return vec![
                deepseek_input_local_shell_call_message(item)
                    .unwrap_or_else(|| json!({"role":"assistant","content":""})),
            ];
        }
        Some("function_call_output") => {
            let call_id =
                deepseek_tool_output_call_id(item).unwrap_or_else(|| "call_1".to_string());
            return vec![json!({
                "role": "tool",
                "tool_call_id": call_id,
                "content": deepseek_tool_output_content(item),
            })];
        }
        Some("custom_tool_call_output") => {
            let call_id =
                deepseek_tool_output_call_id(item).unwrap_or_else(|| "call_1".to_string());
            return vec![json!({
                "role": "tool",
                "tool_call_id": call_id,
                "content": deepseek_tool_output_content(item),
            })];
        }
        Some("mcp_tool_result") | Some("mcp_call_output") => {
            let call_id =
                deepseek_tool_output_call_id(item).unwrap_or_else(|| "call_1".to_string());
            return vec![json!({
                "role": "tool",
                "tool_call_id": call_id,
                "content": deepseek_tool_output_content(item),
            })];
        }
        _ => {}
    }
    let role = item.get("role").and_then(Value::as_str).unwrap_or("user");
    let content = item
        .get("content")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| deepseek_message_content_text(item.get("content")))
        .or_else(|| item.get("text").and_then(Value::as_str).map(str::to_string))
        .unwrap_or_default();
    if role == "tool" {
        let mut message = json!({
            "role": "tool",
            "content": content,
        });
        if let Some(call_id) = deepseek_tool_output_call_id(item)
            && let Some(object) = message.as_object_mut()
        {
            object.insert(
                "tool_call_id".to_string(),
                Value::String(call_id.to_string()),
            );
        }
        return vec![message];
    }
    let tool_calls = deepseek_message_tool_calls(item);
    let mut message = json!({
        "role": role,
        "content": content,
    });
    if let Some(tool_calls) = tool_calls
        && let Some(object) = message.as_object_mut()
    {
        object.insert("tool_calls".to_string(), Value::Array(tool_calls));
    }
    vec![message]
}

fn deepseek_tool_output_call_id(item: &Value) -> Option<String> {
    item.get("call_id")
        .or_else(|| item.get("tool_call_id"))
        .or_else(|| item.get("id"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn deepseek_tool_output_content(item: &Value) -> String {
    item.get("output")
        .or_else(|| item.get("content"))
        .or_else(|| item.get("result"))
        .or_else(|| item.get("error"))
        .map(|value| deepseek_provider_core_responses_content_text(Some(value)))
        .unwrap_or_default()
}
