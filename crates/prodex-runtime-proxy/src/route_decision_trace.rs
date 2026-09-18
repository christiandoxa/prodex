use serde::{Deserialize, Serialize};

use crate::RuntimeRouteKind;

mod reason;
pub use reason::*;

pub const RUNTIME_ROUTE_DECISION_TRACE_SCHEMA_VERSION: u16 = 1;
pub const RUNTIME_ROUTE_DECISION_TRACE_MAX_IDENTIFIER_BYTES: usize = 96;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeRouteDecisionRoute {
    Responses,
    ResponsesCompact,
    ChatCompletions,
    Messages,
    Embeddings,
    Websocket,
    Standard,
}

impl RuntimeRouteDecisionRoute {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Responses => "responses",
            Self::ResponsesCompact => "responses_compact",
            Self::ChatCompletions => "chat_completions",
            Self::Messages => "messages",
            Self::Embeddings => "embeddings",
            Self::Websocket => "websocket",
            Self::Standard => "standard",
        }
    }
}

impl From<RuntimeRouteKind> for RuntimeRouteDecisionRoute {
    fn from(route: RuntimeRouteKind) -> Self {
        match route {
            RuntimeRouteKind::Responses => Self::Responses,
            RuntimeRouteKind::Compact => Self::ResponsesCompact,
            RuntimeRouteKind::Websocket => Self::Websocket,
            RuntimeRouteKind::Standard => Self::Standard,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeRouteDecisionStage {
    Affinity,
    ModelResolution,
    EndpointCapability,
    RequestConstraints,
    Governance,
    Authentication,
    Quota,
    CircuitAndBackoff,
    Admission,
    Ranking,
    FinalSelection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeRouteDecisionStageOutcome {
    Passed,
    Skipped,
    Rejected,
    Selected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeRouteAffinityKind {
    None,
    Strict,
    PreviousResponse,
    TurnState,
    Session,
    PromptCache,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeRouteAffinityOutcome {
    NotApplicable,
    Retained,
    Rejected,
    Exhausted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeRouteCandidateClass {
    Affinity,
    Current,
    Ready,
    Fallback,
    AutoRedeem,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeRouteCandidateEligibility {
    Eligible,
    Rejected,
    Deferred,
    NotEvaluated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeRouteQuotaBand {
    Healthy,
    Thin,
    Critical,
    Exhausted,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeRouteCircuitState {
    Closed,
    Open,
    HalfOpenWait,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeRouteHealthBand {
    Healthy,
    Penalized,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeRouteCandidateDecisionInput {
    pub original_order: usize,
    pub hard_affinity: bool,
    pub class: RuntimeRouteCandidateClass,
    pub eligibility: RuntimeRouteCandidateEligibility,
    pub rejection_stage: Option<RuntimeRouteDecisionStage>,
    pub reason: Option<RuntimeRouteDecisionReason>,
    pub selected: bool,
    pub quota_band: Option<RuntimeRouteQuotaBand>,
    pub circuit_state: Option<RuntimeRouteCircuitState>,
    pub health_band: Option<RuntimeRouteHealthBand>,
    pub inflight_count: Option<usize>,
}

impl RuntimeRouteCandidateDecisionInput {
    pub fn eligible(original_order: usize, class: RuntimeRouteCandidateClass) -> Self {
        Self {
            original_order,
            hard_affinity: false,
            class,
            eligibility: RuntimeRouteCandidateEligibility::Eligible,
            rejection_stage: None,
            reason: None,
            selected: false,
            quota_band: None,
            circuit_state: None,
            health_band: None,
            inflight_count: None,
        }
    }

    pub fn rejected(
        original_order: usize,
        class: RuntimeRouteCandidateClass,
        reason: impl Into<RuntimeRouteDecisionReason>,
    ) -> Self {
        let reason = reason.into();
        Self {
            eligibility: RuntimeRouteCandidateEligibility::Rejected,
            rejection_stage: reason.rejection_stage(),
            reason: Some(reason),
            ..Self::eligible(original_order, class)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeRouteDecisionTerminalOutcome {
    Selected,
    NoCandidate,
    AffinityExhausted,
    Failed,
}

impl RuntimeRouteDecisionTerminalOutcome {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Selected => "selected",
            Self::NoCandidate => "no_candidate",
            Self::AffinityExhausted => "affinity_exhausted",
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeRouteDecisionTrace {
    pub schema_version: u16,
    pub route: RuntimeRouteDecisionRoute,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requested_model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_candidate: Option<String>,
    pub terminal_outcome: RuntimeRouteDecisionTerminalOutcome,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terminal_reason: Option<RuntimeRouteDecisionReason>,
}

pub struct RuntimeRouteDecisionTraceBuilder {
    trace: RuntimeRouteDecisionTrace,
    enabled: bool,
}

impl RuntimeRouteDecisionTraceBuilder {
    pub fn new(route: RuntimeRouteDecisionRoute, requested_model: Option<&str>) -> Self {
        Self {
            trace: RuntimeRouteDecisionTrace {
                schema_version: RUNTIME_ROUTE_DECISION_TRACE_SCHEMA_VERSION,
                route,
                requested_model: requested_model
                    .map(runtime_route_decision_safe_identifier)
                    .map(|v| v.0),
                resolved_model: None,
                selected_candidate: None,
                terminal_outcome: RuntimeRouteDecisionTerminalOutcome::NoCandidate,
                terminal_reason: None,
            },
            enabled: true,
        }
    }

    #[doc(hidden)]
    pub fn without_recording(route: RuntimeRouteDecisionRoute) -> Self {
        let mut builder = Self::new(route, None);
        builder.enabled = false;
        builder
    }

    pub fn set_resolved_model(&mut self, model: Option<&str>) {
        if self.enabled {
            self.trace.resolved_model = model
                .map(runtime_route_decision_safe_identifier)
                .map(|value| value.0);
        }
    }

    pub fn record_stage(
        &mut self,
        _stage: RuntimeRouteDecisionStage,
        _outcome: RuntimeRouteDecisionStageOutcome,
    ) {
    }

    pub fn record_affinity(
        &mut self,
        _kind: RuntimeRouteAffinityKind,
        _candidate_key: Option<&str>,
        _hard: bool,
        _outcome: RuntimeRouteAffinityOutcome,
    ) {
    }

    pub fn record_candidate(
        &mut self,
        candidate_key: &str,
        _input: RuntimeRouteCandidateDecisionInput,
    ) -> Option<String> {
        self.enabled
            .then(|| runtime_route_decision_safe_identifier(candidate_key).0)
    }

    pub fn mark_selected(&mut self, candidate_key: &str) {
        if self.enabled {
            self.trace.selected_candidate =
                Some(runtime_route_decision_safe_identifier(candidate_key).0);
        }
    }

    pub fn finish(
        mut self,
        terminal_outcome: RuntimeRouteDecisionTerminalOutcome,
        terminal_reason: Option<RuntimeRouteDecisionReason>,
    ) -> RuntimeRouteDecisionTrace {
        self.trace.terminal_outcome = terminal_outcome;
        self.trace.terminal_reason = terminal_reason;
        self.trace
    }
}

pub fn runtime_route_decision_safe_identifier(value: &str) -> (String, bool) {
    let value = value.trim();
    if value.len() <= RUNTIME_ROUTE_DECISION_TRACE_MAX_IDENTIFIER_BYTES {
        return (value.to_string(), false);
    }
    let mut end = RUNTIME_ROUTE_DECISION_TRACE_MAX_IDENTIFIER_BYTES;
    while !value.is_char_boundary(end) {
        end = end.saturating_sub(1);
    }
    (value[..end].to_string(), true)
}

#[cfg(test)]
#[path = "../tests/src/route_decision_trace.rs"]
mod tests;
