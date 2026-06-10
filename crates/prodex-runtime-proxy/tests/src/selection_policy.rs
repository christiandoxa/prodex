use super::*;
use crate::{runtime_previous_response_fresh_fallback_shape_label, runtime_route_kind_label};

fn healthy_summary() -> RuntimeSelectionQuotaSummary {
    RuntimeSelectionQuotaSummary {
        five_hour: RuntimeSelectionQuotaWindowSummary {
            status: RuntimeSelectionQuotaWindowStatus::Ready,
            remaining_percent: 80,
        },
        weekly: RuntimeSelectionQuotaWindowSummary {
            status: RuntimeSelectionQuotaWindowStatus::Ready,
            remaining_percent: 80,
        },
        route_band: RuntimeSelectionQuotaPressureBand::Healthy,
    }
}

#[test]
fn hard_affinity_detects_no_rotate_sources() {
    let affinity = RuntimeCandidateAffinity {
        route_kind: RuntimeRouteKind::Responses,
        candidate_name: "main",
        strict_affinity_profile: None,
        pinned_profile: Some("main"),
        turn_state_profile: None,
        session_profile: None,
        trusted_previous_response_affinity: true,
    };

    assert_eq!(
        runtime_candidate_no_rotate_affinity(affinity),
        Some(RuntimeNoRotateAffinity::TrustedPreviousResponse)
    );
    assert!(runtime_candidate_has_hard_affinity(affinity));
}

#[test]
fn hard_affinity_matrix_prioritizes_no_rotate_sources() {
    for route_kind in [
        RuntimeRouteKind::Responses,
        RuntimeRouteKind::Compact,
        RuntimeRouteKind::Websocket,
        RuntimeRouteKind::Standard,
    ] {
        for strict in AffinityProfileCase::ALL {
            for pinned in AffinityProfileCase::ALL {
                for turn_state in AffinityProfileCase::ALL {
                    for session in AffinityProfileCase::ALL {
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
                            let expected = if strict == AffinityProfileCase::Candidate {
                                Some(RuntimeNoRotateAffinity::Strict)
                            } else if turn_state == AffinityProfileCase::Candidate {
                                Some(RuntimeNoRotateAffinity::TurnState)
                            } else if trusted_previous_response_affinity
                                && pinned == AffinityProfileCase::Candidate
                            {
                                Some(RuntimeNoRotateAffinity::TrustedPreviousResponse)
                            } else if route_kind == RuntimeRouteKind::Compact
                                && session == AffinityProfileCase::Candidate
                            {
                                Some(RuntimeNoRotateAffinity::CompactSession)
                            } else {
                                None
                            };
                            let label = format!(
                                "route={} strict={} pinned={} turn_state={} session={} trusted={}",
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
fn quota_blocked_affinity_keeps_nonreplayable_previous_response_shape() {
    let affinity = RuntimeCandidateAffinity {
        route_kind: RuntimeRouteKind::Responses,
        candidate_name: "main",
        strict_affinity_profile: None,
        pinned_profile: Some("main"),
        turn_state_profile: None,
        session_profile: None,
        trusted_previous_response_affinity: true,
    };

    assert_eq!(
        runtime_quota_blocked_affinity_release_policy(RuntimeQuotaBlockedAffinityReleaseRequest {
            affinity,
            fresh_fallback_shape: Some(
                RuntimePreviousResponseFreshFallbackShape::ContextDependentContinuation,
            ),
        },),
        RuntimeQuotaBlockedAffinityReleasePolicy::KeepAffinity
    );
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
        for strict in AffinityProfileCase::ALL {
            for pinned in AffinityProfileCase::ALL {
                for turn_state in AffinityProfileCase::ALL {
                    for session in AffinityProfileCase::ALL {
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
                                    || strict == AffinityProfileCase::Candidate
                                    || turn_state == AffinityProfileCase::Candidate
                                    || (route_kind == RuntimeRouteKind::Compact
                                        && session == AffinityProfileCase::Candidate)
                                {
                                    RuntimeQuotaBlockedAffinityReleasePolicy::KeepAffinity
                                } else {
                                    RuntimeQuotaBlockedAffinityReleasePolicy::ReleaseAffinity
                                };
                                let label = format!(
                                    "route={} strict={} pinned={} turn_state={} session={} trusted={} shape={}",
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

#[test]
fn websocket_stale_previous_response_reuse_uses_injected_threshold() {
    assert!(runtime_websocket_previous_response_reuse_is_stale_at(
        true,
        Some(Duration::from_millis(61)),
        Duration::from_millis(60),
    ));
    assert!(!runtime_websocket_previous_response_reuse_is_stale_at(
        true,
        Some(Duration::from_millis(59)),
        Duration::from_millis(60),
    ));
}

#[test]
fn continuation_priority_detects_any_affinity_marker() {
    assert!(runtime_proxy_has_continuation_priority(
        None,
        None,
        Some("turn"),
        None,
        None,
    ));
    assert!(!runtime_proxy_has_continuation_priority(
        None, None, None, None, None,
    ));
}

#[test]
fn hard_affinity_markers_disable_fresh_current_fallback() {
    struct Case {
        label: &'static str,
        previous_response_id: Option<&'static str>,
        pinned_profile: Option<&'static str>,
        request_turn_state: Option<&'static str>,
        turn_state_profile: Option<&'static str>,
        session_profile: Option<&'static str>,
    }

    let cases = [
        Case {
            label: "previous_response_id",
            previous_response_id: Some("resp_123"),
            pinned_profile: Some("main"),
            request_turn_state: None,
            turn_state_profile: None,
            session_profile: None,
        },
        Case {
            label: "trusted_previous_response_owner",
            previous_response_id: None,
            pinned_profile: Some("main"),
            request_turn_state: None,
            turn_state_profile: None,
            session_profile: None,
        },
        Case {
            label: "request_turn_state",
            previous_response_id: None,
            pinned_profile: None,
            request_turn_state: Some("turn_state"),
            turn_state_profile: None,
            session_profile: None,
        },
        Case {
            label: "turn_state_profile",
            previous_response_id: None,
            pinned_profile: None,
            request_turn_state: None,
            turn_state_profile: Some("main"),
            session_profile: None,
        },
        Case {
            label: "session_profile",
            previous_response_id: None,
            pinned_profile: None,
            request_turn_state: None,
            turn_state_profile: None,
            session_profile: Some("main"),
        },
    ];

    for case in cases {
        assert!(
            runtime_proxy_has_continuation_priority(
                case.previous_response_id,
                case.pinned_profile,
                case.request_turn_state,
                case.turn_state_profile,
                case.session_profile,
            ),
            "{}",
            case.label
        );
        assert!(
            !runtime_proxy_allows_direct_current_profile_fallback(
                case.previous_response_id,
                case.pinned_profile,
                case.request_turn_state,
                case.turn_state_profile,
                case.session_profile,
                false,
                false,
            ),
            "{}",
            case.label
        );
    }
}

#[test]
fn wait_affinity_owner_prefers_no_rotate_sources() {
    assert_eq!(
        runtime_wait_affinity_owner(
            Some("strict"),
            Some("pinned"),
            Some("turn"),
            Some("session"),
            true,
        ),
        Some("strict")
    );
    assert_eq!(
        runtime_wait_affinity_owner(None, Some("pinned"), Some("turn"), Some("session"), true,),
        Some("turn")
    );
    assert_eq!(
        runtime_wait_affinity_owner(None, Some("pinned"), None, Some("session"), true,),
        Some("pinned")
    );
    assert_eq!(
        runtime_wait_affinity_owner(None, Some("pinned"), None, Some("session"), false,),
        Some("session")
    );
}

#[test]
fn noncompact_session_priority_ignores_compact_owner() {
    assert_eq!(
        runtime_noncompact_session_priority_profile(Some("main"), Some("main")),
        None
    );
    assert_eq!(
        runtime_noncompact_session_priority_profile(Some("second"), Some("main")),
        Some("second")
    );
}

#[test]
fn direct_current_profile_fallback_requires_fresh_unfailed_request() {
    assert!(runtime_proxy_allows_direct_current_profile_fallback(
        None, None, None, None, None, false, false,
    ));
    assert!(!runtime_proxy_allows_direct_current_profile_fallback(
        Some("resp"),
        None,
        None,
        None,
        None,
        false,
        false,
    ));
    assert!(!runtime_proxy_allows_direct_current_profile_fallback(
        None, None, None, None, None, true, false,
    ));
    assert!(!runtime_proxy_allows_direct_current_profile_fallback(
        None, None, None, None, None, false, true,
    ));
}

#[test]
fn soft_affinity_blocks_response_on_critical_floor() {
    let mut summary = healthy_summary();
    summary.five_hour = RuntimeSelectionQuotaWindowSummary {
        status: RuntimeSelectionQuotaWindowStatus::Critical,
        remaining_percent: 2,
    };
    summary.route_band = RuntimeSelectionQuotaPressureBand::Critical;

    let input = RuntimeSoftAffinityPolicyInput {
        affinity_kind: RuntimeAffinitySelectionKind::Pinned,
        route_kind: RuntimeRouteKind::Responses,
        quota_summary: summary,
        quota_source: Some(RuntimeSelectionQuotaSource::LiveProbe),
        current_profile_matches_candidate: false,
        has_route_eligible_quota_fallback: true,
        responses_critical_floor_percent: 2,
    };

    assert!(!runtime_soft_affinity_allowed(input));
    assert_eq!(
        runtime_quota_precommit_guard_reason(summary, RuntimeRouteKind::Responses, 2),
        Some("quota_critical_floor_before_send")
    );
}

#[test]
fn soft_affinity_allows_response_when_only_weekly_is_critical() {
    let mut summary = healthy_summary();
    summary.weekly = RuntimeSelectionQuotaWindowSummary {
        status: RuntimeSelectionQuotaWindowStatus::Critical,
        remaining_percent: 1,
    };
    summary.route_band = RuntimeSelectionQuotaPressureBand::Critical;

    let input = RuntimeSoftAffinityPolicyInput {
        affinity_kind: RuntimeAffinitySelectionKind::Pinned,
        route_kind: RuntimeRouteKind::Responses,
        quota_summary: summary,
        quota_source: Some(RuntimeSelectionQuotaSource::LiveProbe),
        current_profile_matches_candidate: false,
        has_route_eligible_quota_fallback: true,
        responses_critical_floor_percent: 2,
    };

    assert!(runtime_soft_affinity_allowed(input));
    assert_eq!(
        runtime_quota_precommit_guard_reason(summary, RuntimeRouteKind::Responses, 2),
        None
    );
}

#[test]
fn soft_affinity_blocks_fallback_when_weekly_is_exhausted() {
    let mut summary = healthy_summary();
    summary.weekly = RuntimeSelectionQuotaWindowSummary {
        status: RuntimeSelectionQuotaWindowStatus::Exhausted,
        remaining_percent: 0,
    };
    summary.route_band = RuntimeSelectionQuotaPressureBand::Exhausted;

    let input = RuntimeSoftAffinityPolicyInput {
        affinity_kind: RuntimeAffinitySelectionKind::Pinned,
        route_kind: RuntimeRouteKind::Responses,
        quota_summary: summary,
        quota_source: Some(RuntimeSelectionQuotaSource::LiveProbe),
        current_profile_matches_candidate: false,
        has_route_eligible_quota_fallback: true,
        responses_critical_floor_percent: 2,
    };

    assert!(!runtime_soft_affinity_allowed(input));
    assert_eq!(
        runtime_quota_precommit_guard_reason(summary, RuntimeRouteKind::Responses, 2),
        None
    );
    assert_eq!(
        runtime_quota_soft_affinity_rejection_reason(
            summary,
            Some(RuntimeSelectionQuotaSource::LiveProbe),
            RuntimeRouteKind::Responses,
            2
        ),
        "quota_exhausted"
    );
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AffinityProfileCase {
    None,
    Candidate,
    Other,
}

impl AffinityProfileCase {
    const ALL: [Self; 3] = [Self::None, Self::Candidate, Self::Other];

    fn label(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Candidate => "candidate",
            Self::Other => "other",
        }
    }

    fn profile(self) -> Option<&'static str> {
        match self {
            Self::None => None,
            Self::Candidate => Some("main"),
            Self::Other => Some("second"),
        }
    }
}
