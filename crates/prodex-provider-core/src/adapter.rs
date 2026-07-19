use crate::{
    ProviderBodyTransform, ProviderCapabilityStatus, ProviderEndpoint, ProviderId,
    ProviderImplementationDescriptor, ProviderModelSpec, ProviderTransformPhase,
    ProviderWireFormat, provider_implementation_registry,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StaticProviderAdapter {
    provider: ProviderId,
}

impl StaticProviderAdapter {
    pub const fn new(provider: ProviderId) -> Self {
        Self { provider }
    }

    fn descriptor(&self) -> &'static ProviderImplementationDescriptor {
        provider_implementation_registry()
            .get(self.provider)
            .expect("built-in provider implementation must be registered")
    }

    pub fn provider(&self) -> ProviderId {
        self.provider
    }

    pub fn client_request_format(&self) -> ProviderWireFormat {
        self.descriptor().client_request_format()
    }

    pub fn upstream_request_format(&self) -> ProviderWireFormat {
        self.descriptor().upstream_request_format()
    }

    pub fn response_format(&self) -> ProviderWireFormat {
        self.descriptor().response_format()
    }

    pub fn canonical_client_endpoint(&self) -> &'static str {
        "/v1/responses"
    }

    pub fn model_list_endpoint(&self) -> &'static str {
        "/v1/models"
    }

    pub fn supports_streaming(&self) -> bool {
        self.descriptor().supports_streaming()
    }

    pub fn supports_model_fallback(&self) -> bool {
        self.descriptor().supports_model_fallback()
    }

    pub fn supported_endpoints(&self) -> &'static [ProviderEndpoint] {
        self.descriptor().supported_endpoints()
    }

    pub fn model_catalog(&self) -> &'static [ProviderModelSpec] {
        self.descriptor().model_catalog()
    }

    pub fn capability_status(&self, endpoint: ProviderEndpoint) -> ProviderCapabilityStatus {
        self.descriptor().capability_status(endpoint)
    }

    pub fn transform_status(&self) -> ProviderCapabilityStatus {
        if self.client_request_format() == self.upstream_request_format()
            && self.upstream_request_format() == self.response_format()
        {
            ProviderCapabilityStatus::Passthrough
        } else {
            ProviderCapabilityStatus::Translated
        }
    }

    pub fn fallback_chain(&self, model: &str) -> Vec<String> {
        crate::provider_model_fallback_chain(self.provider(), model)
    }

    pub fn classify_error(
        &self,
        status: Option<u16>,
        code: Option<&str>,
        text: Option<&str>,
    ) -> crate::ProviderErrorClassification {
        crate::classify_provider_error(status, code, text)
    }

    pub fn estimate_input_tokens(&self, body: &[u8]) -> u64 {
        crate::estimate_request_input_tokens(body)
    }

    pub fn transform_request_body(&self, body: &[u8]) -> ProviderBodyTransform {
        ProviderBodyTransform {
            phase: ProviderTransformPhase::ClientRequestToUpstream,
            provider: self.provider(),
            from_format: self.client_request_format(),
            to_format: self.upstream_request_format(),
            body: body.to_vec(),
            lossy: false,
        }
    }

    pub fn transform_response_body(&self, body: &[u8]) -> ProviderBodyTransform {
        ProviderBodyTransform {
            phase: ProviderTransformPhase::UpstreamResponseToClient,
            provider: self.provider(),
            from_format: self.upstream_request_format(),
            to_format: self.response_format(),
            body: body.to_vec(),
            lossy: false,
        }
    }

    pub fn transform_stream_event(&self, event: &[u8]) -> ProviderBodyTransform {
        ProviderBodyTransform {
            phase: ProviderTransformPhase::UpstreamStreamEventToClient,
            provider: self.provider(),
            from_format: self.upstream_request_format(),
            to_format: self.response_format(),
            body: event.to_vec(),
            lossy: false,
        }
    }
}

pub fn provider_adapter(provider: ProviderId) -> StaticProviderAdapter {
    provider_implementation_registry()
        .get(provider)
        .expect("built-in provider implementation must be registered")
        .adapter()
}
