use std::cmp::Reverse;
use std::collections::BTreeSet;

use crate::{
    RuntimeRouteDecisionReasonKind, RuntimeRouteKind, RuntimeSelectionQuotaPressureBand,
    RuntimeSelectionQuotaSource, RuntimeSelectionQuotaSummary,
    runtime_quota_precommit_guard_reason, runtime_selection_quota_pressure_band_reason,
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
    pub inflight_soft_limited: bool,
    pub provider_priority: usize,
    pub quota_sort_key: RuntimeResponseQuotaPressureSortKey,
    pub in_selection_backoff: bool,
    pub prompt_cache_affinity_sort_key: (u8, u64),
    pub jitter: u64,
}

impl RuntimeResponsePlannedCandidate {
    pub fn ready_skip_reason(&self) -> Option<&'static str> {
        runtime_response_candidate_ready_skip_reason(
            self.auth_failure_active,
            self.quota_guard_reason,
            self.inflight_soft_limited,
        )
    }

    pub fn fallback_skip_reason(&self) -> Option<&'static str> {
        runtime_response_candidate_fallback_skip_reason(
            self.auth_failure_active,
            self.quota_guard_reason,
        )
    }
}

pub fn runtime_response_candidate_ready_skip_reason(
    auth_failure_active: bool,
    quota_guard_reason: Option<&'static str>,
    inflight_soft_limited: bool,
) -> Option<&'static str> {
    runtime_response_candidate_common_skip_reason(auth_failure_active, quota_guard_reason).or_else(
        || {
            inflight_soft_limited
                .then_some(RuntimeRouteDecisionReasonKind::ProfileInflightSoftLimit.as_str())
        },
    )
}

pub fn runtime_response_candidate_fallback_skip_reason(
    auth_failure_active: bool,
    quota_guard_reason: Option<&'static str>,
) -> Option<&'static str> {
    runtime_response_candidate_common_skip_reason(auth_failure_active, quota_guard_reason)
}

pub fn runtime_response_candidate_common_skip_reason(
    auth_failure_active: bool,
    quota_guard_reason: Option<&'static str>,
) -> Option<&'static str> {
    if auth_failure_active {
        Some(RuntimeRouteDecisionReasonKind::AuthFailureBackoff.as_str())
    } else {
        quota_guard_reason
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
    pub reason: RuntimeOptimisticCurrentCandidateSkipReason,
}

impl RuntimeOptimisticCurrentCandidateSkip {
    pub fn reason_label(self) -> &'static str {
        self.reason.reason_label()
    }

    pub fn include_quota_fields(self) -> bool {
        self.reason.include_quota_fields()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeOptimisticCurrentCandidateSkipReason {
    AuthFailureBackoff,
    SelectionBackoff,
    RouteCircuitOpen,
    ProfileHealth,
    ProfilePerformance,
    QuotaProbeUnavailable,
    StalePersistedQuota,
    QuotaPressureBand(RuntimeSelectionQuotaPressureBand),
    ProfileInflightSoftLimit,
    AuthNotQuotaCompatible,
    PromptCacheAffinity,
}

impl RuntimeOptimisticCurrentCandidateSkipReason {
    pub fn reason_label(self) -> &'static str {
        match self {
            Self::AuthFailureBackoff => RuntimeRouteDecisionReasonKind::AuthFailureBackoff.as_str(),
            Self::SelectionBackoff => RuntimeRouteDecisionReasonKind::SelectionBackoff.as_str(),
            Self::RouteCircuitOpen => RuntimeRouteDecisionReasonKind::RouteCircuitOpen.as_str(),
            Self::ProfileHealth => RuntimeRouteDecisionReasonKind::ProfileHealth.as_str(),
            Self::ProfilePerformance => RuntimeRouteDecisionReasonKind::ProfilePerformance.as_str(),
            Self::QuotaProbeUnavailable => {
                RuntimeRouteDecisionReasonKind::QuotaProbeUnavailable.as_str()
            }
            Self::StalePersistedQuota => {
                RuntimeRouteDecisionReasonKind::StalePersistedQuota.as_str()
            }
            Self::QuotaPressureBand(band) => runtime_selection_quota_pressure_band_reason(band),
            Self::ProfileInflightSoftLimit => {
                RuntimeRouteDecisionReasonKind::ProfileInflightSoftLimit.as_str()
            }
            Self::AuthNotQuotaCompatible => {
                RuntimeRouteDecisionReasonKind::AuthNotQuotaCompatible.as_str()
            }
            Self::PromptCacheAffinity => {
                RuntimeRouteDecisionReasonKind::PromptCacheAffinity.as_str()
            }
        }
    }

    pub fn include_quota_fields(self) -> bool {
        !matches!(self, Self::AuthNotQuotaCompatible)
    }
}

pub fn runtime_optimistic_current_candidate_decision(
    input: RuntimeOptimisticCurrentCandidateInput<'_>,
) -> RuntimeOptimisticCurrentCandidateDecision {
    let quota_evidence_required =
        input.has_alternative_quota_compatible_profile && input.quota_source.is_none();
    let live_quota_probe_required = input.has_alternative_quota_compatible_profile
        && matches!(
            input.route_kind,
            RuntimeRouteKind::Responses | RuntimeRouteKind::Websocket
        )
        && !matches!(
            input.quota_source,
            Some(RuntimeSelectionQuotaSource::LiveProbe)
        );
    let unknown_quota_allowed = input.quota_summary.route_band
        == RuntimeSelectionQuotaPressureBand::Unknown
        && !input.has_alternative_quota_compatible_profile;
    let quota_band_blocks_current = input.quota_summary.route_band
        > RuntimeSelectionQuotaPressureBand::Healthy
        && !unknown_quota_allowed;

    let reason = if input.auth_failure_active {
        Some(RuntimeOptimisticCurrentCandidateSkipReason::AuthFailureBackoff)
    } else if input.in_selection_backoff {
        Some(RuntimeOptimisticCurrentCandidateSkipReason::SelectionBackoff)
    } else if input.circuit_open {
        Some(RuntimeOptimisticCurrentCandidateSkipReason::RouteCircuitOpen)
    } else if input.health_score > 0 {
        Some(RuntimeOptimisticCurrentCandidateSkipReason::ProfileHealth)
    } else if input.performance_score > 0 {
        Some(RuntimeOptimisticCurrentCandidateSkipReason::ProfilePerformance)
    } else if quota_evidence_required {
        Some(RuntimeOptimisticCurrentCandidateSkipReason::QuotaProbeUnavailable)
    } else if live_quota_probe_required {
        Some(
            if matches!(
                input.quota_source,
                Some(RuntimeSelectionQuotaSource::PersistedSnapshot)
            ) {
                RuntimeOptimisticCurrentCandidateSkipReason::StalePersistedQuota
            } else {
                RuntimeOptimisticCurrentCandidateSkipReason::QuotaProbeUnavailable
            },
        )
    } else if quota_band_blocks_current {
        Some(
            RuntimeOptimisticCurrentCandidateSkipReason::QuotaPressureBand(
                input.quota_summary.route_band,
            ),
        )
    } else if input.inflight_count >= input.inflight_soft_limit {
        Some(RuntimeOptimisticCurrentCandidateSkipReason::ProfileInflightSoftLimit)
    } else {
        None
    };
    if let Some(reason) = reason {
        return RuntimeOptimisticCurrentCandidateDecision::Skip(
            RuntimeOptimisticCurrentCandidateSkip { reason },
        );
    }

    if !input.current_profile_quota_compatible {
        return RuntimeOptimisticCurrentCandidateDecision::Skip(
            RuntimeOptimisticCurrentCandidateSkip {
                reason: RuntimeOptimisticCurrentCandidateSkipReason::AuthNotQuotaCompatible,
            },
        );
    }

    if prompt_cache_key_present(input.prompt_cache_key)
        && matches!(
            input.route_kind,
            RuntimeRouteKind::Responses | RuntimeRouteKind::Websocket
        )
        && input.has_alternative_quota_compatible_profile
        && input
            .prompt_cache_owner_profile
            .map(str::trim)
            .filter(|owner| !owner.is_empty())
            != Some(input.current_profile)
    {
        return RuntimeOptimisticCurrentCandidateDecision::Skip(
            RuntimeOptimisticCurrentCandidateSkip {
                reason: RuntimeOptimisticCurrentCandidateSkipReason::PromptCacheAffinity,
            },
        );
    }

    RuntimeOptimisticCurrentCandidateDecision::Keep
}

fn prompt_cache_key_present(prompt_cache_key: Option<&str>) -> bool {
    prompt_cache_key
        .map(str::trim)
        .is_some_and(|prompt_cache_key| !prompt_cache_key.is_empty())
}

pub fn build_runtime_response_candidate_execution_plan(
    candidates: Vec<RuntimeResponseCandidatePlanInput>,
    excluded_profiles: &BTreeSet<String>,
    options: RuntimeResponseCandidatePlanOptions<'_>,
) -> RuntimeResponseCandidateExecutionPlan {
    let available_candidates = candidates
        .into_iter()
        .filter(|candidate| !excluded_profiles.contains(&candidate.name))
        .map(|candidate| {
            let quota_guard_reason = matches!(
                options.route_kind,
                RuntimeRouteKind::Responses | RuntimeRouteKind::Websocket
            )
            .then(|| {
                runtime_quota_precommit_guard_reason(
                    candidate.quota_summary,
                    options.route_kind,
                    options.responses_critical_floor_percent,
                )
            })
            .flatten();
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
                inflight_soft_limited: candidate.inflight_count >= options.inflight_soft_limit,
                provider_priority: candidate.provider_priority,
                quota_sort_key: candidate.quota_sort_key,
                in_selection_backoff: candidate.in_selection_backoff,
                prompt_cache_affinity_sort_key: runtime_prompt_cache_affinity_sort_key_with_owner(
                    options.prompt_cache_key,
                    options.prompt_cache_owner_profile,
                    &candidate.name,
                ),
                jitter: candidate.jitter,
            }
        })
        .collect::<Vec<_>>();

    let mut ready_candidates = available_candidates
        .iter()
        .filter(|candidate| !candidate.in_selection_backoff)
        .cloned()
        .collect::<Vec<_>>();
    ready_candidates.sort_by_key(|candidate| {
        (
            candidate.provider_priority,
            candidate.quota_sort_key,
            runtime_response_quota_source_sort_key(options.route_kind, candidate.quota_source),
            candidate.inflight_count,
            candidate.health_sort_key,
            candidate.prompt_cache_affinity_sort_key,
            candidate.order_index,
            candidate.jitter,
        )
    });

    let mut fallback_candidates = available_candidates;
    fallback_candidates.sort_by_key(|candidate| {
        (
            candidate.backoff_sort_key,
            candidate.provider_priority,
            candidate.quota_sort_key,
            runtime_response_quota_source_sort_key(options.route_kind, candidate.quota_source),
            candidate.inflight_count,
            candidate.health_sort_key,
            candidate.prompt_cache_affinity_sort_key,
            candidate.order_index,
            candidate.jitter,
        )
    });

    RuntimeResponseCandidateExecutionPlan {
        ready_candidates,
        fallback_candidates,
    }
}

pub fn runtime_response_quota_source_sort_key(
    route_kind: RuntimeRouteKind,
    source: RuntimeSelectionQuotaSource,
) -> usize {
    match (route_kind, source) {
        (
            RuntimeRouteKind::Responses | RuntimeRouteKind::Websocket,
            RuntimeSelectionQuotaSource::LiveProbe,
        ) => 0,
        (
            RuntimeRouteKind::Responses | RuntimeRouteKind::Websocket,
            RuntimeSelectionQuotaSource::PersistedSnapshot,
        ) => 1,
        _ => 0,
    }
}

pub fn runtime_prompt_cache_affinity_sort_key_with_owner(
    prompt_cache_key: Option<&str>,
    prompt_cache_owner_profile: Option<&str>,
    profile_name: &str,
) -> (u8, u64) {
    if prompt_cache_key
        .map(str::trim)
        .is_none_or(|prompt_cache_key| prompt_cache_key.is_empty())
    {
        return (0, 0);
    }
    if let Some(owner) = prompt_cache_owner_profile
        .map(str::trim)
        .filter(|owner| !owner.is_empty())
    {
        return if owner == profile_name {
            (0, 0)
        } else {
            (
                1,
                runtime_prompt_cache_affinity_sort_key(prompt_cache_key, profile_name),
            )
        };
    }
    (
        0,
        runtime_prompt_cache_affinity_sort_key(prompt_cache_key, profile_name),
    )
}

pub fn runtime_prompt_cache_affinity_sort_key(
    prompt_cache_key: Option<&str>,
    profile_name: &str,
) -> u64 {
    let Some(prompt_cache_key) = prompt_cache_key
        .map(str::trim)
        .filter(|prompt_cache_key| !prompt_cache_key.is_empty())
    else {
        return 0;
    };

    u64::MAX - runtime_prompt_cache_affinity_score(prompt_cache_key, profile_name)
}

fn runtime_prompt_cache_affinity_score(prompt_cache_key: &str, profile_name: &str) -> u64 {
    const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;

    let mut hash = FNV_OFFSET_BASIS;
    for bytes in [
        b"prodex-prompt-cache-affinity-v1".as_slice(),
        b"\0".as_slice(),
        prompt_cache_key.as_bytes(),
        b"\0".as_slice(),
        profile_name.as_bytes(),
    ] {
        for byte in bytes {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(FNV_PRIME);
        }
    }
    hash
}

#[cfg(test)]
#[path = "../tests/src/selection_plan.rs"]
mod tests;
