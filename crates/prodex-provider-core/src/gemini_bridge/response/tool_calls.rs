//! Gemini response tool-call bridge helpers.

use crate::translators::{
    gemini_chat_assistant_tool_call_item_with_call_id, gemini_custom_apply_patch_input,
    gemini_preserve_tool_call_signatures, gemini_response_tool_call_added_item_with_call_id,
    gemini_response_tool_call_item_with_call_id, gemini_response_tool_call_raw_item_with_call_id,
};

pub fn gemini_provider_core_preserve_tool_call_signatures(messages: &mut [serde_json::Value]) {
    gemini_preserve_tool_call_signatures(messages);
}

pub fn gemini_provider_core_custom_tool_input_from_arguments(arguments: &str) -> String {
    serde_json::from_str::<serde_json::Value>(arguments)
        .map(|value| gemini_custom_apply_patch_input(&value))
        .unwrap_or_else(|_| arguments.to_string())
}

pub fn gemini_provider_core_response_tool_call_item(
    part: &serde_json::Value,
    function_call: &serde_json::Value,
    call_id_override: Option<&str>,
    mut blocked_tool_call_message: impl FnMut(&str, &serde_json::Value) -> Option<String>,
) -> serde_json::Value {
    let flat_name = function_call
        .get("name")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("tool_call");
    let args_value = function_call
        .get("args")
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}));
    if let Some(message) = blocked_tool_call_message(flat_name, &args_value) {
        return serde_json::json!({
            "type": "message",
            "role": "assistant",
            "content": [{
                "type": "output_text",
                "text": message,
            }],
        });
    }
    gemini_response_tool_call_item_with_call_id(part, function_call, call_id_override)
}

pub fn gemini_provider_core_response_tool_call_added_item(
    part: &serde_json::Value,
    function_call: &serde_json::Value,
    call_id_override: Option<&str>,
) -> Option<serde_json::Value> {
    gemini_response_tool_call_added_item_with_call_id(part, function_call, call_id_override)
}

pub fn gemini_provider_core_response_tool_call_raw_item(
    part: &serde_json::Value,
    flat_name: &str,
    arguments: &str,
    call_id_override: Option<&str>,
) -> serde_json::Value {
    gemini_response_tool_call_raw_item_with_call_id(part, flat_name, arguments, call_id_override)
}

pub fn gemini_provider_core_chat_assistant_tool_call_item(
    part: &serde_json::Value,
    function_call: &serde_json::Value,
    call_id_override: Option<&str>,
) -> serde_json::Value {
    gemini_chat_assistant_tool_call_item_with_call_id(part, function_call, call_id_override)
}
