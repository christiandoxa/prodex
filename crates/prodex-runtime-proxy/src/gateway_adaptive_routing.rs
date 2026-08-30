use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeGatewayAdaptiveRoutingConfig {
    pub enabled: bool,
    pub shadow_mode: bool,
    pub window_size: usize,
    pub min_samples: u64,
    pub exploration_rate_bps: u16,
}

impl Default for RuntimeGatewayAdaptiveRoutingConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            shadow_mode: true,
            window_size: 128,
            min_samples: 8,
            exploration_rate_bps: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeGatewayAdaptiveFeedbackSignal {
    TaskCompleted,
    CorrectiveUserMessage,
    AdditionalTurn,
    PreviousResponseNotFound,
    InvalidToolCallContinuation,
    Error,
    TokenSavings(u64),
    LatencyMs(u64),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeGatewayAdaptiveQualityWindow {
    pub samples: u64,
    pub task_completed: u64,
    pub corrective_user_messages: u64,
    pub additional_turns: u64,
    pub previous_response_not_found: u64,
    pub invalid_tool_call_continuation: u64,
    pub errors: u64,
    pub token_savings: u64,
    pub latency_ms_total: u64,
}

impl RuntimeGatewayAdaptiveQualityWindow {
    pub fn record(&mut self, signal: RuntimeGatewayAdaptiveFeedbackSignal) {
        self.samples = self.samples.saturating_add(1);
        match signal {
            RuntimeGatewayAdaptiveFeedbackSignal::TaskCompleted => {
                self.task_completed = self.task_completed.saturating_add(1);
            }
            RuntimeGatewayAdaptiveFeedbackSignal::CorrectiveUserMessage => {
                self.corrective_user_messages = self.corrective_user_messages.saturating_add(1);
            }
            RuntimeGatewayAdaptiveFeedbackSignal::AdditionalTurn => {
                self.additional_turns = self.additional_turns.saturating_add(1);
            }
            RuntimeGatewayAdaptiveFeedbackSignal::PreviousResponseNotFound => {
                self.previous_response_not_found =
                    self.previous_response_not_found.saturating_add(1);
            }
            RuntimeGatewayAdaptiveFeedbackSignal::InvalidToolCallContinuation => {
                self.invalid_tool_call_continuation =
                    self.invalid_tool_call_continuation.saturating_add(1);
            }
            RuntimeGatewayAdaptiveFeedbackSignal::Error => {
                self.errors = self.errors.saturating_add(1);
            }
            RuntimeGatewayAdaptiveFeedbackSignal::TokenSavings(tokens) => {
                self.token_savings = self.token_savings.saturating_add(tokens);
            }
            RuntimeGatewayAdaptiveFeedbackSignal::LatencyMs(latency) => {
                self.latency_ms_total = self.latency_ms_total.saturating_add(latency);
            }
        }
    }

    pub fn quality_score_bps(&self) -> i64 {
        #[cfg(feature = "mojo")]
        return prodex_mojo_core::runtime::adaptive_routing_plan(
            &[runtime_gateway_adaptive_quality_input(Some(self))],
            None,
            false,
            0,
            0,
            0,
        )
        .expect("Mojo adaptive quality scoring returned an invalid result")
        .quality_score_bps
        .unwrap_or_default();

        #[cfg(not(feature = "mojo"))]
        self.quality_score_bps_rust()
    }

    #[cfg(any(not(feature = "mojo"), test))]
    fn quality_score_bps_rust(&self) -> i64 {
        let positive = self
            .task_completed
            .saturating_mul(1_000)
            .saturating_add((self.token_savings / 100).min(2_000));
        let negative = self
            .corrective_user_messages
            .saturating_mul(1_200)
            .saturating_add(self.additional_turns.saturating_mul(200))
            .saturating_add(self.previous_response_not_found.saturating_mul(2_000))
            .saturating_add(self.invalid_tool_call_continuation.saturating_mul(2_000))
            .saturating_add(self.errors.saturating_mul(1_500))
            .saturating_add((self.latency_ms_total / 1_000).min(2_000));
        positive as i64 - negative as i64
    }

    #[cfg(any(not(feature = "mojo"), test))]
    pub fn has_min_samples(&self, min_samples: u64) -> bool {
        self.samples >= min_samples
    }

    pub fn record_outcome(&mut self, success: bool, latency_ms: u64) {
        self.samples = self.samples.saturating_add(1);
        if success {
            self.task_completed = self.task_completed.saturating_add(1);
        } else {
            self.errors = self.errors.saturating_add(1);
        }
        self.latency_ms_total = self.latency_ms_total.saturating_add(latency_ms);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeGatewayAdaptiveFeedbackEvent {
    pub owner_model: String,
    pub owner_provider: Option<String>,
    pub owner_account: Option<String>,
    pub signal: RuntimeGatewayAdaptiveFeedbackSignal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeGatewayAdaptiveFeedbackQueue {
    capacity: usize,
    dropped_events: u64,
    events: VecDeque<RuntimeGatewayAdaptiveFeedbackEvent>,
}

impl RuntimeGatewayAdaptiveFeedbackQueue {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            dropped_events: 0,
            events: VecDeque::with_capacity(capacity),
        }
    }

    pub fn push(&mut self, event: RuntimeGatewayAdaptiveFeedbackEvent) {
        if self.capacity == 0 {
            self.dropped_events = self.dropped_events.saturating_add(1);
            return;
        }
        if self.events.len() == self.capacity {
            self.events.pop_front();
            self.dropped_events = self.dropped_events.saturating_add(1);
        }
        self.events.push_back(event);
    }

    pub fn drain(&mut self) -> Vec<RuntimeGatewayAdaptiveFeedbackEvent> {
        self.events.drain(..).collect()
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    pub fn dropped_events(&self) -> u64 {
        self.dropped_events
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeGatewayAdaptiveShadowInput {
    pub config: RuntimeGatewayAdaptiveRoutingConfig,
    pub diagnostic_seed: u64,
    pub actual_model: String,
    pub continuation_owner_model: Option<String>,
    pub candidates: Vec<String>,
    pub quota_blocked_models: Vec<String>,
    pub quality: BTreeMap<String, RuntimeGatewayAdaptiveQualityWindow>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RuntimeGatewayAdaptiveShadowDecision {
    pub actual_model: String,
    pub recommended_model: Option<String>,
    pub quality_score_bps: Option<i64>,
    pub override_reason: &'static str,
}

#[cfg(feature = "mojo")]
fn runtime_gateway_adaptive_quality_input(
    window: Option<&RuntimeGatewayAdaptiveQualityWindow>,
) -> prodex_mojo_core::runtime::AdaptiveQualityInput {
    let Some(window) = window else {
        return prodex_mojo_core::runtime::AdaptiveQualityInput {
            has_window: false,
            samples: 0,
            task_completed: 0,
            corrective_user_messages: 0,
            additional_turns: 0,
            previous_response_not_found: 0,
            invalid_tool_call_continuation: 0,
            errors: 0,
            token_savings: 0,
            latency_ms_total: 0,
        };
    };
    prodex_mojo_core::runtime::AdaptiveQualityInput {
        has_window: true,
        samples: window.samples,
        task_completed: window.task_completed,
        corrective_user_messages: window.corrective_user_messages,
        additional_turns: window.additional_turns,
        previous_response_not_found: window.previous_response_not_found,
        invalid_tool_call_continuation: window.invalid_tool_call_continuation,
        errors: window.errors,
        token_savings: window.token_savings,
        latency_ms_total: window.latency_ms_total,
    }
}

pub fn runtime_gateway_adaptive_shadow_decision(
    input: RuntimeGatewayAdaptiveShadowInput,
) -> RuntimeGatewayAdaptiveShadowDecision {
    #[cfg(feature = "mojo")]
    {
        if let Some(owner) = input.continuation_owner_model.as_ref() {
            return RuntimeGatewayAdaptiveShadowDecision {
                actual_model: input.actual_model,
                recommended_model: Some(owner.clone()),
                quality_score_bps: None,
                override_reason: "continuation_affinity",
            };
        }
        if !input.config.enabled {
            return RuntimeGatewayAdaptiveShadowDecision {
                actual_model: input.actual_model,
                recommended_model: None,
                quality_score_bps: None,
                override_reason: "adaptive_disabled",
            };
        }
        runtime_gateway_adaptive_shadow_decision_mojo(input)
    }

    #[cfg(not(feature = "mojo"))]
    runtime_gateway_adaptive_shadow_decision_rust(input)
}

#[cfg(feature = "mojo")]
fn runtime_gateway_adaptive_shadow_decision_mojo(
    input: RuntimeGatewayAdaptiveShadowInput,
) -> RuntimeGatewayAdaptiveShadowDecision {
    let blocked = input
        .quota_blocked_models
        .iter()
        .map(|model| model.as_str())
        .collect::<BTreeSet<_>>();
    let candidates = input
        .candidates
        .iter()
        .filter(|candidate| !blocked.contains(candidate.as_str()))
        .collect::<Vec<_>>();
    let actual_index = candidates
        .iter()
        .position(|candidate| candidate.as_str() == input.actual_model);
    let quality = candidates
        .iter()
        .map(|candidate| runtime_gateway_adaptive_quality_input(input.quality.get(*candidate)))
        .collect::<Vec<_>>();
    let plan = prodex_mojo_core::runtime::adaptive_routing_plan(
        &quality,
        actual_index,
        input.config.shadow_mode,
        input.config.min_samples,
        input.config.exploration_rate_bps,
        input.diagnostic_seed,
    )
    .expect("Mojo adaptive routing plan returned an invalid result");
    let recommended_model = plan
        .recommended_index
        .map(|index| candidates[index].to_string());
    let override_reason = match plan.reason {
        prodex_mojo_core::runtime::ADAPTIVE_PLAN_REASON_INSUFFICIENT_SAMPLES => {
            "insufficient_samples"
        }
        prodex_mojo_core::runtime::ADAPTIVE_PLAN_REASON_SHADOW_ONLY => "shadow_only",
        prodex_mojo_core::runtime::ADAPTIVE_PLAN_REASON_ADAPTIVE_ENABLED => "adaptive_enabled",
        prodex_mojo_core::runtime::ADAPTIVE_PLAN_REASON_SHADOW_EXPLORATION => "shadow_exploration",
        prodex_mojo_core::runtime::ADAPTIVE_PLAN_REASON_ADAPTIVE_EXPLORATION => {
            "adaptive_exploration"
        }
        _ => "insufficient_samples",
    };
    RuntimeGatewayAdaptiveShadowDecision {
        actual_model: input.actual_model,
        recommended_model,
        quality_score_bps: plan.quality_score_bps,
        override_reason,
    }
}

#[cfg(any(not(feature = "mojo"), test))]
fn runtime_gateway_adaptive_shadow_decision_rust(
    input: RuntimeGatewayAdaptiveShadowInput,
) -> RuntimeGatewayAdaptiveShadowDecision {
    if let Some(owner) = input.continuation_owner_model.as_ref() {
        return RuntimeGatewayAdaptiveShadowDecision {
            actual_model: input.actual_model,
            recommended_model: Some(owner.clone()),
            quality_score_bps: None,
            override_reason: "continuation_affinity",
        };
    }
    if !input.config.enabled {
        return RuntimeGatewayAdaptiveShadowDecision {
            actual_model: input.actual_model,
            recommended_model: None,
            quality_score_bps: None,
            override_reason: "adaptive_disabled",
        };
    }

    let blocked = input
        .quota_blocked_models
        .iter()
        .map(|model| model.as_str())
        .collect::<BTreeSet<_>>();
    let candidates = input
        .candidates
        .iter()
        .filter(|candidate| !blocked.contains(candidate.as_str()))
        .collect::<Vec<_>>();
    if let Some(decision) = adaptive_exploration_decision(&input, &candidates) {
        return decision;
    }
    let mut best: Option<(&str, i64)> = None;
    for candidate in candidates {
        let Some(window) = input.quality.get(candidate) else {
            continue;
        };
        if !window.has_min_samples(input.config.min_samples) {
            continue;
        }
        let score = window.quality_score_bps_rust();
        if best.is_none_or(|(_, best_score)| score > best_score) {
            best = Some((candidate.as_str(), score));
        }
    }

    let Some((model, score)) = best else {
        return RuntimeGatewayAdaptiveShadowDecision {
            actual_model: input.actual_model,
            recommended_model: None,
            quality_score_bps: None,
            override_reason: "insufficient_samples",
        };
    };

    RuntimeGatewayAdaptiveShadowDecision {
        actual_model: input.actual_model,
        recommended_model: Some(model.to_string()),
        quality_score_bps: Some(score),
        override_reason: if input.config.shadow_mode {
            "shadow_only"
        } else {
            "adaptive_enabled"
        },
    }
}

#[cfg(any(not(feature = "mojo"), test))]
fn adaptive_exploration_decision(
    input: &RuntimeGatewayAdaptiveShadowInput,
    candidates: &[&String],
) -> Option<RuntimeGatewayAdaptiveShadowDecision> {
    if input.config.exploration_rate_bps == 0
        || candidates.is_empty()
        || adaptive_seed(input.diagnostic_seed) % 10_000
            >= u64::from(input.config.exploration_rate_bps)
    {
        return None;
    }
    let mut index =
        adaptive_seed(input.diagnostic_seed ^ 0x9e37_79b9_7f4a_7c15) as usize % candidates.len();
    if candidates.len() > 1 && candidates[index].as_str() == input.actual_model {
        index = (index + 1) % candidates.len();
    }
    Some(RuntimeGatewayAdaptiveShadowDecision {
        actual_model: input.actual_model.clone(),
        recommended_model: Some(candidates[index].to_string()),
        quality_score_bps: None,
        override_reason: if input.config.shadow_mode {
            "shadow_exploration"
        } else {
            "adaptive_exploration"
        },
    })
}

#[cfg(any(not(feature = "mojo"), test))]
fn adaptive_seed(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adaptive_quality_window_scores_owner_feedback() {
        let mut good = RuntimeGatewayAdaptiveQualityWindow::default();
        good.record(RuntimeGatewayAdaptiveFeedbackSignal::TaskCompleted);
        good.record(RuntimeGatewayAdaptiveFeedbackSignal::TokenSavings(50_000));
        good.record(RuntimeGatewayAdaptiveFeedbackSignal::LatencyMs(250));

        let mut poor = RuntimeGatewayAdaptiveQualityWindow::default();
        poor.record(RuntimeGatewayAdaptiveFeedbackSignal::CorrectiveUserMessage);
        poor.record(RuntimeGatewayAdaptiveFeedbackSignal::PreviousResponseNotFound);
        poor.record(RuntimeGatewayAdaptiveFeedbackSignal::InvalidToolCallContinuation);

        assert!(good.quality_score_bps() > 0);
        assert!(poor.quality_score_bps() < good.quality_score_bps());
    }

    #[test]
    fn adaptive_feedback_queue_is_bounded_and_drainable() {
        let mut queue = RuntimeGatewayAdaptiveFeedbackQueue::new(2);
        for owner_model in ["a", "b", "c"] {
            queue.push(RuntimeGatewayAdaptiveFeedbackEvent {
                owner_model: owner_model.to_string(),
                owner_provider: Some("openai".to_string()),
                owner_account: Some("acct".to_string()),
                signal: RuntimeGatewayAdaptiveFeedbackSignal::TaskCompleted,
            });
        }

        assert_eq!(queue.len(), 2);
        assert_eq!(queue.dropped_events(), 1);
        let events = queue.drain();
        assert_eq!(
            events
                .iter()
                .map(|event| event.owner_model.as_str())
                .collect::<Vec<_>>(),
            vec!["b", "c"]
        );
        assert_eq!(queue.len(), 0);
    }

    #[test]
    fn adaptive_shadow_recommends_best_sampled_candidate_without_changing_actual() {
        let mut fast = RuntimeGatewayAdaptiveQualityWindow::default();
        for _ in 0..8 {
            fast.record(RuntimeGatewayAdaptiveFeedbackSignal::TaskCompleted);
            fast.record(RuntimeGatewayAdaptiveFeedbackSignal::TokenSavings(10_000));
        }
        let mut slow = RuntimeGatewayAdaptiveQualityWindow::default();
        for _ in 0..8 {
            slow.record(RuntimeGatewayAdaptiveFeedbackSignal::AdditionalTurn);
            slow.record(RuntimeGatewayAdaptiveFeedbackSignal::LatencyMs(2_000));
        }

        let decision =
            runtime_gateway_adaptive_shadow_decision(RuntimeGatewayAdaptiveShadowInput {
                config: RuntimeGatewayAdaptiveRoutingConfig {
                    enabled: true,
                    shadow_mode: true,
                    min_samples: 8,
                    ..RuntimeGatewayAdaptiveRoutingConfig::default()
                },
                diagnostic_seed: 1,
                actual_model: "slow".to_string(),
                continuation_owner_model: None,
                candidates: vec!["slow".to_string(), "fast".to_string()],
                quota_blocked_models: Vec::new(),
                quality: BTreeMap::from([("fast".to_string(), fast), ("slow".to_string(), slow)]),
            });

        assert_eq!(decision.actual_model, "slow");
        assert_eq!(decision.recommended_model.as_deref(), Some("fast"));
        assert_eq!(decision.override_reason, "shadow_only");
        assert!(decision.quality_score_bps.unwrap_or_default() > 0);
    }

    #[test]
    fn adaptive_shadow_never_overrides_continuation_affinity() {
        let decision =
            runtime_gateway_adaptive_shadow_decision(RuntimeGatewayAdaptiveShadowInput {
                config: RuntimeGatewayAdaptiveRoutingConfig {
                    enabled: true,
                    shadow_mode: false,
                    ..RuntimeGatewayAdaptiveRoutingConfig::default()
                },
                diagnostic_seed: 1,
                actual_model: "actual".to_string(),
                continuation_owner_model: Some("owner".to_string()),
                candidates: vec!["other".to_string()],
                quota_blocked_models: Vec::new(),
                quality: BTreeMap::new(),
            });

        assert_eq!(decision.actual_model, "actual");
        assert_eq!(decision.recommended_model.as_deref(), Some("owner"));
        assert_eq!(decision.override_reason, "continuation_affinity");
    }

    #[test]
    fn adaptive_exploration_is_deterministic_and_can_route_unsampled_candidates() {
        let decision =
            runtime_gateway_adaptive_shadow_decision(RuntimeGatewayAdaptiveShadowInput {
                config: RuntimeGatewayAdaptiveRoutingConfig {
                    enabled: true,
                    shadow_mode: false,
                    exploration_rate_bps: 10_000,
                    ..RuntimeGatewayAdaptiveRoutingConfig::default()
                },
                diagnostic_seed: 7,
                actual_model: "a".to_string(),
                continuation_owner_model: None,
                candidates: vec!["a".to_string(), "b".to_string()],
                quota_blocked_models: Vec::new(),
                quality: BTreeMap::new(),
            });

        assert_eq!(decision.recommended_model.as_deref(), Some("b"));
        assert_eq!(decision.override_reason, "adaptive_exploration");
    }

    #[cfg(feature = "mojo")]
    #[test]
    fn adaptive_mojo_matches_rust_oracle_for_sampled_selection() {
        let mut good = RuntimeGatewayAdaptiveQualityWindow::default();
        for _ in 0..8 {
            good.record(RuntimeGatewayAdaptiveFeedbackSignal::TaskCompleted);
            good.record(RuntimeGatewayAdaptiveFeedbackSignal::TokenSavings(10_000));
        }
        let input = RuntimeGatewayAdaptiveShadowInput {
            config: RuntimeGatewayAdaptiveRoutingConfig {
                enabled: true,
                shadow_mode: false,
                min_samples: 8,
                ..RuntimeGatewayAdaptiveRoutingConfig::default()
            },
            diagnostic_seed: 1,
            actual_model: "slow".to_string(),
            continuation_owner_model: None,
            candidates: vec!["slow".to_string(), "good".to_string()],
            quota_blocked_models: Vec::new(),
            quality: BTreeMap::from([("good".to_string(), good)]),
        };

        assert_eq!(
            runtime_gateway_adaptive_shadow_decision(input.clone()),
            runtime_gateway_adaptive_shadow_decision_rust(input),
        );
    }
}
