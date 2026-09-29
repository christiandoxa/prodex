//! Chat message and legacy function-tool shaping for Kiro request compatibility.

use serde_json::{Value, json};

pub fn kiro_provider_core_prompt_from_chat_messages(messages: &[Value]) -> String {
    let sections = messages
        .iter()
        .filter_map(kiro_provider_core_prompt_section)
        .collect::<Vec<_>>();
    if sections.is_empty() {
        "User:\n".to_string()
    } else {
        sections.join("\n\n")
    }
}

fn kiro_provider_core_prompt_section(message: &Value) -> Option<String> {
    let role = message
        .get("role")
        .and_then(Value::as_str)
        .unwrap_or("message");
    let mut block = message
        .get("content")
        .and_then(kiro_provider_core_prompt_message_text)
        .unwrap_or_default();
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
    (!block.trim().is_empty()).then(|| {
        format!(
            "{}:\n{}",
            kiro_provider_core_prompt_role_label(role),
            block.trim()
        )
    })
}

pub fn kiro_provider_core_responses_items_from_chat_message(message: &Value) -> Vec<Value> {
    let message =
        serde_json::to_string(message).expect("Kiro chat message serializes for Mojo rewrite");
    let mut input = prodex_mojo_core::rich::KiroKernelInput::new(
        prodex_mojo_core::rich::KiroKernelOperation::RawResponsesItemsFromChatMessage,
    );
    input.input = Some(&message);
    let body = prodex_mojo_core::rich::kiro_kernel(input)
        .unwrap_or_else(|error| panic!("Mojo Kiro chat-message rewrite failed: {error:?}"));
    serde_json::from_slice(&body).unwrap_or_else(|error| {
        panic!("Mojo Kiro chat-message rewrite returned invalid JSON: {error}")
    })
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
        Value::Array(items) => kiro_provider_core_prompt_array_text(items),
        Value::Object(object) => kiro_provider_core_prompt_object_text(object),
        _ => None,
    }
}

fn kiro_provider_core_prompt_array_text(items: &[Value]) -> Option<String> {
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

fn kiro_provider_core_prompt_object_text(
    object: &serde_json::Map<String, Value>,
) -> Option<String> {
    if let Some(text) = object.get("text").and_then(Value::as_str) {
        return (!text.trim().is_empty()).then(|| text.to_string());
    }
    if let Some(text) = object
        .get("content")
        .and_then(kiro_provider_core_prompt_message_text)
    {
        return Some(text);
    }
    object
        .get("output")
        .and_then(kiro_provider_core_prompt_message_text)
}

fn kiro_provider_core_prompt_role_label(role: &str) -> &'static str {
    match role {
        "system" => "System",
        "assistant" => "Assistant",
        "tool" => "Tool",
        _ => "User",
    }
}
