use super::*;

#[test]
fn runtime_request_previous_response_fresh_fallback_shape_matrix_stays_fail_closed() {
    for shape in PreviousResponseFreshFallbackRequestShape::ALL {
        for session in RuntimeRequestSessionPlacement::ALL {
            let request = session
                .apply(shape.request().previous_response_id("resp_123"))
                .build();
            let actual = runtime_request_previous_response_fresh_fallback_shape(&request);
            let actual_label = runtime_previous_response_fresh_fallback_shape_label(actual);
            let expected_label = shape.expected_shape_label(session.has_session());

            assert_eq!(
                actual_label,
                expected_label,
                "shape={} session={}",
                shape.label(),
                session.label()
            );
            assert_eq!(
                runtime_previous_response_fresh_fallback_shape_allows_recovery(actual),
                shape.can_drop_previous_response_id(session.has_session()),
                "drop previous_response_id safety mismatch for shape={} session={}",
                shape.label(),
                session.label()
            );
        }
    }
}

#[test]
fn websocket_previous_response_fresh_fallback_shape_matrix_promotes_only_safe_session_shapes() {
    for shape in PreviousResponseFreshFallbackRequestShape::ALL {
        for session in WebsocketSessionPlacement::ALL {
            let request_text = websocket_previous_response_fresh_fallback_request_text(
                shape,
                session.uses_client_metadata(),
            );
            let metadata = parse_runtime_websocket_request_metadata(&request_text);
            let actual = runtime_previous_response_fresh_fallback_shape_with_session(
                metadata.previous_response_fresh_fallback_shape,
                session.promotes_header(),
            );
            let actual_label = runtime_previous_response_fresh_fallback_shape_label(actual);
            let expected_label = shape.expected_shape_label(session.has_session());

            assert_eq!(
                metadata.previous_response_id.as_deref(),
                Some("resp_123"),
                "shape={} session={}",
                shape.label(),
                session.label()
            );
            assert_eq!(
                metadata.session_id.as_deref(),
                session.uses_client_metadata().then_some("sess_123"),
                "shape={} session={}",
                shape.label(),
                session.label()
            );
            assert_eq!(
                actual_label,
                expected_label,
                "shape={} session={}",
                shape.label(),
                session.label()
            );
            assert_eq!(
                runtime_previous_response_fresh_fallback_shape_allows_recovery(actual),
                shape.can_drop_previous_response_id(session.has_session()),
                "websocket drop previous_response_id safety mismatch for shape={} session={}",
                shape.label(),
                session.label()
            );
        }
    }
}

#[test]
fn runtime_request_does_not_strip_previous_response_id_from_continuations() {
    let request = tool_output_only_request()
        .previous_response_id("resp_123")
        .build();

    assert!(
        runtime_request_without_previous_response_id(&request).is_none(),
        "runtime must not manufacture a fresh request from a previous_response continuation"
    );
    assert!(
        runtime_request_requires_previous_response_affinity(&request),
        "function call outputs should keep previous_response affinity during normal proxying"
    );
}

#[test]
fn runtime_request_text_does_not_strip_previous_response_id_from_continuations() {
    let request_text = serde_json::json!({
        "previous_response_id": "resp_123",
        "session_id": "sess_123",
        "input": [],
    })
    .to_string();

    assert!(
        runtime_request_text_without_previous_response_id(&request_text).is_none(),
        "websocket continuations must not drop previous_response_id even with session metadata"
    );
}

#[test]
fn runtime_request_allows_fresh_function_call_output_replay_without_previous_response_id() {
    let request = replayable_function_call_transcript_request().build();

    assert!(
        !runtime_request_requires_previous_response_affinity(&request),
        "fresh replayable transcripts should not be pinned just because they include call outputs"
    );
}

#[test]
fn runtime_request_previous_response_fresh_fallback_shape_classifies_tool_output_only() {
    let request = tool_output_only_request()
        .previous_response_id("resp_123")
        .build();

    assert_eq!(
        runtime_request_previous_response_fresh_fallback_shape(&request),
        Some(RuntimePreviousResponseFreshFallbackShape::ToolOutputOnly)
    );
    assert_eq!(
        runtime_previous_response_fresh_fallback_shape_label(
            runtime_request_previous_response_fresh_fallback_shape(&request)
        ),
        "tool_output_only"
    );
}

#[test]
fn runtime_request_previous_response_fresh_fallback_shape_blocks_session_tool_output_replay() {
    let request = tool_output_only_request()
        .previous_response_id("resp_123")
        .session_id("sess_123")
        .build();

    assert_eq!(
        runtime_request_previous_response_fresh_fallback_shape(&request),
        Some(RuntimePreviousResponseFreshFallbackShape::ToolOutputOnly)
    );
    assert!(
        !runtime_previous_response_fresh_fallback_shape_allows_recovery(
            runtime_request_previous_response_fresh_fallback_shape(&request)
        ),
        "session-scoped tool outputs still need previous_response tool-call context"
    );
}

#[test]
fn runtime_request_previous_response_fresh_fallback_shape_blocks_header_session_tool_output_replay()
{
    let request = tool_output_only_request()
        .previous_response_id("resp_123")
        .session_header("sess_123")
        .build();

    assert_eq!(
        runtime_request_previous_response_fresh_fallback_shape(&request),
        Some(RuntimePreviousResponseFreshFallbackShape::ToolOutputOnly)
    );
    assert!(
        !runtime_previous_response_fresh_fallback_shape_allows_recovery(
            runtime_request_previous_response_fresh_fallback_shape(&request)
        ),
        "explicit session headers must not fresh-replay bare tool outputs"
    );
}

#[test]
fn runtime_request_previous_response_fresh_fallback_shape_blocks_message_followups() {
    let request = message_followup_request()
        .previous_response_id("resp_123")
        .build();

    assert_eq!(
        runtime_request_previous_response_fresh_fallback_shape(&request),
        Some(RuntimePreviousResponseFreshFallbackShape::ContextDependentContinuation)
    );
    assert!(
        !runtime_previous_response_fresh_fallback_shape_allows_recovery(
            runtime_request_previous_response_fresh_fallback_shape(&request)
        ),
        "plain message follow-ups still depend on previous_response chain context"
    );
}

#[test]
fn runtime_request_previous_response_fresh_fallback_shape_blocks_header_session_message_followups()
{
    let request = message_followup_request()
        .previous_response_id("resp_123")
        .session_header("sess_123")
        .build();

    assert_eq!(
        runtime_request_previous_response_fresh_fallback_shape(&request),
        Some(RuntimePreviousResponseFreshFallbackShape::ContextDependentContinuation)
    );
    assert!(
        !runtime_previous_response_fresh_fallback_shape_allows_recovery(
            runtime_request_previous_response_fresh_fallback_shape(&request)
        ),
        "session headers must not make incremental message follow-ups replayable"
    );
}

#[test]
fn runtime_request_previous_response_fresh_fallback_shape_blocks_turn_metadata_session_message_followups()
 {
    let request = message_followup_request()
        .previous_response_id("resp_123")
        .turn_metadata_session("sess_123")
        .build();

    assert_eq!(
        runtime_request_previous_response_fresh_fallback_shape(&request),
        Some(RuntimePreviousResponseFreshFallbackShape::ContextDependentContinuation)
    );
    assert!(
        !runtime_previous_response_fresh_fallback_shape_allows_recovery(
            runtime_request_previous_response_fresh_fallback_shape(&request)
        ),
        "turn metadata session ids must not make incremental message follow-ups replayable"
    );
}

#[test]
fn runtime_request_previous_response_fresh_fallback_shape_classifies_session_scoped_empty_input() {
    let request = empty_input_request()
        .previous_response_id("resp_123")
        .session_id("sess_123")
        .build();

    assert_eq!(
        runtime_request_previous_response_fresh_fallback_shape(&request),
        Some(RuntimePreviousResponseFreshFallbackShape::SessionScopedFreshReplay)
    );
    assert!(
        !runtime_previous_response_fresh_fallback_shape_allows_recovery(
            runtime_request_previous_response_fresh_fallback_shape(&request)
        ),
        "session_id is affinity metadata, not replay state"
    );
}

#[test]
fn runtime_request_previous_response_fresh_fallback_shape_blocks_empty_continuation_payloads() {
    let request = empty_input_request()
        .previous_response_id("resp_123")
        .build();

    assert_eq!(
        runtime_request_previous_response_fresh_fallback_shape(&request),
        Some(RuntimePreviousResponseFreshFallbackShape::EmptyInputOnly)
    );
    assert!(
        !runtime_previous_response_fresh_fallback_shape_allows_recovery(
            runtime_request_previous_response_fresh_fallback_shape(&request)
        )
    );
}

#[test]
fn runtime_request_previous_response_fresh_fallback_shape_marks_header_session_empty_input() {
    let request = empty_input_request()
        .previous_response_id("resp_123")
        .session_header("sess_123")
        .build();

    assert_eq!(
        runtime_request_previous_response_fresh_fallback_shape(&request),
        Some(RuntimePreviousResponseFreshFallbackShape::SessionScopedFreshReplay)
    );
    assert!(
        !runtime_previous_response_fresh_fallback_shape_allows_recovery(
            runtime_request_previous_response_fresh_fallback_shape(&request)
        ),
        "session headers must not make empty previous_response continuations replayable"
    );
}

#[test]
fn runtime_previous_response_not_found_decision_matrix_stays_consistent() {
    for route in [
        RuntimePreviousResponseNotFoundRoute::Responses,
        RuntimePreviousResponseNotFoundRoute::Websocket,
    ] {
        for has_turn_state_retry in [false, true] {
            for trusted_previous_response_affinity in [false, true] {
                for fallback_case in PreviousResponseNotFoundDecisionFallbackCase::ALL {
                    for retry_index in 0..=RUNTIME_PREVIOUS_RESPONSE_RETRY_DELAYS_MS.len() {
                        let request_requires_previous_response_affinity =
                            fallback_case.request_requires_previous_response_affinity();
                        let request_turn_state = has_turn_state_retry.then_some("turn_state");
                        let request_requires_locked_previous_response_affinity = match route {
                            RuntimePreviousResponseNotFoundRoute::Responses => {
                                request_requires_previous_response_affinity
                            }
                            RuntimePreviousResponseNotFoundRoute::Websocket => {
                                request_requires_previous_response_affinity
                                    || (trusted_previous_response_affinity
                                        && request_turn_state.is_none())
                            }
                        };
                        let locked_previous_response_retry =
                            matches!(route, RuntimePreviousResponseNotFoundRoute::Websocket)
                                && request_requires_previous_response_affinity
                                && !has_turn_state_retry;
                        let expected_retry_delay = (has_turn_state_retry
                            || locked_previous_response_retry)
                            .then(|| {
                                RUNTIME_PREVIOUS_RESPONSE_RETRY_DELAYS_MS
                                    .get(retry_index)
                                    .copied()
                                    .map(Duration::from_millis)
                            })
                            .flatten();
                        let expected_retry_reason = if has_turn_state_retry {
                            Some("non_blocking_retry")
                        } else if locked_previous_response_retry {
                            Some("locked_affinity_no_turn_state")
                        } else {
                            None
                        };
                        let expected_chain_retry_reason = match route {
                            RuntimePreviousResponseNotFoundRoute::Responses
                                if has_turn_state_retry =>
                            {
                                Some("previous_response_not_found")
                            }
                            RuntimePreviousResponseNotFoundRoute::Websocket
                                if locked_previous_response_retry =>
                            {
                                Some("previous_response_not_found_locked_affinity")
                            }
                            _ => None,
                        };
                        let expected_stale_continuation = !has_turn_state_retry;
                        let expected_fresh_fallback_blocked_without_affinity = !has_turn_state_retry
                            && !request_requires_locked_previous_response_affinity;
                        let expected_observability =
                            if expected_fresh_fallback_blocked_without_affinity {
                                match fallback_case.fresh_fallback_shape() {
                                Some(
                                    RuntimePreviousResponseFreshFallbackShape::ContextDependentContinuation,
                                ) => Some("blocked_nonreplayable_without_affinity"),
                                _ => Some("blocked_without_affinity"),
                            }
                            } else {
                                None
                            };
                        let case_label = format!(
                            "route={} turn_state={} trusted_affinity={} fallback={} retry_index={}",
                            previous_response_not_found_route_label(route),
                            has_turn_state_retry,
                            trusted_previous_response_affinity,
                            fallback_case.label(),
                            retry_index
                        );

                        let decision = runtime_previous_response_not_found_decision(
                            RuntimePreviousResponseNotFoundDecisionInput {
                                route,
                                previous_response_id: Some("resp_123"),
                                has_turn_state_retry,
                                request_requires_previous_response_affinity,
                                trusted_previous_response_affinity,
                                request_turn_state,
                                previous_response_fresh_fallback_used: false,
                                fresh_fallback_shape: fallback_case.fresh_fallback_shape(),
                                retry_index,
                            },
                        );

                        assert_eq!(
                            decision.request_requires_locked_previous_response_affinity,
                            request_requires_locked_previous_response_affinity,
                            "{}",
                            case_label
                        );
                        assert_eq!(decision.retry_delay, expected_retry_delay, "{}", case_label);
                        assert_eq!(
                            decision.retry_reason, expected_retry_reason,
                            "{}",
                            case_label
                        );
                        assert_eq!(
                            decision.chain_retry_reason, expected_chain_retry_reason,
                            "{}",
                            case_label
                        );
                        assert_eq!(
                            decision.stale_continuation, expected_stale_continuation,
                            "{}",
                            case_label
                        );
                        assert!(
                            !decision.fresh_fallback_allowed,
                            "{} should stay fail-closed for fresh fallback",
                            case_label
                        );
                        assert_eq!(
                            decision.fresh_fallback_blocked_without_affinity,
                            expected_fresh_fallback_blocked_without_affinity,
                            "{}",
                            case_label
                        );
                        assert_eq!(
                            runtime_previous_response_not_found_observability_outcome(
                                decision,
                                fallback_case.fresh_fallback_shape()
                            ),
                            expected_observability,
                            "{}",
                            case_label
                        );
                    }
                }
            }
        }
    }
}
