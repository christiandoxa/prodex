use serde_json::Value;

#[path = "request/continuation.rs"]
mod continuation;
#[path = "request/generation_config.rs"]
mod generation_config;
#[path = "request/optional_fields.rs"]
mod optional_fields;
#[path = "request/response_format.rs"]
mod response_format;
#[path = "request/schema.rs"]
mod schema;
#[path = "request/tool_signatures.rs"]
mod tool_signatures;
#[path = "request/tools.rs"]
mod tools;

pub(super) use self::continuation::gemini_continuation_metadata;
pub(crate) use self::generation_config::gemini_generation_config_from_request;
pub use self::generation_config::gemini_provider_core_model_uses_thinking_level;
pub(super) use self::generation_config::{
    gemini_apply_text_format, gemini_insert_basic_generation_config,
    gemini_insert_extended_generation_config, gemini_thinking_config_from_request,
};
pub(super) use self::optional_fields::gemini_apply_optional_request_fields;
pub(super) use self::response_format::gemini_apply_response_format;
pub(crate) use self::schema::sanitize_function_schema;
pub(crate) use self::tool_signatures::gemini_preserve_tool_call_signatures;
pub(super) use self::tools::gemini_tool_from_openai_tool;
pub(crate) use self::tools::{
    gemini_builtin_tools_from_request, gemini_function_declaration_from_openai_tool,
    gemini_tool_config_from_request,
};

pub(crate) fn gemini_request_body_without_tool(body: &[u8], tool_name: &str) -> Option<Vec<u8>> {
    let mut value: Value = serde_json::from_slice(body).ok()?;
    let request = gemini_request_object_mut(&mut value)?;
    let tools = request.get_mut("tools")?.as_array_mut()?;
    let original_len = tools.len();
    tools.retain(|tool| {
        !tool
            .as_object()
            .map(|object| object.contains_key(tool_name))
            .unwrap_or(false)
    });
    if tools.len() == original_len {
        return None;
    }
    if tools.is_empty() {
        request.remove("tools");
    }
    serde_json::to_vec(&value).ok()
}

fn gemini_request_object_mut(value: &mut Value) -> Option<&mut serde_json::Map<String, Value>> {
    if value.get("request").is_some() {
        value.get_mut("request")?.as_object_mut()
    } else {
        value.as_object_mut()
    }
}
