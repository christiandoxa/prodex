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
        RuntimeOptimisticCurrentCandidateDecision::Skip(RuntimeOptimisticCurrentCandidateSkip {
            reason: RuntimeOptimisticCurrentCandidateSkipReason::StalePersistedQuota,
        },)
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
        RuntimeOptimisticCurrentCandidateDecision::Skip(RuntimeOptimisticCurrentCandidateSkip {
            reason: RuntimeOptimisticCurrentCandidateSkipReason::AuthFailureBackoff,
        },)
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
        RuntimeOptimisticCurrentCandidateDecision::Skip(RuntimeOptimisticCurrentCandidateSkip {
            reason: RuntimeOptimisticCurrentCandidateSkipReason::PromptCacheAffinity,
        },)
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
fn candidate_plan_fallback_keeps_full_non_excluded_pool_despite_fresh_penalties() {
    let excluded_profiles = BTreeSet::from(["visited".to_string()]);
    let plan = build_runtime_response_candidate_execution_plan(
        vec![
            candidate(
                "backoff",
                CandidateFixture {
                    order_index: 0,
                    in_selection_backoff: true,
                    backoff_sort_key: (0, 0, 0, 0),
                    ..CandidateFixture::default()
                },
            ),
            candidate(
                "healthy",
                CandidateFixture {
                    order_index: 1,
                    backoff_sort_key: (1, 0, 0, 0),
                    ..CandidateFixture::default()
                },
            ),
            candidate(
                "unhealthy",
                CandidateFixture {
                    order_index: 2,
                    health_sort_key: 10,
                    backoff_sort_key: (2, 0, 0, 0),
                    ..CandidateFixture::default()
                },
            ),
            candidate(
                "busy",
                CandidateFixture {
                    order_index: 3,
                    inflight_count: 3,
                    backoff_sort_key: (3, 0, 0, 0),
                    ..CandidateFixture::default()
                },
            ),
            candidate(
                "visited",
                CandidateFixture {
                    order_index: 4,
                    backoff_sort_key: (4, 0, 0, 0),
                    ..CandidateFixture::default()
                },
            ),
        ],
        &excluded_profiles,
        runtime_response_candidate_plan_options(RuntimeRouteKind::Responses, 3, None, None, 2),
    );

    assert_eq!(
        plan.ready_candidates
            .iter()
            .map(|candidate| candidate.name.as_str())
            .collect::<Vec<_>>(),
        vec!["healthy", "unhealthy", "busy"]
    );
    assert_eq!(
        plan.ready_candidates[2].ready_skip_reason(),
        Some("profile_inflight_soft_limit")
    );
    assert_eq!(
        plan.fallback_candidates
            .iter()
            .map(|candidate| candidate.name.as_str())
            .collect::<Vec<_>>(),
        vec!["backoff", "healthy", "unhealthy", "busy"]
    );
    assert!(
        plan.fallback_candidates
            .iter()
            .all(|candidate| candidate.fallback_skip_reason().is_none())
    );
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
    assert_eq!(plan.ready_candidates[2].ready_skip_reason(), None);
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
    assert_eq!(plan.fallback_candidates[3].fallback_skip_reason(), None);
}

#[derive(Clone, Copy)]
struct CandidateFixture {
    order_index: usize,
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
            order_index: 0,
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
        has_alternative_quota_compatible_profile: fixture.has_alternative_quota_compatible_profile,
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
        order_index: fixture.order_index,
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
