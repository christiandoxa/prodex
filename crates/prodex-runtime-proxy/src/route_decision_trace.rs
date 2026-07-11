use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::RuntimeRouteKind;

mod reason;
pub use reason::*;

pub const RUNTIME_ROUTE_DECISION_TRACE_SCHEMA_VERSION: u16 = 1;
pub const RUNTIME_ROUTE_DECISION_TRACE_MAX_STAGES: usize = 11;
pub const RUNTIME_ROUTE_DECISION_TRACE_MAX_CANDIDATES: usize = 32;
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

impl RuntimeRouteDecisionStageOutcome {
    fn priority(self) -> u8 {
        match self {
            Self::Skipped => 0,
            Self::Passed => 1,
            Self::Rejected => 2,
            Self::Selected => 3,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeRouteDecisionStageSummary {
    pub stage: RuntimeRouteDecisionStage,
    pub outcome: RuntimeRouteDecisionStageOutcome,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeRouteAffinityDecision {
    pub kind: RuntimeRouteAffinityKind,
    pub candidate_id: Option<String>,
    pub hard: bool,
    pub outcome: RuntimeRouteAffinityOutcome,
}

impl Default for RuntimeRouteAffinityDecision {
    fn default() -> Self {
        Self {
            kind: RuntimeRouteAffinityKind::None,
            candidate_id: None,
            hard: false,
            outcome: RuntimeRouteAffinityOutcome::NotApplicable,
        }
    }
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

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeRouteDecisionDiagnostics {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub estimated_input_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_reserve_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_required_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub available_context_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requested_output_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub applied_output_tokens: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeRouteCandidateDecision {
    pub candidate_id: String,
    pub original_order: usize,
    pub provider: Option<String>,
    pub model: Option<String>,
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
    pub diagnostics: RuntimeRouteDecisionDiagnostics,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeRouteCandidateDecisionInput {
    pub original_order: usize,
    pub provider: Option<String>,
    pub model: Option<String>,
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
    pub diagnostics: RuntimeRouteDecisionDiagnostics,
}

impl RuntimeRouteCandidateDecisionInput {
    pub fn eligible(original_order: usize, class: RuntimeRouteCandidateClass) -> Self {
        Self {
            original_order,
            provider: None,
            model: None,
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
            diagnostics: RuntimeRouteDecisionDiagnostics::default(),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeRouteDecisionCommitState {
    PreCommit,
    Committed,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeRouteDecisionTruncation {
    pub truncated: bool,
    pub omitted_stages: usize,
    pub omitted_candidate_records: usize,
    pub truncated_identifiers: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeRouteDecisionTrace {
    pub schema_version: u16,
    pub route: RuntimeRouteDecisionRoute,
    pub requested_model: Option<String>,
    pub resolved_model: Option<String>,
    pub affinity: RuntimeRouteAffinityDecision,
    pub stages: Vec<RuntimeRouteDecisionStageSummary>,
    pub candidates: Vec<RuntimeRouteCandidateDecision>,
    pub selected_candidate: Option<String>,
    pub terminal_outcome: RuntimeRouteDecisionTerminalOutcome,
    pub terminal_reason: Option<RuntimeRouteDecisionReason>,
    pub commit_state: RuntimeRouteDecisionCommitState,
    pub truncation: RuntimeRouteDecisionTruncation,
}

pub struct RuntimeRouteDecisionTraceBuilder {
    trace: RuntimeRouteDecisionTrace,
    candidate_indexes: BTreeMap<String, usize>,
}

impl RuntimeRouteDecisionTraceBuilder {
    pub fn new(route: RuntimeRouteDecisionRoute, requested_model: Option<&str>) -> Self {
        let (requested_model, truncated) = runtime_route_trace_optional_identifier(requested_model);
        let truncation = RuntimeRouteDecisionTruncation {
            truncated,
            truncated_identifiers: usize::from(truncated),
            ..RuntimeRouteDecisionTruncation::default()
        };
        Self {
            trace: RuntimeRouteDecisionTrace {
                schema_version: RUNTIME_ROUTE_DECISION_TRACE_SCHEMA_VERSION,
                route,
                requested_model,
                resolved_model: None,
                affinity: RuntimeRouteAffinityDecision::default(),
                stages: Vec::new(),
                candidates: Vec::new(),
                selected_candidate: None,
                terminal_outcome: RuntimeRouteDecisionTerminalOutcome::NoCandidate,
                terminal_reason: None,
                commit_state: RuntimeRouteDecisionCommitState::PreCommit,
                truncation,
            },
            candidate_indexes: BTreeMap::new(),
        }
    }

    pub fn set_resolved_model(&mut self, model: Option<&str>) {
        let (model, truncated) = runtime_route_trace_optional_identifier(model);
        self.trace.resolved_model = model;
        self.note_identifier_truncation(truncated);
    }

    pub fn record_stage(
        &mut self,
        stage: RuntimeRouteDecisionStage,
        outcome: RuntimeRouteDecisionStageOutcome,
    ) {
        if let Some(existing) = self
            .trace
            .stages
            .iter_mut()
            .find(|summary| summary.stage == stage)
        {
            if outcome.priority() > existing.outcome.priority() {
                existing.outcome = outcome;
            }
            return;
        }
        if self.trace.stages.len() >= RUNTIME_ROUTE_DECISION_TRACE_MAX_STAGES {
            self.trace.truncation.truncated = true;
            self.trace.truncation.omitted_stages =
                self.trace.truncation.omitted_stages.saturating_add(1);
            return;
        }
        self.trace
            .stages
            .push(RuntimeRouteDecisionStageSummary { stage, outcome });
    }

    pub fn record_affinity(
        &mut self,
        kind: RuntimeRouteAffinityKind,
        candidate_key: Option<&str>,
        hard: bool,
        outcome: RuntimeRouteAffinityOutcome,
    ) {
        let candidate_id = candidate_key.and_then(|key| self.candidate_id(key));
        self.trace.affinity = RuntimeRouteAffinityDecision {
            kind,
            candidate_id,
            hard,
            outcome,
        };
        self.record_stage(
            RuntimeRouteDecisionStage::Affinity,
            match outcome {
                RuntimeRouteAffinityOutcome::Retained => RuntimeRouteDecisionStageOutcome::Passed,
                RuntimeRouteAffinityOutcome::Rejected | RuntimeRouteAffinityOutcome::Exhausted => {
                    RuntimeRouteDecisionStageOutcome::Rejected
                }
                RuntimeRouteAffinityOutcome::NotApplicable => {
                    RuntimeRouteDecisionStageOutcome::Skipped
                }
            },
        );
    }

    pub fn record_candidate(
        &mut self,
        candidate_key: &str,
        mut input: RuntimeRouteCandidateDecisionInput,
    ) -> Option<String> {
        let index = if let Some(index) = self.candidate_indexes.get(candidate_key).copied() {
            index
        } else {
            if self.trace.candidates.len() >= RUNTIME_ROUTE_DECISION_TRACE_MAX_CANDIDATES {
                self.trace.truncation.truncated = true;
                self.trace.truncation.omitted_candidate_records = self
                    .trace
                    .truncation
                    .omitted_candidate_records
                    .saturating_add(1);
                return None;
            }
            let index = self.trace.candidates.len();
            self.candidate_indexes
                .insert(candidate_key.to_string(), index);
            let candidate_id = format!("candidate-{:04}", index.saturating_add(1));
            self.trace.candidates.push(RuntimeRouteCandidateDecision {
                candidate_id,
                original_order: input.original_order,
                provider: None,
                model: None,
                hard_affinity: false,
                class: input.class,
                eligibility: RuntimeRouteCandidateEligibility::NotEvaluated,
                rejection_stage: None,
                reason: None,
                selected: false,
                quota_band: None,
                circuit_state: None,
                health_band: None,
                inflight_count: None,
                diagnostics: RuntimeRouteDecisionDiagnostics::default(),
            });
            index
        };

        if self.trace.candidates[index].eligibility == RuntimeRouteCandidateEligibility::Rejected
            && matches!(
                input.eligibility,
                RuntimeRouteCandidateEligibility::Deferred
                    | RuntimeRouteCandidateEligibility::NotEvaluated
            )
            && !input.selected
        {
            return Some(self.trace.candidates[index].candidate_id.clone());
        }

        let (provider, provider_truncated) =
            runtime_route_trace_optional_identifier(input.provider.as_deref());
        let (model, model_truncated) =
            runtime_route_trace_optional_identifier(input.model.as_deref());
        self.note_identifier_truncation(provider_truncated);
        self.note_identifier_truncation(model_truncated);
        input.provider = provider;
        input.model = model;
        let candidate = &mut self.trace.candidates[index];
        candidate.original_order = input.original_order;
        candidate.provider = input.provider;
        candidate.model = input.model;
        candidate.hard_affinity = input.hard_affinity;
        candidate.class = input.class;
        candidate.eligibility = input.eligibility;
        candidate.rejection_stage = input.rejection_stage;
        candidate.reason = input.reason;
        candidate.selected |= input.selected;
        candidate.quota_band = input.quota_band;
        candidate.circuit_state = input.circuit_state;
        candidate.health_band = input.health_band;
        candidate.inflight_count = input.inflight_count;
        candidate.diagnostics = input.diagnostics;
        let candidate_id = candidate.candidate_id.clone();
        if candidate.selected {
            self.trace.selected_candidate = Some(candidate_id.clone());
        }
        if let Some(stage) = candidate.rejection_stage {
            self.record_stage(stage, RuntimeRouteDecisionStageOutcome::Rejected);
        }
        Some(candidate_id)
    }

    pub fn mark_selected(&mut self, candidate_key: &str) {
        let Some(candidate_id) = self.candidate_id(candidate_key) else {
            return;
        };
        if let Some(index) = self.candidate_indexes.get(candidate_key).copied()
            && let Some(candidate) = self.trace.candidates.get_mut(index)
        {
            candidate.selected = true;
            candidate.eligibility = RuntimeRouteCandidateEligibility::Eligible;
            candidate.rejection_stage = None;
        }
        self.trace.selected_candidate = Some(candidate_id);
        self.record_stage(
            RuntimeRouteDecisionStage::FinalSelection,
            RuntimeRouteDecisionStageOutcome::Selected,
        );
    }

    pub fn set_commit_state(&mut self, commit_state: RuntimeRouteDecisionCommitState) {
        self.trace.commit_state = commit_state;
    }

    pub fn finish(
        mut self,
        terminal_outcome: RuntimeRouteDecisionTerminalOutcome,
        terminal_reason: Option<RuntimeRouteDecisionReason>,
    ) -> RuntimeRouteDecisionTrace {
        self.trace.terminal_outcome = terminal_outcome;
        self.trace.terminal_reason = terminal_reason;
        if terminal_outcome != RuntimeRouteDecisionTerminalOutcome::Selected {
            self.record_stage(
                RuntimeRouteDecisionStage::FinalSelection,
                RuntimeRouteDecisionStageOutcome::Rejected,
            );
        }
        self.trace.stages.sort_by_key(|summary| summary.stage);
        self.trace.candidates.sort_by(|left, right| {
            left.original_order
                .cmp(&right.original_order)
                .then_with(|| left.candidate_id.cmp(&right.candidate_id))
        });
        self.trace
    }

    fn candidate_id(&mut self, candidate_key: &str) -> Option<String> {
        if let Some(index) = self.candidate_indexes.get(candidate_key).copied() {
            return self
                .trace
                .candidates
                .get(index)
                .map(|candidate| candidate.candidate_id.clone());
        }
        self.record_candidate(
            candidate_key,
            RuntimeRouteCandidateDecisionInput::eligible(
                self.trace.candidates.len(),
                RuntimeRouteCandidateClass::Affinity,
            ),
        )
    }

    fn note_identifier_truncation(&mut self, truncated: bool) {
        if truncated {
            self.trace.truncation.truncated = true;
            self.trace.truncation.truncated_identifiers = self
                .trace
                .truncation
                .truncated_identifiers
                .saturating_add(1);
        }
    }
}

fn runtime_route_trace_optional_identifier(value: Option<&str>) -> (Option<String>, bool) {
    match value.map(str::trim).filter(|value| !value.is_empty()) {
        Some(value) => {
            let (value, truncated) = runtime_route_trace_identifier(value);
            (Some(value), truncated)
        }
        None => (None, false),
    }
}

pub fn runtime_route_decision_safe_identifier(value: &str) -> (String, bool) {
    runtime_route_trace_identifier(value)
}

fn runtime_route_trace_identifier(value: &str) -> (String, bool) {
    let value = value.trim();
    if value.is_empty()
        || redaction::redaction_redact_secret_like_text(value) != value
        || value.starts_with('/')
        || value.contains("..")
        || value.matches('/').count() > 1
        || value.contains(['@', '\\'])
        || value.chars().any(|ch| {
            !(ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | ':' | '/' | ',' | '+'))
        })
    {
        return ("redacted".to_string(), value != "redacted");
    }
    if value.len() <= RUNTIME_ROUTE_DECISION_TRACE_MAX_IDENTIFIER_BYTES {
        return (value.to_string(), false);
    }
    let mut end = RUNTIME_ROUTE_DECISION_TRACE_MAX_IDENTIFIER_BYTES;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    (value[..end].to_string(), true)
}

#[cfg(test)]
#[path = "../tests/src/route_decision_trace.rs"]
mod tests;
