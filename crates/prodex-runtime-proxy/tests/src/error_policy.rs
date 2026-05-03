use super::*;

fn json_body(value: serde_json::Value) -> Vec<u8> {
    serde_json::to_vec(&value).expect("test json should serialize")
}

#[test]
fn generic_429_passes_through_without_explicit_quota_code() {
    let policy = runtime_http_error_policy(
        429,
        br#"{"error":{"message":"Too Many Requests"}}"#,
        RuntimeHttpErrorPhase::PreCommit,
    );

    assert_eq!(policy.class, RuntimeHttpErrorClass::Other);
    assert_eq!(policy.action, RuntimeHttpErrorAction::PassThrough);
    assert_eq!(policy.rule, None);
}

#[test]
fn explicit_quota_codes_rotate_only_before_commit() {
    for code in ["insufficient_quota", "rate_limit_exceeded"] {
        let body = json_body(serde_json::json!({
            "error": {
                "code": code,
                "message": "Quota exhausted"
            }
        }));

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
fn transient_5xx_retries_only_before_commit() {
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
