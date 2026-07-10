use crate::translator::{ProviderTransformInput, ProviderTransformResult};
use crate::{ProviderEndpoint, ProviderId, ProviderWireFormat};
use serde_json::{Value, json};

#[path = "openai_chat_compat_params.rs"]
mod openai_chat_compat_params;
#[path = "openai_chat_compat_request.rs"]
mod openai_chat_compat_request;
#[path = "openai_chat_compat_response.rs"]
mod openai_chat_compat_response;
#[path = "openai_chat_compat_util.rs"]
mod openai_chat_compat_util;
pub(crate) use self::openai_chat_compat_params::responses_chat_compat_supported_params;
use self::openai_chat_compat_request::{
    responses_messages_to_chat_messages, validate_responses_chat_compat_request,
};
pub(crate) use self::openai_chat_compat_response::{
    translate_chat_response_to_responses, translate_chat_stream_event_to_responses,
};
use self::openai_chat_compat_util::{
    chat_usage_to_responses_usage, copy_first_if_present, copy_if_present,
    message_content_to_output_content, rtk_wrapped_tool_arguments, split_flat_namespace_tool_name,
    stringify_arguments, value_to_text,
};

pub fn translate_responses_request_to_chat(
    provider: ProviderId,
    input: ProviderTransformInput,
    default_model: &str,
) -> ProviderTransformResult {
    if input.endpoint != ProviderEndpoint::Responses {
        return ProviderTransformResult::unsupported(
            provider,
            input.endpoint,
            ProviderWireFormat::OpenAiResponses,
            ProviderWireFormat::OpenAiChatCompletions,
            format!(
                "{} translator only translates responses requests",
                provider.label()
            ),
        );
    }

    let value: Value = match serde_json::from_slice(&input.body) {
        Ok(value) => value,
        Err(error) => {
            return ProviderTransformResult::rejected(
                provider,
                input.endpoint,
                ProviderWireFormat::OpenAiResponses,
                ProviderWireFormat::OpenAiChatCompletions,
                format!("failed to parse Responses request JSON: {error}"),
            );
        }
    };
    let Some(obj) = value.as_object() else {
        return ProviderTransformResult::rejected(
            provider,
            input.endpoint,
            ProviderWireFormat::OpenAiResponses,
            ProviderWireFormat::OpenAiChatCompletions,
            "Responses request body must be a JSON object",
        );
    };
    if let Err(reason) = validate_responses_chat_compat_request(provider, obj) {
        return ProviderTransformResult::rejected(
            provider,
            input.endpoint,
            ProviderWireFormat::OpenAiResponses,
            ProviderWireFormat::OpenAiChatCompletions,
            reason,
        );
    }

    let messages = responses_messages_to_chat_messages(obj);
    if messages.is_empty() {
        return ProviderTransformResult::rejected(
            provider,
            input.endpoint,
            ProviderWireFormat::OpenAiResponses,
            ProviderWireFormat::OpenAiChatCompletions,
            "Responses request must include a textual input or messages array",
        );
    }

    let mut request = serde_json::Map::new();
    request.insert(
        "model".to_string(),
        Value::String(
            obj.get("model")
                .and_then(Value::as_str)
                .or(input.model.as_deref())
                .unwrap_or(default_model)
                .to_string(),
        ),
    );
    request.insert("messages".to_string(), Value::Array(messages));
    request.insert(
        "stream".to_string(),
        Value::Bool(obj.get("stream").and_then(Value::as_bool).unwrap_or(false)),
    );

    copy_if_present(
        obj,
        &mut request,
        &[
            "temperature",
            "top_p",
            "presence_penalty",
            "frequency_penalty",
            "seed",
        ],
    );
    copy_first_if_present(
        obj,
        &mut request,
        "max_tokens",
        &["max_completion_tokens", "max_output_tokens", "max_tokens"],
    );
    copy_if_present(
        obj,
        &mut request,
        &[
            "tools",
            "tool_choice",
            "stop",
            "parallel_tool_calls",
            "user",
        ],
    );

    ProviderTransformResult::lossless(
        provider,
        input.endpoint,
        ProviderWireFormat::OpenAiResponses,
        ProviderWireFormat::OpenAiChatCompletions,
        serde_json::to_vec(&Value::Object(request)).expect("chat compatibility request serializes"),
    )
}
