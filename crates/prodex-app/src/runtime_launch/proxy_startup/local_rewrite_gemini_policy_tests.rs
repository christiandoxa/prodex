use super::*;

#[test]
fn gemini_quota_rotation_predicate_respects_affinity_and_attempt_budget() {
    assert!(runtime_gemini_should_rotate_after_quota_response(
        429, true, false, false, 0, 2
    ));
    assert!(!runtime_gemini_should_rotate_after_quota_response(
        429, false, false, false, 0, 2
    ));
    assert!(!runtime_gemini_should_rotate_after_quota_response(
        429, true, false, false, 0, 1
    ));
    assert!(!runtime_gemini_should_rotate_after_quota_response(
        429, true, true, false, 0, 2
    ));
    assert!(!runtime_gemini_should_rotate_after_quota_response(
        429, true, true, true, 0, 2
    ));
    assert!(!runtime_gemini_should_rotate_after_quota_response(
        429, true, false, false, 1, 2
    ));
    assert!(!runtime_gemini_should_rotate_after_quota_response(
        500, true, false, false, 0, 2
    ));
}

#[test]
fn gemini_rate_limit_inline_retry_is_bounded() {
    assert!(!runtime_gemini_should_inline_rate_limit_retry(0));
    assert!(runtime_gemini_should_inline_rate_limit_retry(10_000));
    assert!(runtime_gemini_should_inline_rate_limit_retry(20_000));
    assert!(runtime_gemini_should_inline_rate_limit_retry(30_000));
    assert!(!runtime_gemini_should_inline_rate_limit_retry(30_001));
    assert!(!runtime_gemini_should_inline_rate_limit_retry(60_000));
}

#[test]
fn gemini_request_validation_rejects_candidate_aliases_before_upstream() {
    for stream in [false, true] {
        for body in [
            serde_json::json!({"candidate_count": 0, "stream": stream}),
            serde_json::json!({"candidateCount": 2, "stream": stream}),
            serde_json::json!({
                "candidate_count": 1,
                "candidateCount": 2,
                "stream": stream
            }),
        ] {
            let reason =
                runtime_gemini_request_validation_error(&serde_json::to_vec(&body).unwrap())
                    .expect("invalid candidate count must be rejected");
            assert!(reason.contains("candidate"), "{reason}");
        }
    }
    assert!(
        runtime_gemini_request_validation_error(
            br#"{"candidate_count":1,"candidateCount":1,"stream":true}"#
        )
        .is_none()
    );
}

#[test]
fn gemini_request_validation_rejects_malformed_function_tools_with_indexed_path() {
    for tools in [
        serde_json::json!([
            {"type": "function", "name": "lookup", "parameters": {"type": "object"}},
            {"type": "function", "name": "missing_schema"}
        ]),
        serde_json::json!([
            {"type": "function", "function": {"name": "lookup", "parameters": {"type": "object"}}},
            {"type": "function", "function": {"name": "missing_schema"}}
        ]),
    ] {
        let body = serde_json::json!({"tools": tools});
        let reason = runtime_gemini_request_validation_error(&serde_json::to_vec(&body).unwrap())
            .expect("malformed function tool must be rejected");
        assert!(reason.contains("tools[1].function.parameters"), "{reason}");
    }
    assert!(
        runtime_gemini_request_validation_error(
            br#"{"tools":[{"type":"custom","name":"apply_patch","format":{"type":"grammar"}}]}"#
        )
        .is_none()
    );
}

#[test]
fn gemini_oauth_code_assist_filters_known_unavailable_models() {
    let mut chain = vec![
        "gemini-3-pro-preview".to_string(),
        "gemini-3.1-pro-preview-customtools".to_string(),
        "gemini-3.5-flash".to_string(),
        "gemini-3-flash".to_string(),
        "gemini-2.5-flash".to_string(),
    ];

    provider_gemini_retain_code_assist_models(&mut chain);

    assert_eq!(
        chain,
        vec![
            "gemini-3-pro-preview".to_string(),
            "gemini-2.5-flash".to_string()
        ]
    );
}
