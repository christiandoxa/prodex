use super::*;

#[test]
fn official_rate_limit_reached_header_preserves_rate_limit_semantics() {
    let policy = runtime_http_error_policy_with_headers(
        429,
        br#"{"error":{"message":"Too Many Requests"}}"#,
        [(
            "X-Codex-Rate-Limit-Reached-Type",
            b"rate_limit_reached".as_slice(),
        )],
        RuntimeHttpErrorPhase::PreCommit,
    );
    assert_eq!(policy.class, RuntimeHttpErrorClass::RateLimited);
    assert_eq!(policy.action, RuntimeHttpErrorAction::RetryProfile);
    assert_eq!(policy.rule, Some("rate_limited"));
}

#[test]
fn official_rate_limit_header_overrides_the_generic_usage_limit_body() {
    let policy = runtime_http_error_policy_with_headers(
        429,
        br#"{"error":{"type":"usage_limit_reached","plan_type":"pro"}}"#,
        [(
            "X-Codex-Rate-Limit-Reached-Type",
            b"rate_limit_reached".as_slice(),
        )],
        RuntimeHttpErrorPhase::PreCommit,
    );
    assert_eq!(policy.class, RuntimeHttpErrorClass::RateLimited);
    assert_eq!(policy.action, RuntimeHttpErrorAction::RetryProfile);
    assert_eq!(policy.rule, Some("rate_limited"));
}

#[test]
fn official_workspace_limit_headers_are_quota_evidence() {
    for value in [
        "workspace_owner_credits_depleted",
        "workspace_member_credits_depleted",
        "workspace_owner_usage_limit_reached",
        "workspace_member_usage_limit_reached",
    ] {
        let policy = runtime_http_error_policy_with_headers(
            429,
            br#"{"error":{"message":"Too Many Requests"}}"#,
            [("x-codex-rate-limit-reached-type", value.as_bytes())],
            RuntimeHttpErrorPhase::PreCommit,
        );
        assert_eq!(policy.class, RuntimeHttpErrorClass::Quota, "{value}");
        assert_eq!(
            policy.action,
            RuntimeHttpErrorAction::RotateProfile,
            "{value}"
        );
    }
}

#[test]
fn unknown_rate_limit_reached_header_remains_generic() {
    let policy = runtime_http_error_policy_with_headers(
        429,
        br#"{"error":{"message":"Too Many Requests"}}"#,
        [("X-Codex-Rate-Limit-Reached-Type", b"future_kind".as_slice())],
        RuntimeHttpErrorPhase::PreCommit,
    );
    assert_eq!(policy.class, RuntimeHttpErrorClass::Other);
    assert_eq!(policy.action, RuntimeHttpErrorAction::PassThrough);
    assert_eq!(policy.rule, None);
}

#[test]
fn workspace_exhaustion_header_overrides_generic_rate_limit_body() {
    let policy = runtime_http_error_policy_with_headers(
        429,
        br#"{"error":{"code":"rate_limit_exceeded","message":"retry later"}}"#,
        [(
            "X-Codex-Rate-Limit-Reached-Type",
            b"workspace_member_credits_depleted".as_slice(),
        )],
        RuntimeHttpErrorPhase::PreCommit,
    );
    assert_eq!(policy.class, RuntimeHttpErrorClass::Quota);
    assert_eq!(policy.action, RuntimeHttpErrorAction::RotateProfile);
}

#[test]
fn observed_upgrade_to_pro_usage_limit_rotates_on_403() {
    let body = br#"{"error":{"message":"You've hit your usage limit. Upgrade to Pro (https://chatgpt.com/explore/pro), visit https://chatgpt.com/codex/settings/usage to purchase more credits or try again at 5:08 PM."}}"#;
    let policy = runtime_http_error_policy(403, body, RuntimeHttpErrorPhase::PreCommit);
    assert_eq!(policy.class, RuntimeHttpErrorClass::Quota);
    assert_eq!(policy.action, RuntimeHttpErrorAction::RotateProfile);
}
