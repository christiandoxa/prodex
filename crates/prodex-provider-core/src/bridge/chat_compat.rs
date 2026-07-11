//! Chat-compatible response and request-shape bridge helpers.

#[path = "chat_compat/response.rs"]
mod response;
#[path = "chat_compat/tool_calls.rs"]
mod tool_calls;
#[path = "chat_compat/usage.rs"]
mod usage;

use std::time::{SystemTime, UNIX_EPOCH};

pub use self::response::{
    provider_core_chat_compatible_responses_value_from_chat_value,
    provider_core_chat_compatible_responses_value_from_chat_value_with_fallback_ids,
};
pub use self::tool_calls::{
    provider_core_chat_compatible_rtk_wrapped_tool_arguments,
    provider_core_chat_compatible_tool_call_thought_signature,
    provider_core_split_flat_namespace_tool_name,
};
pub use self::usage::provider_core_chat_compatible_responses_usage;

pub fn provider_core_chat_compatible_created_at() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

pub fn provider_core_chat_compatible_validate_top_level_request_shape(
    value: &serde_json::Value,
    provider_label: &str,
) -> Result<(), String> {
    if !value.is_object() {
        return Err(format!(
            "{provider_label} request body must be a JSON object"
        ));
    }
    if let Some(model) = value.get("model") {
        let Some(model) = model.as_str() else {
            return Err(format!("{provider_label} model must be a string"));
        };
        if model.trim().is_empty() {
            return Err(format!("{provider_label} model must not be empty"));
        }
    }
    if let Some(stream) = value.get("stream")
        && !stream.is_boolean()
    {
        return Err(format!("{provider_label} stream must be a boolean"));
    }
    if let Some(instructions) = value.get("instructions")
        && !instructions.is_string()
    {
        return Err(format!("{provider_label} instructions must be a string"));
    }
    if let Some(previous_response_id) = value.get("previous_response_id")
        && !previous_response_id.is_string()
    {
        return Err(format!(
            "{provider_label} previous_response_id must be a string"
        ));
    }
    Ok(())
}
