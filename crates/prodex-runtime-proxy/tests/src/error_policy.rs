use super::*;

fn json_body(value: serde_json::Value) -> Vec<u8> {
    serde_json::to_vec(&value).expect("test json should serialize")
}

fn explicit_quota_payload(code: &str, message: &str, shape: u8) -> serde_json::Value {
    match shape % 5 {
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
        2 => serde_json::json!({
            "error": code,
            "message": message,
        }),
        3 => serde_json::json!({
            "error": {
                "status": code,
                "message": message,
            },
        }),
        4 => serde_json::json!({
            "outer": [
                {
                    "inner": {
                        "reason": code,
                        "message": message,
                    },
                },
            ],
        }),
        _ => unreachable!(),
    }
}

#[test]
fn generic_429_payload_corpus_never_rotates_without_explicit_quota_code() {
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
                RuntimeHttpErrorClass::Other,
                "{message} {phase:?}"
            );
            assert_eq!(
                policy.action,
                RuntimeHttpErrorAction::PassThrough,
                "{message} {phase:?}"
            );
            assert_eq!(policy.rule, None, "{message} {phase:?}");
            assert_eq!(policy.message, None, "{message} {phase:?}");
        }
    }
}

#[test]
fn codex_content_policy_errors_pass_through_without_quota_rotation() {
    for code in ["invalid_prompt", "bio_policy", "cyber_policy"] {
        let body = json_body(serde_json::json!({
            "type": "response.failed",
            "response": {
                "error": {
                    "code": code,
                    "message": "Content policy rejected this request."
                }
            }
        }));

        let policy = runtime_http_error_policy(429, &body, RuntimeHttpErrorPhase::PreCommit);

        assert_eq!(policy.class, RuntimeHttpErrorClass::Other, "{code}");
        assert_eq!(policy.action, RuntimeHttpErrorAction::PassThrough, "{code}");
        assert_eq!(policy.rule, None, "{code}");
        assert_eq!(policy.message, None, "{code}");
    }
}

#[test]
fn explicit_quota_payload_corpus_rotates_only_before_commit_for_supported_statuses() {
    for (code, message) in [
        ("insufficient_quota", "Quota exhausted"),
        (" rate_limit_exceeded ", "Rate limit exceeded"),
        ("USAGE_LIMIT_REACHED", "Usage limit reached"),
        ("usage_not_included", "Workspace credits exhausted"),
    ] {
        for shape in 0u8..9 {
            let body = json_body(explicit_quota_payload(code, message, shape));

            for status in [403, 429] {
                let precommit =
                    runtime_http_error_policy(status, &body, RuntimeHttpErrorPhase::PreCommit);
                assert_eq!(precommit.class, RuntimeHttpErrorClass::Quota);
                assert_eq!(precommit.action, RuntimeHttpErrorAction::RotateProfile);
                assert_eq!(precommit.rule, Some("explicit_quota"));
                assert_eq!(precommit.message.as_deref(), Some(message));

                let committed =
                    runtime_http_error_policy(status, &body, RuntimeHttpErrorPhase::Committed);
                assert_eq!(committed.class, RuntimeHttpErrorClass::Quota);
                assert_eq!(committed.action, RuntimeHttpErrorAction::PassThrough);
                assert_eq!(committed.rule, Some("explicit_quota"));
                assert_eq!(committed.message.as_deref(), Some(message));
            }
        }
    }
}

#[test]
fn deactivated_workspace_rotates_only_before_commit_for_profile_statuses() {
    let body = json_body(serde_json::json!({
        "detail": {
            "code": "deactivated_workspace"
        }
    }));

    for status in [402, 403] {
        let precommit = runtime_http_error_policy(status, &body, RuntimeHttpErrorPhase::PreCommit);
        assert_eq!(precommit.class, RuntimeHttpErrorClass::ProfileUnavailable);
        assert_eq!(precommit.action, RuntimeHttpErrorAction::RotateProfile);
        assert_eq!(precommit.rule, Some("profile_unavailable"));
        assert_eq!(
            precommit.message.as_deref(),
            Some("Upstream Codex workspace is deactivated for this profile.")
        );

        let committed = runtime_http_error_policy(status, &body, RuntimeHttpErrorPhase::Committed);
        assert_eq!(committed.class, RuntimeHttpErrorClass::ProfileUnavailable);
        assert_eq!(committed.action, RuntimeHttpErrorAction::PassThrough);
        assert_eq!(committed.rule, Some("profile_unavailable"));
    }

    let generic_429 = runtime_http_error_policy(429, &body, RuntimeHttpErrorPhase::PreCommit);
    assert_eq!(generic_429.class, RuntimeHttpErrorClass::Other);
    assert_eq!(generic_429.action, RuntimeHttpErrorAction::PassThrough);
}

#[test]
fn workspace_credit_message_rotates_only_before_commit_for_explicit_quota_statuses() {
    let body = json_body(serde_json::json!({
        "error": {
            "message": "Your workspace is out of credits. Ask your workspace owner to refill in order to continue."
        }
    }));

    for status in [402, 403, 429] {
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
    let bodies: [(&str, &[u8]); 7] = [
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
fn usage_limit_message_rotates_before_commit_only_for_non_429_quota_statuses() {
    let body = br#"{"error":{"message":"The usage limit has been reached"}}"#;

    for status in [402, 403] {
        let precommit = runtime_http_error_policy(status, body, RuntimeHttpErrorPhase::PreCommit);
        assert_eq!(precommit.class, RuntimeHttpErrorClass::Quota, "{status}");
        assert_eq!(
            precommit.action,
            RuntimeHttpErrorAction::RotateProfile,
            "{status}"
        );
        assert_eq!(precommit.rule, Some("explicit_quota"), "{status}");
        assert_eq!(
            precommit.message.as_deref(),
            Some("The usage limit has been reached"),
            "{status}"
        );

        let committed = runtime_http_error_policy(status, body, RuntimeHttpErrorPhase::Committed);
        assert_eq!(committed.class, RuntimeHttpErrorClass::Quota, "{status}");
        assert_eq!(
            committed.action,
            RuntimeHttpErrorAction::PassThrough,
            "{status}"
        );
        assert_eq!(committed.rule, Some("explicit_quota"), "{status}");
        assert_eq!(
            committed.message.as_deref(),
            Some("The usage limit has been reached"),
            "{status}"
        );
    }

    for phase in [
        RuntimeHttpErrorPhase::PreCommit,
        RuntimeHttpErrorPhase::Committed,
    ] {
        let policy = runtime_http_error_policy(429, body, phase);
        assert_eq!(policy.class, RuntimeHttpErrorClass::Other, "{phase:?}");
        assert_eq!(
            policy.action,
            RuntimeHttpErrorAction::PassThrough,
            "{phase:?}"
        );
        assert_eq!(policy.rule, None, "{phase:?}");
        assert_eq!(policy.message, None, "{phase:?}");
    }
}

#[test]
fn explicit_quota_codes_rotate_only_before_commit() {
    for code in [
        "insufficient_quota",
        "rate_limit_exceeded",
        "usage_not_included",
    ] {
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
        (
            "nested_usage_not_included_type",
            json_body(serde_json::json!({
                "outer": [
                    {
                        "error": {
                            "type": "usage_not_included",
                            "message": "Workspace credits exhausted"
                        }
                    }
                ]
            })),
            "Workspace credits exhausted",
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

#[test]
fn failure_policy_is_transport_parity_safe_before_and_after_commit() {
    #[derive(Clone, Copy, Debug)]
    enum Surface {
        ResponsesHttp,
        ResponsesSse,
        Compact,
        Noncompact,
        WebsocketHandshake,
        WebsocketMessage,
    }

    let surfaces = [
        Surface::ResponsesHttp,
        Surface::ResponsesSse,
        Surface::Compact,
        Surface::Noncompact,
        Surface::WebsocketHandshake,
        Surface::WebsocketMessage,
    ];
    let cases: [(&str, u16, &[u8], RuntimeHttpErrorClass, RuntimeHttpErrorAction); 5] = [
        (
            "generic_429",
            429,
            br#"{"error":{"message":"Too Many Requests"}}"#,
            RuntimeHttpErrorClass::Other,
            RuntimeHttpErrorAction::PassThrough,
        ),
        (
            "explicit_quota",
            429,
            br#"{"error":{"code":"insufficient_quota","message":"Quota exhausted"}}"#,
            RuntimeHttpErrorClass::Quota,
            RuntimeHttpErrorAction::RotateProfile,
        ),
        (
            "workspace_credits",
            429,
            br#"{"error":{"code":"workspace_member_credits_depleted","message":"Workspace credits exhausted"}}"#,
            RuntimeHttpErrorClass::Quota,
            RuntimeHttpErrorAction::RotateProfile,
        ),
        (
            "overload",
            503,
            br#"{"error":{"code":"server_is_overloaded","message":"Server is overloaded"}}"#,
            RuntimeHttpErrorClass::Overload,
            RuntimeHttpErrorAction::RetryProfile,
        ),
        (
            "auth",
            401,
            br#"{"error":{"code":"unauthorized","message":"Unauthorized"}}"#,
            RuntimeHttpErrorClass::Other,
            RuntimeHttpErrorAction::PassThrough,
        ),
    ];

    for surface in surfaces {
        for (label, status, body, expected_class, expected_precommit_action) in cases {
            let classify = |phase| match surface {
                Surface::ResponsesSse | Surface::WebsocketMessage => {
                    runtime_stream_error_policy(body, phase)
                }
                Surface::ResponsesHttp
                | Surface::Compact
                | Surface::Noncompact
                | Surface::WebsocketHandshake => runtime_http_error_policy(status, body, phase),
            };

            let precommit = classify(RuntimeHttpErrorPhase::PreCommit);
            assert_eq!(
                precommit.class, expected_class,
                "{surface:?} {label} precommit class"
            );
            assert_eq!(
                precommit.action, expected_precommit_action,
                "{surface:?} {label} precommit action"
            );

            let committed = classify(RuntimeHttpErrorPhase::Committed);
            assert_eq!(
                committed.class, expected_class,
                "{surface:?} {label} committed class"
            );
            assert_eq!(
                committed.action,
                RuntimeHttpErrorAction::PassThrough,
                "{surface:?} {label} committed action"
            );
        }
    }
}
