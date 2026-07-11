//! Pure DeepSeek simple-request eligibility probe.
//!
//! This keeps runtime send-path shims out of provider-specific request-shape details.

mod input;

use self::input::deepseek_simple_input_item;

pub fn deepseek_provider_core_simple_request(
    body: &[u8],
    mut has_stored_previous_response_id: impl FnMut(&str) -> bool,
) -> bool {
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(body) else {
        return false;
    };
    let Some(object) = value.as_object() else {
        return false;
    };
    if object.contains_key("web_search_options") || object.contains_key("safety_identifier") {
        return false;
    }
    if let Some(response_format) = object.get("response_format")
        && !deepseek_provider_core_response_format(response_format)
    {
        return false;
    }
    if let Some(previous_response_id) = object
        .get("previous_response_id")
        .and_then(serde_json::Value::as_str)
        .filter(|id| !id.trim().is_empty())
        && has_stored_previous_response_id(previous_response_id)
    {
        return false;
    }
    if let Some(tools) = object.get("tools")
        && !deepseek_provider_core_function_tools(tools)
    {
        return false;
    }
    if let Some(tool_choice) = object.get("tool_choice")
        && !deepseek_provider_core_tool_choice(tool_choice)
    {
        return false;
    }
    match object.get("input") {
        Some(serde_json::Value::String(_)) => true,
        Some(serde_json::Value::Array(items)) => items.iter().all(deepseek_simple_input_item),
        _ => false,
    }
}

fn deepseek_provider_core_function_tools(value: &serde_json::Value) -> bool {
    let Some(tools) = value.as_array() else {
        return false;
    };
    tools.iter().all(|tool| {
        tool.get("type").and_then(serde_json::Value::as_str) == Some("function")
            && tool
                .get("function")
                .and_then(|function| function.get("name"))
                .and_then(serde_json::Value::as_str)
                .is_some_and(|name| !name.trim().is_empty())
    })
}

fn deepseek_provider_core_response_format(value: &serde_json::Value) -> bool {
    value
        .get("type")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|value| {
            matches!(
                value,
                "text" | "json_object" | "json_schema" | "json" | "structured_output"
            )
        })
}

fn deepseek_provider_core_tool_choice(value: &serde_json::Value) -> bool {
    if value.is_null() {
        return true;
    }
    if let Some(choice) = value.as_str() {
        return matches!(choice, "auto" | "none" | "required");
    }
    value.as_object().is_some_and(|object| {
        object.get("type").and_then(serde_json::Value::as_str) == Some("function")
            && object
                .get("name")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|name| !name.trim().is_empty())
    })
}
