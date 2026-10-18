//! DeepSeek request-field rejection checks.

use prodex_mojo_core::rich::DeepSeekRequestPolicyOperation;

use super::super::request_policy;

pub fn deepseek_provider_core_reject_unsupported_request_fields(
    value: &serde_json::Value,
    provider_label: &str,
) -> Result<(), String> {
    if !value.is_object() {
        return Ok(());
    }
    let (source, plan) = request_policy::try_plan_value(
        value,
        DeepSeekRequestPolicyOperation::RequestFields,
        false,
        provider_label,
    )?;
    request_policy::validate_plan(
        DeepSeekRequestPolicyOperation::RequestFields,
        &source,
        plan,
        provider_label,
    )
}

pub fn deepseek_provider_core_reject_beta_completion_fields(
    value: &serde_json::Value,
    provider_label: &str,
) -> Result<(), String> {
    if !value.is_object() {
        return Ok(());
    }
    let (source, plan) = request_policy::try_plan_value(
        value,
        DeepSeekRequestPolicyOperation::BetaFields,
        false,
        provider_label,
    )?;
    request_policy::validate_plan(
        DeepSeekRequestPolicyOperation::BetaFields,
        &source,
        plan,
        provider_label,
    )
}
