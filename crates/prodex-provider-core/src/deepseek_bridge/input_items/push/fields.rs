//! DeepSeek Responses input item field extraction helpers.

use crate::{
    deepseek_provider_core_json_string, deepseek_provider_core_json_string_at_path,
    deepseek_provider_core_responses_content_text,
};

pub(in crate::deepseek_bridge::input_items) fn deepseek_provider_core_input_tool_call_id(
    object: &serde_json::Map<String, serde_json::Value>,
) -> String {
    deepseek_provider_core_json_string(object, &["call_id", "tool_call_id", "id"])
        .unwrap_or_else(|| "call_0".to_string())
}

pub(in crate::deepseek_bridge::input_items) fn deepseek_provider_core_input_tool_output_call_id(
    object: &serde_json::Map<String, serde_json::Value>,
) -> String {
    deepseek_provider_core_json_string(object, &["call_id", "tool_call_id", "id"])
        .unwrap_or_else(|| "call_0".to_string())
}

pub(super) fn deepseek_provider_core_input_tool_call_name(
    object: &serde_json::Map<String, serde_json::Value>,
) -> String {
    deepseek_provider_core_json_string(object, &["name", "tool_name"])
        .or_else(|| deepseek_provider_core_json_string_at_path(object, &["function", "name"]))
        .unwrap_or_else(|| "tool_call".to_string())
}

pub(super) fn deepseek_provider_core_input_tool_call_arguments(
    object: &serde_json::Map<String, serde_json::Value>,
) -> String {
    deepseek_provider_core_json_string(object, &["arguments", "input"])
        .or_else(|| deepseek_provider_core_json_string_at_path(object, &["function", "arguments"]))
        .or_else(|| {
            object
                .get("arguments")
                .or_else(|| object.get("input"))
                .map(|value| value.to_string())
        })
        .unwrap_or_else(|| "{}".to_string())
}

pub(in crate::deepseek_bridge::input_items) fn deepseek_provider_core_mcp_call_has_result(
    object: &serde_json::Map<String, serde_json::Value>,
) -> bool {
    object.contains_key("output")
        || object.contains_key("content")
        || object.contains_key("result")
        || object.contains_key("error")
}

pub(in crate::deepseek_bridge::input_items) fn deepseek_provider_core_input_tool_output_text(
    object: &serde_json::Map<String, serde_json::Value>,
) -> String {
    deepseek_provider_core_json_string(object, &["output", "content", "result", "error"])
        .or_else(|| {
            object
                .get("output")
                .or_else(|| object.get("content"))
                .or_else(|| object.get("result"))
                .or_else(|| object.get("error"))
                .map(|value| deepseek_provider_core_responses_content_text(Some(value)))
        })
        .unwrap_or_default()
}
