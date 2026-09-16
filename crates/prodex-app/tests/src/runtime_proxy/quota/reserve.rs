use super::*;

fn reserve_usage() -> UsageResponse {
    serde_json::from_value(serde_json::json!({
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
        rewrite_runtime_luna_reserve_model_for_usage(&usage, Some("acct-luna"), original, true)
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
        true,
    )
    .unwrap()
    .unwrap();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&rewritten).unwrap()["model"],
        "gpt-reserve"
    );
}

#[test]
fn websocket_continuation_cannot_rewrite_or_double_rewrite() {
    let usage = reserve_usage();
    assert!(
        rewrite_runtime_luna_reserve_model_for_usage(
            &usage,
            Some("acct-luna"),
            br#"{"model":"gpt-5.6-luna"}"#,
            false,
        )
        .unwrap()
        .is_none()
    );
    assert!(
        rewrite_runtime_luna_reserve_model_for_usage(
            &usage,
            Some("acct-luna"),
            br#"{"model":"gpt-reserve"}"#,
            true,
        )
        .unwrap()
        .is_none()
    );
}

fn http_request(body: &[u8]) -> RuntimeProxyRequest {
    RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/backend-api/prodex/responses".to_string(),
        headers: Vec::new(),
        body: body.to_vec(),
    }
}

#[test]
fn http_reserve_rewrite_is_limited_to_fresh_responses_requests() {
    let fresh = http_request(br#"{"model":"gpt-5.6-luna"}"#);
    assert!(runtime_luna_reserve_http_rewrite_allowed(
        &fresh,
        RuntimeRouteKind::Responses,
        None,
    ));
    assert!(!runtime_luna_reserve_http_rewrite_allowed(
        &fresh,
        RuntimeRouteKind::Compact,
        None,
    ));
    assert!(!runtime_luna_reserve_http_rewrite_allowed(
        &fresh,
        RuntimeRouteKind::Responses,
        Some("continuation-state"),
    ));

    let mut previous_response =
        http_request(br#"{"model":"gpt-5.6-luna","previous_response_id":"resp_123"}"#);
    assert!(!runtime_luna_reserve_http_rewrite_allowed(
        &previous_response,
        RuntimeRouteKind::Responses,
        None,
    ));

    previous_response
        .headers
        .push(("session_id".to_string(), "session_123".to_string()));
    assert!(!runtime_luna_reserve_http_rewrite_allowed(
        &previous_response,
        RuntimeRouteKind::Responses,
        None,
    ));

    previous_response.headers.clear();
    previous_response
        .headers
        .push(("x-codex-turn-state".to_string(), "turn-state".to_string()));
    assert!(!runtime_luna_reserve_http_rewrite_allowed(
        &previous_response,
        RuntimeRouteKind::Responses,
        None,
    ));
}
