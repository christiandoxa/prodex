//! DeepSeek web-search option validation.

use crate::deepseek_bridge::request_policy::try_plan_value;
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
    let (_, plan) = try_plan_value(
        options,
        DeepSeekRequestPolicyOperation::WebSearchOptions,
        false,
        provider_label,
    )?;
    match plan.tag {
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
    }
}

pub fn deepseek_provider_core_validate_web_search_tool_context_size(
    value: &serde_json::Value,
    provider_label: &str,
) -> Result<(), String> {
    if !value.is_object() {
        return Ok(());
    }
    let (_, plan) = try_plan_value(
        value,
        DeepSeekRequestPolicyOperation::WebSearchContext,
        false,
        provider_label,
    )?;
    match plan.tag {
        0 => Ok(()),
        1 => Err(format!(
            "{provider_label} web_search context_size must be low, medium, or high"
        )),
        _ => Err(format!(
            "{provider_label} web_search context validation returned an unknown result"
        )),
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

        assert_eq!(
            deepseek_provider_core_validate_web_search_options(
                &serde_json::json!({"allowed_domains": ["\u{2003}"]}),
                "DeepSeek",
            )
            .unwrap_err(),
            "DeepSeek web_search allowed_domains entries must be non-empty strings"
        );
    }
}
