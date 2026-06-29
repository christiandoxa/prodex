use super::super::provider_tools::{
    runtime_provider_chat_tool_choice_from_responses_request,
    runtime_provider_chat_tools_from_responses_request,
    runtime_provider_chat_web_search_options_from_responses_request,
};
use anyhow::Result;

pub(super) fn runtime_deepseek_tools_from_responses_request(
    value: &serde_json::Value,
) -> Option<Vec<serde_json::Value>> {
    runtime_provider_chat_tools_from_responses_request(value)
}

pub(super) fn runtime_deepseek_tool_choice_from_responses_request(
    value: &serde_json::Value,
    thinking_enabled: bool,
) -> Option<serde_json::Value> {
    runtime_provider_chat_tool_choice_from_responses_request(value, thinking_enabled)
}

pub(super) fn runtime_deepseek_web_search_options_from_responses_request(
    value: &serde_json::Value,
) -> Result<Option<serde_json::Value>> {
    if let Some(options) = value.get("web_search_options") {
        return Ok(Some(options.clone()));
    }
    runtime_deepseek_validate_web_search_tool_context_size(value)?;
    Ok(runtime_provider_chat_web_search_options_from_responses_request(value))
}

fn runtime_deepseek_validate_web_search_tool_context_size(value: &serde_json::Value) -> Result<()> {
    for tool in value
        .get("tools")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
    {
        let tool_type = tool
            .get("type")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        if !(tool_type == "web_search"
            || tool_type == "web_search_preview"
            || tool_type.starts_with("web_search_preview_"))
        {
            continue;
        }
        if let Some(size) = tool
            .get("search_context_size")
            .or_else(|| tool.get("context_size"))
        {
            let Some(size) = size.as_str() else {
                anyhow::bail!("DeepSeek web_search context_size must be low, medium, or high");
            };
            if !matches!(size, "low" | "medium" | "high") {
                anyhow::bail!("DeepSeek web_search context_size must be low, medium, or high");
            }
        }
    }
    Ok(())
}
