use super::{
    AnthropicChatRequest, anthropic_tool_choice, anthropic_tools, anthropic_web_search_tool,
};
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;

const DEFAULT_MAX_TOKENS: u64 = 4096;

pub(super) fn build_anthropic_chat_request_rust(
    system: &[String],
    messages: Vec<Value>,
    chat: &Map<String, Value>,
) -> Result<AnthropicChatRequest, String> {
    let mut request = Map::new();
    request.insert(
        "model".to_string(),
        chat.get("model")
            .cloned()
            .unwrap_or_else(|| Value::String("auto".to_string())),
    );
    request.insert("messages".to_string(), Value::Array(messages));
    request.insert(
        "max_tokens".to_string(),
        chat.get("max_tokens")
            .cloned()
            .unwrap_or_else(|| Value::from(DEFAULT_MAX_TOKENS)),
    );
    request.insert(
        "stream".to_string(),
        Value::Bool(chat.get("stream").and_then(Value::as_bool).unwrap_or(false)),
    );
    if !system.is_empty() {
        request.insert("system".to_string(), Value::String(system.join("\n\n")));
    }
    for field in ["temperature", "top_p"] {
        if let Some(value) = chat.get(field) {
            request.insert(field.to_string(), value.clone());
        }
    }
    if let Some(stop) = chat.get("stop") {
        request.insert(
            "stop_sequences".to_string(),
            match stop {
                Value::String(_) => Value::Array(vec![stop.clone()]),
                Value::Array(_) => stop.clone(),
                _ => return Err("Responses `stop` must be a string or array".to_string()),
            },
        );
    }
    let mut degradation_details = BTreeMap::new();
    let mut tools = match chat.get("tools") {
        Some(tools) => anthropic_tools(tools)?,
        None => Vec::new(),
    };
    if let Some(options) = chat.get("web_search_options") {
        let (tool, ignored_context_size) = anthropic_web_search_tool(options)?;
        tools.push(tool);
        if let Some(context_size) = ignored_context_size {
            degradation_details.insert(
                "web_search_options.search_context_size".to_string(),
                json!({"from": context_size, "to": "provider_default"}),
            );
        }
    }
    if !tools.is_empty() {
        request.insert("tools".to_string(), Value::Array(tools));
    }
    if let Some(tool_choice) = chat.get("tool_choice") {
        match anthropic_tool_choice(tool_choice)? {
            Some(choice) => {
                request.insert("tool_choice".to_string(), choice);
            }
            None => {
                request.remove("tools");
                degradation_details.clear();
            }
        }
    }
    Ok((request, degradation_details))
}

pub(super) fn validate_anthropic_chat_fields(chat: &Map<String, Value>) -> Result<(), String> {
    for field in chat.keys() {
        match field.as_str() {
            "model" | "messages" | "max_tokens" | "stream" | "temperature" | "top_p" | "stop"
            | "tools" | "tool_choice" | "stream_options" | "web_search_options" => {}
            "parallel_tool_calls" if chat.get(field).and_then(Value::as_bool) == Some(true) => {}
            "parallel_tool_calls" => {
                return Err(
                    "Anthropic Messages only accepts `parallel_tool_calls=true`".to_string()
                );
            }
            _ => {
                return Err(format!(
                    "Anthropic Messages does not translate chat field `{field}`"
                ));
            }
        }
    }
    Ok(())
}
