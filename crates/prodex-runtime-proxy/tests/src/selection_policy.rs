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
fn affinity_outcome_precedence_is_mojo_authoritative() {
    let base = RuntimeAffinityOutcomeInput {
        hard_binding_conflict: false,
        exact_binding_mismatch: false,
        profile_usable: true,
        excluded: false,
        hard_affinity: false,
        soft_policy_allowed: true,
        local_rejection: RuntimeAffinityLocalRejection::None,
    };

    assert_eq!(
        runtime_affinity_outcome(RuntimeAffinityOutcomeInput {
            hard_binding_conflict: true,
            ..base
        }),
        RuntimeAffinityOutcome::Unavailable {
            reason: "hard_binding_conflict",
            hard: true,
        }
    );
    assert_eq!(
        runtime_affinity_outcome(RuntimeAffinityOutcomeInput {
            exact_binding_mismatch: true,
            hard_affinity: true,
            ..base
        }),
        RuntimeAffinityOutcome::Unavailable {
            reason: "binding_identity_mismatch",
            hard: true,
        }
    );
    assert_eq!(
        runtime_affinity_outcome(RuntimeAffinityOutcomeInput {
            profile_usable: false,
            ..base
        }),
        RuntimeAffinityOutcome::Unavailable {
            reason: "hard_binding_unavailable",
            hard: false,
        }
    );
    assert_eq!(
        runtime_affinity_outcome(RuntimeAffinityOutcomeInput {
            excluded: true,
            ..base
        }),
        RuntimeAffinityOutcome::Unavailable {
            reason: "bound_profile_unavailable",
            hard: false,
        }
    );
    assert_eq!(
        runtime_affinity_outcome(RuntimeAffinityOutcomeInput {
            hard_affinity: true,
            ..base
        }),
        RuntimeAffinityOutcome::SelectHard
    );
    assert_eq!(
        runtime_affinity_outcome(RuntimeAffinityOutcomeInput {
            soft_policy_allowed: false,
            ..base
        }),
        RuntimeAffinityOutcome::RejectSoftQuota
    );
    assert_eq!(
        runtime_affinity_outcome(RuntimeAffinityOutcomeInput {
            local_rejection: RuntimeAffinityLocalRejection::SelectionBackoff,
            ..base
        }),
        RuntimeAffinityOutcome::Unavailable {
            reason: "selection_backoff",
            hard: false,
        }
    );
    assert_eq!(
        runtime_affinity_outcome(RuntimeAffinityOutcomeInput {
            local_rejection: RuntimeAffinityLocalRejection::RouteCircuitHalfOpenProbeWait,
            ..base
        }),
        RuntimeAffinityOutcome::Unavailable {
            reason: "route_circuit_half_open_probe_wait",
            hard: false,
        }
    );
    assert_eq!(
        runtime_affinity_outcome(base),
        RuntimeAffinityOutcome::SelectSoft
    );
}

#[test]
fn websocket_invalid_previous_response_plan_preserves_recovery_and_reuse_reason() {
    let reconnect = runtime_websocket_invalid_previous_response_plan(true, true, true, true, false);
    assert!(reconnect.recovery_signal);
    assert!(reconnect.crossed_transport_generation);
    assert_eq!(
        reconnect.chain_reuse_reason,
        RuntimeWebsocketChainReuseReason::UpstreamReconnect
    );
    assert_eq!(
        reconnect.action,
        RuntimeWebsocketInvalidPreviousResponseAction::FullContextRetry
    );

    let unbound = runtime_websocket_invalid_previous_response_plan(true, true, false, false, false);
    assert!(!unbound.recovery_signal);
    assert_eq!(
        unbound.chain_reuse_reason,
        RuntimeWebsocketChainReuseReason::UnboundPreviousResponse
    );
    assert_eq!(
        unbound.action,
        RuntimeWebsocketInvalidPreviousResponseAction::PassThrough
    );
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
fn affinity_name_matching_is_mojo_owned_at_the_proxy_boundary() {
    let mut affinity = RuntimeCandidateAffinity::new(
        RuntimeRouteKind::Responses,
        "owner",
        Some("owner"),
        Some("other"),
        Some("other"),
        None,
        true,
    );
    assert_eq!(
        runtime_candidate_no_rotate_affinity(affinity),
        Some(RuntimeNoRotateAffinity::Strict)
    );

    affinity.candidate_name = "other";
    assert_eq!(
        runtime_candidate_no_rotate_affinity(affinity),
        Some(RuntimeNoRotateAffinity::TurnState)
    );

    affinity.route_kind = RuntimeRouteKind::Compact;
    affinity.turn_state_profile = None;
    affinity.trusted_previous_response_affinity = false;
    affinity.session_profile = Some("other");
    assert_eq!(
        runtime_candidate_no_rotate_affinity(affinity),
        Some(RuntimeNoRotateAffinity::CompactSession)
    );

    affinity.session_profile = Some("owner");
    assert_eq!(runtime_candidate_no_rotate_affinity(affinity), None);
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
    assert!(runtime_websocket_previous_response_reuse_is_stale_at(
        true,
        Some(Duration::from_nanos(2)),
        Duration::from_nanos(1),
    ));
    assert!(!runtime_websocket_previous_response_reuse_is_stale_at(
        true,
        Some(Duration::from_nanos(1)),
        Duration::from_nanos(2),
    ));
    assert!(!runtime_websocket_previous_response_reuse_is_stale_at(
        false,
        Some(Duration::from_secs(1)),
        Duration::ZERO,
    ));
    assert!(!runtime_websocket_previous_response_reuse_is_stale_at(
        true,
        None,
        Duration::ZERO,
    ));
}

#[test]
fn websocket_previous_response_reuse_requires_replayable_continuation() {
    assert!(runtime_websocket_previous_response_reuse_is_nonreplayable(
        Some("resp_123"),
        false,
        None,
    ));
    assert!(!runtime_websocket_previous_response_reuse_is_nonreplayable(
        None, false, None,
    ));
    assert!(!runtime_websocket_previous_response_reuse_is_nonreplayable(
        Some("resp_123"),
        true,
        None,
    ));
    assert!(!runtime_websocket_previous_response_reuse_is_nonreplayable(
        Some("resp_123"),
        false,
        Some("turn_state"),
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
fn soft_affinity_allows_response_until_five_hour_quota_is_exhausted() {
    let mut summary = healthy_summary();
    summary.five_hour = RuntimeSelectionQuotaWindowSummary {
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
        responses_critical_floor_percent: 1,
    };

    assert!(runtime_soft_affinity_allowed(input));
    assert_eq!(
        runtime_quota_precommit_guard_reason(summary, RuntimeRouteKind::Responses, 1),
        None
    );
}

#[test]
fn response_floor_is_advisory_until_authoritative_exhaustion() {
    for remaining_percent in [5, 2, 1] {
        let mut summary = healthy_summary();
        summary.five_hour = RuntimeSelectionQuotaWindowSummary {
            status: RuntimeSelectionQuotaWindowStatus::Critical,
            remaining_percent,
        };
        summary.route_band = RuntimeSelectionQuotaPressureBand::Critical;

        assert_eq!(
            runtime_quota_precommit_guard_reason(summary, RuntimeRouteKind::Responses, 10),
            None,
            "positive remaining quota must remain usable at {remaining_percent}%"
        );
    }
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

#[test]
fn soft_affinity_expected_values_cover_routes_and_missing_or_exhausted_quota() {
    let cases = [
        (
            RuntimeAffinitySelectionKind::Strict,
            RuntimeRouteKind::Responses,
            None,
            false,
            false,
            false,
            true,
            "quota_unknown",
        ),
        (
            RuntimeAffinitySelectionKind::Strict,
            RuntimeRouteKind::Compact,
            None,
            false,
            false,
            false,
            false,
            "quota_windows_unavailable",
        ),
        (
            RuntimeAffinitySelectionKind::Session,
            RuntimeRouteKind::Compact,
            None,
            false,
            false,
            false,
            true,
            "quota_unknown",
        ),
        (
            RuntimeAffinitySelectionKind::Session,
            RuntimeRouteKind::Websocket,
            None,
            true,
            false,
            false,
            true,
            "quota_unknown",
        ),
        (
            RuntimeAffinitySelectionKind::Session,
            RuntimeRouteKind::Websocket,
            None,
            true,
            true,
            false,
            false,
            "quota_windows_unavailable",
        ),
        (
            RuntimeAffinitySelectionKind::Session,
            RuntimeRouteKind::Responses,
            Some(RuntimeSelectionQuotaSource::LiveProbe),
            false,
            false,
            true,
            false,
            "quota_exhausted",
        ),
    ];
    let mut exhausted = healthy_summary();
    exhausted.weekly = RuntimeSelectionQuotaWindowSummary {
        status: RuntimeSelectionQuotaWindowStatus::Exhausted,
        remaining_percent: 0,
    };
    exhausted.route_band = RuntimeSelectionQuotaPressureBand::Exhausted;

    for (
        affinity_kind,
        route_kind,
        quota_source,
        current_matches,
        has_fallback,
        weekly_exhausted,
        expected,
        reason,
    ) in cases
    {
        let input = RuntimeSoftAffinityPolicyInput {
            affinity_kind,
            route_kind,
            quota_summary: if weekly_exhausted {
                exhausted
            } else {
                healthy_summary()
            },
            quota_source,
            current_profile_matches_candidate: current_matches,
            has_route_eligible_quota_fallback: has_fallback,
            responses_critical_floor_percent: 10,
        };
        let label = format!(
            "{affinity_kind:?} {route_kind:?} {quota_source:?} current={current_matches} fallback={has_fallback}"
        );
        assert_eq!(runtime_soft_affinity_allowed(input), expected, "{label}");
        if !expected {
            assert_eq!(
                runtime_soft_affinity_rejection_reason(input),
                reason,
                "{label}"
            );
        }
    }
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

#[test]
fn websocket_failure_disposition_is_mojo_authoritative() {
    assert_eq!(
        runtime_websocket_failure_disposition(false, false),
        RuntimeWebsocketFailureDispositionPlan {
            continue_selection: false,
            mark_backoff: true,
            exclude_profile: false,
        }
    );
    assert_eq!(
        runtime_websocket_failure_disposition(true, false),
        RuntimeWebsocketFailureDispositionPlan {
            continue_selection: true,
            mark_backoff: true,
            exclude_profile: true,
        }
    );
    assert_eq!(
        runtime_websocket_failure_disposition(true, true),
        RuntimeWebsocketFailureDispositionPlan {
            continue_selection: true,
            mark_backoff: false,
            exclude_profile: false,
        }
    );
    assert!(runtime_websocket_full_context_signal_eligible(
        true, true, true
    ));
    assert!(runtime_websocket_full_context_signal_eligible(
        true, false, true
    ));
    assert!(!runtime_websocket_full_context_signal_eligible(
        true, true, false
    ));
    assert_eq!(
        runtime_websocket_quota_fallback_plan(true, true),
        RuntimeWebsocketQuotaFallbackPlan::Ready
    );
    assert_eq!(
        runtime_websocket_quota_fallback_plan(false, true),
        RuntimeWebsocketQuotaFallbackPlan::Unavailable
    );
    assert_eq!(
        runtime_websocket_quota_fallback_plan(false, false),
        RuntimeWebsocketQuotaFallbackPlan::LastChance
    );
}

#[test]
fn websocket_transport_failure_precedence_is_mojo_authoritative() {
    assert_eq!(
        runtime_websocket_transport_failure_plan(false, true, true),
        RuntimeWebsocketTransportFailurePlan::ReuseWatchdog
    );
    assert_eq!(
        runtime_websocket_transport_failure_plan(false, false, true),
        RuntimeWebsocketTransportFailurePlan::RetryTransport
    );
    assert_eq!(
        runtime_websocket_transport_failure_plan(true, true, true),
        RuntimeWebsocketTransportFailurePlan::Error
    );
    assert_eq!(
        runtime_websocket_transport_failure_plan(false, false, false),
        RuntimeWebsocketTransportFailurePlan::Error
    );
}
