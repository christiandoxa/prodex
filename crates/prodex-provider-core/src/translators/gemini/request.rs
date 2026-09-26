#[cfg(not(feature = "mojo"))]
use serde_json::Value;

#[path = "request/continuation.rs"]
mod continuation;
#[path = "request/generation_config.rs"]
mod generation_config;
#[cfg(not(feature = "mojo"))]
#[path = "request/response_format.rs"]
mod response_format;
#[path = "request/schema.rs"]
mod schema;
#[path = "request/tool_signatures.rs"]
mod tool_signatures;
#[path = "request/tools.rs"]
mod tools;

pub(super) use self::continuation::gemini_continuation_metadata;
pub use self::generation_config::gemini_provider_core_model_uses_thinking_level;
#[cfg(not(feature = "mojo"))]
pub(crate) use self::generation_config::gemini_validate_candidate_count;
#[cfg(not(feature = "mojo"))]
pub(super) use self::response_format::gemini_apply_response_format;
pub(crate) use self::schema::sanitize_function_schema;
pub(crate) use self::tool_signatures::gemini_preserve_tool_call_signatures;
pub(super) use self::tools::gemini_tool_from_openai_tool;
pub(crate) use self::tools::{
    gemini_builtin_tools_from_request, gemini_function_declaration_from_openai_tool,
    gemini_is_supported_builtin_tool, gemini_tool_config_from_request,
    gemini_validate_openai_tools,
};

#[cfg(not(feature = "mojo"))]
pub(crate) fn gemini_request_body_without_tool(body: &[u8], tool_name: &str) -> Option<Vec<u8>> {
    let value: Value = serde_json::from_slice(body).ok()?;
    let body = serde_json::to_vec(&value).ok()?;
    let mut input = prodex_mojo_core::provider_constraints::GeminiBridgeRequestKernelInput::new(
        prodex_mojo_core::provider_constraints::GeminiBridgeRequestOperation::RequestBodyWithoutTool,
    );
    input.primary = Some(&body);
    input.secondary = Some(tool_name.as_bytes());
    let body = prodex_mojo_core::provider_constraints::gemini_bridge_request_kernel(input).ok()?;
    (!matches!(body.as_slice(), b"null")).then_some(body)
}
