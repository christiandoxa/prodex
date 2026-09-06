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

#[test]
fn app_server_rate_limits_payload_keeps_regular_and_reserve_buckets_separate() {
    let usage: UsageResponse = serde_json::from_value(serde_json::json!({
        "ordinaryUsageAllowed": false,
        "rateLimitsByLimitId": {
            "codex": {
                "limitId": "codex",
                "planType": "plus",
                "primary": {
                    "usedPercent": 100,
                    "windowDurationMins": 300,
                    "resetsAt": 1700003600
                },
                "secondary": {
                    "usedPercent": 0,
                    "windowDurationMins": 10080,
                    "resetsAt": 1700604800
                }
            },
            "base_model_inference": {
                "limitId": "base_model_inference",
                "limitName": "gpt-luna-reserve",
                "normalModelSlug": "gpt-5.6-luna",
                "primary": {
                    "usedPercent": 0,
                    "windowDurationMins": 300,
                    "resetsAt": 1700007200
                },
                "secondary": {
                    "usedPercent": 10,
                    "windowDurationMins": 10080,
                    "resetsAt": 1700612000
                }
            }
        }
    }))
    .expect("app-server quota payload should deserialize");

    let regular = usage.rate_limit.as_ref().expect("regular quota");
    assert_eq!(usage.plan_type.as_deref(), Some("plus"));
    assert_eq!(regular.allowed, Some(false));
    assert_eq!(
        regular
            .primary_window
            .as_ref()
            .and_then(|window| window.limit_window_seconds),
        Some(18_000)
    );
    let reserve = usage
        .additional_rate_limits
        .iter()
        .find(|additional| additional.limit_id.as_deref() == Some("base_model_inference"))
        .expect("reserve bucket");
    assert_eq!(reserve.limit_name.as_deref(), Some("gpt-luna-reserve"));
    assert_eq!(
        reserve.extra.get("normalModelSlug"),
        Some(&serde_json::json!("gpt-5.6-luna"))
    );
    assert!(additional_rate_limit_is_luna_reserve(reserve));
    assert!(openai_quota_has_ready_limit_for_model(
        &usage,
        Some("gpt-5.6-luna")
    ));
    assert!(!openai_quota_has_ready_limit_for_model(
        &usage,
        Some("gpt-luna-reserve")
    ));
    assert!(!openai_quota_has_ready_limit_for_model(
        &usage,
        Some("gpt-5.6-sol")
    ));
}

#[test]
fn unavailable_ordinary_permission_does_not_recover_from_percentages() {
    let usage: UsageResponse = serde_json::from_value(serde_json::json!({
        "ordinaryUsageAllowed": null,
        "rateLimits": {
            "primary": {
                "usedPercent": 0,
                "resetsAt": 1700003600,
                "windowDurationMins": 300
            },
            "secondary": {
                "usedPercent": 0,
                "resetsAt": 1700604800,
                "windowDurationMins": 10080
            }
        }
    }))
    .expect("unknown ordinary permission should deserialize");

    assert!(!openai_quota_has_ready_regular_limit(&usage));
    assert!(required_main_window_snapshot_at(&usage, "5h", 1_700_000_000).is_none());
    assert!(!openai_usage_supports_model(
        &usage,
        false,
        Some("gpt-5.3-codex")
    ));
}

#[test]
fn missing_usage_percent_stays_unknown_in_window_snapshot_and_json() {
    let usage: UsageResponse = serde_json::from_value(serde_json::json!({
        "rate_limit": {
            "primary_window": {
                "reset_at": 1700003600,
                "limit_window_seconds": 18000
            },
            "secondary_window": {
                "used_percent": 20,
                "reset_at": 1700604800,
                "limit_window_seconds": 604800
            }
        }
    }))
    .expect("partial quota payload should deserialize");

    assert!(required_main_window_snapshot_at(&usage, "5h", 1_700_000_000).is_none());
    assert_eq!(format_main_windows_compact(&usage), "5h ? | weekly 80%");
}
