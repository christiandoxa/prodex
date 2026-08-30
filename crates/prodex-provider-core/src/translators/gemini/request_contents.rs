#[path = "request_contents/items.rs"]
mod items;
#[path = "request_contents/system_instruction.rs"]
mod system_instruction;
#[path = "request_contents/text.rs"]
mod text;

#[cfg(feature = "mojo")]
use serde_json::Value;

#[cfg(feature = "mojo")]
pub(crate) fn gemini_request_content_mojo_value(
    operation: prodex_mojo_core::provider_constraints::GeminiRequestContentOperation,
    primary: Option<&[u8]>,
    secondary: Option<&[u8]>,
    tertiary: Option<&[u8]>,
    quaternary: Option<&[u8]>,
    kind: i64,
) -> Value {
    let mut input =
        prodex_mojo_core::provider_constraints::GeminiRequestContentKernelInput::new(operation);
    input.primary = primary;
    input.secondary = secondary;
    input.tertiary = tertiary;
    input.quaternary = quaternary;
    input.kind = kind;
    let body = prodex_mojo_core::provider_constraints::gemini_request_content_kernel(input)
        .unwrap_or_else(|error| panic!("Mojo Gemini request-content kernel failed: {error:?}"));
    serde_json::from_slice(&body).unwrap_or_else(|error| {
        panic!("Mojo Gemini request-content kernel returned invalid JSON: {error}")
    })
}

pub(crate) use self::items::gemini_contains_local_media_path;
pub(crate) use self::items::gemini_contents_from_request;
pub(super) use self::system_instruction::gemini_system_instruction_from_request;
pub(crate) use self::text::{
    gemini_contextual_user_instruction_text, gemini_is_contextual_user_fragment,
};
