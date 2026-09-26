use super::*;

#[test]
fn previous_response_not_found_with_turn_state_retries_then_blocks_fresh_fallback() {
    let decision = runtime_previous_response_not_found_decision(
        RuntimePreviousResponseNotFoundDecisionInput {
            route: RuntimePreviousResponseNotFoundRoute::Responses,
            previous_response_id: Some("resp_1"),
            has_turn_state_retry: true,
            request_requires_previous_response_affinity: true,
            trusted_previous_response_affinity: true,
            request_turn_state: Some("ts"),
            previous_response_fresh_fallback_used: false,
            fresh_fallback_shape: Some(
                RuntimePreviousResponseFreshFallbackShape::ContextDependentContinuation,
            ),
            retry_index: 1,
        },
    );

    assert_eq!(decision.retry_delay, Some(Duration::from_millis(200)));
    assert_eq!(decision.retry_reason, Some("non_blocking_retry"));
    assert!(!decision.stale_continuation);
    assert!(!decision.fresh_fallback_allowed);
    assert!(!decision.fresh_fallback_blocked_without_affinity);
}

#[test]
fn previous_response_not_found_without_turn_state_fails_closed() {
    let decision = runtime_previous_response_not_found_decision(
        RuntimePreviousResponseNotFoundDecisionInput {
            route: RuntimePreviousResponseNotFoundRoute::Responses,
            previous_response_id: Some("resp_1"),
            has_turn_state_retry: false,
            request_requires_previous_response_affinity: false,
            trusted_previous_response_affinity: false,
            request_turn_state: None,
            previous_response_fresh_fallback_used: false,
            fresh_fallback_shape: Some(
                RuntimePreviousResponseFreshFallbackShape::ContextDependentContinuation,
            ),
            retry_index: 0,
        },
    );

    assert_eq!(decision.retry_delay, None);
    assert!(decision.stale_continuation);
    assert!(decision.fresh_fallback_blocked_without_affinity);
    assert_eq!(
        runtime_previous_response_not_found_observability_outcome(
            decision,
            Some(RuntimePreviousResponseFreshFallbackShape::ContextDependentContinuation),
        ),
        Some("blocked_nonreplayable_without_affinity")
    );
}

#[test]
fn websocket_locked_affinity_retries_without_turn_state() {
    let decision = runtime_previous_response_not_found_decision(
        RuntimePreviousResponseNotFoundDecisionInput {
            route: RuntimePreviousResponseNotFoundRoute::Websocket,
            previous_response_id: Some("resp_1"),
            has_turn_state_retry: false,
            request_requires_previous_response_affinity: true,
            trusted_previous_response_affinity: true,
            request_turn_state: None,
            previous_response_fresh_fallback_used: false,
            fresh_fallback_shape: None,
            retry_index: 0,
        },
    );

    assert_eq!(decision.retry_delay, Some(Duration::from_millis(75)));
    assert_eq!(decision.retry_reason, Some("locked_affinity_no_turn_state"));
    assert_eq!(
        decision.chain_retry_reason,
        Some("previous_response_not_found_locked_affinity")
    );
}

#[test]
fn fresh_fallback_policy_keeps_existing_context_fail_closed() {
    assert_eq!(
        runtime_previous_response_fresh_fallback_policy(
            RuntimePreviousResponseFreshFallbackPolicyInput {
                has_previous_response_context: false,
                request_requires_locked_previous_response_affinity: false,
                fresh_fallback_shape: None,
            },
        ),
        RuntimePreviousResponseFreshFallbackPolicy::NotApplicable,
    );
    assert_eq!(
        runtime_previous_response_fresh_fallback_policy(
            RuntimePreviousResponseFreshFallbackPolicyInput {
                has_previous_response_context: true,
                request_requires_locked_previous_response_affinity: false,
                fresh_fallback_shape: None,
            },
        ),
        RuntimePreviousResponseFreshFallbackPolicy::FailClosed {
            request_shape: RuntimePreviousResponseFreshFallbackPolicyShape::Unknown,
        },
    );
    let policy = runtime_previous_response_fresh_fallback_policy(
        RuntimePreviousResponseFreshFallbackPolicyInput {
            has_previous_response_context: false,
            request_requires_locked_previous_response_affinity: true,
            fresh_fallback_shape: Some(RuntimePreviousResponseFreshFallbackShape::EmptyInputOnly),
        },
    );
    assert_eq!(
        policy,
        RuntimePreviousResponseFreshFallbackPolicy::FailClosed {
            request_shape: RuntimePreviousResponseFreshFallbackPolicyShape::EmptyInputOnly,
        },
    );
    assert!(!policy.allows_fresh_fallback());
}

#[test]
fn session_affinity_promotes_only_empty_input_fallback_shape() {
    assert_eq!(
        runtime_previous_response_fresh_fallback_shape_with_session(
            Some(RuntimePreviousResponseFreshFallbackShape::EmptyInputOnly),
            true,
        ),
        Some(RuntimePreviousResponseFreshFallbackShape::SessionScopedFreshReplay),
    );
    assert_eq!(
        runtime_previous_response_fresh_fallback_shape_with_session(
            Some(RuntimePreviousResponseFreshFallbackShape::ToolOutputOnly),
            true,
        ),
        Some(RuntimePreviousResponseFreshFallbackShape::ToolOutputOnly),
    );
    assert_eq!(
        runtime_previous_response_fresh_fallback_shape_with_session(
            Some(RuntimePreviousResponseFreshFallbackShape::EmptyInputOnly),
            false,
        ),
        Some(RuntimePreviousResponseFreshFallbackShape::EmptyInputOnly),
    );
}

#[test]
fn stale_previous_response_policy_uses_turn_state_or_fails_closed() {
    let request = RuntimePreviousResponseNotFoundFallbackRequest {
        previous_response_id: Some("resp_1"),
        has_turn_state_retry: false,
        request_requires_locked_previous_response_affinity: false,
        previous_response_fresh_fallback_used: false,
        fresh_fallback_shape: Some(
            RuntimePreviousResponseFreshFallbackShape::ContextDependentContinuation,
        ),
    };
    assert_eq!(
        runtime_previous_response_not_found_fallback_policy(request),
        RuntimePreviousResponseNotFoundFallbackPolicy {
            stale_continuation: RuntimePreviousResponseStaleContinuationPolicy::FailClosed,
            fresh_fallback: RuntimePreviousResponseFreshFallbackPolicy::FailClosed {
                request_shape:
                    RuntimePreviousResponseFreshFallbackPolicyShape::ContextDependentContinuation,
            },
        },
    );
    assert_eq!(
        runtime_previous_response_not_found_fallback_policy(
            RuntimePreviousResponseNotFoundFallbackRequest {
                has_turn_state_retry: true,
                ..request
            },
        )
        .stale_continuation,
        RuntimePreviousResponseStaleContinuationPolicy::RetryWithTurnState,
    );
}

#[test]
fn websocket_locked_affinity_tracks_previous_response_and_turn_state() {
    assert!(
        runtime_websocket_request_requires_locked_previous_response_affinity(
            false,
            true,
            Some("resp_1"),
            None,
        )
    );
    assert!(
        !runtime_websocket_request_requires_locked_previous_response_affinity(
            false,
            true,
            Some("resp_1"),
            Some("turn_state"),
        )
    );
    assert!(
        runtime_websocket_request_requires_locked_previous_response_affinity(
            true,
            false,
            None,
            Some("turn_state"),
        )
    );
}

#[test]
fn websocket_turn_state_retry_stays_on_owner_without_inventing_locked_affinity() {
    let decision = runtime_previous_response_not_found_decision(
        RuntimePreviousResponseNotFoundDecisionInput {
            route: RuntimePreviousResponseNotFoundRoute::Websocket,
            previous_response_id: Some("resp_1"),
            has_turn_state_retry: true,
            request_requires_previous_response_affinity: false,
            trusted_previous_response_affinity: true,
            request_turn_state: Some("turn_state"),
            previous_response_fresh_fallback_used: false,
            fresh_fallback_shape: None,
            retry_index: 0,
        },
    );
    assert_eq!(decision.retry_delay, Some(Duration::from_millis(75)));
    assert_eq!(decision.retry_reason, Some("non_blocking_retry"));
    assert_eq!(decision.chain_retry_reason, None);
    assert!(!decision.request_requires_locked_previous_response_affinity);
    assert!(!decision.stale_continuation);
}

#[test]
fn previous_response_retry_schedule_is_bounded() {
    let expected_delays = [
        Some(Duration::from_millis(75)),
        Some(Duration::from_millis(200)),
        Some(Duration::from_millis(500)),
        None,
    ];
    for (retry_index, expected_delay) in expected_delays.into_iter().enumerate() {
        let decision = runtime_previous_response_not_found_decision(
            RuntimePreviousResponseNotFoundDecisionInput {
                route: RuntimePreviousResponseNotFoundRoute::Responses,
                previous_response_id: Some("resp_1"),
                has_turn_state_retry: true,
                request_requires_previous_response_affinity: true,
                trusted_previous_response_affinity: true,
                request_turn_state: Some("turn_state"),
                previous_response_fresh_fallback_used: false,
                fresh_fallback_shape: None,
                retry_index,
            },
        );
        assert_eq!(decision.retry_delay, expected_delay);
    }
}
