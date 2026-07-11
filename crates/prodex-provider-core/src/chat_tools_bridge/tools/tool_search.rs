//! Tool-search Responses tool translation.

use super::super::util::provider_core_json_string;

pub(super) fn provider_core_tool_search_tool(
    tool: &serde_json::Value,
) -> Option<serde_json::Value> {
    let object = tool.as_object()?;
    if object.get("type").and_then(serde_json::Value::as_str) != Some("tool_search") {
        return None;
    }
    let mut function = serde_json::Map::new();
    function.insert(
        "name".to_string(),
        serde_json::Value::String("tool_search".to_string()),
    );
    if let Some(description) = provider_core_json_string(object, &["description"])
        .filter(|description| !description.trim().is_empty())
    {
        function.insert(
            "description".to_string(),
            serde_json::Value::String(description),
        );
    }
    if let Some(parameters) = object.get("parameters") {
        function.insert("parameters".to_string(), parameters.clone());
    }
    Some(serde_json::json!({
        "type": "function",
        "function": function,
    }))
}
