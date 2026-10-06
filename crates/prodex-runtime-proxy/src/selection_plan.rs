use std::cmp::Reverse;
use std::collections::BTreeSet;

use crate::runtime_route_reason_kind_from_tag;
use crate::{
    RuntimeRouteDecisionReasonKind, RuntimeRouteKind, RuntimeSelectionQuotaPressureBand,
    RuntimeSelectionQuotaSource, RuntimeSelectionQuotaSummary,
};

#[path = "selection_prompt_cache_mojo.rs"]
mod prompt_cache;
pub use prompt_cache::{
    runtime_prompt_cache_affinity_batch, runtime_prompt_cache_affinity_sort_key,
    runtime_prompt_cache_affinity_sort_key_with_owner,
};

pub type RuntimeResponseBackoffSortKey = (usize, i64, i64, i64);
pub type RuntimeResponseQuotaPressureSortKey = (
    u8,
    i64,
    i64,
    i64,
    Reverse<i64>,
    Reverse<i64>,
    Reverse<i64>,
    i64,
    i64,
);

/// The selection-visible state of a profile for one route.
///
/// Quota pressure is intentionally not a state here: positive quota remains available and is
/// ordered by pressure. Only authoritative exhaustion is hard-unusable.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeProfileAvailabilityState {
    Ready,
    QuotaExhausted,
    TransientBackoff,
    AuthInvalid,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct RuntimeResponseCandidatePlanInput {
    pub name: String,
    pub order_index: usize,
    pub inflight_count: usize,
    pub health_sort_key: u32,
    pub backoff_sort_key: RuntimeResponseBackoffSortKey,
    pub quota_source: RuntimeSelectionQuotaSource,
    pub quota_summary: RuntimeSelectionQuotaSummary,
    pub auth_failure_active: bool,
    pub provider_priority: usize,
    pub quota_sort_key: RuntimeResponseQuotaPressureSortKey,
    pub in_selection_backoff: bool,
    pub jitter: u64,
}

#[derive(Debug, Clone, Copy)]
pub struct RuntimeResponseCandidatePlanOptions<'a> {
    pub route_kind: RuntimeRouteKind,
    pub inflight_soft_limit: usize,
    pub prompt_cache_key: Option<&'a str>,
    pub prompt_cache_owner_profile: Option<&'a str>,
    pub responses_critical_floor_percent: i64,
}

pub fn runtime_response_candidate_plan_options<'a>(
    route_kind: RuntimeRouteKind,
    inflight_soft_limit: usize,
    prompt_cache_key: Option<&'a str>,
    prompt_cache_owner_profile: Option<&'a str>,
    responses_critical_floor_percent: i64,
) -> RuntimeResponseCandidatePlanOptions<'a> {
    RuntimeResponseCandidatePlanOptions {
        route_kind,
        inflight_soft_limit,
        prompt_cache_key,
        prompt_cache_owner_profile,
        responses_critical_floor_percent,
    }
}

#[derive(Debug, Clone)]
pub struct RuntimeResponseCandidateExecutionPlan {
    pub ready_candidates: Vec<RuntimeResponsePlannedCandidate>,
    pub fallback_candidates: Vec<RuntimeResponsePlannedCandidate>,
}

#[derive(Debug, Clone)]
pub struct RuntimeResponsePlannedCandidate {
    pub name: String,
    pub order_index: usize,
    pub inflight_count: usize,
    pub inflight_soft_limit: usize,
    pub health_sort_key: u32,
    pub backoff_sort_key: RuntimeResponseBackoffSortKey,
    pub quota_source: RuntimeSelectionQuotaSource,
    pub quota_summary: RuntimeSelectionQuotaSummary,
    pub auth_failure_active: bool,
    pub quota_guard_reason: Option<&'static str>,
    pub ready_skip_reason: Option<&'static str>,
    pub fallback_skip_reason: Option<&'static str>,
    pub inflight_soft_limited: bool,
    pub provider_priority: usize,
    pub quota_sort_key: RuntimeResponseQuotaPressureSortKey,
    pub in_selection_backoff: bool,
    pub availability: RuntimeProfileAvailabilityState,
    pub prompt_cache_affinity_sort_key: (u8, u64),
    pub jitter: u64,
}

impl RuntimeResponsePlannedCandidate {
    pub fn ready_skip_reason(&self) -> Option<&'static str> {
        self.ready_skip_reason
    }

    pub fn fallback_skip_reason(&self) -> Option<&'static str> {
        self.fallback_skip_reason
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeOptimisticCurrentCandidateInput<'a> {
    pub current_profile: &'a str,
    pub route_kind: RuntimeRouteKind,
    pub auth_failure_active: bool,
    pub in_selection_backoff: bool,
    pub circuit_open: bool,
    pub health_score: u32,
    pub performance_score: u32,
    pub current_profile_quota_compatible: bool,
    pub has_alternative_quota_compatible_profile: bool,
    pub quota_summary: RuntimeSelectionQuotaSummary,
    pub quota_source: Option<RuntimeSelectionQuotaSource>,
    pub inflight_count: usize,
    pub inflight_soft_limit: usize,
    pub prompt_cache_key: Option<&'a str>,
    pub prompt_cache_owner_profile: Option<&'a str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeOptimisticCurrentCandidateDecision {
    Keep,
    Skip(RuntimeOptimisticCurrentCandidateSkip),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeOptimisticCurrentCandidateSkip {
    pub reason: RuntimeRouteDecisionReasonKind,
}

pub type RuntimeOptimisticCurrentCandidateSkipReason = RuntimeRouteDecisionReasonKind;

impl RuntimeOptimisticCurrentCandidateSkip {
    pub fn reason_label(self) -> &'static str {
        self.reason.as_str()
    }

    pub fn include_quota_fields(self) -> bool {
        prodex_mojo_core::runtime::optimistic_candidate_reason_include_quota(self.reason as u8)
            .expect("Mojo optimistic candidate quota-field policy returned invalid output")
    }
}

pub fn runtime_optimistic_current_candidate_decision(
    input: RuntimeOptimisticCurrentCandidateInput<'_>,
) -> RuntimeOptimisticCurrentCandidateDecision {
    optimistic_current_candidate_decision_mojo(input)
        .expect("Mojo optimistic candidate decision returned an invalid tag")
}

fn optimistic_current_candidate_decision_mojo(
    input: RuntimeOptimisticCurrentCandidateInput<'_>,
) -> Result<RuntimeOptimisticCurrentCandidateDecision, prodex_mojo_core::MojoError> {
    let prompt_cache_present = prompt_cache_key_present(input.prompt_cache_key);
    let prompt_cache_owner_matches = input
        .prompt_cache_owner_profile
        .map(str::trim)
        .filter(|owner| !owner.is_empty())
        == Some(input.current_profile);
    let quota_source = input.quota_source.map(|source| match source {
        RuntimeSelectionQuotaSource::LiveProbe => 0,
        RuntimeSelectionQuotaSource::PersistedSnapshot => 1,
    });
    let result = prodex_mojo_core::runtime::optimistic_current_candidate_decision(
        prodex_mojo_core::runtime::OptimisticCandidateInput {
            route_kind: match input.route_kind {
                RuntimeRouteKind::Responses => 0,
                RuntimeRouteKind::Compact => 1,
                RuntimeRouteKind::Websocket => 2,
                RuntimeRouteKind::Standard => 3,
            },
            auth_failure_active: input.auth_failure_active,
            in_selection_backoff: input.in_selection_backoff,
            circuit_open: input.circuit_open,
            health_score: input.health_score,
            performance_score: input.performance_score,
            current_profile_quota_compatible: input.current_profile_quota_compatible,
            has_alternative_quota_compatible_profile: input
                .has_alternative_quota_compatible_profile,
            quota_band: match input.quota_summary.route_band {
                RuntimeSelectionQuotaPressureBand::Healthy => 0,
                RuntimeSelectionQuotaPressureBand::Thin => 1,
                RuntimeSelectionQuotaPressureBand::Critical => 2,
                RuntimeSelectionQuotaPressureBand::Exhausted => 3,
                RuntimeSelectionQuotaPressureBand::Unknown => 4,
            },
            quota_source,
            inflight_count: input.inflight_count,
            inflight_soft_limit: input.inflight_soft_limit,
            prompt_cache_present,
            prompt_cache_owner_matches,
        },
    )?;
    if result.keep {
        return Ok(RuntimeOptimisticCurrentCandidateDecision::Keep);
    }
    let reason = result
        .reason_kind
        .and_then(runtime_route_reason_kind_from_tag)
        .ok_or(prodex_mojo_core::MojoError::InvalidOutput)?;
    Ok(optimistic_skip(reason))
}

fn optimistic_skip(
    reason: RuntimeRouteDecisionReasonKind,
) -> RuntimeOptimisticCurrentCandidateDecision {
    RuntimeOptimisticCurrentCandidateDecision::Skip(RuntimeOptimisticCurrentCandidateSkip {
        reason,
    })
}

fn prompt_cache_key_present(prompt_cache_key: Option<&str>) -> bool {
    prompt_cache_key
        .map(str::trim)
        .is_some_and(|prompt_cache_key| !prompt_cache_key.is_empty())
}

fn mojo_candidate_availability(tag: i64) -> RuntimeProfileAvailabilityState {
    match tag {
        prodex_mojo_core::runtime::RUNTIME_CANDIDATE_AVAILABILITY_READY => {
            RuntimeProfileAvailabilityState::Ready
        }
        prodex_mojo_core::runtime::RUNTIME_CANDIDATE_AVAILABILITY_QUOTA_EXHAUSTED => {
            RuntimeProfileAvailabilityState::QuotaExhausted
        }
        prodex_mojo_core::runtime::RUNTIME_CANDIDATE_AVAILABILITY_TRANSIENT_BACKOFF => {
            RuntimeProfileAvailabilityState::TransientBackoff
        }
        prodex_mojo_core::runtime::RUNTIME_CANDIDATE_AVAILABILITY_AUTH_INVALID => {
            RuntimeProfileAvailabilityState::AuthInvalid
        }
        prodex_mojo_core::runtime::RUNTIME_CANDIDATE_AVAILABILITY_UNKNOWN => {
            RuntimeProfileAvailabilityState::Unknown
        }
        _ => panic!("validated Mojo candidate availability tag is out of range"),
    }
}

fn mojo_candidate_skip_reason(tag: i64) -> Option<&'static str> {
    let kind = prodex_mojo_core::runtime::candidate_skip_reason_kind(tag)
        .expect("Mojo candidate skip-reason mapping returned an invalid result")?;
    Some(
        prodex_mojo_core::runtime_route_reason::label(kind)
            .expect("Mojo candidate route-reason label returned invalid output"),
    )
}

pub fn build_runtime_response_candidate_execution_plan(
    candidates: Vec<RuntimeResponseCandidatePlanInput>,
    excluded_profiles: &BTreeSet<String>,
    options: RuntimeResponseCandidatePlanOptions<'_>,
) -> RuntimeResponseCandidateExecutionPlan {
    let available_inputs = candidates
        .into_iter()
        .filter(|candidate| !excluded_profiles.contains(&candidate.name))
        .collect::<Vec<_>>();
    let mojo_plan =
        crate::quota::mojo::runtime_response_candidate_plan_batch(&available_inputs, options)
            .expect("Mojo runtime candidate plan returned invalid indices");
    let mut mojo_decisions = mojo_plan.decisions.iter();
    let available_candidates = available_inputs
        .iter()
        .map(|candidate| {
            let decision = mojo_decisions
                .next()
                .expect("Mojo candidate decision count matches inputs");
            let quota_guard_reason = mojo_candidate_skip_reason(decision.quota_guard_reason);
            let ready_skip_reason = mojo_candidate_skip_reason(decision.ready_skip_reason);
            let fallback_skip_reason = mojo_candidate_skip_reason(decision.fallback_skip_reason);
            let availability = mojo_candidate_availability(decision.availability);
            RuntimeResponsePlannedCandidate {
                name: candidate.name.clone(),
                order_index: candidate.order_index,
                inflight_count: candidate.inflight_count,
                inflight_soft_limit: options.inflight_soft_limit,
                health_sort_key: candidate.health_sort_key,
                backoff_sort_key: candidate.backoff_sort_key,
                quota_source: candidate.quota_source,
                quota_summary: candidate.quota_summary,
                auth_failure_active: candidate.auth_failure_active,
                quota_guard_reason,
                ready_skip_reason,
                fallback_skip_reason,
                inflight_soft_limited: decision.inflight_soft_limited,
                provider_priority: candidate.provider_priority,
                quota_sort_key: candidate.quota_sort_key,
                in_selection_backoff: candidate.in_selection_backoff,
                availability,
                prompt_cache_affinity_sort_key: runtime_prompt_cache_affinity_sort_key_with_owner(
                    options.prompt_cache_key,
                    options.prompt_cache_owner_profile,
                    &candidate.name,
                ),
                jitter: candidate.jitter,
            }
        })
        .collect::<Vec<_>>();

    RuntimeResponseCandidateExecutionPlan {
        ready_candidates: mojo_plan
            .ready_indices
            .into_iter()
            .map(|index| available_candidates[index].clone())
            .collect(),
        fallback_candidates: mojo_plan
            .fallback_indices
            .into_iter()
            .map(|index| available_candidates[index].clone())
            .collect(),
    }
}

#[cfg(test)]
#[path = "../tests/src/selection_plan.rs"]
mod tests;
