#[path = "request_contents/items.rs"]
mod items;
#[path = "request_contents/system_instruction.rs"]
mod system_instruction;
#[path = "request_contents/text.rs"]
mod text;

use serde_json::Value;

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

#[cfg(feature = "mojo")]
fn gemini_request_function_part(
    operation: prodex_mojo_core::provider_constraints::GeminiRequestContentOperation,
    name: &str,
    value: &Value,
    call_id: Option<&str>,
) -> Value {
    let name = serde_json::to_vec(name).expect("Gemini function name serializes");
    let value = serde_json::to_vec(value).expect("Gemini function value serializes");
    let call_id = call_id
        .map(|call_id| serde_json::to_vec(call_id).expect("Gemini function call ID serializes"));
    gemini_request_content_mojo_value(
        operation,
        Some(&name),
        Some(&value),
        call_id.as_deref(),
        None,
        0,
    )
}

#[cfg(feature = "mojo")]
pub(crate) fn gemini_text_contents_from_request_mojo(
    value: &Value,
) -> Option<(Option<Value>, Vec<Value>)> {
    let input = serde_json::to_vec(value).expect("Gemini text request serializes");
    let mut kernel = prodex_mojo_core::provider_constraints::GeminiBridgeRequestKernelInput::new(
        prodex_mojo_core::provider_constraints::GeminiBridgeRequestOperation::TextContents,
    );
    kernel.primary = Some(&input);
    let body = prodex_mojo_core::provider_constraints::gemini_bridge_request_kernel(kernel)
        .unwrap_or_else(|error| panic!("Mojo Gemini text-contents kernel failed: {error:?}"));
    let mapped: Value = serde_json::from_slice(&body).unwrap_or_else(|error| {
        panic!("Mojo Gemini text-contents kernel returned invalid JSON: {error}")
    });
    if mapped.is_null() {
        return None;
    }
    let object = mapped
        .as_object()
        .expect("Mojo Gemini text-contents result is an object");
    let system_instruction = object
        .get("systemInstruction")
        .filter(|value| !value.is_null())
        .cloned();
    let contents = object
        .get("contents")
        .and_then(Value::as_array)
        .cloned()
        .expect("Mojo Gemini text-contents result has contents");
    Some((system_instruction, contents))
}

#[cfg(not(feature = "mojo"))]
pub(crate) use self::items::gemini_contains_local_media_path;
pub(crate) use self::items::gemini_contents_from_request;
pub(super) use self::system_instruction::gemini_system_instruction_from_request;
