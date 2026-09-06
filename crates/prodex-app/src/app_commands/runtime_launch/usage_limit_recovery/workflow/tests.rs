use super::*;

const SESSION: &str = "019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9";

#[test]
fn structured_rollout_and_app_server_errors_are_recoverable() {
    for (info, class) in [
        (
            "usage_limit_exceeded",
            RuntimeWorkflowRecoveryClass::UsageLimit,
        ),
        (
            "rate_limit_exceeded",
            RuntimeWorkflowRecoveryClass::RateLimit,
        ),
        ("server_overloaded", RuntimeWorkflowRecoveryClass::Overload),
        ("unauthorized", RuntimeWorkflowRecoveryClass::Auth),
    ] {
        let rollout = serde_json::json!({
            "type": "event_msg",
            "payload": {"type": "error", "codex_error_info": info}
        });
        assert_eq!(
            runtime_workflow_recovery_class(&rollout, SESSION),
            Some(class)
        );
    }
    let notification = serde_json::json!({
        "method": "turn/completed",
        "params": {
            "threadId": SESSION,
            "turn": {
                "status": "failed",
                "error": {
                    "codexErrorInfo": {
                        "responseTooManyFailedAttempts": {"httpStatusCode": 503}
                    }
                }
            }
        }
    });
    assert_eq!(
        runtime_workflow_recovery_class(&notification, SESSION),
        Some(RuntimeWorkflowRecoveryClass::Transport)
    );
}

#[test]
fn global_or_ambiguous_failures_do_not_trigger_recovery() {
    for info in [
        serde_json::json!("badRequest"),
        serde_json::json!("cyberPolicy"),
        serde_json::json!("misalignmentPolicyViolation"),
        serde_json::json!("sandboxError"),
        serde_json::json!("contextWindowExceeded"),
        serde_json::json!("sessionBudgetExceeded"),
        serde_json::json!("activeTurnNotSteerable"),
        serde_json::json!("other"),
        serde_json::json!({"responseTooManyFailedAttempts": {"httpStatusCode": 400}}),
    ] {
        let value = serde_json::json!({
            "type": "event_msg",
            "payload": {"type": "error", "codexErrorInfo": info}
        });
        assert_eq!(runtime_workflow_recovery_class(&value, SESSION), None);
    }
    let cancelled = serde_json::json!({
        "method": "turn/completed",
        "params": {
            "threadId": SESSION,
            "turn": {
                "status": "cancelled",
                "error": {"codexErrorInfo": "usageLimitExceeded"}
            }
        }
    });
    assert_eq!(runtime_workflow_recovery_class(&cancelled, SESSION), None);
    let wrong_session = serde_json::json!({
        "method": "turn/completed",
        "params": {
            "threadId": "019c9e3d-45a0-7ad0-a6ee-b194ac2d4400",
            "turn": {
                "status": "failed",
                "error": {"codexErrorInfo": "usageLimitExceeded"}
            }
        }
    });
    assert_eq!(
        runtime_workflow_recovery_class(&wrong_session, SESSION),
        None
    );
    let intermediate_stream_error = serde_json::json!({
        "type": "stream_error",
        "codexErrorInfo": {"responseStreamDisconnected": {"httpStatusCode": 503}}
    });
    assert_eq!(
        runtime_workflow_recovery_class(&intermediate_stream_error, SESSION),
        None
    );
}

#[test]
fn official_unexpected_status_display_is_classified_narrowly() {
    for (message, class) in [
        (
            "unexpected status 403 Forbidden: You've hit your usage limit. Upgrade to Pro (https://chatgpt.com/explore/pro), url: https://example.com/backend-api/codex/responses",
            RuntimeWorkflowRecoveryClass::UsageLimit,
        ),
        (
            "unexpected status 503 Service Unavailable: service unavailable, url: https://example.com/backend-api/codex/responses",
            RuntimeWorkflowRecoveryClass::Transport,
        ),
        (
            "unexpected status 401 Unauthorized: Unauthorized",
            RuntimeWorkflowRecoveryClass::Auth,
        ),
        (
            "unexpected status 403 Forbidden: {\"detail\":{\"code\":\"deactivated_workspace\",\"message\":\"workspace unavailable\"}}, url: https://example.com/backend-api/codex/responses",
            RuntimeWorkflowRecoveryClass::ProfileUnavailable,
        ),
    ] {
        let value = serde_json::json!({
            "type": "event_msg",
            "payload": {
                "type": "error",
                "message": message,
                "codex_error_info": "other"
            }
        });
        assert_eq!(
            runtime_workflow_recovery_class(&value, SESSION),
            Some(class)
        );
    }

    for message in [
        "unexpected status 403 Forbidden: content policy rejected this request",
        "unexpected status 400 Bad Request: invalid argument",
        "the docs say unexpected status 503 Service Unavailable",
    ] {
        let value = serde_json::json!({
            "type": "event_msg",
            "payload": {
                "type": "error",
                "message": message,
                "codex_error_info": "other"
            }
        });
        assert_eq!(runtime_workflow_recovery_class(&value, SESSION), None);
    }
}

#[test]
fn effective_model_tracks_turn_context_and_official_reroutes() {
    for (value, expected) in [
        (
            serde_json::json!({
                "type": "turn_context",
                "payload": {"model": "gpt-5.6-luna"}
            }),
            "gpt-5.6-luna",
        ),
        (
            serde_json::json!({
                "type": "event_msg",
                "payload": {
                    "type": "model_reroute",
                    "from_model": "gpt-5.3-codex",
                    "to_model": "gpt-5.2",
                    "reason": "high_risk_cyber_activity"
                }
            }),
            "gpt-5.2",
        ),
        (
            serde_json::json!({
                "method": "model/rerouted",
                "params": {"toModel": "gpt-5.2"}
            }),
            "gpt-5.2",
        ),
    ] {
        assert_eq!(
            runtime_workflow_effective_model(&value).as_deref(),
            Some(expected)
        );
    }
    assert_eq!(
        runtime_workflow_effective_model(&serde_json::json!({
            "type": "turn_context",
            "payload": {"model": "bad\nmodel"}
        })),
        None
    );
}

#[test]
fn workflow_evidence_resets_per_user_turn_and_never_infers_non_acceptance() {
    let mut evidence = RuntimeWorkflowEvidence::default();
    observe_runtime_workflow_evidence(
        &serde_json::json!({
            "type": "response_item",
            "payload": {"type": "message", "role": "user"}
        }),
        &mut evidence,
    );
    assert_eq!(evidence.acceptance_state, "accepted_but_uncommitted");
    observe_runtime_workflow_evidence(
        &serde_json::json!({
            "type": "event_msg",
            "payload": {"type": "agent_message"}
        }),
        &mut evidence,
    );
    assert_eq!(evidence.acceptance_state, "committed");
    observe_runtime_workflow_evidence(
        &serde_json::json!({
            "type": "response_item",
            "payload": {"type": "function_call_output"}
        }),
        &mut evidence,
    );
    assert_eq!(evidence.acceptance_state, "side_effect_observed");
    assert_eq!(evidence.side_effect_state, "observed");

    observe_runtime_workflow_evidence(
        &serde_json::json!({
            "type": "event_msg",
            "payload": {"type": "turn_started", "turn_id": "turn-2"}
        }),
        &mut evidence,
    );
    assert_eq!(evidence, RuntimeWorkflowEvidence::default());

    observe_runtime_workflow_evidence(
        &serde_json::json!({
            "type": "event_msg",
            "payload": {"type": "user_message"}
        }),
        &mut evidence,
    );
    assert_eq!(evidence.acceptance_state, "accepted_but_uncommitted");
    assert_eq!(evidence.stream_committed, Some(false));
    assert_eq!(evidence.side_effect_state, "none");
}
