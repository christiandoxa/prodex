//! Provider adapter contract and body-transform metadata.

use serde::Serialize;

use super::{
    ProviderCapabilityStatus, ProviderEndpoint, ProviderId, ProviderModelSpec, ProviderWireFormat,
};

pub trait ProviderAdapterContract {
    fn provider(&self) -> ProviderId;
    fn client_request_format(&self) -> ProviderWireFormat;
    fn upstream_request_format(&self) -> ProviderWireFormat;
    fn response_format(&self) -> ProviderWireFormat;
    fn canonical_client_endpoint(&self) -> &'static str;
    fn model_list_endpoint(&self) -> &'static str;
    fn supports_streaming(&self) -> bool;
    fn supports_model_fallback(&self) -> bool;
    fn supported_endpoints(&self) -> &'static [ProviderEndpoint];
    fn model_catalog(&self) -> &'static [ProviderModelSpec];
    fn capability_status(&self, endpoint: ProviderEndpoint) -> ProviderCapabilityStatus;

    fn transform_status(&self) -> ProviderCapabilityStatus {
        if self.client_request_format() == self.upstream_request_format()
            && self.upstream_request_format() == self.response_format()
        {
            ProviderCapabilityStatus::Passthrough
        } else {
            ProviderCapabilityStatus::Translated
        }
    }

    fn fallback_chain(&self, model: &str) -> Vec<String> {
        crate::provider_model_fallback_chain(self.provider(), model)
    }

    fn classify_error(
        &self,
        status: Option<u16>,
        code: Option<&str>,
        text: Option<&str>,
    ) -> crate::ProviderErrorClassification {
        crate::classify_provider_error(status, code, text)
    }

    fn estimate_input_tokens(&self, body: &[u8]) -> u64 {
        crate::estimate_request_input_tokens(body)
    }

    fn transform_request_body(&self, body: &[u8]) -> ProviderBodyTransform {
        ProviderBodyTransform {
            phase: ProviderTransformPhase::ClientRequestToUpstream,
            provider: self.provider(),
            from_format: self.client_request_format(),
            to_format: self.upstream_request_format(),
            body: body.to_vec(),
            lossy: false,
        }
    }

    fn transform_response_body(&self, body: &[u8]) -> ProviderBodyTransform {
        ProviderBodyTransform {
            phase: ProviderTransformPhase::UpstreamResponseToClient,
            provider: self.provider(),
            from_format: self.upstream_request_format(),
            to_format: self.response_format(),
            body: body.to_vec(),
            lossy: false,
        }
    }

    fn transform_stream_event(&self, event: &[u8]) -> ProviderBodyTransform {
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderTransformPhase {
    ClientRequestToUpstream,
    UpstreamResponseToClient,
    UpstreamStreamEventToClient,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderBodyTransform {
    pub phase: ProviderTransformPhase,
    pub provider: ProviderId,
    pub from_format: ProviderWireFormat,
    pub to_format: ProviderWireFormat,
    pub body: Vec<u8>,
    pub lossy: bool,
}
