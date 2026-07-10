//! Kiro provider request rewriting and Chat Completions compatibility helpers.

#[path = "request/controls.rs"]
mod controls;
#[path = "request/messages.rs"]
mod messages;

use controls::{
    kiro_provider_core_has_requested_nondefault_number,
    kiro_provider_core_has_requested_parallel_tool_calls_control,
    kiro_provider_core_has_requested_sampling_value,
    kiro_provider_core_has_requested_stop_sequences,
    kiro_provider_core_strip_accepted_token_limit_controls,
    kiro_provider_core_supported_chat_response_format,
};
pub use messages::{
    kiro_provider_core_prompt_from_chat_messages,
    kiro_provider_core_responses_items_from_chat_message,
    kiro_provider_core_tool_choice_from_legacy_chat_function_call,
    kiro_provider_core_tool_from_legacy_chat_function,
};
use serde_json::Value;

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

pub fn kiro_provider_core_chat_completions_request_body(
    body: &[u8],
) -> Result<Vec<u8>, KiroProviderCoreRequestError> {
    let mut value: Value = serde_json::from_slice(body).map_err(|_| {
        KiroProviderCoreRequestError::new(
            "Kiro chat completions request body must be valid JSON",
            "invalid_json",
        )
    })?;
    let Some(object) = value.as_object_mut() else {
        return Err(KiroProviderCoreRequestError::new(
            "Kiro chat completions request body must be a JSON object",
            "invalid_request_body",
        ));
    };
    if let Some(response_format) = object.get("response_format")
        && !kiro_provider_core_supported_chat_response_format(response_format)
    {
        return Err(KiroProviderCoreRequestError::new(
            "Kiro provider only supports chat response_format type 'text' right now",
            "unsupported_response_format",
        ));
    }
    if object
        .get("n")
        .and_then(Value::as_u64)
        .is_some_and(|n| n > 1)
    {
        return Err(KiroProviderCoreRequestError::new(
            "Kiro provider only supports chat completion parameter n=1 right now",
            "unsupported_choice_count",
        ));
    }
    if let Some(stop) = object.get("stop") {
        if kiro_provider_core_has_requested_stop_sequences(stop) {
            return Err(KiroProviderCoreRequestError::new(
                "Kiro provider does not support chat stop sequences right now",
                "unsupported_stop",
            ));
        }
        object.remove("stop");
    }
    if let Some(temperature) = object.get("temperature") {
        if kiro_provider_core_has_requested_nondefault_number(temperature, 1.0) {
            return Err(KiroProviderCoreRequestError::new(
                "Kiro provider does not support non-default chat temperature right now",
                "unsupported_temperature",
            ));
        }
        object.remove("temperature");
    }
    if let Some(top_p) = object.get("top_p") {
        if kiro_provider_core_has_requested_nondefault_number(top_p, 1.0) {
            return Err(KiroProviderCoreRequestError::new(
                "Kiro provider does not support non-default chat top_p right now",
                "unsupported_top_p",
            ));
        }
        object.remove("top_p");
    }
    if let Some(presence_penalty) = object.get("presence_penalty") {
        if kiro_provider_core_has_requested_nondefault_number(presence_penalty, 0.0) {
            return Err(KiroProviderCoreRequestError::new(
                "Kiro provider does not support non-default chat presence_penalty right now",
                "unsupported_presence_penalty",
            ));
        }
        object.remove("presence_penalty");
    }
    if let Some(frequency_penalty) = object.get("frequency_penalty") {
        if kiro_provider_core_has_requested_nondefault_number(frequency_penalty, 0.0) {
            return Err(KiroProviderCoreRequestError::new(
                "Kiro provider does not support non-default chat frequency_penalty right now",
                "unsupported_frequency_penalty",
            ));
        }
        object.remove("frequency_penalty");
    }
    if object
        .get("seed")
        .is_some_and(kiro_provider_core_has_requested_sampling_value)
    {
        return Err(KiroProviderCoreRequestError::new(
            "Kiro provider does not support chat seed right now",
            "unsupported_seed",
        ));
    }
    if let Some(parallel_tool_calls) = object.get("parallel_tool_calls") {
        if kiro_provider_core_has_requested_parallel_tool_calls_control(parallel_tool_calls) {
            return Err(KiroProviderCoreRequestError::new(
                "Kiro provider does not support chat parallel_tool_calls right now",
                "unsupported_parallel_tool_calls",
            ));
        }
        object.remove("parallel_tool_calls");
    }
    object.remove("user");
    kiro_provider_core_strip_accepted_token_limit_controls(object)?;
    if object.contains_key("input") || !object.contains_key("messages") {
        return Ok(body.to_vec());
    }
    let messages = object.remove("messages").ok_or_else(|| {
        KiroProviderCoreRequestError::new(
            "Kiro chat completions request is missing messages",
            "missing_messages",
        )
    })?;
    let items = messages
        .as_array()
        .ok_or_else(|| {
            KiroProviderCoreRequestError::new(
                "Kiro chat completions messages must be an array",
                "invalid_messages",
            )
        })?
        .iter()
        .flat_map(kiro_provider_core_responses_items_from_chat_message)
        .collect::<Vec<_>>();
    object.insert("input".to_string(), Value::Array(items));
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
    serde_json::to_vec(&value).map_err(|_| {
        KiroProviderCoreRequestError::new(
            "failed to serialize rewritten Kiro chat completions body",
            "invalid_request_body",
        )
    })
}
