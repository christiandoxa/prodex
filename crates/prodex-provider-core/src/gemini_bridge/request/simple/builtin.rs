//! Gemini simple-request built-in tool classification.

pub(super) fn gemini_provider_core_builtin_tool(tool: &serde_json::Value) -> bool {
    let tool_type = tool
        .get("type")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    tool_type == "web_search"
        || tool_type == "web_search_preview"
        || tool_type.starts_with("web_search_preview_")
        || matches!(
            tool_type,
            "code_interpreter" | "code_execution" | "codeExecution"
        )
        || matches!(
            tool_type,
            "computer" | "computer_use" | "computerUse" | "computer_use_preview"
        )
        || tool_type.starts_with("computer_")
        || matches!(
            tool_type,
            "web_fetch" | "url_context" | "urlContext" | "web_fetch_preview"
        )
        || tool_type.starts_with("web_fetch_preview_")
        || tool.as_object().is_some_and(|object| {
            object.contains_key("computerUse")
                || object.contains_key("codeExecution")
                || object.contains_key("urlContext")
        })
}

pub(super) fn gemini_provider_core_builtin_tool_choice(value: &serde_json::Value) -> bool {
    value.is_null() || matches!(value.as_str(), None | Some("auto") | Some("none"))
}
