//! DeepSeek tool-choice validation.

use std::collections::BTreeSet;

use crate::deepseek_bridge::request_policy::{try_plan_value, validate_plan};
use prodex_mojo_core::rich::DeepSeekRequestPolicyOperation;

use super::{DeepSeekFunctionToolOperation, function_tool_policy};

pub fn deepseek_provider_core_validate_tool_choice_name(
    tool_choice: &serde_json::Value,
    provider_label: &str,
) -> Result<(), String> {
    deepseek_provider_core_validate_tool_choice_name_with_max_bytes(tool_choice, provider_label, 64)
}

pub fn deepseek_provider_core_validate_tool_choice_name_with_max_bytes(
    tool_choice: &serde_json::Value,
    provider_label: &str,
    max_name_bytes: usize,
) -> Result<(), String> {
    function_tool_policy(
        DeepSeekFunctionToolOperation::ValidateChoiceName,
        tool_choice,
        false,
        provider_label,
        max_name_bytes,
        "",
    )
}

pub fn deepseek_provider_core_validate_tool_choice_shape(
    value: &serde_json::Value,
    thinking_enabled: bool,
    provider_label: &str,
) -> Result<(), String> {
    if thinking_enabled || !value.is_object() {
        return Ok(());
    }
    let (source, plan) = try_plan_value(
        value,
        DeepSeekRequestPolicyOperation::ToolChoice,
        false,
        provider_label,
    )?;
    validate_plan(
        DeepSeekRequestPolicyOperation::ToolChoice,
        &source,
        plan,
        provider_label,
    )
}

pub fn deepseek_provider_core_validate_tool_choice_target(
    tool_choice: &serde_json::Value,
    tool_names: &BTreeSet<String>,
    provider_label: &str,
) -> Result<(), String> {
    function_tool_policy(
        DeepSeekFunctionToolOperation::ValidateChoiceTarget,
        &serde_json::json!({"tool_choice": tool_choice, "names": tool_names}),
        false,
        provider_label,
        64,
        "",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_choice_shape_preserves_unicode_names_and_error_details() {
        deepseek_provider_core_validate_tool_choice_shape(
            &serde_json::json!({"tool_choice": {"type": "function", "name": "検索"}}),
            false,
            "DeepSeek",
        )
        .unwrap();
        assert_eq!(
            deepseek_provider_core_validate_tool_choice_shape(
                &serde_json::json!({"tool_choice": "☃"}),
                false,
                "DeepSeek",
            )
            .unwrap_err(),
            "DeepSeek tool_choice string `☃` is not supported"
        );
    }
}
