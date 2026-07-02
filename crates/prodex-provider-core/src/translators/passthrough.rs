use crate::translator::{
    ProviderParamSupport, ProviderTransformInput, ProviderTransformResult, ProviderTranslator,
};
use crate::{ProviderEndpoint, ProviderId, ProviderWireFormat};

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

    fn supported_params(&self, _endpoint: ProviderEndpoint, _model: &str) -> ProviderParamSupport {
        ProviderParamSupport::full()
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
