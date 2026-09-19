//! DeepSeek web-search option validation.

#[cfg(feature = "mojo")]
use crate::deepseek_bridge::request_policy::plan_value;
#[cfg(feature = "mojo")]
use prodex_mojo_core::rich::DeepSeekRequestPolicyOperation;

pub fn deepseek_provider_core_validate_web_search_options(
    options: &serde_json::Value,
    provider_label: &str,
) -> Result<(), String> {
    if !options.is_object() {
        return Err(format!(
            "{provider_label} web_search_options must be an object"
        ));
    }
    #[cfg(feature = "mojo")]
    {
        let (_, plan) = plan_value(
            options,
            DeepSeekRequestPolicyOperation::WebSearchOptions,
            false,
        );
        return match plan.tag {
            0 => Ok(()),
            2 => Err(format!(
                "{provider_label} web_search search_context_size must be low, medium, or high"
            )),
            3 => Err(format!(
                "{provider_label} web_search allowed_domains must be an array of strings"
            )),
            4 => Err(format!(
                "{provider_label} web_search allowed_domains entries must be non-empty strings"
            )),
            5 => Err(format!(
                "{provider_label} web_search blocked_domains must be an array of strings"
            )),
            6 => Err(format!(
                "{provider_label} web_search blocked_domains entries must be non-empty strings"
            )),
            7 => Err(format!(
                "{provider_label} web_search max_uses must be a positive integer"
            )),
            8 => Err(format!(
                "{provider_label} web_search user_location must be an object"
            )),
            _ => Err(format!(
                "{provider_label} web_search validation returned an unknown result"
            )),
        };
    }
    #[cfg(not(feature = "mojo"))]
    {
        let object = options.as_object().expect("validated object");
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
        for field in ["allowed_domains", "blocked_domains"] {
            let Some(domains) = object.get(field) else {
                continue;
            };
            let Some(domains) = domains.as_array() else {
                return Err(format!(
                    "{provider_label} web_search {field} must be an array of strings"
                ));
            };
            if domains.iter().any(|domain| {
                domain
                    .as_str()
                    .is_none_or(|domain| domain.trim().is_empty())
            }) {
                return Err(format!(
                    "{provider_label} web_search {field} entries must be non-empty strings"
                ));
            }
        }
        if object
            .get("max_uses")
            .is_some_and(|value| value.as_u64().is_none_or(|value| value == 0))
        {
            return Err(format!(
                "{provider_label} web_search max_uses must be a positive integer"
            ));
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
}

pub fn deepseek_provider_core_validate_web_search_tool_context_size(
    value: &serde_json::Value,
    provider_label: &str,
) -> Result<(), String> {
    #[cfg(feature = "mojo")]
    {
        let (_, plan) = plan_value(
            value,
            DeepSeekRequestPolicyOperation::WebSearchContext,
            false,
        );
        return match plan.tag {
            0 => Ok(()),
            1 => Err(format!(
                "{provider_label} web_search context_size must be low, medium, or high"
            )),
            _ => Err(format!(
                "{provider_label} web_search context validation returned an unknown result"
            )),
        };
    }
    #[cfg(not(feature = "mojo"))]
    {
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn max_uses_must_be_positive() {
        deepseek_provider_core_validate_web_search_options(
            &serde_json::json!({"max_uses": 3}),
            "DeepSeek",
        )
        .unwrap();
        let error = deepseek_provider_core_validate_web_search_options(
            &serde_json::json!({"max_uses": 0}),
            "DeepSeek",
        )
        .unwrap_err();
        assert!(error.contains("DeepSeek web_search max_uses must be a positive integer"));
    }
}
