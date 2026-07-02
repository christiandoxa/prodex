use crate::{
    ProviderEndpoint, ProviderErrorClassification, ProviderId, ProviderTokenUsage,
    ProviderWireFormat, classify_provider_error, estimate_request_input_tokens,
    extract_usage_tokens,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderTransformLoss {
    Lossless,
    DegradedButSafe {
        reason: String,
        details: BTreeMap<String, serde_json::Value>,
    },
    Rejected {
        reason: String,
    },
    UnsupportedUpstream {
        reason: String,
    },
}

impl ProviderTransformLoss {
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Rejected { .. } | Self::UnsupportedUpstream { .. }
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderTransformInput {
    pub endpoint: ProviderEndpoint,
    pub model: Option<String>,
    pub headers: BTreeMap<String, String>,
    pub status: Option<u16>,
    pub body: Vec<u8>,
}

impl ProviderTransformInput {
    pub fn new(endpoint: ProviderEndpoint, body: impl Into<Vec<u8>>) -> Self {
        Self {
            endpoint,
            model: None,
            headers: BTreeMap::new(),
            status: None,
            body: body.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderTransformResult {
    pub provider: ProviderId,
    pub endpoint: ProviderEndpoint,
    pub from_format: ProviderWireFormat,
    pub to_format: ProviderWireFormat,
    pub body: Option<Vec<u8>>,
    pub headers: BTreeMap<String, String>,
    pub metadata: BTreeMap<String, serde_json::Value>,
    pub loss: ProviderTransformLoss,
}

impl ProviderTransformResult {
    pub fn lossless(
        provider: ProviderId,
        endpoint: ProviderEndpoint,
        from_format: ProviderWireFormat,
        to_format: ProviderWireFormat,
        body: Vec<u8>,
    ) -> Self {
        Self {
            provider,
            endpoint,
            from_format,
            to_format,
            body: Some(body),
            headers: BTreeMap::new(),
            metadata: BTreeMap::new(),
            loss: ProviderTransformLoss::Lossless,
        }
    }

    pub fn degraded(
        provider: ProviderId,
        endpoint: ProviderEndpoint,
        from_format: ProviderWireFormat,
        to_format: ProviderWireFormat,
        body: Vec<u8>,
        reason: impl Into<String>,
        details: BTreeMap<String, serde_json::Value>,
    ) -> Self {
        Self {
            provider,
            endpoint,
            from_format,
            to_format,
            body: Some(body),
            headers: BTreeMap::new(),
            metadata: BTreeMap::new(),
            loss: ProviderTransformLoss::DegradedButSafe {
                reason: reason.into(),
                details,
            },
        }
    }

    pub fn rejected(
        provider: ProviderId,
        endpoint: ProviderEndpoint,
        from_format: ProviderWireFormat,
        to_format: ProviderWireFormat,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            provider,
            endpoint,
            from_format,
            to_format,
            body: None,
            headers: BTreeMap::new(),
            metadata: BTreeMap::new(),
            loss: ProviderTransformLoss::Rejected {
                reason: reason.into(),
            },
        }
    }

    pub fn unsupported(
        provider: ProviderId,
        endpoint: ProviderEndpoint,
        from_format: ProviderWireFormat,
        to_format: ProviderWireFormat,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            provider,
            endpoint,
            from_format,
            to_format,
            body: None,
            headers: BTreeMap::new(),
            metadata: BTreeMap::new(),
            loss: ProviderTransformLoss::UnsupportedUpstream {
                reason: reason.into(),
            },
        }
    }

    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderParamSupport {
    pub supported: bool,
    pub unsupported: Vec<ProviderUnsupportedReason>,
}

impl ProviderParamSupport {
    pub fn full() -> Self {
        Self {
            supported: true,
            unsupported: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderUnsupportedReason {
    pub field: String,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderConformanceCase {
    pub name: String,
    pub provider: ProviderId,
    pub endpoint: ProviderEndpoint,
    pub operation: ProviderConformanceOperation,
    pub model: Option<String>,
    pub input_headers: BTreeMap<String, String>,
    pub input_body: serde_json::Value,
    pub expected_body: Option<serde_json::Value>,
    pub expected_loss: ProviderConformanceExpectedLoss,
    pub expected_usage: Option<ProviderTokenUsage>,
    pub error_status: Option<u16>,
    pub error_code: Option<String>,
    pub error_text: Option<String>,
    pub expected_error_class: Option<ProviderConformanceExpectedErrorClass>,
    pub expected_error_cooldown_ms: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderConformanceOperation {
    Request,
    Response,
    StreamEvent,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderConformanceExpectedLoss {
    Lossless,
    Degraded,
    Rejected,
    Unsupported,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderConformanceExpectedErrorClass {
    Auth,
    Quota,
    RateLimit,
    Transient,
    NotFound,
    Other,
}

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
