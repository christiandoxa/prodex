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
    kiro_validate_chat_completion_format_and_sampling(object)?;
    kiro_validate_chat_completion_penalties(object)?;
    object.remove("n");
    object.remove("user");
    kiro_provider_core_reject_token_limit_controls(object)?;
    if object.contains_key("input") {
        return kiro_validate_serialized_chat_body(&value);
    }
    kiro_rewrite_chat_messages(object)?;
    kiro_validate_serialized_chat_body(&value)
}

fn kiro_validate_chat_completion_format_and_sampling(
    object: &mut serde_json::Map<String, Value>,
) -> Result<(), KiroProviderCoreRequestError> {
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
    kiro_remove_default_chat_control(
        object,
        "stop",
        kiro_provider_core_has_requested_stop_sequences,
        "Kiro provider does not support chat stop sequences right now",
        "unsupported_stop",
    )?;
    kiro_remove_default_number_chat_control(
        object,
        "temperature",
        1.0,
        "Kiro provider does not support non-default chat temperature right now",
        "unsupported_temperature",
    )?;
    kiro_remove_default_number_chat_control(
        object,
        "top_p",
        1.0,
        "Kiro provider does not support non-default chat top_p right now",
        "unsupported_top_p",
    )
}

fn kiro_validate_chat_completion_penalties(
    object: &mut serde_json::Map<String, Value>,
) -> Result<(), KiroProviderCoreRequestError> {
    kiro_remove_default_number_chat_control(
        object,
        "presence_penalty",
        0.0,
        "Kiro provider does not support non-default chat presence_penalty right now",
        "unsupported_presence_penalty",
    )?;
    kiro_remove_default_number_chat_control(
        object,
        "frequency_penalty",
        0.0,
        "Kiro provider does not support non-default chat frequency_penalty right now",
        "unsupported_frequency_penalty",
    )?;
    if object
        .get("seed")
        .is_some_and(kiro_provider_core_has_requested_sampling_value)
    {
        return Err(KiroProviderCoreRequestError::new(
            "Kiro provider does not support chat seed right now",
            "unsupported_seed",
        ));
    }
    kiro_remove_default_chat_control(
        object,
        "parallel_tool_calls",
        kiro_provider_core_has_requested_parallel_tool_calls_control,
        "Kiro provider does not support chat parallel_tool_calls right now",
        "unsupported_parallel_tool_calls",
    )
}

fn kiro_remove_default_chat_control(
    object: &mut serde_json::Map<String, Value>,
    field: &str,
    is_requested: impl Fn(&Value) -> bool,
    message: &str,
    code: &str,
) -> Result<(), KiroProviderCoreRequestError> {
    if let Some(value) = object.get(field) {
        if is_requested(value) {
            return Err(KiroProviderCoreRequestError::new(message, code));
        }
        object.remove(field);
    }
    Ok(())
}

fn kiro_remove_default_number_chat_control(
    object: &mut serde_json::Map<String, Value>,
    field: &str,
    default: f64,
    message: &str,
    code: &str,
) -> Result<(), KiroProviderCoreRequestError> {
    kiro_remove_default_chat_control(
        object,
        field,
        |value| kiro_provider_core_has_requested_nondefault_number(value, default),
        message,
        code,
    )
}

fn kiro_rewrite_chat_messages(
    object: &mut serde_json::Map<String, Value>,
) -> Result<(), KiroProviderCoreRequestError> {
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
    Ok(())
}

fn kiro_validate_serialized_chat_body(
    value: &Value,
) -> Result<Vec<u8>, KiroProviderCoreRequestError> {
    let body = serde_json::to_vec(value).map_err(|_| {
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

    kiro_validate_response_input(object)?;
    kiro_validate_response_generation_controls(object, allow_token_limit)?;
    kiro_validate_response_format_and_tools(object)?;
    kiro_validate_response_reasoning(&value, object)?;
    deepseek_provider_core_reject_beta_completion_fields(&value, "Kiro")
        .map_err(kiro_invalid_request)?;
    deepseek_provider_core_reject_unsupported_request_fields(&value, "Kiro")
        .map_err(kiro_invalid_request)?;
    Ok(body.to_vec())
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

fn kiro_validate_response_generation_controls(
    object: &serde_json::Map<String, Value>,
    allow_token_limit: bool,
) -> Result<(), KiroProviderCoreRequestError> {
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
    kiro_validate_response_logprobs(object)
}

fn kiro_validate_response_logprobs(
    object: &serde_json::Map<String, Value>,
) -> Result<(), KiroProviderCoreRequestError> {
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
    Ok(())
}

fn kiro_validate_response_format_and_tools(
    object: &serde_json::Map<String, Value>,
) -> Result<(), KiroProviderCoreRequestError> {
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
    if let Some(tools) = object.get("tools")
        && !tools.is_null()
        && tools.as_array().is_none_or(|tools| !tools.is_empty())
    {
        return Err(KiroProviderCoreRequestError::new(
            "Kiro ACP owns its tool inventory and cannot execute external tools",
            "unsupported_tools",
        ));
    }
    if object.get("web_search_options").is_some() {
        return Err(KiroProviderCoreRequestError::new(
            "Kiro ACP owns web search and cannot honor web_search_options",
            "unsupported_web_search_options",
        ));
    }
    Ok(())
}

fn kiro_validate_response_reasoning(
    value: &Value,
    object: &serde_json::Map<String, Value>,
) -> Result<(), KiroProviderCoreRequestError> {
    deepseek_provider_core_validate_reasoning_shape(value, "Kiro").map_err(kiro_invalid_request)?;
    if let Some(effort) = object
        .get("reasoning")
        .and_then(|reasoning| reasoning.get("effort"))
        .or_else(|| object.get("reasoning_effort"))
        .and_then(Value::as_str)
        && !matches!(
            effort.trim(),
            "none" | "low" | "medium" | "high" | "xhigh" | "max"
        )
    {
        return Err(KiroProviderCoreRequestError::new(
            format!("Kiro ACP does not support reasoning effort `{effort}`"),
            "unsupported_reasoning_effort",
        ));
    }
    Ok(())
}

fn kiro_invalid_request(message: String) -> KiroProviderCoreRequestError {
    KiroProviderCoreRequestError::new(message, "invalid_request")
}
