//! Canonical provider transform status labels.

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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TransformStatus {
    Lossless,
    Degraded { reason: String },
    Rejected { reason: String },
    Unsupported { reason: String },
}

impl From<&ProviderTransformLoss> for TransformStatus {
    fn from(loss: &ProviderTransformLoss) -> Self {
        match loss {
            ProviderTransformLoss::Lossless => Self::Lossless,
            ProviderTransformLoss::DegradedButSafe { reason, .. } => Self::Degraded {
                reason: reason.clone(),
            },
            ProviderTransformLoss::Rejected { reason } => Self::Rejected {
                reason: reason.clone(),
            },
            ProviderTransformLoss::UnsupportedUpstream { reason } => Self::Unsupported {
                reason: reason.clone(),
            },
        }
    }
}

impl TransformStatus {
    pub fn reason(&self) -> Option<&str> {
        match self {
            Self::Lossless => None,
            Self::Degraded { reason }
            | Self::Rejected { reason }
            | Self::Unsupported { reason } => Some(reason),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransformOutcome<T> {
    pub status: TransformStatus,
    pub value: Option<T>,
}

impl ProviderTransformLoss {
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Rejected { .. } | Self::UnsupportedUpstream { .. }
        )
    }
}
