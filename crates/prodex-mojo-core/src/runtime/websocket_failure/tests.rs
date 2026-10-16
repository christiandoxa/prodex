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
            websocket_failure_frame_classification(class, action, connection, previous, committed)
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
fn stale_frame_plan_preserves_json_shape_precedence() {
    let cases = [
        (
            (false, false, false, false),
            WebsocketStaleFrameAction::Forward,
        ),
        (
            (true, false, false, false),
            WebsocketStaleFrameAction::GenericError,
        ),
        (
            (true, true, false, true),
            WebsocketStaleFrameAction::GenericError,
        ),
        (
            (true, true, true, false),
            WebsocketStaleFrameAction::FailedError,
        ),
        (
            (true, true, true, true),
            WebsocketStaleFrameAction::FailedResponseError,
        ),
    ];
    for ((previous, json, failed, response), expected) in cases {
        assert_eq!(
            websocket_stale_frame_plan(previous, json, failed, response),
            Ok(expected),
            "previous={previous} json={json} failed={failed} response={response}"
        );
    }
}

#[test]
fn stale_frame_plan_rejects_wrong_version_and_non_boolean_input() {
    assert_eq!(
        unsafe { prodex_runtime_websocket_stale_frame_plan_v1(0, 0, 0, 0, 0) },
        -4
    );
    assert_eq!(
        unsafe {
            prodex_runtime_websocket_stale_frame_plan_v1(
                WEBSOCKET_STALE_FRAME_ABI_VERSION,
                2,
                0,
                0,
                0,
            )
        },
        -1
    );
}

#[test]
fn terminal_frame_plan_preserves_code_precedence_and_status_fallback() {
    assert_eq!(
        websocket_terminal_frame_plan(
            Some("error"),
            Some("WEBSOCKET_CONNECTION_LIMIT_REACHED"),
            None,
            None,
            None,
        ),
        Ok(WebsocketTerminalFramePlan {
            connection_limit: true,
            terminal: true,
        })
    );
    assert_eq!(
        websocket_terminal_frame_plan(Some("error"), None, None, None, Some(400)),
        Ok(WebsocketTerminalFramePlan {
            connection_limit: false,
            terminal: true,
        })
    );
    assert_eq!(
        websocket_terminal_frame_plan(Some("error"), None, None, Some(399), Some(400)),
        Ok(WebsocketTerminalFramePlan {
            connection_limit: false,
            terminal: false,
        })
    );
    assert_eq!(
        websocket_terminal_frame_plan(Some("Error"), None, None, Some(400), None),
        Ok(WebsocketTerminalFramePlan {
            connection_limit: false,
            terminal: false,
        })
    );
}

#[test]
fn terminal_frame_plan_abi_rejects_wrong_version_and_invalid_text() {
    assert_eq!(
        unsafe {
            prodex_runtime_websocket_terminal_frame_plan_v1(
                WEBSOCKET_TERMINAL_FRAME_ABI_VERSION + 1,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
            )
        },
        -4
    );
    assert_eq!(
        websocket_terminal_frame_plan(Some("error"), None, None, None, None),
        Ok(WebsocketTerminalFramePlan {
            connection_limit: false,
            terminal: false,
        })
    );
    assert_eq!(
        unsafe {
            prodex_runtime_websocket_terminal_frame_plan_v1(
                WEBSOCKET_TERMINAL_FRAME_ABI_VERSION,
                0,
                0,
                2,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
            )
        },
        -1
    );
    let oversized = "x".repeat(262_145);
    assert_eq!(
        websocket_terminal_frame_plan(Some(&oversized), None, None, None, None),
        Err(MojoError::InvalidInput)
    );
    assert_eq!(
        unsafe {
            prodex_runtime_websocket_terminal_frame_plan_v1(
                WEBSOCKET_TERMINAL_FRAME_ABI_VERSION,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                1,
                65_536,
                0,
                0,
            )
        },
        -1
    );
}

#[test]
fn recovery_policy_covers_replay_and_context_constraint_edges() {
    for bits in 0_u8..32 {
        let previous_response_present = bits & 1 != 0;
        let turn_state_present = bits & 2 != 0;
        let turn_state_owner_matches = bits & 4 != 0;
        let compact_followup_present = bits & 8 != 0;
        let reconstructable_full_history = bits & 16 != 0;
        let expected = !previous_response_present
            && turn_state_present
            && turn_state_owner_matches
            && !compact_followup_present
            && reconstructable_full_history;
        assert_eq!(
            websocket_turn_state_replay_candidate(
                previous_response_present,
                turn_state_present,
                turn_state_owner_matches,
                compact_followup_present,
                reconstructable_full_history,
            ),
            Ok(expected),
            "replay mismatch for input mask {bits:05b}"
        );
    }

    assert!(!websocket_context_constraint(false, false, false, false, false, false).unwrap());
    assert!(websocket_context_constraint(false, false, true, false, false, false).unwrap());
    assert!(websocket_context_constraint(false, false, false, false, false, true).unwrap());
    assert!(websocket_context_constraint(true, false, false, false, false, false).unwrap());
}

#[test]
fn recovery_policy_abi_rejects_wrong_version_and_non_boolean_input() {
    assert_eq!(
        unsafe { prodex_runtime_websocket_turn_state_replay_candidate_v1(0, 0, 1, 1, 0, 1,) },
        -4
    );
    assert_eq!(
        unsafe {
            prodex_runtime_websocket_context_constraint_v1(
                WEBSOCKET_FAILURE_RECOVERY_ABI_VERSION,
                2,
                0,
                0,
                0,
                0,
                0,
            )
        },
        -1
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

#[test]
fn websocket_invalid_id_code_patch_mojo_requires_exact_class_missing_code() {
    for invalid in [false, true] {
        for error_object in [false, true] {
            for code in [false, true] {
                assert_eq!(
                    websocket_invalid_id_code_patch(invalid, error_object, code),
                    Ok(invalid && error_object && !code),
                    "{invalid}/{error_object}/{code}",
                );
            }
        }
    }
}

#[test]
fn websocket_invalid_id_code_patch_mojo_rejects_invalid_flags_and_version() {
    unsafe {
        assert_eq!(
            prodex_runtime_websocket_invalid_id_code_patch_v1(2, 1, 1, 0),
            -4
        );
        assert_eq!(
            prodex_runtime_websocket_invalid_id_code_patch_v1(1, 2, 1, 0),
            -1
        );
        assert_eq!(
            prodex_runtime_websocket_invalid_id_code_patch_v1(1, 1, 1, -1),
            -1
        );
    }
}
