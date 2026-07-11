//! Namespace Responses tool expansion.

use super::super::util::{provider_core_flatten_namespace_tool_name, provider_core_json_string};

pub(super) fn provider_core_namespace_function_tools(
    tool: &serde_json::Value,
) -> Vec<serde_json::Value> {
    let Some(object) = tool.as_object() else {
        return Vec::new();
    };
    if object.get("type").and_then(serde_json::Value::as_str) != Some("namespace") {
        return Vec::new();
    }
    let Some(namespace) =
        provider_core_json_string(object, &["name"]).filter(|name| !name.trim().is_empty())
    else {
        return Vec::new();
    };
    object
        .get("tools")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|tool| provider_core_namespace_function_tool(tool, &namespace))
        .collect()
}

fn provider_core_namespace_function_tool(
    tool: &serde_json::Value,
    namespace: &str,
) -> Option<serde_json::Value> {
    let tool = tool.as_object()?;
    if tool.get("type").and_then(serde_json::Value::as_str) != Some("function") {
        return None;
    }
    let tool_name =
        provider_core_json_string(tool, &["name"]).filter(|name| !name.trim().is_empty())?;
    let mut function = serde_json::Map::new();
    function.insert(
        "name".to_string(),
        serde_json::Value::String(provider_core_flatten_namespace_tool_name(
            namespace, &tool_name,
        )),
    );
    if let Some(description) = provider_core_json_string(tool, &["description"])
        .filter(|description| !description.trim().is_empty())
    {
        function.insert(
            "description".to_string(),
            serde_json::Value::String(description),
        );
    }
    if let Some(parameters) = tool
        .get("parameters")
        .or_else(|| tool.get("parametersJsonSchema"))
        .or_else(|| tool.get("input_schema"))
        .or_else(|| tool.get("schema"))
    {
        function.insert("parameters".to_string(), parameters.clone());
    }
    if let Some(strict) = tool.get("strict").and_then(serde_json::Value::as_bool) {
        function.insert("strict".to_string(), serde_json::Value::Bool(strict));
    }
    Some(serde_json::json!({
        "type": "function",
        "function": function,
    }))
}
