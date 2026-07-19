//! Web-search option extraction for chat-compatible providers.

use super::util::provider_core_json_string;

pub fn provider_core_chat_web_search_options_from_responses_request(
    value: &serde_json::Value,
) -> Option<serde_json::Value> {
    let tool = value
        .get("tools")?
        .as_array()?
        .iter()
        .find(|tool| provider_core_is_web_search_tool(tool))?;
    let object = tool.as_object()?;
    let mut options = serde_json::Map::new();
    if let Some(search_context_size) =
        provider_core_json_string(object, &["search_context_size", "context_size"])
            .filter(|size| matches!(size.as_str(), "low" | "medium" | "high"))
    {
        options.insert(
            "search_context_size".to_string(),
            serde_json::Value::String(search_context_size),
        );
    }
    if let Some(allowed_domains) = object.get("allowed_domains") {
        options.insert("allowed_domains".to_string(), allowed_domains.clone());
    }
    if let Some(blocked_domains) = object.get("blocked_domains") {
        options.insert("blocked_domains".to_string(), blocked_domains.clone());
    }
    if let Some(max_uses) = object.get("max_uses") {
        options.insert("max_uses".to_string(), max_uses.clone());
    }
    if let Some(user_location) = object
        .get("user_location")
        .or_else(|| object.get("location"))
    {
        options.insert("user_location".to_string(), user_location.clone());
    }
    Some(serde_json::Value::Object(options))
}

pub fn provider_core_chat_request_body_without_web_search_options(body: &[u8]) -> Option<Vec<u8>> {
    let mut value: serde_json::Value = serde_json::from_slice(body).ok()?;
    let object = value.as_object_mut()?;
    object.remove("web_search_options")?;
    serde_json::to_vec(&value).ok()
}

pub(super) fn provider_core_is_web_search_tool(tool: &serde_json::Value) -> bool {
    let tool_type = tool
        .get("type")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    tool_type == "web_search"
        || tool_type == "web_search_preview"
        || tool_type.starts_with("web_search_preview_")
}
