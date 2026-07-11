use crate::{
    ProviderEndpoint, ProviderErrorClassification, ProviderId, ProviderTokenUsage,
    ProviderWireFormat, classify_provider_error, estimate_request_input_tokens,
    extract_usage_tokens,
};

#[path = "translator/conformance.rs"]
mod conformance;
#[path = "translator/params.rs"]
mod params;
#[path = "translator/transform.rs"]
mod transform;

pub use self::conformance::{
    ProviderConformanceCase, ProviderConformanceExpectedErrorClass,
    ProviderConformanceExpectedLoss, ProviderConformanceOperation,
};
pub use self::params::{ProviderParamSupport, ProviderUnsupportedReason};
pub use self::transform::{
    ProviderTransformInput, ProviderTransformLoss, ProviderTransformResult, TransformOutcome,
    TransformStatus,
};

pub trait ProviderTranslator: Send + Sync {
    fn provider(&self) -> ProviderId;
    fn client_wire_format(&self) -> ProviderWireFormat;
    fn upstream_wire_format(&self) -> ProviderWireFormat;

    fn supported_params(&self, endpoint: ProviderEndpoint, model: &str) -> ProviderParamSupport;
    fn transform_request(&self, input: ProviderTransformInput) -> ProviderTransformResult;
    fn transform_response(&self, input: ProviderTransformInput) -> ProviderTransformResult;
    fn transform_stream_event(&self, input: ProviderTransformInput) -> ProviderTransformResult;

    fn extract_usage(&self, body: &[u8]) -> ProviderTokenUsage {
        extract_usage_tokens(body)
    }

    fn estimate_input_tokens(&self, body: &[u8]) -> u64 {
        estimate_request_input_tokens(body)
    }

    fn classify_error(
        &self,
        status: Option<u16>,
        code: Option<&str>,
        text: Option<&str>,
    ) -> ProviderErrorClassification {
        classify_provider_error(status, code, text)
    }
}
