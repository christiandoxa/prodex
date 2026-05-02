use std::cmp::Reverse;
use std::collections::BTreeSet;

use crate::{
    RuntimeRouteKind, RuntimeSelectionQuotaPressureBand, RuntimeSelectionQuotaSource,
    RuntimeSelectionQuotaSummary, runtime_quota_precommit_guard_reason,
    runtime_selection_quota_pressure_band_reason,
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
    runtime_response_candidate_common_skip_reason(auth_failure_active, quota_guard_reason)
        .or_else(|| inflight_soft_limited.then_some("profile_inflight_soft_limit"))
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
        Some("auth_failure_backoff")
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
            Self::AuthFailureBackoff => "auth_failure_backoff",
            Self::SelectionBackoff => "selection_backoff",
            Self::RouteCircuitOpen => "route_circuit_open",
            Self::ProfileHealth => "profile_health",
            Self::ProfilePerformance => "profile_performance",
            Self::QuotaProbeUnavailable => "quota_probe_unavailable",
            Self::StalePersistedQuota => "stale_persisted_quota",
            Self::QuotaPressureBand(band) => runtime_selection_quota_pressure_band_reason(band),
            Self::ProfileInflightSoftLimit => "profile_inflight_soft_limit",
            Self::AuthNotQuotaCompatible => "auth_not_quota_compatible",
            Self::PromptCacheAffinity => "prompt_cache_affinity",
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
mod tests {
    use super::*;
    use crate::{
        RuntimeSelectionQuotaPressureBand, RuntimeSelectionQuotaWindowStatus,
        RuntimeSelectionQuotaWindowSummary,
    };

    #[test]
    fn optimistic_current_candidate_keeps_unknown_quota_without_pool_fallback() {
        let decision = runtime_optimistic_current_candidate_decision(optimistic_current_input(
            OptimisticCurrentFixture {
                quota_summary: RuntimeSelectionQuotaSummary {
                    route_band: RuntimeSelectionQuotaPressureBand::Unknown,
                    ..healthy_quota_summary()
                },
                quota_source: None,
                has_alternative_quota_compatible_profile: false,
                ..OptimisticCurrentFixture::default()
            },
        ));

        assert_eq!(decision, RuntimeOptimisticCurrentCandidateDecision::Keep);
    }

    #[test]
    fn optimistic_current_candidate_requires_live_quota_when_pool_fallback_exists() {
        let decision = runtime_optimistic_current_candidate_decision(optimistic_current_input(
            OptimisticCurrentFixture {
                quota_source: Some(RuntimeSelectionQuotaSource::PersistedSnapshot),
                has_alternative_quota_compatible_profile: true,
                ..OptimisticCurrentFixture::default()
            },
        ));

        assert_eq!(
            decision,
            RuntimeOptimisticCurrentCandidateDecision::Skip(
                RuntimeOptimisticCurrentCandidateSkip {
                    reason: RuntimeOptimisticCurrentCandidateSkipReason::StalePersistedQuota,
                },
            )
        );
    }

    #[test]
    fn optimistic_current_candidate_preserves_skip_reason_priority() {
        let decision = runtime_optimistic_current_candidate_decision(optimistic_current_input(
            OptimisticCurrentFixture {
                auth_failure_active: true,
                current_profile_quota_compatible: false,
                quota_summary: critical_quota_summary(),
                inflight_count: 3,
                ..OptimisticCurrentFixture::default()
            },
        ));

        assert_eq!(
            decision,
            RuntimeOptimisticCurrentCandidateDecision::Skip(
                RuntimeOptimisticCurrentCandidateSkip {
                    reason: RuntimeOptimisticCurrentCandidateSkipReason::AuthFailureBackoff,
                },
            )
        );
    }

    #[test]
    fn optimistic_current_candidate_defers_to_prompt_cache_owner_when_pool_available() {
        let decision = runtime_optimistic_current_candidate_decision(optimistic_current_input(
            OptimisticCurrentFixture {
                prompt_cache_key: Some("workspace-cache"),
                prompt_cache_owner_profile: Some("second"),
                has_alternative_quota_compatible_profile: true,
                ..OptimisticCurrentFixture::default()
            },
        ));

        assert_eq!(
            decision,
            RuntimeOptimisticCurrentCandidateDecision::Skip(
                RuntimeOptimisticCurrentCandidateSkip {
                    reason: RuntimeOptimisticCurrentCandidateSkipReason::PromptCacheAffinity,
                },
            )
        );
    }

    #[test]
    fn candidate_plan_separates_ready_and_fallback_attempts() {
        let plan = build_runtime_response_candidate_execution_plan(
            vec![
                candidate(
                    "main",
                    CandidateFixture {
                        inflight_count: 3,
                        health_sort_key: 2,
                        backoff_sort_key: (2, 0, 0, 0),
                        ..CandidateFixture::default()
                    },
                ),
                candidate(
                    "second",
                    CandidateFixture {
                        in_selection_backoff: true,
                        backoff_sort_key: (1, 0, 0, 0),
                        ..CandidateFixture::default()
                    },
                ),
            ],
            &BTreeSet::new(),
            runtime_response_candidate_plan_options(RuntimeRouteKind::Responses, 3, None, None, 2),
        );

        assert_eq!(
            plan.ready_candidates
                .iter()
                .map(|candidate| candidate.name.as_str())
                .collect::<Vec<_>>(),
            vec!["main"]
        );
        assert_eq!(
            plan.ready_candidates[0].ready_skip_reason(),
            Some("profile_inflight_soft_limit")
        );
        assert_eq!(
            plan.fallback_candidates
                .iter()
                .map(|candidate| candidate.name.as_str())
                .collect::<Vec<_>>(),
            vec!["second", "main"]
        );
        assert_eq!(plan.fallback_candidates[0].fallback_skip_reason(), None);
    }

    #[test]
    fn candidate_plan_orders_ready_candidates_by_execution_priority() {
        let healthy_quota = healthy_quota_sort_key();
        let plan = build_runtime_response_candidate_execution_plan(
            vec![
                candidate(
                    "snapshot_same_load",
                    CandidateFixture {
                        inflight_count: 1,
                        quota_source: RuntimeSelectionQuotaSource::PersistedSnapshot,
                        quota_sort_key: healthy_quota,
                        ..CandidateFixture::default()
                    },
                ),
                candidate(
                    "live_busy",
                    CandidateFixture {
                        inflight_count: 2,
                        quota_sort_key: healthy_quota,
                        ..CandidateFixture::default()
                    },
                ),
                candidate(
                    "live_idle_healthy",
                    CandidateFixture {
                        inflight_count: 1,
                        quota_sort_key: healthy_quota,
                        ..CandidateFixture::default()
                    },
                ),
                candidate(
                    "live_idle_unhealthy",
                    CandidateFixture {
                        inflight_count: 1,
                        health_sort_key: 5,
                        quota_sort_key: healthy_quota,
                        ..CandidateFixture::default()
                    },
                ),
                candidate(
                    "lower_priority_provider",
                    CandidateFixture {
                        provider_priority: 1,
                        quota_sort_key: healthy_quota,
                        ..CandidateFixture::default()
                    },
                ),
            ],
            &BTreeSet::new(),
            runtime_response_candidate_plan_options(RuntimeRouteKind::Responses, 3, None, None, 2),
        );

        assert_eq!(
            plan.ready_candidates
                .iter()
                .map(|candidate| candidate.name.as_str())
                .collect::<Vec<_>>(),
            vec![
                "live_idle_healthy",
                "live_idle_unhealthy",
                "live_busy",
                "snapshot_same_load",
                "lower_priority_provider",
            ]
        );
        assert!(
            plan.ready_candidates
                .iter()
                .all(|candidate| candidate.ready_skip_reason().is_none())
        );
    }

    #[test]
    fn candidate_plan_uses_prompt_cache_affinity_as_tie_breaker() {
        let prompt_cache_key = "workspace-cache:abc123";
        let plan = build_runtime_response_candidate_execution_plan(
            vec![
                candidate("main", CandidateFixture::default()),
                candidate("second", CandidateFixture::default()),
                candidate("third", CandidateFixture::default()),
            ],
            &BTreeSet::new(),
            runtime_response_candidate_plan_options(
                RuntimeRouteKind::Responses,
                3,
                Some(prompt_cache_key),
                None,
                2,
            ),
        );

        let mut expected = vec!["main", "second", "third"];
        expected.sort_by_key(|profile_name| {
            runtime_prompt_cache_affinity_sort_key(Some(prompt_cache_key), profile_name)
        });
        assert_eq!(
            plan.ready_candidates
                .iter()
                .map(|candidate| candidate.name.as_str())
                .collect::<Vec<_>>(),
            expected
        );
    }

    #[test]
    fn candidate_plan_prioritizes_prompt_cache_owner_profile() {
        let plan = build_runtime_response_candidate_execution_plan(
            vec![
                candidate("main", CandidateFixture::default()),
                candidate("second", CandidateFixture::default()),
            ],
            &BTreeSet::new(),
            runtime_response_candidate_plan_options(
                RuntimeRouteKind::Responses,
                3,
                Some("workspace-cache:owner"),
                Some("second"),
                2,
            ),
        );

        assert_eq!(
            plan.ready_candidates
                .iter()
                .map(|candidate| candidate.name.as_str())
                .collect::<Vec<_>>(),
            vec!["second", "main"]
        );
    }

    #[test]
    fn candidate_plan_keeps_health_ahead_of_prompt_cache_affinity() {
        let prompt_cache_key = "workspace-cache:health";
        let mut profiles = ["alpha", "beta"];
        profiles.sort_by_key(|profile_name| {
            runtime_prompt_cache_affinity_sort_key(Some(prompt_cache_key), profile_name)
        });
        let cache_preferred_profile = profiles[0];
        let healthy_profile = profiles[1];

        let plan = build_runtime_response_candidate_execution_plan(
            vec![
                candidate(
                    cache_preferred_profile,
                    CandidateFixture {
                        health_sort_key: 5,
                        ..CandidateFixture::default()
                    },
                ),
                candidate(healthy_profile, CandidateFixture::default()),
            ],
            &BTreeSet::new(),
            runtime_response_candidate_plan_options(
                RuntimeRouteKind::Responses,
                3,
                Some(prompt_cache_key),
                None,
                2,
            ),
        );

        assert_eq!(
            plan.ready_candidates
                .iter()
                .map(|candidate| candidate.name.as_str())
                .collect::<Vec<_>>(),
            vec![healthy_profile, cache_preferred_profile]
        );
    }

    #[test]
    fn candidate_plan_orders_fallback_candidates_and_reports_skip_reasons() {
        let plan = build_runtime_response_candidate_execution_plan(
            vec![
                candidate(
                    "quota",
                    CandidateFixture {
                        quota_summary: critical_quota_summary(),
                        quota_sort_key: critical_quota_sort_key(),
                        backoff_sort_key: (3, 0, 0, 0),
                        ..CandidateFixture::default()
                    },
                ),
                candidate(
                    "auth",
                    CandidateFixture {
                        auth_failure_active: true,
                        health_sort_key: 1,
                        backoff_sort_key: (2, 0, 0, 0),
                        ..CandidateFixture::default()
                    },
                ),
                candidate(
                    "backoff",
                    CandidateFixture {
                        in_selection_backoff: true,
                        backoff_sort_key: (0, 0, 0, 0),
                        ..CandidateFixture::default()
                    },
                ),
                candidate(
                    "fresh",
                    CandidateFixture {
                        backoff_sort_key: (1, 0, 0, 0),
                        ..CandidateFixture::default()
                    },
                ),
            ],
            &BTreeSet::new(),
            runtime_response_candidate_plan_options(RuntimeRouteKind::Responses, 3, None, None, 2),
        );

        assert_eq!(
            plan.ready_candidates
                .iter()
                .map(|candidate| candidate.name.as_str())
                .collect::<Vec<_>>(),
            vec!["fresh", "auth", "quota"]
        );
        assert_eq!(plan.ready_candidates[0].ready_skip_reason(), None);
        assert_eq!(
            plan.ready_candidates[1].ready_skip_reason(),
            Some("auth_failure_backoff")
        );
        assert_eq!(
            plan.ready_candidates[2].ready_skip_reason(),
            Some("quota_critical_floor_before_send")
        );
        assert_eq!(
            plan.fallback_candidates
                .iter()
                .map(|candidate| candidate.name.as_str())
                .collect::<Vec<_>>(),
            vec!["backoff", "fresh", "auth", "quota"]
        );
        assert_eq!(plan.fallback_candidates[0].fallback_skip_reason(), None);
        assert_eq!(plan.fallback_candidates[1].fallback_skip_reason(), None);
        assert_eq!(
            plan.fallback_candidates[2].fallback_skip_reason(),
            Some("auth_failure_backoff")
        );
        assert_eq!(
            plan.fallback_candidates[3].fallback_skip_reason(),
            Some("quota_critical_floor_before_send")
        );
    }

    #[derive(Clone, Copy)]
    struct CandidateFixture {
        inflight_count: usize,
        health_sort_key: u32,
        backoff_sort_key: RuntimeResponseBackoffSortKey,
        quota_source: RuntimeSelectionQuotaSource,
        quota_summary: RuntimeSelectionQuotaSummary,
        auth_failure_active: bool,
        provider_priority: usize,
        quota_sort_key: RuntimeResponseQuotaPressureSortKey,
        in_selection_backoff: bool,
        jitter: u64,
    }

    impl Default for CandidateFixture {
        fn default() -> Self {
            Self {
                inflight_count: 0,
                health_sort_key: 0,
                backoff_sort_key: (0, 0, 0, 0),
                quota_source: RuntimeSelectionQuotaSource::LiveProbe,
                quota_summary: healthy_quota_summary(),
                auth_failure_active: false,
                provider_priority: 0,
                quota_sort_key: healthy_quota_sort_key(),
                in_selection_backoff: false,
                jitter: 0,
            }
        }
    }

    #[derive(Clone, Copy)]
    struct OptimisticCurrentFixture<'a> {
        current_profile: &'a str,
        route_kind: RuntimeRouteKind,
        auth_failure_active: bool,
        in_selection_backoff: bool,
        circuit_open: bool,
        health_score: u32,
        performance_score: u32,
        current_profile_quota_compatible: bool,
        has_alternative_quota_compatible_profile: bool,
        quota_summary: RuntimeSelectionQuotaSummary,
        quota_source: Option<RuntimeSelectionQuotaSource>,
        inflight_count: usize,
        inflight_soft_limit: usize,
        prompt_cache_key: Option<&'a str>,
        prompt_cache_owner_profile: Option<&'a str>,
    }

    impl Default for OptimisticCurrentFixture<'_> {
        fn default() -> Self {
            Self {
                current_profile: "main",
                route_kind: RuntimeRouteKind::Responses,
                auth_failure_active: false,
                in_selection_backoff: false,
                circuit_open: false,
                health_score: 0,
                performance_score: 0,
                current_profile_quota_compatible: true,
                has_alternative_quota_compatible_profile: false,
                quota_summary: healthy_quota_summary(),
                quota_source: Some(RuntimeSelectionQuotaSource::LiveProbe),
                inflight_count: 0,
                inflight_soft_limit: 3,
                prompt_cache_key: None,
                prompt_cache_owner_profile: None,
            }
        }
    }

    fn optimistic_current_input(
        fixture: OptimisticCurrentFixture<'_>,
    ) -> RuntimeOptimisticCurrentCandidateInput<'_> {
        RuntimeOptimisticCurrentCandidateInput {
            current_profile: fixture.current_profile,
            route_kind: fixture.route_kind,
            auth_failure_active: fixture.auth_failure_active,
            in_selection_backoff: fixture.in_selection_backoff,
            circuit_open: fixture.circuit_open,
            health_score: fixture.health_score,
            performance_score: fixture.performance_score,
            current_profile_quota_compatible: fixture.current_profile_quota_compatible,
            has_alternative_quota_compatible_profile: fixture
                .has_alternative_quota_compatible_profile,
            quota_summary: fixture.quota_summary,
            quota_source: fixture.quota_source,
            inflight_count: fixture.inflight_count,
            inflight_soft_limit: fixture.inflight_soft_limit,
            prompt_cache_key: fixture.prompt_cache_key,
            prompt_cache_owner_profile: fixture.prompt_cache_owner_profile,
        }
    }

    fn candidate(name: &str, fixture: CandidateFixture) -> RuntimeResponseCandidatePlanInput {
        RuntimeResponseCandidatePlanInput {
            name: name.to_string(),
            order_index: 0,
            inflight_count: fixture.inflight_count,
            health_sort_key: fixture.health_sort_key,
            backoff_sort_key: fixture.backoff_sort_key,
            quota_source: fixture.quota_source,
            quota_summary: fixture.quota_summary,
            auth_failure_active: fixture.auth_failure_active,
            provider_priority: fixture.provider_priority,
            quota_sort_key: fixture.quota_sort_key,
            in_selection_backoff: fixture.in_selection_backoff,
            jitter: fixture.jitter,
        }
    }

    fn healthy_quota_summary() -> RuntimeSelectionQuotaSummary {
        RuntimeSelectionQuotaSummary {
            five_hour: RuntimeSelectionQuotaWindowSummary {
                status: RuntimeSelectionQuotaWindowStatus::Ready,
                remaining_percent: 80,
            },
            weekly: RuntimeSelectionQuotaWindowSummary {
                status: RuntimeSelectionQuotaWindowStatus::Ready,
                remaining_percent: 85,
            },
            route_band: RuntimeSelectionQuotaPressureBand::Healthy,
        }
    }

    fn critical_quota_summary() -> RuntimeSelectionQuotaSummary {
        RuntimeSelectionQuotaSummary {
            five_hour: RuntimeSelectionQuotaWindowSummary {
                status: RuntimeSelectionQuotaWindowStatus::Critical,
                remaining_percent: 2,
            },
            weekly: RuntimeSelectionQuotaWindowSummary {
                status: RuntimeSelectionQuotaWindowStatus::Ready,
                remaining_percent: 50,
            },
            route_band: RuntimeSelectionQuotaPressureBand::Critical,
        }
    }

    fn healthy_quota_sort_key() -> RuntimeResponseQuotaPressureSortKey {
        (
            0,
            0,
            0,
            0,
            Reverse(80),
            Reverse(85),
            Reverse(80),
            i64::MAX,
            i64::MAX,
        )
    }

    fn critical_quota_sort_key() -> RuntimeResponseQuotaPressureSortKey {
        (
            2,
            2,
            0,
            2,
            Reverse(2),
            Reverse(50),
            Reverse(2),
            i64::MAX,
            i64::MAX,
        )
    }
}
