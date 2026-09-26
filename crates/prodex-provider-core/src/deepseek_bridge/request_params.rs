//! DeepSeek Responses request-parameter mapping and validation.
//!
//! Pure request-field handling only; routing, retry, and transport side effects stay outside provider-core.

mod metadata;
mod reasoning;
mod reject;

use super::request_policy::plan_value;
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
    let (_, plan) = plan_value(value, DeepSeekRequestPolicyOperation::Stop, false);
    let error = match plan.tag {
        0 => None,
        1 => Some("stop must be a string or array of strings"),
        2 => Some("supports at most 16 stop sequences"),
        3 => Some("stop sequences must be strings"),
        _ => Some("stop validation returned an unknown result"),
    };
    if let Some(error) = error {
        return Err(format!("{provider_label} {error}"));
    }
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
    let (_, plan) = plan_value(value, DeepSeekRequestPolicyOperation::PrimitiveCore, false);
    let error = match plan.tag {
        0 => None,
        1 => Some("temperature must be a number"),
        2 => Some("top_p must be a number"),
        3 => Some("max_output_tokens must be a positive integer"),
        4 => Some("max_tokens must be a positive integer"),
        5 => Some("max_completion_tokens must be a positive integer"),
        6 => Some("logprobs must be a boolean"),
        _ => Some("request validation returned an unknown result"),
    };
    match error {
        Some(error) => Err(format!("{provider_label} {error}")),
        None => Ok(()),
    }
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
    let (_, plan) = plan_value(value, DeepSeekRequestPolicyOperation::TopLogprobs, false);
    let error = match plan.tag {
        0 => None,
        1 => Some("top_logprobs must be an integer"),
        2 => Some("top_logprobs must be <= 20"),
        3 => Some("top_logprobs requires logprobs=true"),
        _ => Some("top_logprobs validation returned an unknown result"),
    };
    if let Some(error) = error {
        return Err(format!("{provider_label} {error}"));
    }
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

pub(crate) fn deepseek_provider_core_validate_responses_request_params(
    source: &str,
    provider_label: &str,
) -> Result<(), String> {
    let plan = prodex_mojo_core::rich::deepseek_request_policy(
        DeepSeekRequestPolicyOperation::ResponsesRequestParams,
        source,
        false,
        0,
    )
    .map_err(|error| format!("{provider_label} request parameter policy failed: {error:?}"))?;
    let error = match plan.tag {
        0 => return Ok(()),
        1 => "temperature must be a number",
        2 => "top_p must be a number",
        3 => "max_output_tokens must be a positive integer",
        4 => "max_tokens must be a positive integer",
        5 => "max_completion_tokens must be a positive integer",
        6 => "logprobs must be a boolean",
        7 => "top_logprobs must be an integer",
        8 => "top_logprobs must be <= 20",
        9 => "top_logprobs requires logprobs=true",
        10 => "stop must be a string or array of strings",
        11 => "supports at most 16 stop sequences",
        12 => "stop sequences must be strings",
        tag => {
            return Err(format!(
                "{provider_label} request parameter validation returned unknown result {tag}"
            ));
        }
    };
    Err(format!("{provider_label} {error}"))
}
