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
    kiro_provider_core_reject_token_limit_controls,
    kiro_provider_core_supported_chat_response_format,
};
pub use messages::{
    kiro_provider_core_prompt_from_chat_messages,
    kiro_provider_core_responses_items_from_chat_message,
    kiro_provider_core_tool_choice_from_legacy_chat_function_call,
    kiro_provider_core_tool_from_legacy_chat_function,
};
use serde_json::Value;

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
    if let Some(n) = object.get("n")
        && !n.is_null()
        && n.as_u64() != Some(1)
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
    object.remove("n");
    object.remove("user");
    kiro_provider_core_reject_token_limit_controls(object)?;
    if object.contains_key("input") {
        let body = serde_json::to_vec(&value).map_err(|_| {
            KiroProviderCoreRequestError::new(
                "failed to serialize rewritten Kiro chat completions body",
                "invalid_request_body",
            )
        })?;
        return kiro_provider_core_responses_request_body(&body, false);
    }
    if !object.contains_key("messages") {
        return Err(KiroProviderCoreRequestError::new(
            "Kiro chat completions request is missing messages",
            "missing_messages",
        ));
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
    let body = serde_json::to_vec(&value).map_err(|_| {
        KiroProviderCoreRequestError::new(
            "failed to serialize rewritten Kiro chat completions body",
            "invalid_request_body",
        )
    })?;
    kiro_provider_core_responses_request_body(&body, false)
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

    if let Some(input) = object.get("input").and_then(Value::as_array) {
        for item in input {
            deepseek_provider_core_validate_supported_input_item(item, false, "Kiro")
                .map_err(kiro_invalid_request)?;
        }
    }

    for field in ["temperature", "top_p", "seed"] {
        if object.get(field).is_some_and(|value| !value.is_null()) {
            return Err(KiroProviderCoreRequestError::new(
                format!("Kiro ACP does not expose the {field} control"),
                "unsupported_generation_control",
            ));
        }
    }
    if !allow_token_limit {
        kiro_provider_core_reject_token_limit_controls(object)?;
    }
    for field in ["stop", "stop_sequences", "stopSequences"] {
        if object
            .get(field)
            .is_some_and(kiro_provider_core_has_requested_stop_sequences)
        {
            return Err(KiroProviderCoreRequestError::new(
                "Kiro ACP does not expose stop-sequence controls",
                "unsupported_stop",
            ));
        }
    }
    if let Some(logprobs) = object.get("logprobs") {
        match logprobs {
            Value::Null | Value::Bool(false) => {}
            Value::Bool(true) => {
                return Err(KiroProviderCoreRequestError::new(
                    "Kiro ACP does not expose log probabilities",
                    "unsupported_logprobs",
                ));
            }
            _ => {
                return Err(KiroProviderCoreRequestError::new(
                    "Kiro logprobs must be a boolean",
                    "invalid_logprobs",
                ));
            }
        }
    }
    if object
        .get("top_logprobs")
        .is_some_and(|value| !value.is_null())
    {
        return Err(KiroProviderCoreRequestError::new(
            "Kiro ACP does not expose top_logprobs",
            "unsupported_logprobs",
        ));
    }

    for format in [
        object.get("response_format"),
        object.get("text").and_then(|text| text.get("format")),
    ]
    .into_iter()
    .flatten()
    {
        if !kiro_provider_core_supported_chat_response_format(format) {
            return Err(KiroProviderCoreRequestError::new(
                "Kiro ACP supports only text response format",
                "unsupported_response_format",
            ));
        }
    }
    if let Some(choice) = object.get("tool_choice")
        && !choice.is_null()
        && choice.as_str() != Some("auto")
    {
        return Err(KiroProviderCoreRequestError::new(
            "Kiro ACP owns tool selection and cannot honor tool_choice",
            "unsupported_tool_choice",
        ));
    }
    if object.get("web_search_options").is_some() {
        return Err(KiroProviderCoreRequestError::new(
            "Kiro ACP owns web search and cannot honor web_search_options",
            "unsupported_web_search_options",
        ));
    }
    deepseek_provider_core_validate_reasoning_shape(&value, "Kiro")
        .map_err(kiro_invalid_request)?;
    if let Some(effort) = object
        .get("reasoning")
        .and_then(|reasoning| reasoning.get("effort"))
        .or_else(|| object.get("reasoning_effort"))
        .and_then(Value::as_str)
        && !matches!(effort.trim(), "low" | "medium" | "high" | "xhigh" | "max")
    {
        return Err(KiroProviderCoreRequestError::new(
            format!("Kiro ACP does not support reasoning effort `{effort}`"),
            "unsupported_reasoning_effort",
        ));
    }
    deepseek_provider_core_reject_beta_completion_fields(&value, "Kiro")
        .map_err(kiro_invalid_request)?;
    deepseek_provider_core_reject_unsupported_request_fields(&value, "Kiro")
        .map_err(kiro_invalid_request)?;
    Ok(body.to_vec())
}

fn kiro_invalid_request(message: String) -> KiroProviderCoreRequestError {
    KiroProviderCoreRequestError::new(message, "invalid_request")
}
