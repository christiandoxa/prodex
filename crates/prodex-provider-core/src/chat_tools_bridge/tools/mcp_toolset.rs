//! MCP toolset expansion for chat-compatible function tools.

use super::super::util::{provider_core_function_name_segment, provider_core_json_string};

pub(super) fn provider_core_mcp_toolset_function_tools(
    tool: &serde_json::Value,
) -> Vec<serde_json::Value> {
    let Some(object) = tool.as_object() else {
        return Vec::new();
    };
    let tool_type = object
        .get("type")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    if !matches!(tool_type, "mcp" | "mcp_toolset") {
        return Vec::new();
    }
    let Some(server_name) = provider_core_json_string(
        object,
        &["mcp_server_name", "server_label", "server_name", "name"],
    )
    .filter(|server_name| !server_name.trim().is_empty()) else {
        return Vec::new();
    };
    let Some(server_label) = provider_core_function_name_segment(&server_name) else {
        return Vec::new();
    };
    provider_core_mcp_toolset_tool_names(object)
        .into_iter()
        .filter_map(|tool_name| {
            let tool_label = provider_core_function_name_segment(&tool_name)?;
            let name = if tool_name.trim().starts_with("mcp__") {
                tool_label
            } else {
                format!("mcp__{server_label}__{tool_label}")
            };
            let description = provider_core_json_string(object, &["description"])
                .filter(|description| !description.trim().is_empty())
                .unwrap_or_else(|| format!("MCP tool {tool_name} from {server_name}."));
            Some(serde_json::json!({
                "type": "function",
                "function": {
                    "name": name,
                    "description": description,
                    "parameters": {
                        "type": "object",
                        "additionalProperties": true
                    }
                }
            }))
        })
        .collect()
}

fn provider_core_mcp_toolset_tool_names(
    object: &serde_json::Map<String, serde_json::Value>,
) -> Vec<String> {
    let mut names = Vec::new();
    if let Some(allowed_tools) = object
        .get("allowed_tools")
        .and_then(serde_json::Value::as_array)
    {
        names.extend(
            allowed_tools
                .iter()
                .filter_map(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .map(str::to_string),
        );
    }
    let default_enabled = object
        .get("default_config")
        .and_then(serde_json::Value::as_object)
        .and_then(|config| config.get("enabled"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(true);
    if let Some(configs) = object.get("configs").and_then(serde_json::Value::as_object) {
        names.extend(configs.iter().filter_map(|(tool_name, config)| {
            let enabled = config
                .as_object()
                .and_then(|config| config.get("enabled"))
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(default_enabled);
            enabled.then(|| tool_name.to_string())
        }));
    }
    names.sort();
    names.dedup();
    names
}
