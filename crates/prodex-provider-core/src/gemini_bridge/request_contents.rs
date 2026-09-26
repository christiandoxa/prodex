//! Rust-side views for the reachable Gemini request Mojo kernels.

use prodex_mojo_core::MojoError;
use serde_json::Value;

use prodex_mojo_core::provider_constraints::{
    GeminiBridgeRequestKernelInput, GeminiBridgeRequestOperation,
};
fn gemini_bridge_request_bytes(
    input: GeminiBridgeRequestKernelInput<'_>,
) -> Result<Vec<u8>, MojoError> {
    prodex_mojo_core::provider_constraints::gemini_bridge_request_kernel(input)
}

fn gemini_bridge_request_value(input: GeminiBridgeRequestKernelInput<'_>) -> Result<Value, String> {
    let body = gemini_bridge_request_bytes(input)
        .map_err(|error| format!("Mojo Gemini bridge request kernel failed: {error:?}"))?;
    serde_json::from_slice(&body).map_err(|error| {
        format!("Mojo Gemini bridge request kernel returned invalid JSON: {error}")
    })
}

#[cfg(feature = "mojo")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GeminiTranslatorValidationPlan {
    pub tag: i64,
    pub index: Option<usize>,
    pub detail: Option<String>,
}

#[cfg(feature = "mojo")]
pub(crate) fn gemini_bridge_validate_translator(
    body: &[u8],
) -> Result<GeminiTranslatorValidationPlan, String> {
    let value = gemini_bridge_request_value(GeminiBridgeRequestKernelInput {
        operation: GeminiBridgeRequestOperation::ValidateTranslatorRequest,
        primary: Some(body),
        ..GeminiBridgeRequestKernelInput::new(
            GeminiBridgeRequestOperation::ValidateTranslatorRequest,
        )
    })?;
    let tag = value
        .get("tag")
        .and_then(Value::as_i64)
        .ok_or_else(|| "Mojo Gemini translator validation returned an invalid tag".to_string())?;
    let index = value
        .get("index")
        .and_then(Value::as_i64)
        .and_then(|value| usize::try_from(value).ok());
    let detail = value
        .get("detail")
        .and_then(Value::as_str)
        .map(str::to_string);
    Ok(GeminiTranslatorValidationPlan { tag, index, detail })
}

#[cfg(feature = "mojo")]
pub(crate) fn gemini_bridge_raw_translator_request(
    original: &Value,
    system_instruction: Option<&Value>,
    contents: &[Value],
    tools: Option<&Value>,
    tool_config: Option<&Value>,
    model: &str,
) -> Result<Vec<u8>, String> {
    let original = serde_json::to_vec(original)
        .map_err(|error| format!("failed to serialize Gemini translator request: {error}"))?;
    let system_instruction = system_instruction
        .map(|value| {
            serde_json::to_vec(value)
                .map_err(|error| format!("failed to serialize Gemini system instruction: {error}"))
        })
        .transpose()?;
    let contents = serde_json::to_vec(contents)
        .map_err(|error| format!("failed to serialize Gemini contents: {error}"))?;
    let tools = tools
        .map(|value| {
            serde_json::to_vec(value)
                .map_err(|error| format!("failed to serialize Gemini tools: {error}"))
        })
        .transpose()?;
    let tool_config = tool_config
        .map(|value| {
            serde_json::to_vec(value)
                .map_err(|error| format!("failed to serialize Gemini tool config: {error}"))
        })
        .transpose()?;
    let model = serde_json::to_vec(model)
        .map_err(|error| format!("failed to serialize Gemini model: {error}"))?;
    gemini_bridge_request_bytes(GeminiBridgeRequestKernelInput {
        operation: GeminiBridgeRequestOperation::RawTranslatorRequest,
        primary: Some(&original),
        secondary: system_instruction.as_deref(),
        tertiary: Some(&contents),
        quaternary: tools.as_deref(),
        quinary: tool_config.as_deref(),
        senary: Some(&model),
        ..GeminiBridgeRequestKernelInput::new(GeminiBridgeRequestOperation::RawTranslatorRequest)
    })
    .map_err(|error| format!("Mojo Gemini raw translator request failed: {error:?}"))
}

#[cfg(feature = "mojo")]
pub(super) fn gemini_bridge_request_simple(body: &[u8]) -> bool {
    let Ok(body) = gemini_bridge_request_bytes(GeminiBridgeRequestKernelInput {
        operation: GeminiBridgeRequestOperation::SimpleRequest,
        primary: Some(body),
        ..GeminiBridgeRequestKernelInput::new(GeminiBridgeRequestOperation::SimpleRequest)
    }) else {
        return false;
    };
    serde_json::from_slice::<Value>(&body)
        .ok()
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
}

#[cfg(feature = "mojo")]
pub(super) fn gemini_bridge_request_candidate_count(value: &Value) -> Result<(), String> {
    let input = serde_json::to_vec(value)
        .map_err(|error| format!("failed to serialize Gemini candidate-count input: {error}"))?;
    let body = gemini_bridge_request_bytes(GeminiBridgeRequestKernelInput {
        operation: GeminiBridgeRequestOperation::ValidateCandidateCount,
        primary: Some(&input),
        ..GeminiBridgeRequestKernelInput::new(GeminiBridgeRequestOperation::ValidateCandidateCount)
    })
    .map_err(|error| format!("invalid_candidate_count: Mojo validation failed: {error:?}"))?;
    match serde_json::from_slice::<Value>(&body) {
        Ok(Value::Null) => Ok(()),
        Ok(Value::String(error)) => Err(error),
        _ => Err("invalid_candidate_count: Mojo returned an invalid validation result".to_string()),
    }
}

pub(super) fn gemini_bridge_request_tool_config(value: &Value) -> Result<Option<Value>, String> {
    let input = serde_json::to_vec(value)
        .map_err(|error| format!("failed to serialize Gemini tool config: {error}"))?;
    let value = gemini_bridge_request_value(GeminiBridgeRequestKernelInput {
        operation: GeminiBridgeRequestOperation::ToolConfig,
        primary: Some(&input),
        ..GeminiBridgeRequestKernelInput::new(GeminiBridgeRequestOperation::ToolConfig)
    })?;
    Ok((!value.is_null()).then_some(value))
}

pub(super) fn gemini_bridge_request_generation_config(
    original: &Value,
    chat: &Value,
    model: &str,
    thinking_budget_tokens: Option<u64>,
) -> Result<Value, String> {
    let original = serde_json::to_vec(original)
        .map_err(|error| format!("failed to serialize Gemini original request: {error}"))?;
    let chat = serde_json::to_vec(chat)
        .map_err(|error| format!("failed to serialize Gemini chat request: {error}"))?;
    let model = serde_json::to_vec(model)
        .map_err(|error| format!("failed to serialize Gemini model: {error}"))?;
    let budget = thinking_budget_tokens.map(|value| value.to_string());
    gemini_bridge_request_value(GeminiBridgeRequestKernelInput {
        operation: GeminiBridgeRequestOperation::GenerationConfig,
        primary: Some(&original),
        secondary: Some(&chat),
        tertiary: Some(&model),
        quaternary: budget.as_deref().map(str::as_bytes),
        kind: i64::from(thinking_budget_tokens.is_some()),
        ..GeminiBridgeRequestKernelInput::new(GeminiBridgeRequestOperation::GenerationConfig)
    })
}

pub(super) fn gemini_bridge_request_map(
    original: &Value,
    system_instruction: Option<&Value>,
    contents: &[Value],
    tools: Option<&Value>,
    tool_config: Option<&Value>,
    generation_config: &Value,
) -> Result<serde_json::Map<String, Value>, String> {
    let original = serde_json::to_vec(original)
        .map_err(|error| format!("failed to serialize Gemini original request: {error}"))?;
    let system_instruction = system_instruction
        .map(|value| {
            serde_json::to_vec(value)
                .map_err(|error| format!("failed to serialize Gemini system instruction: {error}"))
        })
        .transpose()?;
    let contents = serde_json::to_vec(contents)
        .map_err(|error| format!("failed to serialize Gemini contents: {error}"))?;
    let tools = tools
        .map(|value| {
            serde_json::to_vec(value)
                .map_err(|error| format!("failed to serialize Gemini tools: {error}"))
        })
        .transpose()?;
    let tool_config = tool_config
        .map(|value| {
            serde_json::to_vec(value)
                .map_err(|error| format!("failed to serialize Gemini tool config: {error}"))
        })
        .transpose()?;
    let generation_config = serde_json::to_vec(generation_config)
        .map_err(|error| format!("failed to serialize Gemini generation config: {error}"))?;
    let value = gemini_bridge_request_value(GeminiBridgeRequestKernelInput {
        operation: GeminiBridgeRequestOperation::GenerateContentRequest,
        primary: Some(&original),
        secondary: system_instruction.as_deref(),
        tertiary: Some(&contents),
        quaternary: tools.as_deref(),
        quinary: tool_config.as_deref(),
        senary: Some(&generation_config),
        ..GeminiBridgeRequestKernelInput::new(GeminiBridgeRequestOperation::GenerateContentRequest)
    })?;
    value
        .as_object()
        .cloned()
        .ok_or_else(|| "Mojo Gemini request map is not an object".to_string())
}

pub(super) fn gemini_bridge_request_body(
    model: &str,
    project_id: Option<&str>,
    code_assist: bool,
    request: &serde_json::Map<String, Value>,
) -> Result<Value, String> {
    let model = serde_json::to_vec(model)
        .map_err(|error| format!("failed to serialize Gemini model: {error}"))?;
    let project = project_id
        .map(|value| {
            serde_json::to_vec(value)
                .map_err(|error| format!("failed to serialize Gemini project: {error}"))
        })
        .transpose()?
        .unwrap_or_else(|| b"null".to_vec());
    let request = serde_json::to_vec(request)
        .map_err(|error| format!("failed to serialize Gemini request: {error}"))?;
    gemini_bridge_request_value(GeminiBridgeRequestKernelInput {
        operation: GeminiBridgeRequestOperation::GenerateContentBody,
        primary: Some(&model),
        secondary: Some(&project),
        tertiary: Some(&request),
        kind: i64::from(code_assist),
        ..GeminiBridgeRequestKernelInput::new(GeminiBridgeRequestOperation::GenerateContentBody)
    })
}

#[cfg(feature = "mojo")]
pub(super) fn gemini_bridge_request_native_project(
    body: &[u8],
    project_id: &str,
) -> Result<Vec<u8>, String> {
    let project = serde_json::to_vec(project_id)
        .map_err(|error| format!("failed to serialize Gemini project: {error}"))?;
    gemini_bridge_request_bytes(GeminiBridgeRequestKernelInput {
        operation: GeminiBridgeRequestOperation::NativeProject,
        primary: Some(body),
        secondary: Some(&project),
        ..GeminiBridgeRequestKernelInput::new(GeminiBridgeRequestOperation::NativeProject)
    })
    .map_err(|error| format!("Mojo Gemini bridge request kernel failed: {error:?}"))
}

#[cfg(feature = "mojo")]
pub(super) fn gemini_bridge_request_without_tool(body: &[u8], tool_name: &str) -> Option<Vec<u8>> {
    let result = gemini_bridge_request_bytes(GeminiBridgeRequestKernelInput {
        operation: GeminiBridgeRequestOperation::RequestBodyWithoutTool,
        primary: Some(body),
        secondary: Some(tool_name.as_bytes()),
        ..GeminiBridgeRequestKernelInput::new(GeminiBridgeRequestOperation::RequestBodyWithoutTool)
    })
    .ok()?;
    (!matches!(result.as_slice(), b"null")).then_some(result)
}
