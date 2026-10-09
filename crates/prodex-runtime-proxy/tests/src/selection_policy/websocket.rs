use super::*;

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
