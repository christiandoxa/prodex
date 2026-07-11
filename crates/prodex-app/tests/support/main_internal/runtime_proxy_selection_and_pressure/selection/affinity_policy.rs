use super::*;

#[test]
fn runtime_candidate_no_rotate_affinity_makes_hard_affinity_explicit() {
    assert_eq!(
        runtime_candidate_no_rotate_affinity(RuntimeCandidateAffinity::new(
            RuntimeRouteKind::Responses,
            "main",
            Some("main"),
            None,
            None,
            None,
            false,
        )),
        Some(RuntimeNoRotateAffinity::Strict)
    );
    assert_eq!(
        runtime_candidate_no_rotate_affinity(RuntimeCandidateAffinity::new(
            RuntimeRouteKind::Responses,
            "main",
            None,
            Some("main"),
            None,
            None,
            true,
        )),
        Some(RuntimeNoRotateAffinity::TrustedPreviousResponse)
    );
    assert_eq!(
        runtime_candidate_no_rotate_affinity(RuntimeCandidateAffinity::new(
            RuntimeRouteKind::Responses,
            "main",
            None,
            Some("main"),
            None,
            None,
            false,
        )),
        None
    );
    assert_eq!(
        runtime_candidate_no_rotate_affinity(RuntimeCandidateAffinity::new(
            RuntimeRouteKind::Compact,
            "main",
            None,
            None,
            None,
            Some("main"),
            false,
        )),
        Some(RuntimeNoRotateAffinity::CompactSession)
    );
}

#[test]
fn runtime_candidate_no_rotate_affinity_matrix_prioritizes_hard_bindings() {
    for route_kind in [
        RuntimeRouteKind::Responses,
        RuntimeRouteKind::Compact,
        RuntimeRouteKind::Websocket,
        RuntimeRouteKind::Standard,
    ] {
        for strict in RuntimeAffinityProfileCase::ALL {
            for pinned in RuntimeAffinityProfileCase::ALL {
                for turn_state in RuntimeAffinityProfileCase::ALL {
                    for session in RuntimeAffinityProfileCase::ALL {
                        for trusted_previous_response_affinity in [false, true] {
                            let affinity = RuntimeCandidateAffinity::new(
                                route_kind,
                                "main",
                                strict.profile(),
                                pinned.profile(),
                                turn_state.profile(),
                                session.profile(),
                                trusted_previous_response_affinity,
                            );
                            let expected = if strict == RuntimeAffinityProfileCase::Candidate {
                                Some(RuntimeNoRotateAffinity::Strict)
                            } else if turn_state == RuntimeAffinityProfileCase::Candidate {
                                Some(RuntimeNoRotateAffinity::TurnState)
                            } else if trusted_previous_response_affinity
                                && pinned == RuntimeAffinityProfileCase::Candidate
                            {
                                Some(RuntimeNoRotateAffinity::TrustedPreviousResponse)
                            } else if route_kind == RuntimeRouteKind::Compact
                                && session == RuntimeAffinityProfileCase::Candidate
                            {
                                Some(RuntimeNoRotateAffinity::CompactSession)
                            } else {
                                None
                            };
                            let label = format!(
                                "route={} strict={} pinned={} turn_state={} session={} trusted_previous_response_affinity={}",
                                runtime_route_kind_label(route_kind),
                                strict.label(),
                                pinned.label(),
                                turn_state.label(),
                                session.label(),
                                trusted_previous_response_affinity
                            );

                            assert_eq!(
                                runtime_candidate_no_rotate_affinity(affinity),
                                expected,
                                "{label}"
                            );
                            assert_eq!(
                                runtime_candidate_has_hard_affinity(affinity),
                                expected.is_some(),
                                "{label}"
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn quota_blocked_affinity_release_matrix_keeps_hard_or_classified_continuations() {
    let shapes = [
        None,
        Some(RuntimePreviousResponseFreshFallbackShape::ToolOutputOnly),
        Some(RuntimePreviousResponseFreshFallbackShape::EmptyInputOnly),
        Some(RuntimePreviousResponseFreshFallbackShape::SessionScopedFreshReplay),
        Some(RuntimePreviousResponseFreshFallbackShape::ContextDependentContinuation),
    ];

    for route_kind in [
        RuntimeRouteKind::Responses,
        RuntimeRouteKind::Compact,
        RuntimeRouteKind::Websocket,
        RuntimeRouteKind::Standard,
    ] {
        for strict in RuntimeAffinityProfileCase::ALL {
            for pinned in RuntimeAffinityProfileCase::ALL {
                for turn_state in RuntimeAffinityProfileCase::ALL {
                    for session in RuntimeAffinityProfileCase::ALL {
                        for trusted_previous_response_affinity in [false, true] {
                            for fresh_fallback_shape in shapes {
                                let affinity = RuntimeCandidateAffinity::new(
                                    route_kind,
                                    "main",
                                    strict.profile(),
                                    pinned.profile(),
                                    turn_state.profile(),
                                    session.profile(),
                                    trusted_previous_response_affinity,
                                );
                                let expected = if fresh_fallback_shape.is_some()
                                    || strict == RuntimeAffinityProfileCase::Candidate
                                    || turn_state == RuntimeAffinityProfileCase::Candidate
                                    || (route_kind == RuntimeRouteKind::Compact
                                        && session == RuntimeAffinityProfileCase::Candidate)
                                {
                                    RuntimeQuotaBlockedAffinityReleasePolicy::KeepAffinity
                                } else {
                                    RuntimeQuotaBlockedAffinityReleasePolicy::ReleaseAffinity
                                };
                                let label = format!(
                                    "route={} strict={} pinned={} turn_state={} session={} trusted_previous_response_affinity={} shape={}",
                                    runtime_route_kind_label(route_kind),
                                    strict.label(),
                                    pinned.label(),
                                    turn_state.label(),
                                    session.label(),
                                    trusted_previous_response_affinity,
                                    runtime_previous_response_fresh_fallback_shape_label(
                                        fresh_fallback_shape
                                    )
                                );

                                assert_eq!(
                                    runtime_quota_blocked_affinity_release_policy(
                                        RuntimeQuotaBlockedAffinityReleaseRequest {
                                            affinity,
                                            fresh_fallback_shape,
                                        }
                                    ),
                                    expected,
                                    "{label}"
                                );
                                assert_eq!(
                                    runtime_quota_blocked_affinity_is_releasable(
                                        affinity,
                                        false,
                                        fresh_fallback_shape,
                                    ),
                                    expected
                                        == RuntimeQuotaBlockedAffinityReleasePolicy::ReleaseAffinity,
                                    "{label}"
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}
