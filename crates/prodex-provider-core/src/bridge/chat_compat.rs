//! Chat-compatible response and request-shape bridge helpers.

#[path = "chat_compat/response.rs"]
mod response;
#[path = "chat_compat/tool_calls.rs"]
mod tool_calls;
#[path = "chat_compat/usage.rs"]
mod usage;

#[cfg(test)]
#[path = "chat_compat/shape_tests.rs"]
mod shape_tests;

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
    let mut document = crate::mojo_json::Document::default();
    let _ = document.borrowed_shape(value);
    prodex_mojo_core::json::chat_compatible_request_shape_error(&document.nodes, provider_label)
        .map_err(|error| format!("{provider_label} request shape validation failed: {error:?}"))?
        .map_or(Ok(()), Err)
}
