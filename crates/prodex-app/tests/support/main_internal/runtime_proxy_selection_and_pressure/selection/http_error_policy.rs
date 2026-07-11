use super::*;

#[test]
fn runtime_http_error_policy_passes_generic_429_through() {
    let policy = runtime_http_error_policy(
        429,
        br#"{"error":{"message":"Too Many Requests"}}"#,
        RuntimeHttpErrorPhase::PreCommit,
    );

    assert_eq!(policy.class, RuntimeHttpErrorClass::Other);
    assert_eq!(policy.action, RuntimeHttpErrorAction::PassThrough);
    assert_eq!(policy.rule, None);
    assert!(!policy.may_retry_or_rotate());
}

#[test]
fn runtime_http_error_policy_rotates_explicit_quota_codes_before_commit_only() {
    for code in ["insufficient_quota", "rate_limit_exceeded"] {
        let body = serde_json::to_vec(&serde_json::json!({
            "error": {
                "code": code,
                "message": "Quota exhausted"
            }
        }))
        .expect("quota body should serialize");

        let precommit = runtime_http_error_policy(429, &body, RuntimeHttpErrorPhase::PreCommit);
        assert_eq!(precommit.class, RuntimeHttpErrorClass::Quota, "{code}");
        assert_eq!(
            precommit.action,
            RuntimeHttpErrorAction::RotateProfile,
            "{code}"
        );
        assert_eq!(precommit.rule, Some("explicit_quota"), "{code}");
        assert_eq!(precommit.message.as_deref(), Some("Quota exhausted"));

        let committed = runtime_http_error_policy(429, &body, RuntimeHttpErrorPhase::Committed);
        assert_eq!(committed.class, RuntimeHttpErrorClass::Quota, "{code}");
        assert_eq!(
            committed.action,
            RuntimeHttpErrorAction::PassThrough,
            "{code}"
        );
    }
}

#[test]
fn runtime_http_error_policy_rotates_deactivated_workspace_before_commit_only() {
    let body = br#"{"detail":{"code":"deactivated_workspace"}}"#;

    let precommit = runtime_http_error_policy(402, body, RuntimeHttpErrorPhase::PreCommit);
    assert_eq!(precommit.class, RuntimeHttpErrorClass::ProfileUnavailable);
    assert_eq!(precommit.action, RuntimeHttpErrorAction::RotateProfile);
    assert_eq!(precommit.rule, Some("profile_unavailable"));
    assert!(precommit.may_retry_or_rotate());

    let committed = runtime_http_error_policy(402, body, RuntimeHttpErrorPhase::Committed);
    assert_eq!(committed.class, RuntimeHttpErrorClass::ProfileUnavailable);
    assert_eq!(committed.action, RuntimeHttpErrorAction::PassThrough);
    assert!(!committed.may_retry_or_rotate());
}

#[test]
fn runtime_http_error_policy_retries_transient_5xx_before_commit_only() {
    for status in [500, 502, 503, 504, 529] {
        let precommit = runtime_http_error_policy(
            status,
            b"backend unavailable",
            RuntimeHttpErrorPhase::PreCommit,
        );
        assert_eq!(
            precommit.class,
            RuntimeHttpErrorClass::TransientServer,
            "{status}"
        );
        assert_eq!(
            precommit.action,
            RuntimeHttpErrorAction::RetryProfile,
            "{status}"
        );
        assert_eq!(precommit.rule, Some("transient_5xx"), "{status}");

        let committed = runtime_http_error_policy(
            status,
            b"backend unavailable",
            RuntimeHttpErrorPhase::Committed,
        );
        assert_eq!(
            committed.action,
            RuntimeHttpErrorAction::PassThrough,
            "{status}"
        );
    }
}
