//! DeepSeek response-message compatibility helpers.
//!
//! Pure message shaping only; transport, retry, and runtime state stay outside provider-core.

mod mojo;

#[cfg(test)]
mod mojo_tests;

use super::deepseek_provider_core_rtk_wrapped_tool_arguments;

pub fn deepseek_provider_core_repair_tool_call_adjacency(messages: &mut Vec<serde_json::Value>) {
    mojo::repair_adjacency(messages)
}

pub fn deepseek_provider_core_normalize_thinking_tool_call_messages(
    messages: &mut [serde_json::Value],
) {
    mojo::normalize_thinking(messages)
}

pub fn deepseek_provider_core_normalize_assistant_tool_call_content(
    message: serde_json::Value,
) -> serde_json::Value {
    mojo::normalize_content(message)
}

pub fn deepseek_provider_core_merge_response_metadata(
    response: &mut serde_json::Value,
    metadata: Option<serde_json::Value>,
) {
    mojo::merge_metadata(response, metadata)
}

pub fn deepseek_provider_core_chat_assistant_messages_from_response_value(
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
        assistant["tool_calls"] = deepseek_provider_core_rtk_wrapped_chat_tool_calls(tool_calls);
    }
    vec![assistant]
}

fn deepseek_provider_core_rtk_wrapped_chat_tool_calls(
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
            serde_json::Value::String(deepseek_provider_core_rtk_wrapped_tool_arguments(
                &name, &arguments,
            )),
        );
    }
    tool_calls
}
