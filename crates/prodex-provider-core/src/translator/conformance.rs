//! Provider conformance fixture DTOs.

use crate::{ProviderEndpoint, ProviderId, ProviderTokenUsage};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use super::transform::TransformStatus;

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

impl ProviderConformanceExpectedLoss {
    pub fn matches_status(&self, status: &TransformStatus) -> bool {
        matches!(
            (self, status),
            (Self::Lossless, TransformStatus::Lossless)
                | (Self::Degraded, TransformStatus::Degraded { .. })
                | (Self::Rejected, TransformStatus::Rejected { .. })
                | (Self::Unsupported, TransformStatus::Unsupported { .. })
        )
    }

    pub fn requires_reason(&self) -> bool {
        !matches!(self, Self::Lossless)
    }

    pub const fn label(&self) -> &'static str {
        match self {
            Self::Lossless => "lossless",
            Self::Degraded => "degraded",
            Self::Rejected => "rejected",
            Self::Unsupported => "unsupported",
        }
    }
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
