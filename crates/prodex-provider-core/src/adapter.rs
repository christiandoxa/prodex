use crate::{
    ProviderAdapterContract, ProviderCapabilityStatus, ProviderEndpoint, ProviderId,
    ProviderModelSpec, ProviderWireFormat, provider_model_catalog, provider_supported_endpoints,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StaticProviderAdapter {
    provider: ProviderId,
}

impl StaticProviderAdapter {
    pub const fn new(provider: ProviderId) -> Self {
        Self { provider }
    }
}

impl ProviderAdapterContract for StaticProviderAdapter {
    fn provider(&self) -> ProviderId {
        self.provider
    }

    fn client_request_format(&self) -> ProviderWireFormat {
        match self.provider {
            ProviderId::OpenAi | ProviderId::Local => ProviderWireFormat::OpenAiResponses,
            ProviderId::Anthropic | ProviderId::Copilot | ProviderId::DeepSeek => {
                ProviderWireFormat::OpenAiResponses
            }
            ProviderId::Gemini => ProviderWireFormat::OpenAiResponses,
            ProviderId::Kiro => ProviderWireFormat::OpenAiResponses,
        }
    }

    fn upstream_request_format(&self) -> ProviderWireFormat {
        match self.provider {
            ProviderId::OpenAi | ProviderId::Local => ProviderWireFormat::OpenAiResponses,
            ProviderId::Anthropic | ProviderId::Copilot | ProviderId::DeepSeek => {
                ProviderWireFormat::OpenAiChatCompletions
            }
            ProviderId::Gemini => ProviderWireFormat::GeminiGenerateContent,
            ProviderId::Kiro => ProviderWireFormat::Passthrough,
        }
    }

    fn response_format(&self) -> ProviderWireFormat {
        match self.provider {
            ProviderId::OpenAi | ProviderId::Local => ProviderWireFormat::OpenAiResponses,
            ProviderId::Anthropic | ProviderId::Copilot | ProviderId::DeepSeek => {
                ProviderWireFormat::OpenAiResponses
            }
            ProviderId::Gemini => ProviderWireFormat::OpenAiResponses,
            ProviderId::Kiro => ProviderWireFormat::OpenAiResponses,
        }
    }

    fn canonical_client_endpoint(&self) -> &'static str {
        "/v1/responses"
    }

    fn model_list_endpoint(&self) -> &'static str {
        "/v1/models"
    }

    fn supports_streaming(&self) -> bool {
        true
    }

    fn supports_model_fallback(&self) -> bool {
        !matches!(
            self.provider,
            ProviderId::OpenAi | ProviderId::Kiro | ProviderId::Local
        )
    }

    fn supported_endpoints(&self) -> &'static [ProviderEndpoint] {
        provider_supported_endpoints(self.provider)
    }

    fn model_catalog(&self) -> &'static [ProviderModelSpec] {
        provider_model_catalog(self.provider)
    }

    fn capability_status(&self, endpoint: ProviderEndpoint) -> ProviderCapabilityStatus {
        if !self.supported_endpoints().contains(&endpoint) {
            return ProviderCapabilityStatus::Unsupported;
        }
        match self.provider {
            ProviderId::OpenAi => ProviderCapabilityStatus::Native,
            ProviderId::Local => ProviderCapabilityStatus::Passthrough,
            ProviderId::Anthropic | ProviderId::Copilot | ProviderId::DeepSeek => match endpoint {
                ProviderEndpoint::Responses => ProviderCapabilityStatus::Translated,
                ProviderEndpoint::ResponsesCompact if self.provider == ProviderId::Copilot => {
                    ProviderCapabilityStatus::Passthrough
                }
                ProviderEndpoint::ChatCompletions | ProviderEndpoint::Messages => {
                    ProviderCapabilityStatus::Passthrough
                }
                ProviderEndpoint::Models => ProviderCapabilityStatus::Emulated,
                _ => ProviderCapabilityStatus::Unsupported,
            },
            ProviderId::Gemini => match endpoint {
                ProviderEndpoint::Responses => ProviderCapabilityStatus::Translated,
                ProviderEndpoint::ResponsesCompact => ProviderCapabilityStatus::Emulated,
                ProviderEndpoint::ChatCompletions
                | ProviderEndpoint::Messages
                | ProviderEndpoint::Embeddings => ProviderCapabilityStatus::Passthrough,
                ProviderEndpoint::Models => ProviderCapabilityStatus::Emulated,
                _ => ProviderCapabilityStatus::Unsupported,
            },
            ProviderId::Kiro => match endpoint {
                ProviderEndpoint::Responses
                | ProviderEndpoint::ChatCompletions
                | ProviderEndpoint::Messages => ProviderCapabilityStatus::Translated,
                ProviderEndpoint::ResponsesCompact | ProviderEndpoint::Models => {
                    ProviderCapabilityStatus::Emulated
                }
                _ => ProviderCapabilityStatus::Unsupported,
            },
        }
    }
}

pub fn provider_adapter(provider: ProviderId) -> StaticProviderAdapter {
    StaticProviderAdapter::new(provider)
}
