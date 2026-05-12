use super::*;
use proptest::prelude::*;
use proptest::test_runner::Config as ProptestConfig;

fn json_body(value: serde_json::Value) -> Vec<u8> {
    serde_json::to_vec(&value).expect("test json should serialize")
}

fn non_explicit_quota_text() -> impl Strategy<Value = String> {
    "[A-Za-z0-9 .,!?/-]{0,64}"
}

fn explicit_quota_code() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("insufficient_quota".to_string()),
        Just(" rate_limit_exceeded ".to_string()),
        Just("USAGE_LIMIT_REACHED".to_string()),
    ]
}

fn explicit_quota_payload(code: &str, message: &str, shape: u8) -> serde_json::Value {
    match shape % 3 {
        0 => serde_json::json!({
            "error": {
                "code": code,
                "message": message,
            },
        }),
        1 => serde_json::json!({
            "error": {
                "type": code,
                "detail": message,
            },
        }),
        _ => serde_json::json!({
            "outer": [
                {
                    "inner": {
                        "code": code,
                        "message": message,
                    },
                },
            ],
        }),
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn generic_429_payloads_never_rotate_without_explicit_quota_code(
        message in non_explicit_quota_text(),
        code in prop::option::of(non_explicit_quota_text()),
        error_type in prop::option::of(non_explicit_quota_text()),
        detail in non_explicit_quota_text(),
    ) {
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

            prop_assert_eq!(policy.class, RuntimeHttpErrorClass::Other);
            prop_assert_eq!(policy.action, RuntimeHttpErrorAction::PassThrough);
            prop_assert_eq!(policy.rule, None);
            prop_assert_eq!(policy.message, None);
        }
    }

    #[test]
    fn explicit_quota_payloads_rotate_only_before_commit_for_supported_statuses(
        code in explicit_quota_code(),
        message in "[A-Za-z0-9 .,!?/-]{1,64}",
        shape in 0u8..9,
    ) {
        let body = json_body(explicit_quota_payload(&code, &message, shape));

        for status in [403, 429] {
            let precommit =
                runtime_http_error_policy(status, &body, RuntimeHttpErrorPhase::PreCommit);
            prop_assert_eq!(precommit.class, RuntimeHttpErrorClass::Quota);
            prop_assert_eq!(precommit.action, RuntimeHttpErrorAction::RotateProfile);
            prop_assert_eq!(precommit.rule, Some("explicit_quota"));
            prop_assert_eq!(precommit.message.as_deref(), Some(message.as_str()));

            let committed =
                runtime_http_error_policy(status, &body, RuntimeHttpErrorPhase::Committed);
            prop_assert_eq!(committed.class, RuntimeHttpErrorClass::Quota);
            prop_assert_eq!(committed.action, RuntimeHttpErrorAction::PassThrough);
            prop_assert_eq!(committed.rule, Some("explicit_quota"));
            prop_assert_eq!(committed.message.as_deref(), Some(message.as_str()));
        }
    }
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
fn generic_429_matrix_passes_through_without_explicit_quota_or_rate_limit_code() {
    let bodies: [(&str, &[u8]); 8] = [
        ("empty", b"" as &[u8]),
        ("plain_too_many_requests", b"Too Many Requests" as &[u8]),
        (
            "json_too_many_requests",
            br#"{"error":{"message":"Too Many Requests"}}"# as &[u8],
        ),
        (
            "json_usage_message_without_code",
            br#"{"error":{"message":"The usage limit has been reached"}}"# as &[u8],
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
    ];

    for phase in [
        RuntimeHttpErrorPhase::PreCommit,
        RuntimeHttpErrorPhase::Committed,
    ] {
        for (label, body) in bodies {
            let policy = runtime_http_error_policy(429, body, phase);

            assert_eq!(
                policy.class,
                RuntimeHttpErrorClass::Other,
                "{label} {phase:?}"
            );
            assert_eq!(
                policy.action,
                RuntimeHttpErrorAction::PassThrough,
                "{label} {phase:?}"
            );
            assert_eq!(policy.rule, None, "{label} {phase:?}");
            assert_eq!(policy.message, None, "{label} {phase:?}");
        }
    }
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
fn explicit_quota_code_matrix_rotates_only_before_commit() {
    let cases = [
        (
            "code_insufficient_quota",
            json_body(serde_json::json!({
                "error": {
                    "code": "insufficient_quota",
                    "message": "Quota exhausted"
                }
            })),
            "Quota exhausted",
        ),
        (
            "type_rate_limit_exceeded",
            json_body(serde_json::json!({
                "error": {
                    "type": "rate_limit_exceeded",
                    "message": "Rate limit exceeded"
                }
            })),
            "Rate limit exceeded",
        ),
        (
            "trimmed_case_insensitive_code",
            json_body(serde_json::json!({
                "error": {
                    "code": " RATE_LIMIT_EXCEEDED ",
                    "message": "Rate limit exceeded"
                }
            })),
            "Rate limit exceeded",
        ),
        (
            "nested_usage_limit_code",
            json_body(serde_json::json!({
                "outer": [
                    {
                        "error": {
                            "code": "usage_limit_reached",
                            "message": "Usage limit reached"
                        }
                    }
                ]
            })),
            "Usage limit reached",
        ),
    ];

    for status in [403, 429] {
        for (label, body, message) in &cases {
            let precommit =
                runtime_http_error_policy(status, body, RuntimeHttpErrorPhase::PreCommit);
            assert_eq!(
                precommit.class,
                RuntimeHttpErrorClass::Quota,
                "{label} {status}"
            );
            assert_eq!(
                precommit.action,
                RuntimeHttpErrorAction::RotateProfile,
                "{label} {status}"
            );
            assert_eq!(precommit.rule, Some("explicit_quota"), "{label} {status}");
            assert_eq!(
                precommit.message.as_deref(),
                Some(*message),
                "{label} {status}"
            );

            let committed =
                runtime_http_error_policy(status, body, RuntimeHttpErrorPhase::Committed);
            assert_eq!(
                committed.class,
                RuntimeHttpErrorClass::Quota,
                "{label} {status}"
            );
            assert_eq!(
                committed.action,
                RuntimeHttpErrorAction::PassThrough,
                "{label} {status}"
            );
            assert_eq!(committed.rule, Some("explicit_quota"), "{label} {status}");
        }
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
