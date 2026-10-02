use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::RuntimeRouteDecisionStage;

#[repr(u8)]
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
    pub fn as_str(self) -> &'static str {
        prodex_mojo_core::runtime_route_reason::label(self as u8)
            .expect("Mojo route-decision reason label returned invalid output")
    }

    pub fn from_label(label: &str) -> Option<Self> {
        let lookup = prodex_mojo_core::runtime_route_reason::lookup(label)
            .expect("Mojo route-decision reason lookup returned invalid output");
        lookup.kind.and_then(runtime_route_reason_kind_from_tag)
    }

    pub fn rejection_stage(self) -> RuntimeRouteDecisionStage {
        let tag = prodex_mojo_core::runtime_route_reason::stage(self as u8)
            .expect("Mojo route-decision rejection-stage mapping returned invalid output");
        runtime_route_decision_stage_from_tag(tag).expect("validated Mojo route-decision stage tag")
    }
}

fn runtime_route_reason_kind_from_tag(tag: u8) -> Option<RuntimeRouteDecisionReasonKind> {
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
    VALUES.get(usize::from(tag)).copied()
}

fn runtime_route_decision_stage_from_tag(tag: u8) -> Option<RuntimeRouteDecisionStage> {
    const VALUES: &[RuntimeRouteDecisionStage] = &[
        RuntimeRouteDecisionStage::Affinity,
        RuntimeRouteDecisionStage::ModelResolution,
        RuntimeRouteDecisionStage::EndpointCapability,
        RuntimeRouteDecisionStage::RequestConstraints,
        RuntimeRouteDecisionStage::Governance,
        RuntimeRouteDecisionStage::Authentication,
        RuntimeRouteDecisionStage::Quota,
        RuntimeRouteDecisionStage::CircuitAndBackoff,
        RuntimeRouteDecisionStage::Admission,
        RuntimeRouteDecisionStage::Ranking,
        RuntimeRouteDecisionStage::FinalSelection,
    ];
    VALUES.get(usize::from(tag)).copied()
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
    prodex_mojo_core::runtime_route_reason::normalize_unknown(value)
        .expect("Mojo route-decision unknown-label normalization returned invalid output")
        .unwrap_or("unknown")
        .to_string()
}
