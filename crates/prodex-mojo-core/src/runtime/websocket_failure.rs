use super::{
    CONTINUATION_WEBSOCKET_INVALID_PREVIOUS_RESPONSE_PLAN, continuation_status_transition,
    selection_planning::WebsocketChainReuseReason,
    selection_planning::{
        WebsocketInvalidPreviousResponseAction, WebsocketInvalidPreviousResponsePlan,
    },
};
use crate::MojoError;

const WEBSOCKET_FAILURE_DECISION_ABI_VERSION: i64 = 2;
const WEBSOCKET_FAILURE_DECISION_FIELD_COUNT: usize = 12;

const WEBSOCKET_FAILURE_PLAN_FIELD_COUNT: usize = 5;

/// Failure class whose pre-commit state transition is planned by Mojo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebsocketFailureKind {
    RateLimited,
    AuthFailed,
    Overloaded,
    LocalSelectionBlocked,
}

/// Side-effect instructions for one pre-commit WebSocket failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WebsocketFailureStatePlan {
    pub clear_affinity: bool,
    pub store_last_failure: bool,
    pub last_failure_retryable: bool,
    pub record_rate_limit_failure: bool,
    pub record_overload_failure: bool,
}

/// Failure classes accepted by the versioned WebSocket pre-commit decision ABI.
#[repr(i64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebsocketFailureClass {
    RateLimited = 0,
    AuthFailed = 1,
    Overloaded = 2,
    LocalSelectionBlocked = 3,
    QuotaBlocked = 4,
    TransportFailed = 5,
    PreviousResponseNotFound = 6,
    Rejected = 7,
}

/// Host actions selected by Mojo; Rust applies the socket and state effects.
#[repr(i64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebsocketFailureAction {
    PassThrough = 0,
    Rotate = 1,
    FullContextRetry = 2,
    RetryTransport = 3,
    ReuseWatchdog = 4,
    Continue = 5,
    Error = 6,
}

/// Deterministic WebSocket failure state and terminal-action plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WebsocketFailureDecisionPlan {
    pub action: WebsocketFailureAction,
    pub mark_backoff: bool,
    pub exclude_profile: bool,
    pub clear_affinity: bool,
    pub retryable_failure: bool,
    pub record_rate_limit_failure: bool,
    pub record_overload_failure: bool,
    pub store_last_failure: bool,
    pub last_failure_retryable: bool,
    pub release_affinity: bool,
    pub terminal: bool,
    pub reset_retry_index: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WebsocketFailureDecisionInput {
    pub failure_class: WebsocketFailureClass,
    pub stream_committed: bool,
    pub hard_affinity: bool,
    pub affinity_releasable: bool,
    pub inflight_saturated: bool,
    pub full_context_retry_available: bool,
    pub quota_fallback_available: bool,
    pub direct_current_fallback: bool,
    pub reuse_existing_session: bool,
    pub precommit_transport_retry_allowed: bool,
    pub reset_retry_index: bool,
}

unsafe extern "C" {
    fn prodex_runtime_websocket_failure_state_plan_v1(
        failure_kind: i64,
        affinity_releasable: i64,
        output: *mut i64,
    ) -> i64;
    fn prodex_runtime_websocket_failure_decision_v2(
        abi_version: i64,
        failure_kind: i64,
        stream_committed: i64,
        hard_affinity: i64,
        affinity_releasable: i64,
        inflight_saturated: i64,
        full_context_retry_available: i64,
        quota_fallback_available: i64,
        direct_current_fallback: i64,
        reuse_existing_session: i64,
        precommit_transport_retry_allowed: i64,
        reset_retry_index: i64,
        output: *mut i64,
    ) -> i64;
    fn prodex_runtime_websocket_failure_frame_classification_v1(
        abi_version: i64,
        http_class: i64,
        http_action: i64,
        connection_limit: i64,
        previous_response_not_found: i64,
        stream_committed: i64,
    ) -> i64;
}

/// Runs the checked versioned failure decision without a Rust fallback.
pub fn websocket_failure_decision(
    input: WebsocketFailureDecisionInput,
) -> Result<WebsocketFailureDecisionPlan, MojoError> {
    let mut output = [-1_i64; WEBSOCKET_FAILURE_DECISION_FIELD_COUNT];
    let status = unsafe {
        prodex_runtime_websocket_failure_decision_v2(
            WEBSOCKET_FAILURE_DECISION_ABI_VERSION,
            input.failure_class as i64,
            i64::from(input.stream_committed),
            i64::from(input.hard_affinity),
            i64::from(input.affinity_releasable),
            i64::from(input.inflight_saturated),
            i64::from(input.full_context_retry_available),
            i64::from(input.quota_fallback_available),
            i64::from(input.direct_current_fallback),
            i64::from(input.reuse_existing_session),
            i64::from(input.precommit_transport_retry_allowed),
            i64::from(input.reset_retry_index),
            output.as_mut_ptr(),
        )
    };
    if status == 4 {
        return Err(MojoError::AbiMismatch);
    }
    if status != 0 || output[1..].iter().any(|value| !matches!(value, 0 | 1)) {
        return Err(if status == 1 {
            MojoError::InvalidInput
        } else {
            MojoError::InvalidOutput
        });
    }
    let action = match output[0] {
        0 => WebsocketFailureAction::PassThrough,
        1 => WebsocketFailureAction::Rotate,
        2 => WebsocketFailureAction::FullContextRetry,
        3 => WebsocketFailureAction::RetryTransport,
        4 => WebsocketFailureAction::ReuseWatchdog,
        5 => WebsocketFailureAction::Continue,
        6 => WebsocketFailureAction::Error,
        _ => return Err(MojoError::InvalidOutput),
    };
    Ok(WebsocketFailureDecisionPlan {
        action,
        mark_backoff: output[1] == 1,
        exclude_profile: output[2] == 1,
        clear_affinity: output[3] == 1,
        retryable_failure: output[4] == 1,
        record_rate_limit_failure: output[5] == 1,
        record_overload_failure: output[6] == 1,
        store_last_failure: output[7] == 1,
        last_failure_retryable: output[8] == 1,
        release_affinity: output[9] == 1,
        terminal: output[10] == 1,
        reset_retry_index: output[11] == 1,
    })
}

/// Classifies a parsed WebSocket error after Rust has collected JSON facts.
pub fn websocket_failure_frame_classification(
    http_class: i64,
    http_action: i64,
    connection_limit: bool,
    previous_response_not_found: bool,
    stream_committed: bool,
) -> Result<i64, MojoError> {
    let result = unsafe {
        prodex_runtime_websocket_failure_frame_classification_v1(
            1,
            http_class,
            http_action,
            i64::from(connection_limit),
            i64::from(previous_response_not_found),
            i64::from(stream_committed),
        )
    };
    match result {
        0..=5 => Ok(result),
        -4 => Err(MojoError::AbiMismatch),
        -1 => Err(MojoError::InvalidInput),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Applies continuation-status precedence to an invalid WebSocket response id.
pub fn websocket_invalid_previous_response_recovery_plan(
    previous_response_present: bool,
    session_present: bool,
    owner_matches: bool,
    owner_generation_present: bool,
    owner_generation_matches: bool,
    recovery_available: bool,
    stream_committed: bool,
) -> Result<WebsocketInvalidPreviousResponsePlan, MojoError> {
    let output = continuation_status_transition::<5>(
        CONTINUATION_WEBSOCKET_INVALID_PREVIOUS_RESPONSE_PLAN,
        &[
            i64::from(previous_response_present),
            i64::from(session_present),
            i64::from(owner_matches),
            i64::from(owner_generation_present),
            i64::from(owner_generation_matches),
            i64::from(recovery_available),
            i64::from(stream_committed),
        ],
    )?;
    let chain_reuse_reason = match output[2] {
        0 => WebsocketChainReuseReason::UpstreamReconnect,
        1 => WebsocketChainReuseReason::BoundProfileAffinity,
        2 => WebsocketChainReuseReason::UnboundPreviousResponse,
        _ => return Err(MojoError::InvalidOutput),
    };
    let action = match output[0] {
        0 => WebsocketInvalidPreviousResponseAction::PassThrough,
        1 => WebsocketInvalidPreviousResponseAction::FullContextRetry,
        _ => return Err(MojoError::InvalidOutput),
    };
    Ok(WebsocketInvalidPreviousResponsePlan {
        recovery_signal: output[3] == 1,
        crossed_transport_generation: output[1] == 1,
        chain_reuse_reason,
        action,
    })
}

/// Plans state transitions while Rust retains all WebSocket effects.
pub fn websocket_failure_state_plan(
    failure_kind: WebsocketFailureKind,
    affinity_releasable: bool,
) -> Result<WebsocketFailureStatePlan, MojoError> {
    let failure_kind = match failure_kind {
        WebsocketFailureKind::RateLimited => 0,
        WebsocketFailureKind::AuthFailed => 1,
        WebsocketFailureKind::Overloaded => 2,
        WebsocketFailureKind::LocalSelectionBlocked => 3,
    };
    let mut output = [-1_i64; WEBSOCKET_FAILURE_PLAN_FIELD_COUNT];
    let status = unsafe {
        prodex_runtime_websocket_failure_state_plan_v1(
            failure_kind,
            i64::from(affinity_releasable),
            output.as_mut_ptr(),
        )
    };
    if status != 0 || output.iter().any(|value| !matches!(value, 0 | 1)) {
        return Err(MojoError::InvalidOutput);
    }
    Ok(WebsocketFailureStatePlan {
        clear_affinity: output[0] == 1,
        store_last_failure: output[1] == 1,
        last_failure_retryable: output[2] == 1,
        record_rate_limit_failure: output[3] == 1,
        record_overload_failure: output[4] == 1,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decision_input(
        failure_class: WebsocketFailureClass,
        hard_affinity: bool,
        inflight_saturated: bool,
        full_context_retry_available: bool,
        quota_fallback_available: bool,
    ) -> WebsocketFailureDecisionInput {
        WebsocketFailureDecisionInput {
            failure_class,
            stream_committed: false,
            hard_affinity,
            affinity_releasable: !hard_affinity,
            inflight_saturated,
            full_context_retry_available,
            quota_fallback_available,
            direct_current_fallback: false,
            reuse_existing_session: false,
            precommit_transport_retry_allowed: false,
            reset_retry_index: false,
        }
    }

    #[test]
    fn failure_decision_matrix_keeps_affinity_and_failure_precedence() {
        let cases = [
            (
                decision_input(
                    WebsocketFailureClass::QuotaBlocked,
                    true,
                    false,
                    false,
                    true,
                ),
                WebsocketFailureAction::PassThrough,
            ),
            (
                decision_input(WebsocketFailureClass::QuotaBlocked, true, false, true, true),
                WebsocketFailureAction::FullContextRetry,
            ),
            (
                decision_input(
                    WebsocketFailureClass::QuotaBlocked,
                    false,
                    false,
                    false,
                    true,
                ),
                WebsocketFailureAction::Rotate,
            ),
            (
                decision_input(
                    WebsocketFailureClass::RateLimited,
                    false,
                    false,
                    false,
                    true,
                ),
                WebsocketFailureAction::Rotate,
            ),
            (
                decision_input(WebsocketFailureClass::AuthFailed, false, false, false, true),
                WebsocketFailureAction::Rotate,
            ),
            (
                decision_input(WebsocketFailureClass::Overloaded, true, false, true, true),
                WebsocketFailureAction::FullContextRetry,
            ),
            (
                decision_input(
                    WebsocketFailureClass::LocalSelectionBlocked,
                    false,
                    true,
                    false,
                    true,
                ),
                WebsocketFailureAction::Continue,
            ),
            (
                decision_input(
                    WebsocketFailureClass::LocalSelectionBlocked,
                    true,
                    true,
                    false,
                    true,
                ),
                WebsocketFailureAction::PassThrough,
            ),
            (
                decision_input(
                    WebsocketFailureClass::TransportFailed,
                    false,
                    false,
                    false,
                    false,
                ),
                WebsocketFailureAction::Rotate,
            ),
        ];
        let mut confirmed = 0;
        for (input, expected) in cases {
            let plan = websocket_failure_decision(input).expect("valid WebSocket decision");
            assert_eq!(plan.action, expected);
            assert_eq!(
                plan.terminal,
                matches!(expected, WebsocketFailureAction::PassThrough)
            );
            confirmed += 1;
        }
        assert_eq!(confirmed, 9);

        let committed = websocket_failure_decision(WebsocketFailureDecisionInput {
            stream_committed: true,
            ..decision_input(WebsocketFailureClass::Overloaded, true, false, true, true)
        })
        .unwrap();
        assert_eq!(committed.action, WebsocketFailureAction::PassThrough);
        assert!(committed.terminal);
    }

    #[test]
    fn failure_frame_classification_preserves_connection_and_previous_response_precedence() {
        let cases = [
            (5, 2, true, true, false, 1),
            (0, 1, false, true, false, 5),
            (1, 2, false, false, false, 3),
            (3, 2, false, false, false, 4),
            (0, 1, false, false, false, 2),
            (1, 2, false, false, true, 0),
        ];
        let mut confirmed = 0;
        for (class, action, connection, previous, committed, expected) in cases {
            assert_eq!(
                websocket_failure_frame_classification(
                    class, action, connection, previous, committed
                )
                .unwrap(),
                expected
            );
            confirmed += 1;
        }
        assert_eq!(confirmed, 6);
    }

    #[test]
    fn failure_decision_abi_rejects_wrong_version_and_invalid_affinity_tie() {
        let mut output = [-1_i64; WEBSOCKET_FAILURE_DECISION_FIELD_COUNT];
        assert_eq!(
            unsafe {
                prodex_runtime_websocket_failure_decision_v2(
                    1,
                    0,
                    0,
                    0,
                    1,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                    output.as_mut_ptr(),
                )
            },
            4
        );
        assert_eq!(
            unsafe {
                prodex_runtime_websocket_failure_decision_v2(
                    WEBSOCKET_FAILURE_DECISION_ABI_VERSION,
                    0,
                    0,
                    1,
                    1,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                    output.as_mut_ptr(),
                )
            },
            1
        );
        assert_eq!(
            websocket_failure_frame_classification(9, 0, false, false, false),
            Err(MojoError::InvalidInput)
        );
    }

    #[test]
    fn continuation_recovery_matrix_keeps_generation_and_commit_boundaries() {
        let reconnect = websocket_invalid_previous_response_recovery_plan(
            true, true, true, true, false, true, false,
        )
        .unwrap();
        assert!(reconnect.recovery_signal);
        assert!(reconnect.crossed_transport_generation);
        assert_eq!(
            reconnect.chain_reuse_reason,
            WebsocketChainReuseReason::UpstreamReconnect
        );
        assert_eq!(
            reconnect.action,
            WebsocketInvalidPreviousResponseAction::FullContextRetry
        );

        let committed = websocket_invalid_previous_response_recovery_plan(
            true, true, true, true, false, true, true,
        )
        .unwrap();
        assert!(!committed.recovery_signal);
        assert_eq!(
            committed.action,
            WebsocketInvalidPreviousResponseAction::PassThrough
        );

        let unbound = websocket_invalid_previous_response_recovery_plan(
            true, true, false, false, false, true, false,
        )
        .unwrap();
        assert!(!unbound.recovery_signal);
        assert_eq!(
            unbound.chain_reuse_reason,
            WebsocketChainReuseReason::UnboundPreviousResponse
        );
    }

    const KINDS: [WebsocketFailureKind; 4] = [
        WebsocketFailureKind::RateLimited,
        WebsocketFailureKind::AuthFailed,
        WebsocketFailureKind::Overloaded,
        WebsocketFailureKind::LocalSelectionBlocked,
    ];

    #[test]
    fn failure_state_plan_covers_the_complete_affinity_pressure_matrix() {
        let mut confirmed = 0;
        for kind in KINDS {
            for affinity_releasable in [false, true] {
                let plan = websocket_failure_state_plan(kind, affinity_releasable)
                    .expect("Mojo websocket failure state plan should be valid");
                match kind {
                    WebsocketFailureKind::RateLimited => {
                        assert!(!plan.clear_affinity);
                        assert!(plan.store_last_failure);
                        assert!(!plan.last_failure_retryable);
                        assert!(plan.record_rate_limit_failure);
                        assert!(!plan.record_overload_failure);
                    }
                    WebsocketFailureKind::AuthFailed => {
                        assert_eq!(plan.clear_affinity, affinity_releasable);
                        assert!(plan.store_last_failure);
                        assert!(plan.last_failure_retryable);
                    }
                    WebsocketFailureKind::Overloaded => {
                        assert!(!plan.clear_affinity);
                        assert!(plan.store_last_failure);
                        assert!(!plan.last_failure_retryable);
                        assert!(plan.record_overload_failure);
                        assert!(!plan.record_rate_limit_failure);
                    }
                    WebsocketFailureKind::LocalSelectionBlocked => {
                        assert_eq!(plan.clear_affinity, affinity_releasable);
                        assert!(!plan.store_last_failure);
                    }
                }
                confirmed += 1;
            }
        }
        assert_eq!(confirmed, 8);
    }

    #[test]
    fn repeated_failures_keep_terminal_owner_and_releasable_candidates_distinct() {
        let mut excluded = false;
        let mut retryable_failures = 0;
        for (kind, affinity_releasable) in [
            (WebsocketFailureKind::RateLimited, true),
            (WebsocketFailureKind::RateLimited, true),
            (WebsocketFailureKind::Overloaded, true),
            (WebsocketFailureKind::Overloaded, false),
        ] {
            let plan = websocket_failure_state_plan(kind, affinity_releasable)
                .expect("repeated failure plan should be valid");
            if affinity_releasable {
                excluded = true;
            }
            if affinity_releasable && plan.store_last_failure && !plan.last_failure_retryable {
                retryable_failures += 1;
            }
            if !affinity_releasable {
                assert!(!plan.clear_affinity);
            }
        }
        assert!(excluded);
        assert_eq!(retryable_failures, 3);
    }
}
