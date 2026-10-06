use super::*;

#[test]
fn generic_429_payload_corpus_retries_precommit_without_semantic_error_code() {
    let cases = [
        ("", None, None, ""),
        ("Too Many Requests", None, None, "retry later"),
        ("The usage limit has been reached", Some("quota"), None, ""),
        (
            "Quota exhausted",
            Some("too_many_requests"),
            Some("rate_limit"),
            "plain rate limit type is not enough",
        ),
        ("rate limit exceeded words", None, Some("server_error"), ""),
        (
            "The docs mention rate_limit_exceeded and insufficient_quota.",
            None,
            None,
            "non-error prose",
        ),
        (
            "generic punctuation .,!?",
            Some("usage_limit"),
            Some("insufficient"),
            "near misses",
        ),
    ];

    for (message, code, error_type, detail) in cases {
        let body = json_body(serde_json::json!({
            "error": {
                "code": code,
                "type": error_type,
                "message": message,
                "detail": detail,
            },
        }));

        for phase in [
            RuntimeHttpErrorPhase::PreCommit,
            RuntimeHttpErrorPhase::Committed,
        ] {
            let policy = runtime_http_error_policy(429, &body, phase);

            assert_eq!(
                policy.class,
                RuntimeHttpErrorClass::RateLimited,
                "{message} {phase:?}"
            );
            assert_eq!(
                policy.action,
                if phase == RuntimeHttpErrorPhase::PreCommit {
                    RuntimeHttpErrorAction::RetryProfile
                } else {
                    RuntimeHttpErrorAction::PassThrough
                },
                "{message} {phase:?}"
            );
            assert_eq!(policy.rule, Some("rate_limited"), "{message} {phase:?}");
        }
    }
}

#[test]
fn workspace_credit_message_without_explicit_429_code_still_retries_precommit() {
    let body = json_body(serde_json::json!({
        "error": {
            "message": "Your workspace is out of credits. Ask your workspace owner to refill in order to continue."
        }
    }));

    for status in [402, 403] {
        let precommit = runtime_http_error_policy(status, &body, RuntimeHttpErrorPhase::PreCommit);
        assert_eq!(precommit.class, RuntimeHttpErrorClass::Quota, "{status}");
        assert_eq!(
            precommit.action,
            RuntimeHttpErrorAction::RotateProfile,
            "{status}"
        );
        assert_eq!(precommit.rule, Some("explicit_quota"), "{status}");
        assert_eq!(
            precommit.message.as_deref(),
            Some(
                "Your workspace is out of credits. Ask your workspace owner to refill in order to continue."
            )
        );

        let committed = runtime_http_error_policy(status, &body, RuntimeHttpErrorPhase::Committed);
        assert_eq!(committed.class, RuntimeHttpErrorClass::Quota, "{status}");
        assert_eq!(
            committed.action,
            RuntimeHttpErrorAction::PassThrough,
            "{status}"
        );
    }

    for phase in [
        RuntimeHttpErrorPhase::PreCommit,
        RuntimeHttpErrorPhase::Committed,
    ] {
        let policy = runtime_http_error_policy(429, &body, phase);
        assert_eq!(policy.class, RuntimeHttpErrorClass::RateLimited);
        assert_eq!(
            policy.action,
            if phase == RuntimeHttpErrorPhase::PreCommit {
                RuntimeHttpErrorAction::RetryProfile
            } else {
                RuntimeHttpErrorAction::PassThrough
            }
        );
        assert_eq!(policy.rule, Some("rate_limited"));
    }
}

#[test]
fn generic_429_is_retryable_before_commit_without_explicit_quota_code() {
    let policy = runtime_http_error_policy(
        429,
        br#"{"error":{"message":"Too Many Requests"}}"#,
        RuntimeHttpErrorPhase::PreCommit,
    );

    assert_eq!(policy.class, RuntimeHttpErrorClass::RateLimited);
    assert_eq!(policy.action, RuntimeHttpErrorAction::RetryProfile);
    assert_eq!(policy.rule, Some("rate_limited"));
}

#[test]
fn generic_429_matrix_retries_precommit_and_passes_through_after_commit() {
    let bodies: [(&str, &[u8]); 11] = [
        ("empty", b"" as &[u8]),
        ("plain_too_many_requests", b"Too Many Requests" as &[u8]),
        (
            "json_too_many_requests",
            br#"{"error":{"message":"Too Many Requests"}}"# as &[u8],
        ),
        (
            "json_rate_limit_type_without_exceeded_code",
            br#"{"error":{"type":"rate_limit","message":"Too Many Requests"}}"# as &[u8],
        ),
        (
            "json_too_many_requests_code",
            br#"{"error":{"code":"too_many_requests","message":"Too Many Requests"}}"# as &[u8],
        ),
        (
            "json_quota_word_code",
            br#"{"error":{"code":"quota","message":"Quota exhausted"}}"# as &[u8],
        ),
        (
            "json_nested_generic_429",
            br#"{"items":[{"error":{"status":429,"message":"Too Many Requests"}}]}"# as &[u8],
        ),
        ("plain_code_shaped_text", b"rate_limit_exceeded" as &[u8]),
        (
            "json_error_string",
            br#"{"error":"rate_limit_exceeded"}"# as &[u8],
        ),
        (
            "json_message_code_shaped_text",
            br#"{"error":{"message":"insufficient_quota"}}"# as &[u8],
        ),
        (
            "malformed_json_explicit_quota_code",
            br#"{"error":{"code":"insufficient_quota"}"# as &[u8],
        ),
    ];

    for phase in [
        RuntimeHttpErrorPhase::PreCommit,
        RuntimeHttpErrorPhase::Committed,
    ] {
        for (label, body) in bodies {
            let policy = runtime_http_error_policy(429, body, phase);

            assert_eq!(
                policy.class,
                RuntimeHttpErrorClass::RateLimited,
                "{label} {phase:?}"
            );
            assert_eq!(
                policy.action,
                if phase == RuntimeHttpErrorPhase::PreCommit {
                    RuntimeHttpErrorAction::RetryProfile
                } else {
                    RuntimeHttpErrorAction::PassThrough
                },
                "{label} {phase:?}"
            );
            assert_eq!(policy.rule, Some("rate_limited"), "{label} {phase:?}");
        }
    }
}
