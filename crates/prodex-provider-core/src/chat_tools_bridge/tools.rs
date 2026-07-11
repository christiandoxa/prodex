//! Responses tool-shape translation to chat-compatible function tools.

mod custom;
mod mcp_toolset;
mod namespace;
mod tool_search;

use self::custom::provider_core_custom_tool_from_responses_tool;
use self::mcp_toolset::provider_core_mcp_toolset_function_tools;
use self::namespace::provider_core_namespace_function_tools;
use self::tool_search::provider_core_tool_search_tool;
use super::util::provider_core_json_string;
use super::web_search::provider_core_is_web_search_tool;

pub fn provider_core_chat_tools_from_responses_request(
    value: &serde_json::Value,
) -> Option<Vec<serde_json::Value>> {
    let tools = value.get("tools")?.as_array()?;
    let mut translated = Vec::new();
    for tool in tools {
        for translated_tool in provider_core_tools_from_responses_tool(tool) {
            if provider_core_translated_tool_name(&translated_tool).is_none() {
                continue;
            }
            translated.push(translated_tool);
        }
    }
    (!translated.is_empty()).then_some(translated)
}

fn provider_core_tools_from_responses_tool(tool: &serde_json::Value) -> Vec<serde_json::Value> {
    if provider_core_is_web_search_tool(tool) {
        return Vec::new();
    }
    provider_core_tool_from_responses_tool(tool)
        .into_iter()
        .chain(provider_core_namespace_function_tools(tool))
        .chain(provider_core_tool_search_tool(tool))
        .chain(provider_core_mcp_toolset_function_tools(tool))
        .collect()
}

fn provider_core_translated_tool_name(tool: &serde_json::Value) -> Option<String> {
    tool.get("function")
        .and_then(|function| function.get("name"))
        .and_then(serde_json::Value::as_str)
        .filter(|name| !name.trim().is_empty())
        .map(str::to_string)
}

fn provider_core_tool_from_responses_tool(tool: &serde_json::Value) -> Option<serde_json::Value> {
    let object = tool.as_object()?;
    if let Some(tool) = provider_core_custom_tool_from_responses_tool(object) {
        return Some(tool);
    }
    let function_object = provider_core_function_like_tool_object(object)?;
    let name = provider_core_json_string(function_object, &["name"])
        .filter(|name| !name.trim().is_empty())?;
    let mut function = serde_json::Map::new();
    function.insert("name".to_string(), serde_json::Value::String(name));
    if let Some(description) = provider_core_json_string(function_object, &["description"])
        .filter(|description| !description.trim().is_empty())
    {
        function.insert(
            "description".to_string(),
            serde_json::Value::String(description),
        );
    }
    if let Some(parameters) = function_object
        .get("parameters")
        .or_else(|| function_object.get("parametersJsonSchema"))
        .or_else(|| function_object.get("input_schema"))
        .or_else(|| function_object.get("schema"))
    {
        function.insert("parameters".to_string(), parameters.clone());
    }
    if let Some(strict) = function_object
        .get("strict")
        .and_then(serde_json::Value::as_bool)
    {
        function.insert("strict".to_string(), serde_json::Value::Bool(strict));
    }

    Some(serde_json::json!({
        "type": "function",
        "function": function,
    }))
}

fn provider_core_function_like_tool_object(
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
    let name = provider_core_json_string(object, &["name"])?;
    let has_schema = object.contains_key("parameters")
        || object.contains_key("parametersJsonSchema")
        || object.contains_key("input_schema")
        || object.contains_key("schema");
    let is_mcp_tool = tool_type.starts_with("mcp") || name.starts_with("mcp__");
    (has_schema && is_mcp_tool).then_some(object)
}
