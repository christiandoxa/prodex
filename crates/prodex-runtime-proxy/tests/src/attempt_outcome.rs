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

#[cfg(feature = "mojo")]
#[test]
fn previous_response_plan_matches_rust_oracle() {
    let shapes = [
        None,
        Some(RuntimePreviousResponseFreshFallbackShape::ToolOutputOnly),
        Some(RuntimePreviousResponseFreshFallbackShape::EmptyInputOnly),
        Some(RuntimePreviousResponseFreshFallbackShape::SessionScopedFreshReplay),
        Some(RuntimePreviousResponseFreshFallbackShape::ContextDependentContinuation),
    ];
    for route in [
        RuntimePreviousResponseNotFoundRoute::Responses,
        RuntimePreviousResponseNotFoundRoute::Websocket,
    ] {
        for previous_response_present in [false, true] {
            for has_turn_state_retry in [false, true] {
                for request_requires_previous_response_affinity in [false, true] {
                    for trusted_previous_response_affinity in [false, true] {
                        for request_turn_state_present in [false, true] {
                            for previous_response_fresh_fallback_used in [false, true] {
                                for fresh_fallback_shape in shapes {
                                    for retry_index in 0..=4 {
                                        let input = RuntimePreviousResponseNotFoundDecisionInput {
                                            route,
                                            previous_response_id: previous_response_present
                                                .then_some("resp_1"),
                                            has_turn_state_retry,
                                            request_requires_previous_response_affinity,
                                            trusted_previous_response_affinity,
                                            request_turn_state: request_turn_state_present
                                                .then_some("state"),
                                            previous_response_fresh_fallback_used,
                                            fresh_fallback_shape,
                                            retry_index,
                                        };
                                        assert_eq!(
                                            runtime_previous_response_not_found_decision(input),
                                            runtime_previous_response_not_found_decision_rust(
                                                input
                                            ),
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
