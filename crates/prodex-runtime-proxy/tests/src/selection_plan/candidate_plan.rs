use super::*;

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
    )
    .expect("Mojo candidate plan should be valid");

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
    assert!(plan.ready_candidates[0].inflight_soft_limited);
    assert_eq!(
        plan.fallback_candidates
            .iter()
            .map(|candidate| candidate.name.as_str())
            .collect::<Vec<_>>(),
        vec!["second", "main"]
    );
    assert_eq!(plan.fallback_candidates[0].fallback_skip_reason(), None);
    assert!(!plan.fallback_candidates[0].inflight_soft_limited);
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
    )
    .expect("Mojo candidate plan should be valid");

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
    assert_eq!(plan.fallback_candidates[0].fallback_skip_reason(), None);
    assert!(
        plan.fallback_candidates[1..]
            .iter()
            .all(|candidate| candidate.fallback_skip_reason().is_none())
    );
}

#[test]
fn candidate_plan_reports_auth_quota_backoff_and_unknown_availability() {
    let mut exhausted_quota = healthy_quota_summary();
    exhausted_quota.five_hour.status = RuntimeSelectionQuotaWindowStatus::Exhausted;
    exhausted_quota.route_band = RuntimeSelectionQuotaPressureBand::Exhausted;
    let mut unknown_quota = healthy_quota_summary();
    unknown_quota.five_hour.status = RuntimeSelectionQuotaWindowStatus::Unknown;
    unknown_quota.route_band = RuntimeSelectionQuotaPressureBand::Unknown;
    let plan = build_runtime_response_candidate_execution_plan(
        vec![
            candidate(
                "auth",
                CandidateFixture {
                    auth_failure_active: true,
                    ..CandidateFixture::default()
                },
            ),
            candidate(
                "quota",
                CandidateFixture {
                    quota_summary: exhausted_quota,
                    ..CandidateFixture::default()
                },
            ),
            candidate(
                "backoff",
                CandidateFixture {
                    in_selection_backoff: true,
                    ..CandidateFixture::default()
                },
            ),
            candidate(
                "unknown",
                CandidateFixture {
                    quota_summary: unknown_quota,
                    ..CandidateFixture::default()
                },
            ),
        ],
        &BTreeSet::new(),
        runtime_response_candidate_plan_options(RuntimeRouteKind::Responses, 3, None, None, 2),
    )
    .expect("Mojo candidate plan should be valid");

    assert_eq!(
        plan.fallback_candidates
            .iter()
            .map(|candidate| (candidate.name.as_str(), candidate.availability))
            .collect::<Vec<_>>(),
        vec![
            ("auth", RuntimeProfileAvailabilityState::AuthInvalid),
            ("quota", RuntimeProfileAvailabilityState::QuotaExhausted),
            ("backoff", RuntimeProfileAvailabilityState::TransientBackoff),
            ("unknown", RuntimeProfileAvailabilityState::Unknown),
        ]
    );
    assert_eq!(
        plan.fallback_candidates
            .iter()
            .map(RuntimeResponsePlannedCandidate::fallback_skip_reason)
            .collect::<Vec<_>>(),
        vec![
            Some("auth_failure_backoff"),
            Some("quota_exhausted_before_send"),
            None,
            None,
        ]
    );
}

#[test]
fn candidate_plan_exhausts_five_hour_quota_for_every_route() {
    let mut exhausted_quota = healthy_quota_summary();
    exhausted_quota.five_hour.status = RuntimeSelectionQuotaWindowStatus::Exhausted;
    exhausted_quota.route_band = RuntimeSelectionQuotaPressureBand::Exhausted;

    for route_kind in [
        RuntimeRouteKind::Responses,
        RuntimeRouteKind::Websocket,
        RuntimeRouteKind::Compact,
        RuntimeRouteKind::Standard,
    ] {
        let plan = build_runtime_response_candidate_execution_plan(
            vec![candidate(
                "exhausted",
                CandidateFixture {
                    quota_summary: exhausted_quota,
                    ..CandidateFixture::default()
                },
            )],
            &BTreeSet::new(),
            runtime_response_candidate_plan_options(route_kind, 3, None, None, 2),
        )
        .expect("Mojo candidate plan should be valid");

        assert_eq!(
            plan.fallback_candidates[0].availability,
            RuntimeProfileAvailabilityState::QuotaExhausted,
            "route {route_kind:?}"
        );
        assert_eq!(
            plan.fallback_candidates[0].fallback_skip_reason(),
            Some("quota_exhausted_before_send"),
            "route {route_kind:?}"
        );
    }
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
    )
    .expect("Mojo candidate plan should be valid");

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
fn candidate_plan_uses_route_specific_quota_source_order() {
    for (route_kind, expected) in [
        (RuntimeRouteKind::Responses, vec!["live", "persisted"]),
        (RuntimeRouteKind::Websocket, vec!["live", "persisted"]),
        (RuntimeRouteKind::Compact, vec!["persisted", "live"]),
        (RuntimeRouteKind::Standard, vec!["persisted", "live"]),
    ] {
        let plan = build_runtime_response_candidate_execution_plan(
            vec![
                candidate(
                    "persisted",
                    CandidateFixture {
                        order_index: 0,
                        quota_source: RuntimeSelectionQuotaSource::PersistedSnapshot,
                        ..CandidateFixture::default()
                    },
                ),
                candidate(
                    "live",
                    CandidateFixture {
                        order_index: 1,
                        ..CandidateFixture::default()
                    },
                ),
            ],
            &BTreeSet::new(),
            runtime_response_candidate_plan_options(route_kind, 3, None, None, 2),
        )
        .expect("Mojo candidate plan should be valid");

        assert_eq!(
            plan.ready_candidates
                .iter()
                .map(|candidate| candidate.name.as_str())
                .collect::<Vec<_>>(),
            expected,
            "ready route {route_kind:?}"
        );
        assert_eq!(
            plan.fallback_candidates
                .iter()
                .map(|candidate| candidate.name.as_str())
                .collect::<Vec<_>>(),
            expected,
            "fallback route {route_kind:?}"
        );
    }
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
    )
    .expect("Mojo candidate plan should be valid");

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
    )
    .expect("Mojo candidate plan should be valid");

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
    )
    .expect("Mojo candidate plan should be valid");

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
    )
    .expect("Mojo candidate plan should be valid");

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
