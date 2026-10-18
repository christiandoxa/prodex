//! DeepSeek Responses request-parameter mapping and validation.
//!
//! Pure request-field handling only; routing, retry, and transport side effects stay outside provider-core.

mod metadata;
mod reasoning;
mod reject;

use super::request_policy::{try_plan_value, validate_plan};
use prodex_mojo_core::rich::{
    DeepSeekKernelInput, DeepSeekKernelOperation, DeepSeekRequestPolicyOperation,
};

fn deepseek_provider_core_mojo_value(
    input: DeepSeekKernelInput<'_>,
) -> Result<serde_json::Value, String> {
    let body = prodex_mojo_core::rich::deepseek_kernel(input)
        .map_err(|error| format!("DeepSeek Mojo kernel failed: {error:?}"))?;
    serde_json::from_slice(&body)
        .map_err(|error| format!("DeepSeek Mojo kernel returned invalid JSON: {error}"))
}

pub use self::metadata::{
    deepseek_provider_core_ensure_json_prompt_instruction,
    deepseek_provider_core_note_thinking_tool_choice_omission,
    deepseek_provider_core_response_format_from_responses_request,
    deepseek_provider_core_response_metadata_from_responses_request,
};

pub use self::reasoning::{
    deepseek_provider_core_apply_reasoning_from_responses_request,
    deepseek_provider_core_thinking_enabled, deepseek_provider_core_validate_reasoning_shape,
};

pub use self::reject::{
    deepseek_provider_core_reject_beta_completion_fields,
    deepseek_provider_core_reject_unsupported_request_fields,
};

pub fn deepseek_provider_core_stop_from_responses_request(
    value: &serde_json::Value,
    provider_label: &str,
) -> Result<Option<serde_json::Value>, String> {
    let (source, plan) = try_plan_value(
        value,
        DeepSeekRequestPolicyOperation::Stop,
        false,
        provider_label,
    )?;
    validate_plan(
        DeepSeekRequestPolicyOperation::Stop,
        &source,
        plan,
        provider_label,
    )?;
    Ok(value
        .get("stop")
        .or_else(|| value.get("stop_sequences"))
        .or_else(|| value.get("stopSequences"))
        .cloned())
}

pub fn deepseek_provider_core_insert_primitive_request_fields(
    value: &serde_json::Value,
    request: &mut serde_json::Map<String, serde_json::Value>,
    provider_label: &str,
) -> Result<(), String> {
    validate_primitive_request_fields_mojo(value, provider_label)?;
    insert_primitive_request_fields_mojo(value, request, provider_label)
}

fn validate_primitive_request_fields_mojo(
    value: &serde_json::Value,
    provider_label: &str,
) -> Result<(), String> {
    let (source, plan) = try_plan_value(
        value,
        DeepSeekRequestPolicyOperation::PrimitiveCore,
        false,
        provider_label,
    )?;
    validate_plan(
        DeepSeekRequestPolicyOperation::PrimitiveCore,
        &source,
        plan,
        provider_label,
    )
}

fn insert_primitive_request_fields_mojo(
    value: &serde_json::Value,
    request: &mut serde_json::Map<String, serde_json::Value>,
    provider_label: &str,
) -> Result<(), String> {
    let source = serde_json::to_string(value)
        .map_err(|error| format!("{provider_label} request serialization failed: {error}"))?;
    let mut input = DeepSeekKernelInput::new(DeepSeekKernelOperation::PrimitiveRequestFields);
    input.input = Some(&source);
    let fields = deepseek_provider_core_mojo_value(input).map_err(|error| {
        format!("{provider_label} primitive request fields could not be normalized: {error}")
    })?;
    let Some(fields) = fields.as_object() else {
        return Err(format!(
            "{provider_label} primitive request fields normalization returned a non-object"
        ));
    };
    request.extend(
        fields
            .iter()
            .map(|(key, value)| (key.clone(), value.clone())),
    );
    Ok(())
}

pub fn deepseek_provider_core_top_logprobs_from_responses_request(
    value: &serde_json::Value,
    provider_label: &str,
) -> Result<Option<serde_json::Value>, String> {
    let (source, plan) = try_plan_value(
        value,
        DeepSeekRequestPolicyOperation::TopLogprobs,
        false,
        provider_label,
    )?;
    validate_plan(
        DeepSeekRequestPolicyOperation::TopLogprobs,
        &source,
        plan,
        provider_label,
    )?;
    Ok(value.get("top_logprobs").cloned())
}

pub fn deepseek_provider_core_user_id_from_responses_request(
    value: &serde_json::Value,
    provider_label: &str,
) -> Result<Option<String>, String> {
    let Some(user_id) = value
        .get("user_id")
        .or_else(|| value.get("user"))
        .or_else(|| value.get("safety_identifier"))
    else {
        return Ok(None);
    };
    let Some(user_id) = user_id.as_str() else {
        return Err(format!("{provider_label} user_id must be a string"));
    };
    let mut input = DeepSeekKernelInput::new(DeepSeekKernelOperation::UserId);
    input.input = Some(user_id);
    let normalized = deepseek_provider_core_mojo_value(input)
        .map_err(|error| format!("{provider_label} user_id could not be normalized: {error}"))?;
    if normalized.is_null() {
        return Err(format!(
            "{provider_label} user_id must use only letters, numbers, underscores, or dashes and be at most 512 bytes"
        ));
    }
    normalized
        .as_str()
        .filter(|user_id| !user_id.is_empty())
        .map(str::to_string)
        .map_or(Ok(None), |user_id| Ok(Some(user_id)))
}
