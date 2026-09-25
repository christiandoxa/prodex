use crate::translator::{ProviderTransformInput, ProviderTransformResult};
use crate::{ProviderEndpoint, ProviderId, ProviderWireFormat};
use serde_json::Value;

#[path = "openai_chat_compat_params.rs"]
mod openai_chat_compat_params;
#[path = "openai_chat_compat_request_mojo.rs"]
mod openai_chat_compat_request_mojo;
#[cfg(test)]
#[path = "openai_chat_compat_request_mojo_tests.rs"]
mod openai_chat_compat_request_mojo_tests;
#[path = "openai_chat_compat_response.rs"]
mod openai_chat_compat_response;
pub(crate) use self::openai_chat_compat_params::responses_chat_compat_supported_params;
pub(crate) use self::openai_chat_compat_response::{
    translate_chat_response_to_responses, translate_chat_stream_event_to_responses,
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
    let transformed = openai_chat_compat_request_mojo::transform(
        provider,
        &value,
        input.model.as_deref(),
        default_model,
    );
    match transformed {
        Ok(body) => ProviderTransformResult::lossless(
            provider,
            input.endpoint,
            ProviderWireFormat::OpenAiResponses,
            ProviderWireFormat::OpenAiChatCompletions,
            body,
        ),
        Err(reason) => ProviderTransformResult::rejected(
            provider,
            input.endpoint,
            ProviderWireFormat::OpenAiResponses,
            ProviderWireFormat::OpenAiChatCompletions,
            reason,
        ),
    }
}
