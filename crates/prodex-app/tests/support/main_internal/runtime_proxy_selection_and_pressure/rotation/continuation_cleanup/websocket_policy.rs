use super::*;

#[test]
fn websocket_previous_response_not_found_requires_stale_continuation_without_turn_state() {
    assert!(
        runtime_websocket_previous_response_not_found_requires_stale_continuation(
            Some("resp-second"),
            false,
            true,
            Some(RuntimePreviousResponseFreshFallbackShape::ContextDependentContinuation),
        ),
        "locked-affinity websocket continuations without replayable turn state must fail locally"
    );
    assert!(
        !runtime_websocket_previous_response_not_found_requires_stale_continuation(
            Some("resp-second"),
            true,
            true,
            Some(RuntimePreviousResponseFreshFallbackShape::ContextDependentContinuation),
        ),
        "available replay turn state should keep previous_response retries alive"
    );
    assert!(
        runtime_websocket_previous_response_not_found_requires_stale_continuation(
            Some("resp-second"),
            false,
            false,
            Some(RuntimePreviousResponseFreshFallbackShape::SessionScopedFreshReplay),
        ),
        "session metadata is not enough to replay a missing previous_response chain"
    );
    assert!(
        runtime_websocket_previous_response_not_found_requires_stale_continuation(
            Some("resp-second"),
            false,
            false,
            Some(RuntimePreviousResponseFreshFallbackShape::ContextDependentContinuation),
        ),
        "continuation-only websocket requests without turn state must fail locally instead of surfacing raw upstream 400s"
    );
    assert!(
        runtime_websocket_previous_response_not_found_requires_stale_continuation(
            Some("resp-second"),
            false,
            false,
            None,
        ),
        "unknown fallback shape should stay conservative when websocket turn state is missing"
    );
    assert!(
        !runtime_websocket_previous_response_not_found_requires_stale_continuation(
            None,
            false,
            true,
            Some(RuntimePreviousResponseFreshFallbackShape::ContextDependentContinuation),
        ),
        "requests without previous_response_id should not be classified as stale continuations"
    );
}

#[test]
fn websocket_reuse_watchdog_fresh_fallback_stays_blocked_for_locked_affinity() {
    assert!(
        !runtime_websocket_reuse_watchdog_previous_response_fresh_fallback_allowed(
            RuntimeWebsocketReuseWatchdogPreviousResponseFallback {
                profile_name: "second",
                previous_response_id: Some("resp-second"),
                previous_response_fresh_fallback_used: false,
                bound_profile: Some("second"),
                pinned_profile: None,
                request_requires_previous_response_affinity: true,
                trusted_previous_response_affinity: false,
                request_turn_state: None,
                fresh_fallback_shape: Some(
                    RuntimePreviousResponseFreshFallbackShape::ContextDependentContinuation,
                ),
            },
        ),
        "watchdog fallback should stay blocked for non-replayable locked-affinity continuations"
    );
}

#[test]
fn noncompact_session_priority_ignores_compact_session_profile() {
    assert_eq!(
        runtime_noncompact_session_priority_profile(Some("main"), Some("main")),
        None
    );
    assert_eq!(
        runtime_noncompact_session_priority_profile(Some("second"), Some("main")),
        Some("second")
    );
    assert_eq!(
        runtime_noncompact_session_priority_profile(Some("main"), None),
        Some("main")
    );
}
