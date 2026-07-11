//! DeepSeek Responses input item to chat message push helpers.

mod fields;

pub(super) use self::fields::{
    deepseek_provider_core_input_tool_call_id, deepseek_provider_core_input_tool_output_call_id,
    deepseek_provider_core_input_tool_output_text, deepseek_provider_core_mcp_call_has_result,
};

use self::fields::{
    deepseek_provider_core_input_tool_call_arguments, deepseek_provider_core_input_tool_call_name,
};
use super::super::{
    deepseek_provider_core_json_string, deepseek_provider_core_responses_content_text_value,
    deepseek_provider_core_tool_call_thought_signature,
};

pub(super) fn deepseek_provider_core_push_chat_tool_call_message(
    object: &serde_json::Map<String, serde_json::Value>,
    call_id: String,
    messages: &mut Vec<serde_json::Value>,
) {
    let name = deepseek_provider_core_input_tool_call_name(object);
    let arguments = deepseek_provider_core_input_tool_call_arguments(object);
    let mut tool_call = serde_json::json!({
        "id": call_id,
        "type": "function",
        "function": {
            "name": name,
            "arguments": arguments,
        },
    });
    if let Some(signature) = deepseek_provider_core_tool_call_thought_signature(object) {
        tool_call["gemini_thought_signature"] = serde_json::Value::String(signature);
    }
    messages.push(serde_json::json!({
        "role": "assistant",
        "content": "",
        "tool_calls": [tool_call],
    }));
}

pub(super) fn deepseek_provider_core_push_chat_custom_tool_call_message(
    object: &serde_json::Map<String, serde_json::Value>,
    call_id: String,
    messages: &mut Vec<serde_json::Value>,
) {
    let name = deepseek_provider_core_input_tool_call_name(object);
    let input = deepseek_provider_core_json_string(object, &["input"])
        .or_else(|| {
            object
                .get("input")
                .map(deepseek_provider_core_responses_content_text_value)
        })
        .unwrap_or_default();
    let arguments = serde_json::to_string(&serde_json::json!({ "input": input }))
        .unwrap_or_else(|_| "{\"input\":\"\"}".to_string());
    deepseek_provider_core_push_chat_tool_call_message_with_arguments(
        call_id, name, arguments, messages,
    );
}

pub(super) fn deepseek_provider_core_push_chat_local_shell_call_message(
    object: &serde_json::Map<String, serde_json::Value>,
    call_id: String,
    messages: &mut Vec<serde_json::Value>,
) {
    let command = object
        .get("action")
        .and_then(|action| action.get("command"))
        .and_then(|command| {
            command.as_array().map(|parts| {
                parts
                    .iter()
                    .filter_map(serde_json::Value::as_str)
                    .collect::<Vec<_>>()
                    .join(" ")
            })
        })
        .filter(|command| !command.trim().is_empty())
        .or_else(|| deepseek_provider_core_json_string(object, &["command"]))
        .unwrap_or_default();
    let mut shell_arguments = serde_json::Map::new();
    shell_arguments.insert("command".to_string(), serde_json::Value::String(command));
    deepseek_provider_core_copy_shell_argument(object, &mut shell_arguments, "cwd");
    deepseek_provider_core_copy_shell_argument(object, &mut shell_arguments, "timeout");
    deepseek_provider_core_copy_shell_argument(object, &mut shell_arguments, "env");
    let arguments = serde_json::to_string(&serde_json::Value::Object(shell_arguments))
        .unwrap_or_else(|_| "{\"command\":\"\"}".to_string());
    deepseek_provider_core_push_chat_tool_call_message_with_arguments(
        call_id,
        "shell_command".to_string(),
        arguments,
        messages,
    );
}

pub(super) fn deepseek_provider_core_copy_shell_argument(
    object: &serde_json::Map<String, serde_json::Value>,
    arguments: &mut serde_json::Map<String, serde_json::Value>,
    key: &str,
) {
    if let Some(value) = object
        .get(key)
        .or_else(|| object.get("action").and_then(|action| action.get(key)))
    {
        arguments.insert(key.to_string(), value.clone());
    }
}

pub(super) fn deepseek_provider_core_push_chat_tool_call_message_with_arguments(
    call_id: String,
    name: String,
    arguments: String,
    messages: &mut Vec<serde_json::Value>,
) {
    let tool_call = serde_json::json!({
        "id": call_id,
        "type": "function",
        "function": {
            "name": name,
            "arguments": arguments,
        },
    });
    messages.push(serde_json::json!({
        "role": "assistant",
        "content": "",
        "tool_calls": [tool_call],
    }));
}
