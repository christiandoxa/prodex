use super::*;

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
