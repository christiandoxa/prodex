//! Gemini simple-response fast-path classification.

pub fn gemini_provider_core_simple_response(
    value: &serde_json::Value,
    mut blocked_tool_call_message: impl FnMut(&str, &serde_json::Value) -> Option<String>,
) -> bool {
    let Some(candidate) = value
        .get("candidates")
        .and_then(serde_json::Value::as_array)
        .and_then(|candidates| candidates.first())
    else {
        return true;
    };
    let Some(parts) = candidate
        .get("content")
        .and_then(|content| content.get("parts"))
        .and_then(serde_json::Value::as_array)
    else {
        return true;
    };
    gemini_provider_core_message_only_parts(parts)
        || gemini_provider_core_function_call_only_parts(parts, &mut blocked_tool_call_message)
}

fn gemini_provider_core_message_only_parts(parts: &[serde_json::Value]) -> bool {
    parts.iter().all(gemini_provider_core_message_only_part)
}

fn gemini_provider_core_message_only_part(part: &serde_json::Value) -> bool {
    if part.get("functionCall").is_some() {
        return false;
    }
    if part
        .get("text")
        .and_then(serde_json::Value::as_str)
        .is_some()
    {
        return part.as_object().is_some_and(|object| {
            object
                .keys()
                .all(|key| matches!(key.as_str(), "text" | "thought"))
        });
    }
    part.as_object().is_some_and(|object| {
        object.keys().all(|key| {
            matches!(
                key.as_str(),
                "inlineData"
                    | "inline_data"
                    | "fileData"
                    | "file_data"
                    | "executableCode"
                    | "codeExecutionResult"
                    | "videoMetadata"
            )
        })
    })
}

fn gemini_provider_core_function_call_only_parts(
    parts: &[serde_json::Value],
    blocked_tool_call_message: &mut impl FnMut(&str, &serde_json::Value) -> Option<String>,
) -> bool {
    !parts.is_empty()
        && parts.iter().all(|part| {
            gemini_provider_core_part_is_function_call_only(part, blocked_tool_call_message)
        })
}

fn gemini_provider_core_part_is_function_call_only(
    part: &serde_json::Value,
    blocked_tool_call_message: &mut impl FnMut(&str, &serde_json::Value) -> Option<String>,
) -> bool {
    if part.get("text").is_some()
        && !part
            .get("thought")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false)
    {
        return false;
    }
    let Some(function_call) = part
        .get("functionCall")
        .and_then(serde_json::Value::as_object)
    else {
        return part.as_object().is_some_and(|object| {
            object
                .keys()
                .all(|key| matches!(key.as_str(), "text" | "thought"))
        });
    };
    let Some(name) = function_call
        .get("name")
        .and_then(serde_json::Value::as_str)
        .filter(|name| !name.trim().is_empty())
    else {
        return false;
    };
    let args = function_call
        .get("args")
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}));
    blocked_tool_call_message(name, &args).is_none()
        && part.as_object().is_some_and(|object| {
            object
                .keys()
                .all(|key| matches!(key.as_str(), "functionCall" | "thoughtSignature"))
        })
}
