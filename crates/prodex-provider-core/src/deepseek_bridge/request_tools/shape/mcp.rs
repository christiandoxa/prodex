//! DeepSeek MCP tool-shape validators.

use crate::deepseek_provider_core_json_string;

pub(in crate::deepseek_bridge::request_tools) fn deepseek_provider_core_validate_mcp_function_tool_shape(
    object: &serde_json::Map<String, serde_json::Value>,
    provider_label: &str,
) -> Result<(), String> {
    let Some(name) = deepseek_provider_core_json_string(object, &["name"])
        .filter(|name| !name.trim().is_empty())
    else {
        return Err(format!(
            "{provider_label} MCP function tools require a name"
        ));
    };
    let has_schema = [
        "parameters",
        "parametersJsonSchema",
        "input_schema",
        "schema",
    ]
    .iter()
    .any(|key| object.get(*key).is_some());
    if !has_schema {
        return Err(format!(
            "{provider_label} MCP function tool `{name}` requires a schema"
        ));
    }
    Ok(())
}

pub(in crate::deepseek_bridge::request_tools) fn deepseek_provider_core_validate_mcp_toolset_shape(
    object: &serde_json::Map<String, serde_json::Value>,
    provider_label: &str,
) -> Result<(), String> {
    let declares_tools = object.contains_key("allowed_tools") || object.contains_key("configs");
    if !declares_tools {
        return Ok(());
    }
    let Some(server_name) = deepseek_provider_core_json_string(
        object,
        &["mcp_server_name", "server_label", "server_name", "name"],
    )
    .filter(|server_name| !server_name.trim().is_empty()) else {
        return Err(format!(
            "{provider_label} MCP toolsets require a server name"
        ));
    };
    let allowed_count = object
        .get("allowed_tools")
        .and_then(serde_json::Value::as_array)
        .map(|tools| {
            tools
                .iter()
                .filter_map(serde_json::Value::as_str)
                .filter(|name| !name.trim().is_empty())
                .count()
        })
        .unwrap_or(0);
    let config_count = object
        .get("configs")
        .and_then(serde_json::Value::as_object)
        .map(|configs| {
            let default_enabled = object
                .get("default_config")
                .and_then(serde_json::Value::as_object)
                .and_then(|config| config.get("enabled"))
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(true);
            configs
                .iter()
                .filter(|(_, config)| {
                    config
                        .as_object()
                        .and_then(|config| config.get("enabled"))
                        .and_then(serde_json::Value::as_bool)
                        .unwrap_or(default_enabled)
                })
                .count()
        })
        .unwrap_or(0);
    if allowed_count == 0 && config_count == 0 {
        return Err(format!(
            "{provider_label} MCP toolset `{server_name}` requires allowed_tools or enabled configs"
        ));
    }
    Ok(())
}
