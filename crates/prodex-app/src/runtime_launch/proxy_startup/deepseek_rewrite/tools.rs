use super::json_helpers::{runtime_deepseek_json_string, runtime_deepseek_json_string_at_path};
use std::collections::BTreeSet;

pub(super) fn runtime_deepseek_tools_from_responses_request(
    value: &serde_json::Value,
) -> Option<Vec<serde_json::Value>> {
    let tools = value.get("tools")?.as_array()?;
    let mut translated = Vec::new();
    let mut seen_names = BTreeSet::new();
    for tool in tools {
        for translated_tool in runtime_deepseek_tools_from_responses_tool(tool) {
            let Some(name) = runtime_deepseek_translated_tool_name(&translated_tool) else {
                continue;
            };
            if seen_names.insert(name) {
                translated.push(translated_tool);
            }
        }
    }
    if translated.is_empty() {
        None
    } else {
        Some(translated)
    }
}

fn runtime_deepseek_tools_from_responses_tool(tool: &serde_json::Value) -> Vec<serde_json::Value> {
    runtime_deepseek_tool_from_responses_tool(tool)
        .into_iter()
        .chain(runtime_deepseek_mcp_toolset_function_tools(tool))
        .collect()
}

fn runtime_deepseek_translated_tool_name(tool: &serde_json::Value) -> Option<String> {
    tool.get("function")
        .and_then(|function| function.get("name"))
        .and_then(serde_json::Value::as_str)
        .filter(|name| !name.trim().is_empty())
        .map(str::to_string)
}

fn runtime_deepseek_tool_from_responses_tool(
    tool: &serde_json::Value,
) -> Option<serde_json::Value> {
    let object = tool.as_object()?;
    let function_object = runtime_deepseek_function_like_tool_object(object)?;
    let name = runtime_deepseek_json_string(function_object, &["name"])
        .filter(|name| !name.trim().is_empty())?;
    let mut function = serde_json::Map::new();
    function.insert("name".to_string(), serde_json::Value::String(name));
    if let Some(description) = runtime_deepseek_json_string(function_object, &["description"])
        .filter(|description| !description.trim().is_empty())
    {
        function.insert(
            "description".to_string(),
            serde_json::Value::String(description),
        );
    }
    if let Some(parameters) = function_object
        .get("parameters")
        .or_else(|| function_object.get("input_schema"))
        .or_else(|| function_object.get("schema"))
    {
        function.insert("parameters".to_string(), parameters.clone());
    }

    Some(serde_json::json!({
        "type": "function",
        "function": function,
    }))
}

fn runtime_deepseek_mcp_toolset_function_tools(tool: &serde_json::Value) -> Vec<serde_json::Value> {
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
    let Some(server_name) = runtime_deepseek_json_string(
        object,
        &["mcp_server_name", "server_label", "server_name", "name"],
    )
    .filter(|server_name| !server_name.trim().is_empty()) else {
        return Vec::new();
    };
    let Some(server_label) = runtime_deepseek_function_name_segment(&server_name) else {
        return Vec::new();
    };
    runtime_deepseek_mcp_toolset_tool_names(object)
        .into_iter()
        .filter_map(|tool_name| {
            let tool_label = runtime_deepseek_function_name_segment(&tool_name)?;
            let name = if tool_name.trim().starts_with("mcp__") {
                tool_label
            } else {
                format!("mcp__{server_label}__{tool_label}")
            };
            let description = runtime_deepseek_json_string(object, &["description"])
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

fn runtime_deepseek_mcp_toolset_tool_names(
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

fn runtime_deepseek_function_name_segment(value: &str) -> Option<String> {
    let segment = value
        .trim()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim_matches('_')
        .to_string();
    (!segment.is_empty()).then_some(segment)
}

fn runtime_deepseek_function_like_tool_object(
    object: &serde_json::Map<String, serde_json::Value>,
) -> Option<&serde_json::Map<String, serde_json::Value>> {
    if object.get("type").and_then(serde_json::Value::as_str) == Some("function") {
        return Some(
            object
                .get("function")
                .and_then(serde_json::Value::as_object)
                .unwrap_or(object),
        );
    }

    let tool_type = object
        .get("type")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    let name = runtime_deepseek_json_string(object, &["name"])?;
    let has_schema = object.contains_key("parameters")
        || object.contains_key("input_schema")
        || object.contains_key("schema");
    let is_mcp_tool = tool_type.starts_with("mcp") || name.starts_with("mcp__");
    (has_schema && is_mcp_tool).then_some(object)
}

pub(super) fn runtime_deepseek_tool_choice_from_responses_request(
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
    let name = runtime_deepseek_json_string(object, &["name"])
        .or_else(|| runtime_deepseek_json_string_at_path(object, &["function", "name"]))
        .filter(|name| !name.trim().is_empty())?;
    Some(serde_json::json!({
        "type": "function",
        "function": {
            "name": name,
        },
    }))
}
