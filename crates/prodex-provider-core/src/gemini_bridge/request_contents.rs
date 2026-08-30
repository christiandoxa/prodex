//! Rust-side views for the reachable Gemini request Mojo kernels.

#[cfg(feature = "mojo")]
use prodex_mojo_core::MojoError;
#[cfg(feature = "mojo")]
use serde_json::Value;

#[cfg(feature = "mojo")]
use prodex_mojo_core::provider_constraints::{
    GeminiBridgeRequestKernelInput, GeminiBridgeRequestOperation, GeminiRequestContentKernelInput,
    GeminiRequestContentOperation,
};

#[cfg(feature = "mojo")]
pub(super) fn gemini_request_content_value(
    operation: GeminiRequestContentOperation,
    primary: Option<&[u8]>,
    secondary: Option<&[u8]>,
    tertiary: Option<&[u8]>,
    quaternary: Option<&[u8]>,
    kind: i64,
) -> Value {
    let mut input = GeminiRequestContentKernelInput::new(operation);
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
fn gemini_bridge_request_bytes(
    operation: GeminiBridgeRequestOperation,
    primary: Option<&[u8]>,
    secondary: Option<&[u8]>,
    tertiary: Option<&[u8]>,
    quaternary: Option<&[u8]>,
    quinary: Option<&[u8]>,
    senary: Option<&[u8]>,
    septenary: Option<&[u8]>,
    octonary: Option<&[u8]>,
    kind: i64,
) -> Result<Vec<u8>, MojoError> {
    let mut input = GeminiBridgeRequestKernelInput::new(operation);
    input.primary = primary;
    input.secondary = secondary;
    input.tertiary = tertiary;
    input.quaternary = quaternary;
    input.quinary = quinary;
    input.senary = senary;
    input.septenary = septenary;
    input.octonary = octonary;
    input.kind = kind;
    prodex_mojo_core::provider_constraints::gemini_bridge_request_kernel(input)
}

#[cfg(feature = "mojo")]
pub(super) fn gemini_bridge_request_value(
    operation: GeminiBridgeRequestOperation,
    primary: Option<&[u8]>,
    secondary: Option<&[u8]>,
    tertiary: Option<&[u8]>,
    quaternary: Option<&[u8]>,
    quinary: Option<&[u8]>,
    senary: Option<&[u8]>,
    septenary: Option<&[u8]>,
    octonary: Option<&[u8]>,
    kind: i64,
) -> Value {
    let body = gemini_bridge_request_bytes(
        operation, primary, secondary, tertiary, quaternary, quinary, senary, septenary, octonary,
        kind,
    )
    .unwrap_or_else(|error| panic!("Mojo Gemini bridge request kernel failed: {error:?}"));
    serde_json::from_slice(&body).unwrap_or_else(|error| {
        panic!("Mojo Gemini bridge request kernel returned invalid JSON: {error}")
    })
}

#[cfg(feature = "mojo")]
pub(super) fn gemini_bridge_request_simple(body: &[u8]) -> bool {
    let Ok(body) = gemini_bridge_request_bytes(
        GeminiBridgeRequestOperation::SimpleRequest,
        Some(body),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        0,
    ) else {
        return false;
    };
    serde_json::from_slice::<Value>(&body)
        .ok()
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
}

#[cfg(feature = "mojo")]
pub(super) fn gemini_bridge_request_candidate_count(value: &Value) -> Result<(), String> {
    let input = serde_json::to_vec(value).expect("Gemini candidate-count input serializes");
    let body = gemini_bridge_request_bytes(
        GeminiBridgeRequestOperation::ValidateCandidateCount,
        Some(&input),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        0,
    )
    .map_err(|error| format!("invalid_candidate_count: Mojo validation failed: {error:?}"))?;
    match serde_json::from_slice::<Value>(&body) {
        Ok(Value::Null) => Ok(()),
        Ok(Value::String(error)) => Err(error),
        _ => Err("invalid_candidate_count: Mojo returned an invalid validation result".to_string()),
    }
}

#[cfg(feature = "mojo")]
pub(super) fn gemini_bridge_request_generation_config(
    original: &Value,
    chat: &Value,
    model: &str,
    thinking_budget_tokens: Option<u64>,
) -> Value {
    let original = serde_json::to_vec(original).expect("Gemini original request serializes");
    let chat = serde_json::to_vec(chat).expect("Gemini chat request serializes");
    let model = serde_json::to_vec(model).expect("Gemini model serializes");
    let budget = thinking_budget_tokens.map(|value| value.to_string());
    gemini_bridge_request_value(
        GeminiBridgeRequestOperation::GenerationConfig,
        Some(&original),
        Some(&chat),
        Some(&model),
        budget.as_deref().map(str::as_bytes),
        None,
        None,
        None,
        None,
        i64::from(thinking_budget_tokens.is_some()),
    )
}

#[cfg(feature = "mojo")]
pub(super) fn gemini_bridge_request_map(
    original: &Value,
    system_instruction: Option<&Value>,
    contents: &[Value],
    tools: Option<&Value>,
    tool_config: Option<&Value>,
    generation_config: &Value,
) -> serde_json::Map<String, Value> {
    let original = serde_json::to_vec(original).expect("Gemini original request serializes");
    let system_instruction = system_instruction
        .map(|value| serde_json::to_vec(value).expect("Gemini system instruction serializes"));
    let contents = serde_json::to_vec(contents).expect("Gemini contents serialize");
    let tools = tools.map(|value| serde_json::to_vec(value).expect("Gemini tools serialize"));
    let tool_config =
        tool_config.map(|value| serde_json::to_vec(value).expect("Gemini tool config serializes"));
    let generation_config =
        serde_json::to_vec(generation_config).expect("Gemini generation config serializes");
    let value = gemini_bridge_request_value(
        GeminiBridgeRequestOperation::GenerateContentRequest,
        Some(&original),
        system_instruction.as_deref(),
        Some(&contents),
        tools.as_deref(),
        tool_config.as_deref(),
        Some(&generation_config),
        None,
        None,
        0,
    );
    value
        .as_object()
        .cloned()
        .expect("Mojo Gemini request map is an object")
}

#[cfg(feature = "mojo")]
pub(super) fn gemini_bridge_request_body(
    model: &str,
    project_id: Option<&str>,
    code_assist: bool,
    request: &serde_json::Map<String, Value>,
) -> Value {
    let model = serde_json::to_vec(model).expect("Gemini model serializes");
    let project = project_id
        .map(|value| serde_json::to_vec(value).expect("Gemini project serializes"))
        .unwrap_or_else(|| b"null".to_vec());
    let request = serde_json::to_vec(request).expect("Gemini request serializes");
    gemini_bridge_request_value(
        GeminiBridgeRequestOperation::GenerateContentBody,
        Some(&model),
        Some(&project),
        Some(&request),
        None,
        None,
        None,
        None,
        None,
        i64::from(code_assist),
    )
}

#[cfg(feature = "mojo")]
pub(super) fn gemini_bridge_request_native_project(body: &[u8], project_id: &str) -> Vec<u8> {
    let project = serde_json::to_vec(project_id).expect("Gemini project serializes");
    gemini_bridge_request_bytes(
        GeminiBridgeRequestOperation::NativeProject,
        Some(body),
        Some(&project),
        None,
        None,
        None,
        None,
        None,
        None,
        0,
    )
    .unwrap_or_else(|error| panic!("Mojo Gemini bridge request kernel failed: {error:?}"))
}

#[cfg(feature = "mojo")]
pub(super) fn gemini_bridge_request_without_tool(body: &[u8], tool_name: &str) -> Option<Vec<u8>> {
    let result = gemini_bridge_request_bytes(
        GeminiBridgeRequestOperation::RequestBodyWithoutTool,
        Some(body),
        Some(tool_name.as_bytes()),
        None,
        None,
        None,
        None,
        None,
        None,
        0,
    )
    .ok()?;
    (!matches!(result.as_slice(), b"null")).then_some(result)
}
