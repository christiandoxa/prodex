#[cfg(test)]
#[path = "request_contents/items_tests.rs"]
mod items_tests;

use serde_json::Value;

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

pub(crate) fn gemini_request_contents_from_request_mojo(
    value: &Value,
) -> Result<GeminiRequestContents, String> {
    let input = serde_json::to_vec(value)
        .map_err(|error| format!("failed to serialize Gemini text request: {error}"))?;
    let mut kernel = prodex_mojo_core::provider_constraints::GeminiBridgeRequestKernelInput::new(
        prodex_mojo_core::provider_constraints::GeminiBridgeRequestOperation::TextContents,
    );
    kernel.primary = Some(&input);
    let body = prodex_mojo_core::provider_constraints::gemini_bridge_request_kernel(kernel)
        .map_err(|error| format!("Mojo Gemini request-content kernel failed: {error:?}"))?;
    let mapped: Value = serde_json::from_slice(&body).map_err(|error| {
        format!("Mojo Gemini text-contents kernel returned invalid JSON: {error}")
    })?;
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
    Ok((system_instruction, contents))
}

pub(crate) fn gemini_contents_from_request(value: &Value) -> Result<Vec<Value>, String> {
    gemini_request_contents_from_request_mojo(value).map(|(_, contents)| contents)
}
