//! Mojo-backed Kiro request capability validation.

use super::{
    KiroProviderCoreRequestError,
    controls::{
        kiro_provider_core_has_requested_nondefault_number,
        kiro_provider_core_has_requested_parallel_tool_calls_control,
        kiro_provider_core_has_requested_sampling_value,
        kiro_provider_core_has_requested_stop_sequences,
        kiro_provider_core_supported_chat_response_format,
    },
};
use prodex_mojo_core::rich::{
    KiroRequestValidationInput, KiroRequestValidationMode, KiroRequestValidationPlan,
};
use serde_json::{Map, Value};

fn token_limit(object: &Map<String, Value>) -> Option<(u8, bool)> {
    ["max_output_tokens", "max_tokens", "max_completion_tokens"]
        .into_iter()
        .enumerate()
        .find_map(|(index, field)| {
            let value = object.get(field).filter(|value| !value.is_null())?;
            Some((
                u8::try_from(index).ok()?,
                value.as_u64().is_some_and(|count| count > 0),
            ))
        })
}

pub(super) fn chat_input(object: &Map<String, Value>) -> KiroRequestValidationInput {
    let token_limit = token_limit(object);
    let mut flags = 0;
    if !object
        .get("response_format")
        .is_none_or(kiro_provider_core_supported_chat_response_format)
    {
        flags |= KiroRequestValidationInput::FLAG_CHAT_RESPONSE_FORMAT;
    }
    if !object
        .get("n")
        .is_none_or(|value| value.is_null() || value.as_u64() == Some(1))
    {
        flags |= KiroRequestValidationInput::FLAG_CHAT_CHOICE_COUNT;
    }
    if object
        .get("stop")
        .is_some_and(kiro_provider_core_has_requested_stop_sequences)
    {
        flags |= KiroRequestValidationInput::FLAG_CHAT_STOP;
    }
    if object
        .get("temperature")
        .is_some_and(|value| kiro_provider_core_has_requested_nondefault_number(value, 1.0))
    {
        flags |= KiroRequestValidationInput::FLAG_CHAT_TEMPERATURE;
    }
    if object
        .get("top_p")
        .is_some_and(|value| kiro_provider_core_has_requested_nondefault_number(value, 1.0))
    {
        flags |= KiroRequestValidationInput::FLAG_CHAT_TOP_P;
    }
    if object
        .get("presence_penalty")
        .is_some_and(|value| kiro_provider_core_has_requested_nondefault_number(value, 0.0))
    {
        flags |= KiroRequestValidationInput::FLAG_CHAT_PRESENCE_PENALTY;
    }
    if object
        .get("frequency_penalty")
        .is_some_and(|value| kiro_provider_core_has_requested_nondefault_number(value, 0.0))
    {
        flags |= KiroRequestValidationInput::FLAG_CHAT_FREQUENCY_PENALTY;
    }
    if object
        .get("seed")
        .is_some_and(kiro_provider_core_has_requested_sampling_value)
    {
        flags |= KiroRequestValidationInput::FLAG_CHAT_SEED;
    }
    if object
        .get("parallel_tool_calls")
        .is_some_and(kiro_provider_core_has_requested_parallel_tool_calls_control)
    {
        flags |= KiroRequestValidationInput::FLAG_CHAT_PARALLEL_TOOL_CALLS;
    }
    if let Some((_, positive)) = token_limit {
        flags |= KiroRequestValidationInput::FLAG_TOKEN_LIMIT;
        if !positive {
            flags |= KiroRequestValidationInput::FLAG_TOKEN_LIMIT_INVALID;
        }
    }
    KiroRequestValidationInput {
        mode: KiroRequestValidationMode::ChatCompletions,
        flags,
        detail: token_limit.map(|(field, _)| i64::from(field)).unwrap_or(-1),
        allow_token_limit: false,
    }
}

pub(super) fn response_input(
    object: &Map<String, Value>,
    allow_token_limit: bool,
) -> KiroRequestValidationInput {
    let token_limit = token_limit(object);
    let response_format_supported = [
        object.get("response_format"),
        object.get("text").and_then(|text| text.get("format")),
    ]
    .into_iter()
    .flatten()
    .all(kiro_provider_core_supported_chat_response_format);
    let logprobs_kind = match object.get("logprobs") {
        None | Some(Value::Null) | Some(Value::Bool(false)) => 0,
        Some(Value::Bool(true)) => 1,
        Some(_) => 2,
    };
    let reasoning_effort_supported = object
        .get("reasoning")
        .and_then(|reasoning| reasoning.get("effort"))
        .or_else(|| object.get("reasoning_effort"))
        .and_then(Value::as_str)
        .is_none_or(|effort| {
            matches!(
                effort.trim(),
                "none" | "low" | "medium" | "high" | "xhigh" | "max"
            )
        });
    let generation_detail = ["temperature", "top_p", "seed"]
        .into_iter()
        .position(|field| object.get(field).is_some_and(|value| !value.is_null()))
        .map(|index| i64::try_from(index).unwrap_or(-1))
        .unwrap_or(-1);
    let flags = response_flags(
        object,
        token_limit,
        response_format_supported,
        logprobs_kind,
        reasoning_effort_supported,
        generation_detail,
    );
    KiroRequestValidationInput {
        mode: KiroRequestValidationMode::Responses,
        flags,
        detail: if generation_detail >= 0 {
            generation_detail
        } else {
            token_limit.map(|(field, _)| i64::from(field)).unwrap_or(-1)
        },
        allow_token_limit,
    }
}

fn response_flags(
    object: &Map<String, Value>,
    token_limit: Option<(u8, bool)>,
    response_format_supported: bool,
    logprobs_kind: u8,
    reasoning_effort_supported: bool,
    generation_detail: i64,
) -> u64 {
    let mut flags = 0;
    if generation_detail >= 0 {
        flags |= KiroRequestValidationInput::FLAG_GENERATION_CONTROL;
    }
    if let Some((_, positive)) = token_limit {
        flags |= KiroRequestValidationInput::FLAG_TOKEN_LIMIT;
        if !positive {
            flags |= KiroRequestValidationInput::FLAG_TOKEN_LIMIT_INVALID;
        }
    }
    if ["stop", "stop_sequences", "stopSequences"]
        .into_iter()
        .any(|field| {
            object
                .get(field)
                .is_some_and(kiro_provider_core_has_requested_stop_sequences)
        })
    {
        flags |= KiroRequestValidationInput::FLAG_RESPONSE_STOP;
    }
    match logprobs_kind {
        1 => flags |= KiroRequestValidationInput::FLAG_LOGPROBS_UNSUPPORTED,
        2 => flags |= KiroRequestValidationInput::FLAG_LOGPROBS_INVALID,
        _ => {}
    }
    if object
        .get("top_logprobs")
        .is_some_and(|value| !value.is_null())
    {
        flags |= KiroRequestValidationInput::FLAG_TOP_LOGPROBS;
    }
    if !response_format_supported {
        flags |= KiroRequestValidationInput::FLAG_RESPONSE_FORMAT;
    }
    if !object
        .get("tool_choice")
        .is_none_or(|value| value.is_null() || value.as_str() == Some("auto"))
    {
        flags |= KiroRequestValidationInput::FLAG_TOOL_CHOICE;
    }
    if object.get("tools").is_some_and(|tools| {
        !tools.is_null() && tools.as_array().is_none_or(|items| !items.is_empty())
    }) {
        flags |= KiroRequestValidationInput::FLAG_TOOLS;
    }
    if object.contains_key("web_search_options") {
        flags |= KiroRequestValidationInput::FLAG_WEB_SEARCH;
    }
    if !reasoning_effort_supported {
        flags |= KiroRequestValidationInput::FLAG_REASONING_EFFORT;
    }
    flags
}

pub(super) fn error(
    plan: KiroRequestValidationPlan,
    object: &Map<String, Value>,
) -> Result<(), KiroProviderCoreRequestError> {
    let error = match plan.reason {
        KiroRequestValidationPlan::REASON_NONE => return Ok(()),
        KiroRequestValidationPlan::REASON_CHAT_RESPONSE_FORMAT => (
            "Kiro provider only supports chat response_format type 'text' right now",
            "unsupported_response_format",
        ),
        KiroRequestValidationPlan::REASON_CHAT_CHOICE_COUNT => (
            "Kiro provider only supports chat completion parameter n=1 right now",
            "unsupported_choice_count",
        ),
        KiroRequestValidationPlan::REASON_CHAT_STOP => (
            "Kiro provider does not support chat stop sequences right now",
            "unsupported_stop",
        ),
        KiroRequestValidationPlan::REASON_CHAT_TEMPERATURE => (
            "Kiro provider does not support non-default chat temperature right now",
            "unsupported_temperature",
        ),
        KiroRequestValidationPlan::REASON_CHAT_TOP_P => (
            "Kiro provider does not support non-default chat top_p right now",
            "unsupported_top_p",
        ),
        KiroRequestValidationPlan::REASON_CHAT_PRESENCE_PENALTY => (
            "Kiro provider does not support non-default chat presence_penalty right now",
            "unsupported_presence_penalty",
        ),
        KiroRequestValidationPlan::REASON_CHAT_FREQUENCY_PENALTY => (
            "Kiro provider does not support non-default chat frequency_penalty right now",
            "unsupported_frequency_penalty",
        ),
        KiroRequestValidationPlan::REASON_CHAT_SEED => (
            "Kiro provider does not support chat seed right now",
            "unsupported_seed",
        ),
        KiroRequestValidationPlan::REASON_CHAT_PARALLEL_TOOL_CALLS => (
            "Kiro provider does not support chat parallel_tool_calls right now",
            "unsupported_parallel_tool_calls",
        ),
        KiroRequestValidationPlan::REASON_TOKEN_LIMIT => {
            let fields = ["max_output_tokens", "max_tokens", "max_completion_tokens"];
            let Some(field) = plan
                .detail
                .try_into()
                .ok()
                .and_then(|index: usize| fields.get(index))
            else {
                return Err(KiroProviderCoreRequestError::new(
                    "Kiro request capability validation returned an invalid token field",
                    "invalid_request",
                ));
            };
            return Err(KiroProviderCoreRequestError::new(
                if plan.detail_is_invalid {
                    format!("Kiro {field} must be a positive integer")
                } else {
                    format!("Kiro ACP does not expose the {field} control")
                },
                "unsupported_token_limit",
            ));
        }
        KiroRequestValidationPlan::REASON_GENERATION_CONTROL => {
            let field = ["temperature", "top_p", "seed"]
                .get(usize::try_from(plan.detail).unwrap_or_default())
                .copied()
                .unwrap_or("temperature");
            return Err(KiroProviderCoreRequestError::new(
                format!("Kiro ACP does not expose the {field} control"),
                "unsupported_generation_control",
            ));
        }
        KiroRequestValidationPlan::REASON_RESPONSE_STOP => (
            "Kiro ACP does not expose stop-sequence controls",
            "unsupported_stop",
        ),
        KiroRequestValidationPlan::REASON_LOGPROBS => {
            if plan.detail_is_invalid {
                return Err(KiroProviderCoreRequestError::new(
                    "Kiro logprobs must be a boolean",
                    "invalid_logprobs",
                ));
            }
            (
                "Kiro ACP does not expose log probabilities",
                "unsupported_logprobs",
            )
        }
        KiroRequestValidationPlan::REASON_TOP_LOGPROBS => (
            "Kiro ACP does not expose top_logprobs",
            "unsupported_logprobs",
        ),
        KiroRequestValidationPlan::REASON_RESPONSE_FORMAT => (
            "Kiro ACP supports only text response format",
            "unsupported_response_format",
        ),
        KiroRequestValidationPlan::REASON_TOOL_CHOICE => (
            "Kiro ACP owns tool selection and cannot honor tool_choice",
            "unsupported_tool_choice",
        ),
        KiroRequestValidationPlan::REASON_TOOLS => (
            "Kiro ACP owns its tool inventory and cannot execute external tools",
            "unsupported_tools",
        ),
        KiroRequestValidationPlan::REASON_WEB_SEARCH => (
            "Kiro ACP owns web search and cannot honor web_search_options",
            "unsupported_web_search_options",
        ),
        KiroRequestValidationPlan::REASON_REASONING_EFFORT => {
            let effort = object
                .get("reasoning")
                .and_then(|reasoning| reasoning.get("effort"))
                .or_else(|| object.get("reasoning_effort"))
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            return Err(KiroProviderCoreRequestError::new(
                format!("Kiro ACP does not support reasoning effort `{effort}`"),
                "unsupported_reasoning_effort",
            ));
        }
        _ => {
            return Err(KiroProviderCoreRequestError::new(
                "Kiro request capability validation returned an unknown reason",
                "invalid_request",
            ));
        }
    };
    Err(KiroProviderCoreRequestError::new(error.0, error.1))
}

pub(super) fn remove_chat_defaults(object: &mut Map<String, Value>) {
    if object
        .get("stop")
        .is_some_and(|value| !kiro_provider_core_has_requested_stop_sequences(value))
    {
        object.remove("stop");
    }
    for (field, default) in [("temperature", 1.0), ("top_p", 1.0)] {
        if object.get(field).is_some_and(|value| {
            !kiro_provider_core_has_requested_nondefault_number(value, default)
        }) {
            object.remove(field);
        }
    }
    for (field, default) in [("presence_penalty", 0.0), ("frequency_penalty", 0.0)] {
        if object.get(field).is_some_and(|value| {
            !kiro_provider_core_has_requested_nondefault_number(value, default)
        }) {
            object.remove(field);
        }
    }
    if object
        .get("parallel_tool_calls")
        .is_some_and(|value| !kiro_provider_core_has_requested_parallel_tool_calls_control(value))
    {
        object.remove("parallel_tool_calls");
    }
}
