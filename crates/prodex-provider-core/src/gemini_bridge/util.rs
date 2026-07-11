//! Small Gemini bridge value parsing and collection helpers.

pub fn gemini_provider_core_collect_string_values(
    value: Option<&serde_json::Value>,
    values: &mut Vec<String>,
) {
    match value {
        Some(serde_json::Value::String(value)) => values.push(value.to_string()),
        Some(serde_json::Value::Array(items)) => {
            for item in items {
                gemini_provider_core_collect_string_values(Some(item), values);
            }
        }
        _ => {}
    }
}

pub fn gemini_provider_core_collect_path_values(
    value: Option<&serde_json::Value>,
    paths: &mut Vec<std::path::PathBuf>,
) {
    match value {
        Some(serde_json::Value::String(path)) if !path.trim().is_empty() => {
            paths.push(std::path::PathBuf::from(path.trim()));
        }
        Some(serde_json::Value::Array(items)) => {
            for item in items {
                gemini_provider_core_collect_path_values(Some(item), paths);
            }
        }
        _ => {}
    }
}

pub fn gemini_provider_core_collect_input_texts(
    value: Option<&serde_json::Value>,
    texts: &mut Vec<String>,
) {
    match value {
        Some(serde_json::Value::String(text)) => texts.push(text.clone()),
        Some(serde_json::Value::Array(items)) => {
            for item in items {
                gemini_provider_core_collect_input_texts(Some(item), texts);
            }
        }
        Some(serde_json::Value::Object(object)) => {
            if let Some(text) = object
                .get("text")
                .or_else(|| object.get("content"))
                .and_then(serde_json::Value::as_str)
            {
                texts.push(text.to_string());
            }
            gemini_provider_core_collect_input_texts(object.get("content"), texts);
        }
        _ => {}
    }
}

pub fn gemini_provider_core_bool_value(value: &serde_json::Value) -> Option<bool> {
    match value {
        serde_json::Value::Bool(value) => Some(*value),
        serde_json::Value::Number(number) => Some(number.as_i64().unwrap_or_default() != 0),
        serde_json::Value::String(value) => gemini_provider_core_bool_str(value),
        _ => None,
    }
}

pub fn gemini_provider_core_bool_str(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

pub fn gemini_provider_core_parse_command_specific_tool(value: &str) -> Option<(String, String)> {
    let value = value.trim();
    let open = value.find('(')?;
    let close = value.rfind(')')?;
    if close <= open {
        return None;
    }
    let tool = value[..open].trim();
    let pattern = value[open + 1..close].trim();
    if tool.is_empty() || pattern.is_empty() {
        return None;
    }
    Some((tool.to_string(), pattern.to_string()))
}

pub fn gemini_provider_core_skip_context_path_name(name: &str) -> bool {
    matches!(
        name,
        ".git" | "node_modules" | "target" | "dist" | "build" | "__pycache__" | "vendor"
    )
}

pub fn gemini_provider_core_stream_error(value: &serde_json::Value) -> Option<(String, String)> {
    let error = value.get("error")?;
    let code = error
        .get("status")
        .or_else(|| error.get("code"))
        .and_then(|code| {
            code.as_str()
                .map(str::to_string)
                .or_else(|| code.as_i64().map(|code| code.to_string()))
        })
        .unwrap_or_else(|| "provider_stream_error".to_string());
    let message = error
        .get("message")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("Gemini stream returned an embedded provider error")
        .to_string();
    Some((code, message))
}
