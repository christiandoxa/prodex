//! DeepSeek web-search option validation.

pub fn deepseek_provider_core_validate_web_search_options(
    options: &serde_json::Value,
    provider_label: &str,
) -> Result<(), String> {
    let Some(object) = options.as_object() else {
        return Err(format!(
            "{provider_label} web_search_options must be an object"
        ));
    };
    if let Some(size) = object.get("search_context_size") {
        let Some(size) = size.as_str() else {
            return Err(format!(
                "{provider_label} web_search search_context_size must be low, medium, or high"
            ));
        };
        if !matches!(size, "low" | "medium" | "high") {
            return Err(format!(
                "{provider_label} web_search search_context_size must be low, medium, or high"
            ));
        }
    }
    if let Some(allowed_domains) = object.get("allowed_domains") {
        let Some(domains) = allowed_domains.as_array() else {
            return Err(format!(
                "{provider_label} web_search allowed_domains must be an array of strings"
            ));
        };
        if domains.iter().any(|domain| {
            domain
                .as_str()
                .is_none_or(|domain| domain.trim().is_empty())
        }) {
            return Err(format!(
                "{provider_label} web_search allowed_domains entries must be non-empty strings"
            ));
        }
    }
    if object
        .get("user_location")
        .is_some_and(|location| !location.is_object())
    {
        return Err(format!(
            "{provider_label} web_search user_location must be an object"
        ));
    }
    Ok(())
}

pub fn deepseek_provider_core_validate_web_search_tool_context_size(
    value: &serde_json::Value,
    provider_label: &str,
) -> Result<(), String> {
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
                return Err(format!(
                    "{provider_label} web_search context_size must be low, medium, or high"
                ));
            };
            if !matches!(size, "low" | "medium" | "high") {
                return Err(format!(
                    "{provider_label} web_search context_size must be low, medium, or high"
                ));
            }
        }
    }
    Ok(())
}
