#[path = "request_contents/items.rs"]
mod items;
#[path = "request_contents/system_instruction.rs"]
mod system_instruction;
#[path = "request_contents/text.rs"]
mod text;

use serde_json::Value;

#[cfg(feature = "mojo")]
type GeminiRequestContents = (Option<Value>, Vec<Value>);

pub(crate) fn gemini_request_content_mojo_value(
    operation: prodex_mojo_core::provider_constraints::GeminiRequestContentOperation,
    primary: Option<&[u8]>,
    secondary: Option<&[u8]>,
    tertiary: Option<&[u8]>,
    quaternary: Option<&[u8]>,
    kind: i64,
) -> Result<Value, String> {
    let mut input =
        prodex_mojo_core::provider_constraints::GeminiRequestContentKernelInput::new(operation);
    input.primary = primary;
    input.secondary = secondary;
    input.tertiary = tertiary;
    input.quaternary = quaternary;
    input.kind = kind;
    let body = prodex_mojo_core::provider_constraints::gemini_request_content_kernel(input)
        .map_err(|error| format!("Mojo Gemini request-content kernel failed: {error:?}"))?;
    serde_json::from_slice(&body).map_err(|error| {
        format!("Mojo Gemini request-content kernel returned invalid JSON: {error}")
    })
}

pub(crate) fn gemini_request_content_mojo_value_or_panic(
    operation: prodex_mojo_core::provider_constraints::GeminiRequestContentOperation,
    primary: Option<&[u8]>,
    secondary: Option<&[u8]>,
    tertiary: Option<&[u8]>,
    quaternary: Option<&[u8]>,
    kind: i64,
) -> Value {
    gemini_request_content_mojo_value(operation, primary, secondary, tertiary, quaternary, kind)
        .unwrap_or_else(|error| panic!("{error}"))
}

#[cfg(feature = "mojo")]
fn gemini_request_function_part(
    operation: prodex_mojo_core::provider_constraints::GeminiRequestContentOperation,
    name: &str,
    value: &Value,
    call_id: Option<&str>,
) -> Result<Value, String> {
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
) -> Result<Option<GeminiRequestContents>, String> {
    let input = serde_json::to_vec(value)
        .map_err(|error| format!("failed to serialize Gemini text request: {error}"))?;
    let mut kernel = prodex_mojo_core::provider_constraints::GeminiBridgeRequestKernelInput::new(
        prodex_mojo_core::provider_constraints::GeminiBridgeRequestOperation::TextContents,
    );
    kernel.primary = Some(&input);
    let body = prodex_mojo_core::provider_constraints::gemini_bridge_request_kernel(kernel)
        .map_err(|error| format!("Mojo Gemini text-contents kernel failed: {error:?}"))?;
    let mapped: Value = serde_json::from_slice(&body).map_err(|error| {
        format!("Mojo Gemini text-contents kernel returned invalid JSON: {error}")
    })?;
    if mapped.is_null() {
        return Ok(None);
    }
    let object = mapped
        .as_object()
        .ok_or_else(|| "Mojo Gemini text-contents result is not an object".to_string())?;
    let system_instruction = object
        .get("systemInstruction")
        .filter(|value| !value.is_null())
        .cloned();
    let contents = object
        .get("contents")
        .and_then(Value::as_array)
        .cloned()
        .ok_or_else(|| "Mojo Gemini text-contents result has no contents array".to_string())?;
    Ok(Some((system_instruction, contents)))
}

#[cfg(not(feature = "mojo"))]
pub(crate) use self::items::gemini_contains_local_media_path;
pub(crate) use self::items::gemini_contents_from_request;
pub(super) use self::system_instruction::gemini_system_instruction_from_request;
