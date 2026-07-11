use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SmartContextSegmentSafetyClass {
    ProtocolExact,
    ContinuationExact,
    CriticalExact,
    RehydratableExact,
    LosslessTransformable,
    Condensable,
    DroppableDuplicate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmartContextSegmentFailureScope {
    SegmentLocal,
    RequestGlobal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartContextSegmentSafetyEnvelope {
    pub class: SmartContextSegmentSafetyClass,
    pub failure_scope: SmartContextSegmentFailureScope,
    pub reason: Option<SmartContextExactnessReason>,
}

impl SmartContextSegmentSafetyEnvelope {
    pub fn requires_exact_segment(&self) -> bool {
        matches!(
            self.class,
            SmartContextSegmentSafetyClass::ProtocolExact
                | SmartContextSegmentSafetyClass::ContinuationExact
                | SmartContextSegmentSafetyClass::CriticalExact
                | SmartContextSegmentSafetyClass::RehydratableExact
        )
    }
}

pub fn smart_context_exactness_reason_segment_envelope(
    reason: SmartContextExactnessReason,
) -> SmartContextSegmentSafetyEnvelope {
    let class = match reason {
        SmartContextExactnessReason::ExplicitExactMode => {
            SmartContextSegmentSafetyClass::ProtocolExact
        }
        SmartContextExactnessReason::PreviousResponseAffinity
        | SmartContextExactnessReason::TurnStateAffinity
        | SmartContextExactnessReason::SessionAffinity => {
            SmartContextSegmentSafetyClass::ContinuationExact
        }
        SmartContextExactnessReason::ToolOutputWithoutArtifact => {
            SmartContextSegmentSafetyClass::CriticalExact
        }
    };
    let failure_scope = if matches!(reason, SmartContextExactnessReason::ExplicitExactMode) {
        SmartContextSegmentFailureScope::RequestGlobal
    } else {
        SmartContextSegmentFailureScope::SegmentLocal
    };
    SmartContextSegmentSafetyEnvelope {
        class,
        failure_scope,
        reason: Some(reason),
    }
}

pub fn smart_context_missing_rehydrate_ref_segment_envelope() -> SmartContextSegmentSafetyEnvelope {
    SmartContextSegmentSafetyEnvelope {
        class: SmartContextSegmentSafetyClass::RehydratableExact,
        failure_scope: SmartContextSegmentFailureScope::SegmentLocal,
        reason: None,
    }
}
