use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::{RUNTIME_ROUTE_DECISION_TRACE_MAX_IDENTIFIER_BYTES, RuntimeRouteDecisionStage};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeRouteDecisionReasonKind {
    AuthFailureBackoff,
    SelectionBackoff,
    RouteCircuitOpen,
    RouteCircuitHalfOpenProbeWait,
    ProfileHealth,
    ProfilePerformance,
    QuotaProbeUnavailable,
    StalePersistedQuota,
    QuotaHealthy,
    QuotaThin,
    QuotaCritical,
    QuotaExhausted,
    QuotaUnknown,
    QuotaExhaustedBeforeSend,
    QuotaWindowsUnavailable,
    ProfileInflightSoftLimit,
    AuthNotQuotaCompatible,
    PromptCacheAffinity,
    NegativeCache,
    Excluded,
    AffinityOwnerUnavailable,
    SelectionFailed,
    Compatible,
    EndpointUnsupported,
    RequiredCapabilityMissing,
    CatalogEntryUnavailable,
    ContextWindowUnknown,
    ContextWindowExceeded,
    OutputLimitUnknown,
    RequestedOutputExceedsModelLimit,
    ReasoningReserveUnsupported,
    ReasoningReserveExcessive,
    MalformedRequestLimits,
    OutputLimitClamped,
}

impl RuntimeRouteDecisionReasonKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AuthFailureBackoff => "auth_failure_backoff",
            Self::SelectionBackoff => "selection_backoff",
            Self::RouteCircuitOpen => "route_circuit_open",
            Self::RouteCircuitHalfOpenProbeWait => "route_circuit_half_open_probe_wait",
            Self::ProfileHealth => "profile_health",
            Self::ProfilePerformance => "profile_performance",
            Self::QuotaProbeUnavailable => "quota_probe_unavailable",
            Self::StalePersistedQuota => "stale_persisted_quota",
            Self::QuotaHealthy => "quota_healthy",
            Self::QuotaThin => "quota_thin",
            Self::QuotaCritical => "quota_critical",
            Self::QuotaExhausted => "quota_exhausted",
            Self::QuotaUnknown => "quota_unknown",
            Self::QuotaExhaustedBeforeSend => "quota_exhausted_before_send",
            Self::QuotaWindowsUnavailable => "quota_windows_unavailable",
            Self::ProfileInflightSoftLimit => "profile_inflight_soft_limit",
            Self::AuthNotQuotaCompatible => "auth_not_quota_compatible",
            Self::PromptCacheAffinity => "prompt_cache_affinity",
            Self::NegativeCache => "negative_cache",
            Self::Excluded => "excluded",
            Self::AffinityOwnerUnavailable => "affinity_owner_unavailable",
            Self::SelectionFailed => "selection_failed",
            Self::Compatible => "compatible",
            Self::EndpointUnsupported => "endpoint_unsupported",
            Self::RequiredCapabilityMissing => "required_capability_missing",
            Self::CatalogEntryUnavailable => "catalog_entry_unavailable",
            Self::ContextWindowUnknown => "context_window_unknown",
            Self::ContextWindowExceeded => "context_window_exceeded",
            Self::OutputLimitUnknown => "output_limit_unknown",
            Self::RequestedOutputExceedsModelLimit => "requested_output_exceeds_model_limit",
            Self::ReasoningReserveUnsupported => "reasoning_reserve_unsupported",
            Self::ReasoningReserveExcessive => "reasoning_reserve_excessive",
            Self::MalformedRequestLimits => "malformed_request_limits",
            Self::OutputLimitClamped => "output_limit_clamped",
        }
    }

    pub fn from_label(label: &str) -> Option<Self> {
        const VALUES: &[RuntimeRouteDecisionReasonKind] = &[
            RuntimeRouteDecisionReasonKind::AuthFailureBackoff,
            RuntimeRouteDecisionReasonKind::SelectionBackoff,
            RuntimeRouteDecisionReasonKind::RouteCircuitOpen,
            RuntimeRouteDecisionReasonKind::RouteCircuitHalfOpenProbeWait,
            RuntimeRouteDecisionReasonKind::ProfileHealth,
            RuntimeRouteDecisionReasonKind::ProfilePerformance,
            RuntimeRouteDecisionReasonKind::QuotaProbeUnavailable,
            RuntimeRouteDecisionReasonKind::StalePersistedQuota,
            RuntimeRouteDecisionReasonKind::QuotaHealthy,
            RuntimeRouteDecisionReasonKind::QuotaThin,
            RuntimeRouteDecisionReasonKind::QuotaCritical,
            RuntimeRouteDecisionReasonKind::QuotaExhausted,
            RuntimeRouteDecisionReasonKind::QuotaUnknown,
            RuntimeRouteDecisionReasonKind::QuotaExhaustedBeforeSend,
            RuntimeRouteDecisionReasonKind::QuotaWindowsUnavailable,
            RuntimeRouteDecisionReasonKind::ProfileInflightSoftLimit,
            RuntimeRouteDecisionReasonKind::AuthNotQuotaCompatible,
            RuntimeRouteDecisionReasonKind::PromptCacheAffinity,
            RuntimeRouteDecisionReasonKind::NegativeCache,
            RuntimeRouteDecisionReasonKind::Excluded,
            RuntimeRouteDecisionReasonKind::AffinityOwnerUnavailable,
            RuntimeRouteDecisionReasonKind::SelectionFailed,
            RuntimeRouteDecisionReasonKind::Compatible,
            RuntimeRouteDecisionReasonKind::EndpointUnsupported,
            RuntimeRouteDecisionReasonKind::RequiredCapabilityMissing,
            RuntimeRouteDecisionReasonKind::CatalogEntryUnavailable,
            RuntimeRouteDecisionReasonKind::ContextWindowUnknown,
            RuntimeRouteDecisionReasonKind::ContextWindowExceeded,
            RuntimeRouteDecisionReasonKind::OutputLimitUnknown,
            RuntimeRouteDecisionReasonKind::RequestedOutputExceedsModelLimit,
            RuntimeRouteDecisionReasonKind::ReasoningReserveUnsupported,
            RuntimeRouteDecisionReasonKind::ReasoningReserveExcessive,
            RuntimeRouteDecisionReasonKind::MalformedRequestLimits,
            RuntimeRouteDecisionReasonKind::OutputLimitClamped,
        ];
        VALUES.iter().copied().find(|value| value.as_str() == label)
    }

    pub const fn rejection_stage(self) -> RuntimeRouteDecisionStage {
        match self {
            Self::AuthFailureBackoff | Self::AuthNotQuotaCompatible => {
                RuntimeRouteDecisionStage::Authentication
            }
            Self::SelectionBackoff
            | Self::RouteCircuitOpen
            | Self::RouteCircuitHalfOpenProbeWait => RuntimeRouteDecisionStage::CircuitAndBackoff,
            Self::QuotaProbeUnavailable
            | Self::StalePersistedQuota
            | Self::QuotaHealthy
            | Self::QuotaThin
            | Self::QuotaCritical
            | Self::QuotaExhausted
            | Self::QuotaUnknown
            | Self::QuotaExhaustedBeforeSend
            | Self::QuotaWindowsUnavailable => RuntimeRouteDecisionStage::Quota,
            Self::ProfileInflightSoftLimit => RuntimeRouteDecisionStage::Admission,
            Self::ProfileHealth | Self::ProfilePerformance | Self::PromptCacheAffinity => {
                RuntimeRouteDecisionStage::Ranking
            }
            Self::NegativeCache | Self::Excluded | Self::AffinityOwnerUnavailable => {
                RuntimeRouteDecisionStage::Affinity
            }
            Self::SelectionFailed => RuntimeRouteDecisionStage::FinalSelection,
            Self::Compatible
            | Self::ContextWindowUnknown
            | Self::ContextWindowExceeded
            | Self::OutputLimitUnknown
            | Self::RequestedOutputExceedsModelLimit
            | Self::ReasoningReserveUnsupported
            | Self::ReasoningReserveExcessive
            | Self::MalformedRequestLimits
            | Self::OutputLimitClamped => RuntimeRouteDecisionStage::RequestConstraints,
            Self::EndpointUnsupported | Self::RequiredCapabilityMissing => {
                RuntimeRouteDecisionStage::EndpointCapability
            }
            Self::CatalogEntryUnavailable => RuntimeRouteDecisionStage::ModelResolution,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeRouteDecisionReason {
    Known(RuntimeRouteDecisionReasonKind),
    Unknown(String),
}

impl RuntimeRouteDecisionReason {
    pub fn known(reason: RuntimeRouteDecisionReasonKind) -> Self {
        Self::Known(reason)
    }

    pub fn from_label(label: &str) -> Self {
        RuntimeRouteDecisionReasonKind::from_label(label)
            .map(Self::Known)
            .unwrap_or_else(|| Self::Unknown(runtime_route_trace_reason_label(label)))
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::Known(reason) => reason.as_str(),
            Self::Unknown(reason) => reason,
        }
    }

    pub fn rejection_stage(&self) -> Option<RuntimeRouteDecisionStage> {
        match self {
            Self::Known(reason) => Some(reason.rejection_stage()),
            Self::Unknown(_) => None,
        }
    }
}

impl Serialize for RuntimeRouteDecisionReason {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for RuntimeRouteDecisionReason {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer).map(|label| Self::from_label(&label))
    }
}

impl From<RuntimeRouteDecisionReasonKind> for RuntimeRouteDecisionReason {
    fn from(reason: RuntimeRouteDecisionReasonKind) -> Self {
        Self::Known(reason)
    }
}

fn runtime_route_trace_reason_label(value: &str) -> String {
    let value = value.trim();
    if value.is_empty()
        || value.len() > RUNTIME_ROUTE_DECISION_TRACE_MAX_IDENTIFIER_BYTES
        || value
            .chars()
            .any(|ch| !(ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_'))
    {
        "unknown".to_string()
    } else {
        value.to_string()
    }
}
