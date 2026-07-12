//! Transform input/result DTOs.

#[path = "transform/status.rs"]
mod status;

use crate::{ProviderEndpoint, ProviderId, ProviderWireFormat};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fmt};

pub use self::status::{ProviderTransformLoss, TransformOutcome, TransformStatus};

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderTransformInput {
    pub endpoint: ProviderEndpoint,
    pub model: Option<String>,
    pub headers: BTreeMap<String, String>,
    pub status: Option<u16>,
    pub body: Vec<u8>,
}

impl fmt::Debug for ProviderTransformInput {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProviderTransformInput")
            .field("endpoint", &self.endpoint)
            .field("model", &self.model)
            .field("header_count", &self.headers.len())
            .field("headers", &"<redacted>")
            .field("status", &self.status)
            .field("body_len", &self.body.len())
            .field("body", &"<redacted>")
            .finish()
    }
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

    pub fn status(&self) -> TransformStatus {
        TransformStatus::from(&self.loss)
    }

    pub fn outcome(&self) -> TransformOutcome<Vec<u8>> {
        TransformOutcome {
            status: self.status(),
            value: self.body.clone(),
        }
    }
}
