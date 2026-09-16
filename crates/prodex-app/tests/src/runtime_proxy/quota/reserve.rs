use super::*;

fn reserve_usage() -> UsageResponse {
    serde_json::from_value(serde_json::json!({
        "accountId": "acct-luna",
        "ordinaryUsageAllowed": false,
        "rateLimitUpsell": { "banner_type": "luna_reserve" },
        "rateLimitResetCredits": { "availableCount": 2 },
        "rateLimitsByLimitId": {
            "codex": {
                "limitId": "codex",
                "primary": { "usedPercent": 100, "windowDurationMins": 300 },
                "secondary": { "usedPercent": 100, "windowDurationMins": 10080 }
            },
            "base_model_inference": {
                "limitId": "base_model_inference",
                "limitName": "gpt-reserve",
                "normalModelSlug": "gpt-5.6-luna",
                "primary": { "usedPercent": 0, "windowDurationMins": 300 },
                "secondary": { "usedPercent": 0, "windowDurationMins": 10080 }
            }
        }
    }))
    .unwrap()
}

#[test]
fn luna_reserve_rewrite_changes_only_upstream_body() {
    let usage = reserve_usage();
    let original = br#"{"model":"gpt-5.6-luna","input":"hello"}"#;
    let rewritten =
        rewrite_runtime_luna_reserve_model_for_usage(&usage, Some("acct-luna"), original)
            .unwrap()
            .unwrap();

    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&rewritten).unwrap()["model"],
        "gpt-reserve"
    );
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(original).unwrap()["model"],
        "gpt-5.6-luna"
    );
    assert_eq!(
        usage.rate_limit_reset_credits.unwrap().available_count,
        2,
        "Reserve activation must not consume reset credits"
    );
}

#[test]
fn websocket_fresh_send_rewrites_to_reserve() {
    let rewritten = rewrite_runtime_luna_reserve_model_for_usage(
        &reserve_usage(),
        Some("acct-luna"),
        br#"{"type":"response.create","model":"gpt-5.6-luna"}"#,
    )
    .unwrap()
    .unwrap();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&rewritten).unwrap()["model"],
        "gpt-reserve"
    );
}

#[test]
fn websocket_reused_continuation_rewrites_once_without_double_rewrite() {
    let usage = reserve_usage();
    let rewritten = rewrite_runtime_luna_reserve_model_for_usage(
        &usage,
        Some("acct-luna"),
        br#"{"model":"gpt-5.6-luna","previous_response_id":"resp_1"}"#,
    );
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&rewritten.unwrap().unwrap()).unwrap()["model"],
        "gpt-reserve"
    );
    assert!(
        rewrite_runtime_luna_reserve_model_for_usage(
            &usage,
            Some("acct-luna"),
            br#"{"model":"gpt-reserve"}"#,
        )
        .unwrap()
        .is_none()
    );
}
