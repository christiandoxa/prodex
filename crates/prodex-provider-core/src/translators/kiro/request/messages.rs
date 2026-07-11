//! Chat message and legacy function-tool shaping for Kiro request compatibility.

#[path = "messages/text.rs"]
mod text;

use self::text::kiro_provider_core_chat_message_text;
use serde_json::{Value, json};

pub fn kiro_provider_core_prompt_from_chat_messages(messages: &[Value]) -> String {
    let mut sections = Vec::new();
    for message in messages {
        let role = message
            .get("role")
            .and_then(Value::as_str)
            .unwrap_or("message");
        let mut block = String::new();
        if let Some(content) = message
            .get("content")
            .and_then(kiro_provider_core_prompt_message_text)
        {
            block.push_str(&content);
        }
        if let Some(tool_calls) = message.get("tool_calls").and_then(Value::as_array) {
            for tool_call in tool_calls {
                let name = tool_call
                    .get("function")
                    .and_then(|v| v.get("name"))
                    .and_then(Value::as_str)
                    .unwrap_or("tool_call");
                let arguments = tool_call
                    .get("function")
                    .and_then(|v| v.get("arguments"))
                    .and_then(Value::as_str)
                    .unwrap_or("{}");
                if !block.is_empty() {
                    block.push('\n');
                }
                block.push_str(&format!("Tool call {name}: {arguments}"));
            }
        }
        if block.trim().is_empty() {
            continue;
        }
        sections.push(format!(
            "{}:\n{}",
            kiro_provider_core_prompt_role_label(role),
            block.trim()
        ));
    }
    if sections.is_empty() {
        "User:\n".to_string()
    } else {
        sections.join("\n\n")
    }
}

pub fn kiro_provider_core_responses_items_from_chat_message(message: &Value) -> Vec<Value> {
    let Some(object) = message.as_object() else {
        return Vec::new();
    };
    let role = object.get("role").and_then(Value::as_str).unwrap_or("user");
    let mut items = Vec::new();
    if !matches!(role, "tool" | "function")
        && let Some(text) = object
            .get("content")
            .and_then(kiro_provider_core_chat_message_text)
    {
        items.push(json!({
            "type": "message",
            "role": role,
            "content": [{
                "type": "input_text",
                "text": text,
            }],
        }));
    }
    if role == "assistant"
        && let Some(tool_calls) = object.get("tool_calls").and_then(Value::as_array)
    {
        for tool_call in tool_calls {
            let Some(function) = tool_call.get("function").and_then(Value::as_object) else {
                continue;
            };
            let name = function
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("tool_call");
            let arguments = function
                .get("arguments")
                .and_then(Value::as_str)
                .unwrap_or("{}");
            items.push(json!({
                "type": "function_call",
                "call_id": tool_call
                    .get("id")
                    .and_then(Value::as_str)
                    .unwrap_or("call_kiro"),
                "name": name,
                "arguments": arguments,
            }));
        }
    }
    if role == "assistant"
        && !object.contains_key("tool_calls")
        && let Some(function_call) = object.get("function_call").and_then(Value::as_object)
    {
        let name = function_call
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("tool_call");
        let arguments = function_call
            .get("arguments")
            .and_then(Value::as_str)
            .unwrap_or("{}");
        let call_id = function_call
            .get("call_id")
            .or_else(|| function_call.get("id"))
            .and_then(Value::as_str)
            .unwrap_or(name);
        items.push(json!({
            "type": "function_call",
            "call_id": call_id,
            "name": name,
            "arguments": arguments,
        }));
    }
    if matches!(role, "tool" | "function") {
        let output = object
            .get("content")
            .and_then(kiro_provider_core_chat_message_text)
            .unwrap_or_default();
        items.push(json!({
            "type": "function_call_output",
            "call_id": object
                .get("tool_call_id")
                .or_else(|| object.get("call_id"))
                .or_else(|| object.get("name"))
                .and_then(Value::as_str)
                .unwrap_or("call_kiro"),
            "output": output,
        }));
    }
    items
}

pub fn kiro_provider_core_tool_from_legacy_chat_function(function: &Value) -> Option<Value> {
    let function = function.as_object()?;
    let name = function.get("name")?.as_str()?.trim();
    if name.is_empty() {
        return None;
    }
    let mut tool_function = serde_json::Map::new();
    tool_function.insert("name".to_string(), Value::String(name.to_string()));
    if let Some(description) = function
        .get("description")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|description| !description.is_empty())
    {
        tool_function.insert(
            "description".to_string(),
            Value::String(description.to_string()),
        );
    }
    if let Some(parameters) = function.get("parameters") {
        tool_function.insert("parameters".to_string(), parameters.clone());
    }
    Some(json!({
        "type": "function",
        "function": Value::Object(tool_function),
    }))
}

pub fn kiro_provider_core_tool_choice_from_legacy_chat_function_call(
    function_call: &Value,
) -> Option<Value> {
    if let Some(choice) = function_call
        .as_str()
        .filter(|choice| matches!(*choice, "auto" | "none"))
    {
        return Some(Value::String(choice.to_string()));
    }
    let object = function_call.as_object()?;
    let name = object.get("name")?.as_str()?.trim();
    if name.is_empty() {
        return None;
    }
    Some(json!({
        "type": "function",
        "function": {
            "name": name,
        }
    }))
}

fn kiro_provider_core_prompt_message_text(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => (!text.trim().is_empty()).then(|| text.to_string()),
        Value::Array(items) => {
            let mut text = String::new();
            for item in items {
                if let Some(chunk) = kiro_provider_core_prompt_message_text(item) {
                    if !text.is_empty() {
                        text.push('\n');
                    }
                    text.push_str(&chunk);
                }
            }
            (!text.trim().is_empty()).then_some(text)
        }
        Value::Object(object) => {
            if let Some(text) = object.get("text").and_then(Value::as_str) {
                return (!text.trim().is_empty()).then(|| text.to_string());
            }
            if let Some(text) = object
                .get("content")
                .and_then(kiro_provider_core_prompt_message_text)
            {
                return Some(text);
            }
            if let Some(tool_output) = object
                .get("output")
                .and_then(kiro_provider_core_prompt_message_text)
            {
                return Some(tool_output);
            }
            None
        }
        _ => None,
    }
}

fn kiro_provider_core_prompt_role_label(role: &str) -> &'static str {
    match role {
        "system" => "System",
        "assistant" => "Assistant",
        "tool" => "Tool",
        _ => "User",
    }
}
