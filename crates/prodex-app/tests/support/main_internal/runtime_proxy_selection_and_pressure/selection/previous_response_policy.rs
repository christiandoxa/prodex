use super::*;

#[test]
fn parse_runtime_websocket_request_metadata_extracts_affinity_fields() {
    let metadata = parse_runtime_websocket_request_metadata(
        r#"{"previous_response_id":"resp_123","client_metadata":{"session_id":"sess_123"},"input":[{"type":"function_call_output","call_id":"call_123","output":"ok"}]}"#,
    );

    assert_eq!(metadata.previous_response_id.as_deref(), Some("resp_123"));
    assert_eq!(metadata.session_id.as_deref(), Some("sess_123"));
    assert!(metadata.requires_previous_response_affinity);
    assert_eq!(
        metadata.previous_response_fresh_fallback_shape,
        Some(RuntimePreviousResponseFreshFallbackShape::ToolOutputOnly)
    );
    assert!(
        !runtime_previous_response_fresh_fallback_shape_allows_recovery(
            metadata.previous_response_fresh_fallback_shape
        ),
        "websocket metadata should keep tool outputs chained to prior tool-call context"
    );
}

#[test]
fn parse_runtime_websocket_request_metadata_blocks_message_followup_replay() {
    let metadata = parse_runtime_websocket_request_metadata(
        r#"{"previous_response_id":"resp_123","input":[{"type":"message","role":"user","content":[{"type":"input_text","text":"continue"}]}]}"#,
    );

    assert_eq!(metadata.previous_response_id.as_deref(), Some("resp_123"));
    assert_eq!(metadata.session_id.as_deref(), None);
    assert!(!metadata.requires_previous_response_affinity);
    assert_eq!(
        metadata.previous_response_fresh_fallback_shape,
        Some(RuntimePreviousResponseFreshFallbackShape::ContextDependentContinuation)
    );
    assert!(
        !runtime_previous_response_fresh_fallback_shape_allows_recovery(
            metadata.previous_response_fresh_fallback_shape
        ),
        "websocket message follow-ups should stay pinned to prior context"
    );
}

#[test]
fn websocket_session_header_does_not_make_message_followup_replayable() {
    let metadata = parse_runtime_websocket_request_metadata(
        r#"{"previous_response_id":"resp_123","input":[{"type":"message","role":"user","content":[{"type":"input_text","text":"continue"}]}]}"#,
    );

    let shape = runtime_previous_response_fresh_fallback_shape_with_session(
        metadata.previous_response_fresh_fallback_shape,
        true,
    );

    assert_eq!(
        shape,
        Some(RuntimePreviousResponseFreshFallbackShape::ContextDependentContinuation)
    );
    assert!(
        !runtime_previous_response_fresh_fallback_shape_allows_recovery(shape),
        "websocket session headers must not make incremental message follow-ups replayable"
    );
}

#[test]
fn runtime_previous_response_fresh_fallback_policy_is_explicitly_fail_closed() {
    let policy = runtime_previous_response_fresh_fallback_policy(
        RuntimePreviousResponseFreshFallbackPolicyInput {
            has_previous_response_context: true,
            request_requires_locked_previous_response_affinity: false,
            fresh_fallback_shape: Some(
                RuntimePreviousResponseFreshFallbackShape::SessionScopedFreshReplay,
            ),
        },
    );

    assert_eq!(
        policy,
        RuntimePreviousResponseFreshFallbackPolicy::FailClosed {
            request_shape:
                RuntimePreviousResponseFreshFallbackPolicyShape::SessionScopedFreshReplay,
        }
    );
    assert!(!policy.allows_fresh_fallback());
}

#[test]
fn runtime_previous_response_fresh_fallback_policy_is_not_applicable_without_context() {
    assert_eq!(
        runtime_previous_response_fresh_fallback_policy(
            RuntimePreviousResponseFreshFallbackPolicyInput {
                has_previous_response_context: false,
                request_requires_locked_previous_response_affinity: false,
                fresh_fallback_shape: None,
            }
        ),
        RuntimePreviousResponseFreshFallbackPolicy::NotApplicable
    );
}

#[test]
fn websocket_previous_response_not_found_fallback_policy_is_explicitly_fail_closed() {
    let policy = runtime_previous_response_not_found_fallback_policy(
        RuntimePreviousResponseNotFoundFallbackRequest {
            previous_response_id: Some("resp_123"),
            has_turn_state_retry: false,
            request_requires_locked_previous_response_affinity: false,
            previous_response_fresh_fallback_used: false,
            fresh_fallback_shape: Some(
                RuntimePreviousResponseFreshFallbackShape::ContextDependentContinuation,
            ),
        },
    );

    assert_eq!(
        policy.stale_continuation,
        RuntimePreviousResponseStaleContinuationPolicy::FailClosed
    );
    assert_eq!(
        policy.fresh_fallback,
        RuntimePreviousResponseFreshFallbackPolicy::FailClosed {
            request_shape:
                RuntimePreviousResponseFreshFallbackPolicyShape::ContextDependentContinuation,
        }
    );
    assert!(policy.fresh_fallback.blocks_without_affinity(false, false));
}

#[test]
fn previous_response_not_found_fallback_matrix_fails_closed_for_context_dependent_replay() {
    for previous_response_id in [None, Some("resp_123")] {
        for has_turn_state_retry in [false, true] {
            for request_requires_locked_previous_response_affinity in [false, true] {
                for previous_response_fresh_fallback_used in [false, true] {
                    let policy = runtime_previous_response_not_found_fallback_policy(
                        RuntimePreviousResponseNotFoundFallbackRequest {
                            previous_response_id,
                            has_turn_state_retry,
                            request_requires_locked_previous_response_affinity,
                            previous_response_fresh_fallback_used,
                            fresh_fallback_shape: Some(
                                RuntimePreviousResponseFreshFallbackShape::ContextDependentContinuation,
                            ),
                        },
                    );
                    let label = format!(
                        "previous_response={} turn_state_retry={} locked_affinity={} fresh_fallback_used={}",
                        previous_response_id.is_some(),
                        has_turn_state_retry,
                        request_requires_locked_previous_response_affinity,
                        previous_response_fresh_fallback_used
                    );

                    assert_eq!(
                        policy.fresh_fallback,
                        RuntimePreviousResponseFreshFallbackPolicy::FailClosed {
                            request_shape:
                                RuntimePreviousResponseFreshFallbackPolicyShape::ContextDependentContinuation,
                        },
                        "{label}"
                    );
                    assert!(!policy.fresh_fallback.allows_fresh_fallback(), "{label}");
                    assert_eq!(
                        policy.stale_continuation.requires_stale_continuation(),
                        previous_response_id.is_some() && !has_turn_state_retry,
                        "{label}"
                    );
                }
            }
        }
    }
}

#[test]
fn quota_blocked_affinity_release_policy_preserves_unclassified_release_behavior() {
    let policy =
        runtime_quota_blocked_affinity_release_policy(RuntimeQuotaBlockedAffinityReleaseRequest {
            affinity: RuntimeCandidateAffinity::new(
                RuntimeRouteKind::Responses,
                "main",
                None,
                Some("main"),
                None,
                None,
                true,
            ),
            fresh_fallback_shape: None,
        });

    assert_eq!(
        policy,
        RuntimeQuotaBlockedAffinityReleasePolicy::ReleaseAffinity
    );
}
