use std::time::Duration;

use super::*;

#[test]
fn plan_retries_owner_before_releasing_affinity() {
    let plan = runtime_previous_response_not_found_plan(RuntimePreviousResponseNotFoundPlanInput {
        route: RuntimePreviousResponseNotFoundRoute::Responses,
        previous_response_id: Some("resp_1"),
        has_turn_state_retry: true,
        request_requires_previous_response_affinity: true,
        trusted_previous_response_affinity: true,
        request_turn_state: Some("turn_state"),
        previous_response_fresh_fallback_used: false,
        fresh_fallback_shape: Some(
            RuntimePreviousResponseFreshFallbackShape::ContextDependentContinuation,
        ),
        retry_index: 1,
        policy: RuntimePreviousResponseNotFoundPolicy::responses(false),
    });

    assert_eq!(
        plan.action,
        RuntimePreviousResponseNotFoundAction::RetryOwner
    );
    assert_eq!(plan.decision.retry_delay, Some(Duration::from_millis(200)));
    assert_eq!(plan.next_retry_index, 2);
    assert!(!plan.reset_retry_state);
    assert!(!plan.release_affinity);
    assert!(!plan.clear_response_profile_affinity);
}

#[test]
fn websocket_plan_fails_stale_continuation_closed() {
    let plan = runtime_previous_response_not_found_plan(RuntimePreviousResponseNotFoundPlanInput {
        route: RuntimePreviousResponseNotFoundRoute::Websocket,
        previous_response_id: Some("resp_1"),
        has_turn_state_retry: false,
        request_requires_previous_response_affinity: false,
        trusted_previous_response_affinity: true,
        request_turn_state: None,
        previous_response_fresh_fallback_used: false,
        fresh_fallback_shape: Some(
            RuntimePreviousResponseFreshFallbackShape::ContextDependentContinuation,
        ),
        retry_index: 3,
        policy: RuntimePreviousResponseNotFoundPolicy::websocket(false, true),
    });

    assert_eq!(
        plan.action,
        RuntimePreviousResponseNotFoundAction::StaleContinuation
    );
    assert_eq!(plan.next_retry_index, 0);
    assert!(plan.reset_retry_state);
    assert!(!plan.release_affinity);
    assert!(!plan.clear_trusted_affinity);
}

#[test]
fn responses_plan_rotates_after_nonreplayable_affinity_failure() {
    let plan = runtime_previous_response_not_found_plan(RuntimePreviousResponseNotFoundPlanInput {
        route: RuntimePreviousResponseNotFoundRoute::Responses,
        previous_response_id: Some("resp_1"),
        has_turn_state_retry: false,
        request_requires_previous_response_affinity: false,
        trusted_previous_response_affinity: true,
        request_turn_state: None,
        previous_response_fresh_fallback_used: false,
        fresh_fallback_shape: Some(
            RuntimePreviousResponseFreshFallbackShape::ContextDependentContinuation,
        ),
        retry_index: 0,
        policy: RuntimePreviousResponseNotFoundPolicy::responses(true),
    });

    assert_eq!(plan.action, RuntimePreviousResponseNotFoundAction::Rotate);
    assert!(plan.decision.stale_continuation);
    assert!(plan.log_fresh_fallback_blocked);
    assert!(plan.release_affinity);
    assert!(plan.clear_response_profile_affinity);
    assert!(plan.clear_trusted_affinity);
}
