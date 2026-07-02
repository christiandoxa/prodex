use crate::translator::{
    ProviderParamSupport, ProviderTransformInput, ProviderTransformResult, ProviderTranslator,
    ProviderUnsupportedReason,
};
use crate::{ProviderEndpoint, ProviderId, ProviderWireFormat, provider_supported_endpoints};

#[derive(Clone, Copy)]
pub struct PassthroughTranslator {
    provider: ProviderId,
}

impl PassthroughTranslator {
    pub const fn new(provider: ProviderId) -> Self {
        Self { provider }
    }
}

impl ProviderTranslator for PassthroughTranslator {
    fn provider(&self) -> ProviderId {
        self.provider
    }

    fn client_wire_format(&self) -> ProviderWireFormat {
        ProviderWireFormat::OpenAiResponses
    }

    fn upstream_wire_format(&self) -> ProviderWireFormat {
        ProviderWireFormat::OpenAiResponses
    }

    fn supported_params(&self, endpoint: ProviderEndpoint, _model: &str) -> ProviderParamSupport {
        if provider_supported_endpoints(self.provider).contains(&endpoint) {
            ProviderParamSupport::full()
        } else {
            ProviderParamSupport {
                supported: false,
                unsupported: vec![ProviderUnsupportedReason {
                    field: endpoint.label().to_string(),
                    reason: format!(
                        "{} does not expose {}",
                        self.provider.label(),
                        endpoint.label()
                    ),
                }],
            }
        }
    }

    fn transform_request(&self, input: ProviderTransformInput) -> ProviderTransformResult {
        ProviderTransformResult::lossless(
            self.provider,
            input.endpoint,
            self.client_wire_format(),
            self.upstream_wire_format(),
            input.body,
        )
    }

    fn transform_response(&self, input: ProviderTransformInput) -> ProviderTransformResult {
        ProviderTransformResult::lossless(
            self.provider,
            input.endpoint,
            self.upstream_wire_format(),
            self.client_wire_format(),
            input.body,
        )
    }

    fn transform_stream_event(&self, input: ProviderTransformInput) -> ProviderTransformResult {
        ProviderTransformResult::lossless(
            self.provider,
            input.endpoint,
            self.upstream_wire_format(),
            self.client_wire_format(),
            input.body,
        )
    }
}
