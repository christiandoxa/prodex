mod request;
mod response;

pub use self::request::{
    copilot_provider_core_request_body_with_canonical_model,
    copilot_provider_core_request_body_without_encrypted_content,
    copilot_provider_core_request_has_agent_input, copilot_provider_core_request_has_vision_input,
};
pub use self::response::copilot_provider_core_response_id_from_value;
use crate::translator::{
    ProviderParamSupport, ProviderTransformInput, ProviderTransformResult, ProviderTranslator,
    ProviderUnsupportedReason,
};
use crate::translators::openai_chat_compat::{
    responses_chat_compat_supported_params, translate_chat_response_to_responses,
    translate_chat_stream_event_to_responses, translate_responses_request_to_chat,
};
use crate::{ProviderEndpoint, ProviderId, ProviderWireFormat, provider_supported_endpoints};

#[derive(Clone, Copy)]
pub struct CopilotTranslator;

impl ProviderTranslator for CopilotTranslator {
    fn provider(&self) -> ProviderId {
        ProviderId::Copilot
    }

    fn client_wire_format(&self) -> ProviderWireFormat {
        ProviderWireFormat::OpenAiResponses
    }

    fn upstream_wire_format(&self) -> ProviderWireFormat {
        // ponytail: reuse the shared chat-compat path for Responses; keep compact passthrough until Copilot needs different behavior.
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
            translate_responses_request_to_chat(ProviderId::Copilot, input, "codex")
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
            translate_chat_response_to_responses(ProviderId::Copilot, input)
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
            translate_chat_stream_event_to_responses(ProviderId::Copilot, input)
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

#[cfg(test)]
#[path = "copilot/tests.rs"]
mod tests;
