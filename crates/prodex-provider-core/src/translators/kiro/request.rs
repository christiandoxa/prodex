//! Kiro provider request rewriting and Chat Completions compatibility helpers.

#[path = "request/messages.rs"]
mod messages;
#[cfg(test)]
#[path = "request/semantics_tests.rs"]
mod semantics_tests;

pub use messages::{
    kiro_provider_core_prompt_from_chat_messages,
    kiro_provider_core_responses_items_from_chat_message,
    kiro_provider_core_tool_choice_from_legacy_chat_function_call,
    kiro_provider_core_tool_from_legacy_chat_function,
};
use serde_json::Value;

use prodex_mojo_core::rich::{
    KiroChatRewriteIssue, KiroKernelInput, KiroKernelOperation, KiroRequestValidationMode,
    KiroRequestValidationPlan, kiro_kernel, kiro_rewrite_chat_request_json,
    kiro_validate_request_json,
};

use crate::{
    deepseek_provider_core_reject_beta_completion_fields,
    deepseek_provider_core_reject_unsupported_request_fields,
    deepseek_provider_core_validate_reasoning_shape,
    deepseek_provider_core_validate_supported_input_item,
    provider_core_chat_compatible_validate_top_level_request_shape,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KiroProviderCoreRequestError {
    pub message: String,
    pub code: String,
}

impl KiroProviderCoreRequestError {
    fn new(message: impl Into<String>, code: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            code: code.into(),
        }
    }
}

pub(super) fn kiro_mojo_body(input: KiroKernelInput<'_>) -> Vec<u8> {
    kiro_kernel(input).unwrap_or_else(|error| panic!("Mojo Kiro kernel failed: {error:?}"))
}

fn kiro_validation_error(
    plan: KiroRequestValidationPlan,
    object: &serde_json::Map<String, Value>,
) -> Result<(), KiroProviderCoreRequestError> {
    if plan.reason == KiroRequestValidationPlan::REASON_NONE {
        return Ok(());
    }
    let effort = object
        .get("reasoning")
        .and_then(|reasoning| reasoning.get("effort"))
        .or_else(|| object.get("reasoning_effort"))
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let mut input = KiroKernelInput::new(KiroKernelOperation::RequestValidationError);
    input.request_id = u64::try_from(plan.reason).unwrap_or_default();
    input.used = u64::try_from(plan.detail).unwrap_or(u64::MAX);
    input.include_role = plan.detail_is_invalid;
    input.reason = Some(effort);
    let rendered =
        String::from_utf8(kiro_mojo_body(input)).expect("Mojo Kiro validation error is UTF-8");
    let (code, message) = rendered
        .split_once('\n')
        .expect("Mojo Kiro validation error contains code and message");
    Err(KiroProviderCoreRequestError::new(message, code))
}

pub fn kiro_provider_core_chat_completions_request_body(
    body: &[u8],
) -> Result<Vec<u8>, KiroProviderCoreRequestError> {
    let value: Value = serde_json::from_slice(body).map_err(|_| {
        KiroProviderCoreRequestError::new(
            "Kiro chat completions request body must be valid JSON",
            "invalid_json",
        )
    })?;
    let Some(object) = value.as_object() else {
        return Err(KiroProviderCoreRequestError::new(
            "Kiro chat completions request body must be a JSON object",
            "invalid_request_body",
        ));
    };

    let canonical = serde_json::to_string(&value).map_err(|_| {
        KiroProviderCoreRequestError::new(
            "failed to serialize Kiro chat completions request",
            "invalid_request_body",
        )
    })?;
    let plan = kiro_validate_request_json(
        KiroRequestValidationMode::ChatCompletions,
        &canonical,
        false,
    )
    .unwrap_or_else(|error| panic!("Mojo Kiro raw request validation failed: {error:?}"));
    kiro_validation_error(plan, object)?;

    let had_input = object.contains_key("input");
    let rewritten = kiro_rewrite_chat_request_json(&canonical)
        .unwrap_or_else(|error| panic!("Mojo Kiro raw chat rewrite failed: {error:?}"));
    match rewritten.issue {
        KiroChatRewriteIssue::None => {}
        KiroChatRewriteIssue::MissingMessages => {
            return Err(KiroProviderCoreRequestError::new(
                "Kiro chat completions request is missing messages",
                "missing_messages",
            ));
        }
        KiroChatRewriteIssue::InvalidMessages => {
            return Err(KiroProviderCoreRequestError::new(
                "Kiro chat completions messages must be an array",
                "invalid_messages",
            ));
        }
    }
    let mut rewritten: Value = serde_json::from_slice(&rewritten.body).map_err(|_| {
        KiroProviderCoreRequestError::new(
            "failed to serialize rewritten Kiro chat completions body",
            "invalid_request_body",
        )
    })?;
    let object = rewritten.as_object_mut().ok_or_else(|| {
        KiroProviderCoreRequestError::new(
            "failed to serialize rewritten Kiro chat completions body",
            "invalid_request_body",
        )
    })?;
    if !had_input {
        kiro_rewrite_legacy_chat_tools(object);
    }
    let body = serde_json::to_vec(&rewritten).map_err(|_| {
        KiroProviderCoreRequestError::new(
            "failed to serialize rewritten Kiro chat completions body",
            "invalid_request_body",
        )
    })?;
    kiro_provider_core_responses_request_body(&body, false)
}
fn kiro_rewrite_legacy_chat_tools(object: &mut serde_json::Map<String, Value>) {
    if !object.contains_key("tools")
        && let Some(functions) = object.remove("functions")
        && let Some(functions) = functions.as_array()
    {
        object.insert(
            "tools".to_string(),
            Value::Array(
                functions
                    .iter()
                    .filter_map(kiro_provider_core_tool_from_legacy_chat_function)
                    .collect(),
            ),
        );
    }
    if !object.contains_key("tool_choice")
        && let Some(function_call) = object.remove("function_call")
        && let Some(tool_choice) =
            kiro_provider_core_tool_choice_from_legacy_chat_function_call(&function_call)
    {
        object.insert("tool_choice".to_string(), tool_choice);
    }
}

pub(super) fn kiro_provider_core_responses_request_body(
    body: &[u8],
    allow_token_limit: bool,
) -> Result<Vec<u8>, KiroProviderCoreRequestError> {
    let value: Value = serde_json::from_slice(body).map_err(|_| {
        KiroProviderCoreRequestError::new(
            "Kiro Responses request body must be valid JSON",
            "invalid_json",
        )
    })?;
    provider_core_chat_compatible_validate_top_level_request_shape(&value, "Kiro")
        .map_err(kiro_invalid_request)?;
    let object = value.as_object().expect("validated request object");

    kiro_validate_response_input(object)?;
    let raw = std::str::from_utf8(body).expect("valid JSON is valid UTF-8");
    let plan =
        kiro_validate_request_json(KiroRequestValidationMode::Responses, raw, allow_token_limit)
            .unwrap_or_else(|error| panic!("Mojo Kiro raw request validation failed: {error:?}"));
    if matches!(
        plan.reason,
        KiroRequestValidationPlan::REASON_NONE | KiroRequestValidationPlan::REASON_REASONING_EFFORT
    ) {
        deepseek_provider_core_validate_reasoning_shape(&value, "Kiro")
            .map_err(kiro_invalid_request)?;
    }
    kiro_validation_error(plan, object)?;
    deepseek_provider_core_reject_beta_completion_fields(&value, "Kiro")
        .map_err(kiro_invalid_request)?;
    deepseek_provider_core_reject_unsupported_request_fields(&value, "Kiro")
        .map_err(kiro_invalid_request)?;
    if allow_token_limit && object.contains_key("messages") && !object.contains_key("input") {
        let canonical =
            serde_json::to_string(&value).expect("Kiro Anthropic Messages request serializes");
        let mut input_value = KiroKernelInput::new(KiroKernelOperation::AnthropicRequestRewrite);
        input_value.input = Some(&canonical);
        return Ok(kiro_mojo_body(input_value));
    }
    let model = object.get("model").and_then(Value::as_str);
    let input = object
        .get("input")
        .map(|value| serde_json::to_string(value).expect("Kiro request input serializes"));
    let extra = object
        .iter()
        .filter(|(key, _)| !matches!(key.as_str(), "model" | "input"))
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect::<serde_json::Map<_, _>>();
    let extra =
        serde_json::to_string(&Value::Object(extra)).expect("Kiro request extra fields serialize");
    let mut input_value = KiroKernelInput::new(KiroKernelOperation::RequestBody);
    input_value.model = model;
    input_value.input = input.as_deref();
    input_value.extra = Some(&extra);
    Ok(kiro_mojo_body(input_value))
}

fn kiro_validate_response_input(
    object: &serde_json::Map<String, Value>,
) -> Result<(), KiroProviderCoreRequestError> {
    if let Some(input) = object.get("input").and_then(Value::as_array) {
        for item in input {
            deepseek_provider_core_validate_supported_input_item(item, false, "Kiro")
                .map_err(kiro_invalid_request)?;
        }
    }
    Ok(())
}

fn kiro_invalid_request(message: String) -> KiroProviderCoreRequestError {
    KiroProviderCoreRequestError::new(message, "invalid_request")
}
