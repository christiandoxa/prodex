//! Gemini simple-request fast-path classification.

mod builtin;

use self::builtin::{gemini_provider_core_builtin_tool, gemini_provider_core_builtin_tool_choice};

pub fn gemini_provider_core_simple_request(body: &[u8]) -> bool {
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(body) else {
        return false;
    };
    let Some(object) = value.as_object() else {
        return false;
    };
    if let Some(tools) = object.get("tools") {
        let Some(tools) = tools.as_array() else {
            return false;
        };
        if !tools.iter().all(gemini_provider_core_builtin_tool) {
            return false;
        }
        if object
            .get("tool_choice")
            .is_some_and(|value| !gemini_provider_core_builtin_tool_choice(value))
        {
            return false;
        }
    }
    match object.get("input") {
        Some(serde_json::Value::String(_)) => true,
        Some(serde_json::Value::Array(items)) => items.iter().all(gemini_simple_input_item),
        _ => false,
    }
}

fn gemini_simple_input_item(item: &serde_json::Value) -> bool {
    let Some(object) = item.as_object() else {
        return false;
    };
    if object
        .get("type")
        .is_some_and(|value| value.as_str() != Some("message"))
    {
        return false;
    }
    let role = object
        .get("role")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("user");
    if !matches!(role, "system" | "user" | "assistant" | "tool") {
        return false;
    }
    match object.get("content") {
        Some(serde_json::Value::String(_)) | None => {}
        Some(serde_json::Value::Array(items)) if items.iter().all(gemini_simple_content_item) => {}
        _ => return false,
    }
    if object.get("gemini_native_parts").is_some() {
        return false;
    }
    if role == "assistant" {
        return object
            .get("tool_calls")
            .is_none_or(gemini_simple_tool_calls);
    }
    if role == "tool" {
        return object
            .get("tool_call_id")
            .is_none_or(serde_json::Value::is_string)
            && object.get("name").is_none_or(serde_json::Value::is_string);
    }
    object.get("tool_calls").is_none()
}

fn gemini_simple_content_item(value: &serde_json::Value) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    if object
        .get("text")
        .and_then(serde_json::Value::as_str)
        .is_some()
        || object
            .get("content")
            .and_then(serde_json::Value::as_str)
            .is_some()
    {
        return object.get("type").is_none_or(|value| {
            matches!(value.as_str(), Some("input_text" | "output_text" | "text"))
        });
    }
    false
}

fn gemini_simple_tool_calls(value: &serde_json::Value) -> bool {
    let Some(tool_calls) = value.as_array() else {
        return false;
    };
    tool_calls.iter().all(gemini_simple_tool_call)
}

fn gemini_simple_tool_call(value: &serde_json::Value) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    if object
        .get("id")
        .is_some_and(|value| !serde_json::Value::is_string(value))
    {
        return false;
    }
    let Some(function) = object
        .get("function")
        .and_then(serde_json::Value::as_object)
    else {
        return false;
    };
    function
        .get("name")
        .is_some_and(serde_json::Value::is_string)
        && function
            .get("arguments")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|arguments| serde_json::from_str::<serde_json::Value>(arguments).is_ok())
}
