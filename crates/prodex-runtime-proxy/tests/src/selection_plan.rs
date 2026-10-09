use super::*;
use crate::{
    RuntimeSelectionQuotaPressureBand, RuntimeSelectionQuotaWindowStatus,
    RuntimeSelectionQuotaWindowSummary,
};

#[path = "selection_plan/large_pool.rs"]
mod large_pool;

#[test]
fn prompt_cache_affinity_preserves_trimmed_key_hash_and_owner_priority() {
    let profiles = ["alpha", "beta", "third"];
    let expected = [
        (0, 11_164_400_351_952_061_751),
        (0, 7_149_616_626_519_032_185),
        (0, 12_567_574_820_160_767_546),
    ];
    assert_eq!(
        runtime_prompt_cache_affinity_batch(Some("workspace-cache"), None, &profiles).unwrap(),
        expected
    );
    assert_eq!(
        runtime_prompt_cache_affinity_batch(
            Some(" \u{2003}workspace-cache\u{2003} "),
            Some("\u{2003} beta \u{00a0}"),
            &profiles,
        )
        .unwrap(),
        [(1, expected[0].1), (0, 0), (1, expected[2].1)]
    );
    let long_key = format!("{}workspace-cache", " ".repeat(4_100));
    assert_eq!(
        runtime_prompt_cache_affinity_batch(Some(&long_key), None, &["alpha"]).unwrap(),
        [expected[0]]
    );
    let many_profiles = vec!["alpha"; 257];
    assert_eq!(
        runtime_prompt_cache_affinity_batch(Some("workspace-cache"), None, &many_profiles).unwrap(),
        vec![expected[0]; many_profiles.len()]
    );
}

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
fn optimistic_current_candidate_quota_source_requirement_is_route_specific() {
    for (route_kind, expected) in [
        (
            RuntimeRouteKind::Responses,
            RuntimeOptimisticCurrentCandidateDecision::Skip(
                RuntimeOptimisticCurrentCandidateSkip {
                    reason: RuntimeOptimisticCurrentCandidateSkipReason::StalePersistedQuota,
                },
            ),
        ),
        (
            RuntimeRouteKind::Websocket,
            RuntimeOptimisticCurrentCandidateDecision::Skip(
                RuntimeOptimisticCurrentCandidateSkip {
                    reason: RuntimeOptimisticCurrentCandidateSkipReason::StalePersistedQuota,
                },
            ),
        ),
        (
            RuntimeRouteKind::Compact,
            RuntimeOptimisticCurrentCandidateDecision::Keep,
        ),
        (
            RuntimeRouteKind::Standard,
            RuntimeOptimisticCurrentCandidateDecision::Keep,
        ),
    ] {
        let decision = runtime_optimistic_current_candidate_decision(optimistic_current_input(
            OptimisticCurrentFixture {
                route_kind,
                quota_source: Some(RuntimeSelectionQuotaSource::PersistedSnapshot),
                has_alternative_quota_compatible_profile: true,
                ..OptimisticCurrentFixture::default()
            },
        ));

        assert_eq!(decision, expected, "route {route_kind:?}");
    }
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

#[path = "selection_plan/candidate_plan.rs"]
mod candidate_plan;
