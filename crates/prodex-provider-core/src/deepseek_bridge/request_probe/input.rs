//! DeepSeek simple-request input item checks.

pub(super) fn deepseek_simple_input_item(value: &serde_json::Value) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    match object.get("type").and_then(serde_json::Value::as_str) {
        Some("message") | None => {}
        Some("function_call_output") => {
            return object
                .get("call_id")
                .is_some_and(serde_json::Value::is_string)
                && object.get("output").is_some();
        }
        Some("mcp_tool_result") | Some("mcp_call_output") => {
            return object
                .get("call_id")
                .or_else(|| object.get("tool_call_id"))
                .or_else(|| object.get("id"))
                .is_some_and(serde_json::Value::is_string)
                && ["output", "content", "result", "error"]
                    .iter()
                    .any(|key| object.contains_key(*key));
        }
        Some("custom_tool_call_output") => {
            return object
                .get("call_id")
                .is_some_and(serde_json::Value::is_string)
                && ["output", "content", "result", "error"]
                    .iter()
                    .any(|key| object.contains_key(*key));
        }
        Some("function_call") => {
            let has_name = object
                .get("name")
                .or_else(|| object.get("tool_name"))
                .or_else(|| {
                    object
                        .get("function")
                        .and_then(|function| function.get("name"))
                })
                .is_some_and(serde_json::Value::is_string);
            return has_name
                && object
                    .get("call_id")
                    .or_else(|| object.get("tool_call_id"))
                    .or_else(|| object.get("id"))
                    .is_none_or(serde_json::Value::is_string);
        }
        Some("mcp_call") => {
            let has_name = object
                .get("name")
                .or_else(|| object.get("tool_name"))
                .or_else(|| {
                    object
                        .get("function")
                        .and_then(|function| function.get("name"))
                })
                .is_some_and(serde_json::Value::is_string);
            return has_name
                && object
                    .get("call_id")
                    .or_else(|| object.get("tool_call_id"))
                    .or_else(|| object.get("id"))
                    .is_none_or(serde_json::Value::is_string);
        }
        Some("custom_tool_call") => {
            let has_name = object
                .get("name")
                .or_else(|| object.get("tool_name"))
                .is_some_and(serde_json::Value::is_string);
            return has_name
                && object
                    .get("call_id")
                    .or_else(|| object.get("tool_call_id"))
                    .or_else(|| object.get("id"))
                    .is_none_or(serde_json::Value::is_string);
        }
        Some("local_shell_call") => {
            let has_call_id = object
                .get("call_id")
                .or_else(|| object.get("tool_call_id"))
                .or_else(|| object.get("id"))
                .is_none_or(serde_json::Value::is_string);
            let has_command = object
                .get("command")
                .is_some_and(serde_json::Value::is_string)
                || object
                    .get("action")
                    .and_then(|action| action.get("command"))
                    .and_then(serde_json::Value::as_array)
                    .is_some_and(|parts| parts.iter().all(serde_json::Value::is_string));
            return has_call_id && has_command;
        }
        Some(_) => return false,
    }
    let role = object
        .get("role")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("user");
    if !matches!(role, "system" | "user" | "assistant" | "tool") {
        return false;
    }
    match object.get("content") {
        Some(serde_json::Value::String(_)) | None => true,
        Some(serde_json::Value::Array(parts)) => parts.iter().all(deepseek_simple_content_item),
        _ => false,
    }
}

fn deepseek_simple_content_item(value: &serde_json::Value) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    if object
        .get("type")
        .is_some_and(|value| !matches!(value.as_str(), Some("input_text" | "output_text" | "text")))
    {
        return false;
    }
    object
        .get("text")
        .or_else(|| object.get("input_text"))
        .or_else(|| object.get("output_text"))
        .is_some_and(serde_json::Value::is_string)
}
