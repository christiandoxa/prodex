//! Tool-choice translation for OpenAI Responses tool shapes.

use super::util::{
    provider_core_flatten_namespace_tool_name, provider_core_json_string,
    provider_core_json_string_at_path,
};

pub fn provider_core_chat_tool_choice_from_responses_request(
    value: &serde_json::Value,
    thinking_enabled: bool,
) -> Option<serde_json::Value> {
    if thinking_enabled {
        return None;
    }
    let choice = value.get("tool_choice")?;
    if let Some(choice) = choice.as_str() {
        return matches!(choice, "auto" | "none" | "required")
            .then(|| serde_json::Value::String(choice.to_string()));
    }

    let object = choice.as_object()?;
    let choice_type = object
        .get("type")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    if choice_type != "function" && !choice_type.starts_with("mcp") {
        return None;
    }
    let name = provider_core_json_string(object, &["name"])
        .or_else(|| provider_core_json_string_at_path(object, &["function", "name"]))
        .filter(|name| !name.trim().is_empty())?;
    let name = provider_core_tool_choice_function_name(object, choice_type, &name);
    Some(serde_json::json!({
        "type": "function",
        "function": {
            "name": name,
        },
    }))
}

fn provider_core_tool_choice_function_name(
    object: &serde_json::Map<String, serde_json::Value>,
    choice_type: &str,
    name: &str,
) -> String {
    let Some(mut namespace) = provider_core_json_string(
        object,
        &[
            "namespace",
            "server_label",
            "mcp_server_name",
            "server_name",
        ],
    )
    .or_else(|| provider_core_json_string_at_path(object, &["function", "namespace"]))
    .filter(|namespace| !namespace.trim().is_empty()) else {
        return name.to_string();
    };
    if choice_type.starts_with("mcp") && !namespace.starts_with("mcp__") {
        namespace = format!("mcp__{namespace}");
    }
    provider_core_flatten_namespace_tool_name(&namespace, name)
}
