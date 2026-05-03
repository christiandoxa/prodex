use super::*;

#[test]
fn websocket_previous_response_logs_keep_existing_fields() {
    let context = RuntimePreviousResponseLogContext {
        request_id: 17,
        transport: "websocket",
        route: "websocket",
        websocket_session: Some(23),
        via: Some("direct_current_profile_fallback"),
    };

    assert_eq!(
        runtime_previous_response_not_found_log_message(context, "alpha", 2, Some("turn-state"),),
        "request=17 transport=websocket route=websocket websocket_session=23 previous_response_not_found profile=alpha retry_index=2 replay_turn_state=Some(\"turn-state\") via=direct_current_profile_fallback"
    );
    assert_eq!(
        runtime_previous_response_retry_immediate_log_message(
            context,
            "alpha",
            250,
            "non_blocking_retry",
        ),
        "request=17 websocket_session=23 previous_response_retry_immediate profile=alpha delay_ms=250 reason=non_blocking_retry via=direct_current_profile_fallback"
    );
    assert_eq!(
        runtime_previous_response_stale_continuation_log_message(context, "alpha"),
        "request=17 websocket_session=23 stale_continuation reason=previous_response_not_found_locked_affinity profile=alpha via=direct_current_profile_fallback"
    );
    assert_eq!(
        runtime_previous_response_affinity_released_log_message(context, "alpha"),
        "request=17 websocket_session=23 previous_response_affinity_released profile=alpha via=direct_current_profile_fallback"
    );
}

#[test]
fn http_previous_response_logs_keep_existing_fields() {
    let context = RuntimePreviousResponseLogContext {
        request_id: 41,
        transport: "http",
        route: "responses",
        websocket_session: None,
        via: None,
    };

    assert_eq!(
        runtime_previous_response_not_found_log_message(context, "beta", 1, None),
        "request=41 transport=http route=responses previous_response_not_found profile=beta retry_index=1 replay_turn_state=None"
    );
    assert_eq!(
        runtime_previous_response_retry_immediate_log_message(context, "beta", 500, "-",),
        "request=41 transport=http previous_response_retry_immediate profile=beta delay_ms=500 reason=-"
    );
    assert_eq!(
        runtime_previous_response_affinity_released_log_message(context, "beta"),
        "request=41 transport=http previous_response_affinity_released profile=beta"
    );
    assert_eq!(
        runtime_previous_response_not_found_fresh_fallback_log_message(
            context,
            RuntimePreviousResponseNotFoundDecision {
                retry_delay: None,
                retry_reason: None,
                chain_retry_reason: None,
                request_requires_locked_previous_response_affinity: false,
                stale_continuation: false,
                fresh_fallback_allowed: false,
                fresh_fallback_blocked_without_affinity: true,
            },
            Some(RuntimePreviousResponseFreshFallbackShape::ContextDependentContinuation),
            "beta",
            true,
        ),
        "request=41 transport=http previous_response_fresh_fallback_blocked reason=previous_response_not_found request_shape=continuation_only outcome=blocked_nonreplayable_without_affinity profile=beta"
    );
}
