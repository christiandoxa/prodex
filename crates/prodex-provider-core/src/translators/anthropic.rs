use crate::translator::{
    ProviderParamSupport, ProviderTransformInput, ProviderTransformResult, ProviderTranslator,
    ProviderUnsupportedReason,
};
use crate::translators::openai_chat_compat::{
    responses_chat_compat_supported_params, translate_chat_response_to_responses,
    translate_chat_stream_event_to_responses, translate_responses_request_to_chat,
};
use crate::{ProviderEndpoint, ProviderId, ProviderWireFormat, provider_supported_endpoints};

#[path = "anthropic/messages.rs"]
mod messages;

#[derive(Clone, Copy)]
pub struct AnthropicTranslator;

#[derive(Clone, Copy)]
pub struct AnthropicMessagesTranslator;

pub fn translate_openai_chat_request_to_anthropic_messages(
    input: ProviderTransformInput,
) -> ProviderTransformResult {
    messages::translate_chat_request_to_anthropic(input)
}

impl ProviderTranslator for AnthropicTranslator {
    fn provider(&self) -> ProviderId {
        ProviderId::Anthropic
    }

    fn client_wire_format(&self) -> ProviderWireFormat {
        ProviderWireFormat::OpenAiResponses
    }

    fn upstream_wire_format(&self) -> ProviderWireFormat {
        // ponytail: reuse the shared chat-compat path for Responses; add provider-specific endpoints only when they diverge.
        ProviderWireFormat::OpenAiChatCompletions
    }

    fn supported_params(&self, endpoint: ProviderEndpoint, _model: &str) -> ProviderParamSupport {
        if endpoint == ProviderEndpoint::Responses {
            return responses_chat_compat_supported_params(self.provider());
        }
        if provider_supported_endpoints(self.provider()).contains(&endpoint) {
            ProviderParamSupport::full()
        } else {
            ProviderParamSupport {
                supported: false,
                unsupported: vec![ProviderUnsupportedReason {
                    field: endpoint.label().to_string(),
                    reason: format!(
                        "{} does not expose {}",
                        self.provider().label(),
                        endpoint.label()
                    ),
                }],
            }
        }
    }

    fn transform_request(&self, input: ProviderTransformInput) -> ProviderTransformResult {
        if input.endpoint == ProviderEndpoint::Responses {
            translate_responses_request_to_chat(ProviderId::Anthropic, input, "auto")
        } else {
            ProviderTransformResult::lossless(
                self.provider(),
                input.endpoint,
                self.client_wire_format(),
                self.upstream_wire_format(),
                input.body,
            )
        }
    }

    fn transform_response(&self, input: ProviderTransformInput) -> ProviderTransformResult {
        if input.endpoint == ProviderEndpoint::Responses {
            translate_chat_response_to_responses(ProviderId::Anthropic, input)
        } else {
            ProviderTransformResult::lossless(
                self.provider(),
                input.endpoint,
                self.upstream_wire_format(),
                self.client_wire_format(),
                input.body,
            )
        }
    }

    fn transform_stream_event(&self, input: ProviderTransformInput) -> ProviderTransformResult {
        if input.endpoint == ProviderEndpoint::Responses {
            translate_chat_stream_event_to_responses(ProviderId::Anthropic, input)
        } else {
            ProviderTransformResult::lossless(
                self.provider(),
                input.endpoint,
                self.upstream_wire_format(),
                self.client_wire_format(),
                input.body,
            )
        }
    }
}

impl ProviderTranslator for AnthropicMessagesTranslator {
    fn provider(&self) -> ProviderId {
        ProviderId::Anthropic
    }

    fn client_wire_format(&self) -> ProviderWireFormat {
        ProviderWireFormat::OpenAiResponses
    }

    fn upstream_wire_format(&self) -> ProviderWireFormat {
        ProviderWireFormat::AnthropicMessages
    }

    fn supported_params(&self, endpoint: ProviderEndpoint, _model: &str) -> ProviderParamSupport {
        if endpoint == ProviderEndpoint::Responses {
            return responses_chat_compat_supported_params(self.provider());
        }
        ProviderParamSupport {
            supported: false,
            unsupported: vec![ProviderUnsupportedReason {
                field: endpoint.label().to_string(),
                reason: format!(
                    "{} native Messages translator does not expose {}",
                    self.provider().label(),
                    endpoint.label()
                ),
            }],
        }
    }

    fn transform_request(&self, input: ProviderTransformInput) -> ProviderTransformResult {
        messages::translate_responses_request_to_anthropic(input)
    }

    fn transform_response(&self, input: ProviderTransformInput) -> ProviderTransformResult {
        messages::translate_anthropic_response_to_responses(input)
    }

    fn transform_stream_event(&self, input: ProviderTransformInput) -> ProviderTransformResult {
        messages::translate_anthropic_stream_event_to_responses(input)
    }
}
