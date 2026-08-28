use super::*;

#[test]
fn openai_quota_deserializes_rate_limit_reset_credits() {
    let camel_usage: UsageResponse = serde_json::from_value(serde_json::json!({
        "email": "user@example.com",
        "plan_type": "plus",
        "rate_limit": null,
        "code_review_rate_limit": null,
        "rate_limit_reset_credits": {
            "availableCount": 3
        }
    }))
    .expect("usage response should deserialize reset credits");

    let camel_credits = camel_usage
        .rate_limit_reset_credits
        .as_ref()
        .expect("camel reset credits");
    assert_eq!(camel_credits.available_count, 3);

    let snake_usage: UsageResponse = serde_json::from_value(serde_json::json!({
        "rate_limit_reset_credits": {
            "available_count": 4
        }
    }))
    .expect("usage response should deserialize backend reset credits");

    let snake_credits = snake_usage
        .rate_limit_reset_credits
        .as_ref()
        .expect("snake reset credits");
    assert_eq!(snake_credits.available_count, 4);
}

#[test]
fn additional_rate_limit_preserves_admission_and_future_fields() {
    let usage: UsageResponse = serde_json::from_value(serde_json::json!({
        "plan_type": "plus",
        "rate_limit": null,
        "additional_rate_limits": [{
            "limit_id": "future_codex_special",
            "limit_name": "Future Special",
            "metered_feature": "future_feature",
            "rate_limit": {
                "allowed": false,
                "limit_reached": true,
                "primary_window": {"used_percent": 100, "reset_at": 123, "limit_window_seconds": 60},
                "secondary_window": null,
                "nested_future_field": [1, 2, 3]
            },
            "future_field": {"status": "unknown"}
        }]
    }))
    .expect("additional rate limit should deserialize");

    let additional = &usage.additional_rate_limits[0];
    assert_eq!(additional.limit_id.as_deref(), Some("future_codex_special"));
    assert_eq!(additional.rate_limit.allowed, Some(false));
    assert_eq!(additional.rate_limit.limit_reached, Some(true));
    assert!(!additional_rate_limit_is_usable(additional));
    assert_eq!(
        additional.extra.get("future_field"),
        Some(&serde_json::json!({"status": "unknown"}))
    );
    assert_eq!(
        additional.rate_limit.extra.get("nested_future_field"),
        Some(&serde_json::json!([1, 2, 3]))
    );

    let encoded = serde_json::to_value(&usage).expect("usage should serialize");
    assert_eq!(
        encoded["additional_rate_limits"][0]["future_field"]["status"],
        "unknown"
    );
}
