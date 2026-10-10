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

#[test]
fn websocket_failure_state_plan_adapter_exercises_every_class_and_tie() {
    let cases = [
        (RuntimeWebsocketFailureKind::RateLimited, true, false),
        (RuntimeWebsocketFailureKind::AuthFailed, true, false),
        (RuntimeWebsocketFailureKind::Overloaded, true, false),
        (
            RuntimeWebsocketFailureKind::LocalSelectionBlocked,
            true,
            true,
        ),
        (
            RuntimeWebsocketFailureKind::LocalSelectionBlocked,
            false,
            true,
        ),
    ];
    let mut confirmed = 0;
    for (kind, affinity_releasable, inflight_saturated) in cases {
        let plan = runtime_websocket_failure_state_plan(kind, affinity_releasable);
        if matches!(kind, RuntimeWebsocketFailureKind::LocalSelectionBlocked) {
            let disposition =
                runtime_websocket_failure_disposition(affinity_releasable, inflight_saturated);
            assert_eq!(disposition.mark_backoff, !inflight_saturated);
            assert_eq!(disposition.continue_selection, affinity_releasable);
        }
        assert_eq!(
            plan.clear_affinity,
            matches!(
                kind,
                RuntimeWebsocketFailureKind::AuthFailed
                    | RuntimeWebsocketFailureKind::LocalSelectionBlocked
            ) && affinity_releasable
        );
        confirmed += 1;
    }
    assert!(confirmed > 0);
}

#[test]
fn websocket_failure_decision_adapter_covers_affinity_pressure_transport_and_commit() {
    let cases = [
        (
            RuntimeWebsocketFailureDecisionInput {
                failure_class: RuntimeWebsocketFailureClass::QuotaBlocked,
                stream_committed: false,
                hard_affinity: false,
                affinity_releasable: true,
                inflight_saturated: false,
                full_context_retry_available: false,
                quota_fallback_available: true,
                direct_current_fallback: false,
                reuse_existing_session: false,
                precommit_transport_retry_allowed: false,
                reset_retry_index: false,
            },
            RuntimeWebsocketFailureAction::Rotate,
        ),
        (
            RuntimeWebsocketFailureDecisionInput {
                failure_class: RuntimeWebsocketFailureClass::AuthFailed,
                stream_committed: false,
                hard_affinity: true,
                affinity_releasable: false,
                inflight_saturated: false,
                full_context_retry_available: true,
                quota_fallback_available: true,
                direct_current_fallback: false,
                reuse_existing_session: false,
                precommit_transport_retry_allowed: false,
                reset_retry_index: false,
            },
            RuntimeWebsocketFailureAction::FullContextRetry,
        ),
        (
            RuntimeWebsocketFailureDecisionInput {
                failure_class: RuntimeWebsocketFailureClass::LocalSelectionBlocked,
                stream_committed: false,
                hard_affinity: false,
                affinity_releasable: true,
                inflight_saturated: true,
                full_context_retry_available: false,
                quota_fallback_available: true,
                direct_current_fallback: false,
                reuse_existing_session: false,
                precommit_transport_retry_allowed: false,
                reset_retry_index: false,
            },
            RuntimeWebsocketFailureAction::Continue,
        ),
        (
            RuntimeWebsocketFailureDecisionInput {
                failure_class: RuntimeWebsocketFailureClass::TransportFailed,
                stream_committed: true,
                hard_affinity: false,
                affinity_releasable: true,
                inflight_saturated: false,
                full_context_retry_available: false,
                quota_fallback_available: false,
                direct_current_fallback: false,
                reuse_existing_session: false,
                precommit_transport_retry_allowed: true,
                reset_retry_index: false,
            },
            RuntimeWebsocketFailureAction::Error,
        ),
    ];
    let mut confirmed = 0;
    for (input, expected) in cases {
        assert_eq!(
            runtime_websocket_failure_decision(input).unwrap().action,
            expected
        );
        confirmed += 1;
    }
    assert_eq!(confirmed, 4);
}
