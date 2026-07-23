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
};
use crate::translators::PassthroughTranslator;
use crate::{ProviderEndpoint, ProviderId, ProviderWireFormat};

#[derive(Clone, Copy)]
pub struct CopilotTranslator;

impl CopilotTranslator {
    fn passthrough() -> PassthroughTranslator {
        PassthroughTranslator::new(ProviderId::Copilot)
    }
}

impl ProviderTranslator for CopilotTranslator {
    fn provider(&self) -> ProviderId {
        ProviderId::Copilot
    }

    fn client_wire_format(&self) -> ProviderWireFormat {
        Self::passthrough().client_wire_format()
    }

    fn upstream_wire_format(&self) -> ProviderWireFormat {
        Self::passthrough().upstream_wire_format()
    }

    fn supported_params(&self, endpoint: ProviderEndpoint, model: &str) -> ProviderParamSupport {
        Self::passthrough().supported_params(endpoint, model)
    }

    fn transform_request(&self, input: ProviderTransformInput) -> ProviderTransformResult {
        Self::passthrough().transform_request(input)
    }

    fn transform_response(&self, input: ProviderTransformInput) -> ProviderTransformResult {
        Self::passthrough().transform_response(input)
    }

    fn transform_stream_event(&self, input: ProviderTransformInput) -> ProviderTransformResult {
        Self::passthrough().transform_stream_event(input)
    }
}

#[cfg(test)]
#[path = "copilot/tests.rs"]
mod tests;
