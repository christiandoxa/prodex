//! DeepSeek Responses request-parameter mapping and validation.
//!
//! Pure request-field handling only; routing, retry, and transport side effects stay outside provider-core.

mod metadata;
mod reasoning;
mod reject;

#[cfg(feature = "mojo")]
use prodex_mojo_core::rich::{DeepSeekKernelInput, DeepSeekKernelOperation};

#[cfg(feature = "mojo")]
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
    let Some(stop) = value
        .get("stop")
        .or_else(|| value.get("stop_sequences"))
        .or_else(|| value.get("stopSequences"))
    else {
        return Ok(None);
    };
    if stop.as_str().is_some() {
        return Ok(Some(stop.clone()));
    }
    let Some(stops) = stop.as_array() else {
        return Err(format!(
            "{provider_label} stop must be a string or array of strings"
        ));
    };
    if stops.len() > 16 {
        return Err(format!(
            "{provider_label} supports at most 16 stop sequences"
        ));
    }
    if stops.iter().any(|stop| !stop.is_string()) {
        return Err(format!("{provider_label} stop sequences must be strings"));
    }
    Ok(Some(stop.clone()))
}

pub fn deepseek_provider_core_insert_primitive_request_fields(
    value: &serde_json::Value,
    request: &mut serde_json::Map<String, serde_json::Value>,
    provider_label: &str,
) -> Result<(), String> {
    for field in ["temperature", "top_p"] {
        if let Some(next) = value.get(field) {
            if !next.is_number() {
                return Err(format!("{provider_label} {field} must be a number"));
            }
        }
    }
    for field in ["max_output_tokens", "max_tokens", "max_completion_tokens"] {
        if let Some(next) = value.get(field) {
            if next.as_u64().is_none_or(|count| count == 0) {
                return Err(format!(
                    "{provider_label} {field} must be a positive integer"
                ));
            }
        }
    }
    if let Some(logprobs) = value.get("logprobs") {
        if !logprobs.is_boolean() {
            return Err(format!("{provider_label} logprobs must be a boolean"));
        }
    }
    #[cfg(feature = "mojo")]
    {
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
        return Ok(());
    }
    #[cfg(not(feature = "mojo"))]
    {
        for field in ["temperature", "top_p"] {
            if let Some(next) = value.get(field) {
                request.insert(field.to_string(), next.clone());
            }
        }
        for field in ["max_output_tokens", "max_tokens", "max_completion_tokens"] {
            if let Some(next) = value.get(field) {
                request.insert("max_tokens".to_string(), next.clone());
            }
        }
        if let Some(logprobs) = value.get("logprobs") {
            request.insert("logprobs".to_string(), logprobs.clone());
        }
    }
    #[cfg(not(feature = "mojo"))]
    Ok(())
}

pub fn deepseek_provider_core_top_logprobs_from_responses_request(
    value: &serde_json::Value,
    provider_label: &str,
) -> Result<Option<serde_json::Value>, String> {
    let Some(top_logprobs) = value.get("top_logprobs") else {
        return Ok(None);
    };
    let Some(count) = top_logprobs.as_u64() else {
        return Err(format!("{provider_label} top_logprobs must be an integer"));
    };
    if count > 20 {
        return Err(format!("{provider_label} top_logprobs must be <= 20"));
    }
    if value.get("logprobs").and_then(serde_json::Value::as_bool) != Some(true) {
        return Err(format!(
            "{provider_label} top_logprobs requires logprobs=true"
        ));
    }
    Ok(Some(top_logprobs.clone()))
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
    let raw_user_id = user_id;
    let user_id = raw_user_id.trim();
    if user_id.is_empty() {
        #[cfg(feature = "mojo")]
        {
            let mut input = DeepSeekKernelInput::new(DeepSeekKernelOperation::UserId);
            input.input = Some(raw_user_id);
            let _ = deepseek_provider_core_mojo_value(input).map_err(|error| {
                format!("{provider_label} user_id could not be normalized: {error}")
            })?;
        }
        return Ok(None);
    }
    if user_id.len() > 512
        || !user_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(format!(
            "{provider_label} user_id must use only letters, numbers, underscores, or dashes and be at most 512 bytes"
        ));
    }
    #[cfg(feature = "mojo")]
    {
        let mut input = DeepSeekKernelInput::new(DeepSeekKernelOperation::UserId);
        input.input = Some(raw_user_id);
        let normalized = deepseek_provider_core_mojo_value(input).map_err(|error| {
            format!("{provider_label} user_id could not be normalized: {error}")
        })?;
        return normalized
            .as_str()
            .filter(|user_id| !user_id.is_empty())
            .map(str::to_string)
            .map_or(Ok(None), |user_id| Ok(Some(user_id)));
    }
    #[cfg(not(feature = "mojo"))]
    Ok(Some(user_id.to_string()))
}
