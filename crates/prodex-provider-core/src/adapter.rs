use crate::{
    ProviderAdapterContract, ProviderCapabilityStatus, ProviderEndpoint, ProviderId,
    ProviderImplementationDescriptor, ProviderModelSpec, ProviderWireFormat,
    provider_implementation_registry,
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
}

impl ProviderAdapterContract for StaticProviderAdapter {
    fn provider(&self) -> ProviderId {
        self.provider
    }

    fn client_request_format(&self) -> ProviderWireFormat {
        self.descriptor().client_request_format()
    }

    fn upstream_request_format(&self) -> ProviderWireFormat {
        self.descriptor().upstream_request_format()
    }

    fn response_format(&self) -> ProviderWireFormat {
        self.descriptor().response_format()
    }

    fn canonical_client_endpoint(&self) -> &'static str {
        "/v1/responses"
    }

    fn model_list_endpoint(&self) -> &'static str {
        "/v1/models"
    }

    fn supports_streaming(&self) -> bool {
        self.descriptor().supports_streaming()
    }

    fn supports_model_fallback(&self) -> bool {
        self.descriptor().supports_model_fallback()
    }

    fn supported_endpoints(&self) -> &'static [ProviderEndpoint] {
        self.descriptor().supported_endpoints()
    }

    fn model_catalog(&self) -> &'static [ProviderModelSpec] {
        self.descriptor().model_catalog()
    }

    fn capability_status(&self, endpoint: ProviderEndpoint) -> ProviderCapabilityStatus {
        self.descriptor().capability_status(endpoint)
    }
}

pub fn provider_adapter(provider: ProviderId) -> StaticProviderAdapter {
    provider_implementation_registry()
        .get(provider)
        .expect("built-in provider implementation must be registered")
        .adapter()
}
