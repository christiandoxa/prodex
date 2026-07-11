//! Request-shape validation for the shared Responses-to-chat bridge.

#[path = "validation/input_content.rs"]
mod input_content;

use super::super::{ProviderId, Value};

use self::input_content::{
    responses_input_has_custom_tool_or_tool_search, responses_input_has_non_text_content,
};

pub(super) fn validate_responses_chat_compat_request(
    provider: ProviderId,
    obj: &serde_json::Map<String, Value>,
) -> Result<(), String> {
    if obj.contains_key("messages") {
        return Err(format!(
            "{} Responses chat-compat expects Responses input, not raw chat-completions messages",
            provider.label()
        ));
    }
    if obj.contains_key("response_format") {
        return Err(format!(
            "{} Responses chat-compat does not translate response_format controls",
            provider.label()
        ));
    }
    if obj.contains_key("reasoning") {
        return Err(format!(
            "{} Responses chat-compat does not map Responses reasoning controls",
            provider.label()
        ));
    }
    if obj.contains_key("previous_response_id") {
        return Err(format!(
            "{} Responses chat-compat does not map previous_response_id continuation state",
            provider.label()
        ));
    }
    if obj
        .get("text")
        .and_then(Value::as_object)
        .is_some_and(|text| text.contains_key("format"))
    {
        return Err(format!(
            "{} Responses chat-compat does not translate text.format controls",
            provider.label()
        ));
    }
    if obj.get("n").and_then(Value::as_u64).is_some_and(|n| n > 1) {
        return Err(format!(
            "{} Responses chat-compat returns only the first choice and does not support n>1",
            provider.label()
        ));
    }
    if obj.contains_key("metadata") {
        return Err(format!(
            "{} Responses chat-compat does not translate request metadata",
            provider.label()
        ));
    }
    if obj.contains_key("safety_identifier") {
        return Err(format!(
            "{} Responses chat-compat does not translate safety_identifier",
            provider.label()
        ));
    }
    if obj.contains_key("web_search_options") {
        return Err(format!(
            "{} Responses chat-compat does not translate web_search_options",
            provider.label()
        ));
    }
    if obj
        .get("tools")
        .and_then(Value::as_array)
        .is_some_and(|tools| {
            tools
                .iter()
                .any(|tool| tool.get("type").and_then(Value::as_str) != Some("function"))
        })
    {
        return Err(format!(
            "{} Responses chat-compat only forwards function tools",
            provider.label()
        ));
    }
    if let Some(tool_choice) = obj.get("tool_choice")
        && !responses_chat_compat_tool_choice_supported(tool_choice)
    {
        return Err(format!(
            "{} Responses chat-compat only forwards function tool_choice controls",
            provider.label()
        ));
    }
    if matches!(
        obj.get("parallel_tool_calls").and_then(Value::as_bool),
        Some(false)
    ) {
        return Err(format!(
            "{} Responses chat-compat does not prove a compatible parallel_tool_calls=false control",
            provider.label()
        ));
    }
    if obj.contains_key("logprobs") || obj.contains_key("top_logprobs") {
        return Err(format!(
            "{} Responses chat-compat does not translate logprobs controls",
            provider.label()
        ));
    }
    if obj.contains_key("stop_sequences") {
        return Err(format!(
            "{} Responses chat-compat does not translate stop_sequences",
            provider.label()
        ));
    }
    if let Some(input) = obj.get("input") {
        if responses_input_has_custom_tool_or_tool_search(input) {
            return Err(format!(
                "{} Responses chat-compat only translates message/function-call history items",
                provider.label()
            ));
        }
        if responses_input_has_non_text_content(input) {
            return Err(format!(
                "{} Responses chat-compat currently translates only text input content",
                provider.label()
            ));
        }
    }
    Ok(())
}

fn responses_chat_compat_tool_choice_supported(value: &Value) -> bool {
    match value {
        Value::String(choice) => matches!(choice.as_str(), "auto" | "none" | "required"),
        Value::Object(object) => object.get("type").and_then(Value::as_str) == Some("function"),
        _ => false,
    }
}
