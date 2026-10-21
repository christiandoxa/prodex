use super::*;
use serde_json::json;

fn summary(credits: serde_json::Value) -> RateLimitResetCreditsSummary {
    let mut summary = RateLimitResetCreditsSummary {
        available_count: 3,
        ..Default::default()
    };
    summary
        .update_next_expiry_from_details(&serde_json::to_vec(&json!({"credits": credits})).unwrap())
        .unwrap();
    summary
}

#[test]
fn reset_credit_expiry_compares_rfc3339_instants_and_ignores_unavailable_credits() {
    // The lexically later local time is the earlier instant.
    let credits = json!([
        {"status":"available","expires_at":"2026-10-22T19:00:00Z"},
        {"status":"available","granted_at":"2026-10-20T00:00:00Z",
         "expires_at":"2026-10-23T01:23:45.987+07:00"},
        {"status":"available","expires_at":null},
        {"status":"consumed","expires_at":"2026-10-20T00:00:00Z"},
        {"status":"expired","expires_at":"invalid"},
        {"status":"revoked","expires_at":false},
        {"status":"Available","expires_at":"2020-01-01T00:00:00Z"},
        {"status":"available_later"}
    ]);
    let expected = RateLimitResetCreditExpiry::At(
        chrono::DateTime::parse_from_rfc3339("2026-10-22T18:23:45Z")
            .unwrap()
            .timestamp(),
    );
    assert_eq!(summary(credits.clone()).next_expiry, expected);
    let mut reversed = credits.as_array().unwrap().clone();
    reversed.reverse();
    assert_eq!(summary(json!(reversed)).next_expiry, expected);
}

#[test]
fn reset_credit_expiry_distinguishes_non_expiring_from_incomplete_details() {
    assert_eq!(
        summary(json!([{"status":"available","expires_at":null}])).next_expiry,
        RateLimitResetCreditExpiry::Never
    );
    for credits in [
        json!([]),
        json!([{"status":"consumed","expires_at":null}]),
        json!([{"status":"available"}]),
        json!([{"status":"available","expires_at":"2026-10-23 01:23:45"}]),
        json!([{"status":"available","expires_at":1792693425}]),
    ] {
        assert_eq!(
            summary(credits).next_expiry,
            RateLimitResetCreditExpiry::Unavailable
        );
    }
    for invalid_first in [false, true] {
        let mut credits = vec![
            json!({"status":"available","expires_at":"2026-10-23T01:23:45+07:00"}),
            json!({"status":"available","expires_at":"invalid"}),
        ];
        if invalid_first {
            credits.reverse();
        }
        assert_eq!(
            summary(json!(credits)).next_expiry,
            RateLimitResetCreditExpiry::Unavailable
        );
    }
}

#[test]
fn reset_credit_expiry_zero_count_and_failed_updates_cannot_retain_stale_expiry() {
    let mut credits = RateLimitResetCreditsSummary {
        available_count: 0,
        next_expiry: RateLimitResetCreditExpiry::At(42),
    };
    credits
        .update_next_expiry_from_details(
            br#"{"credits":[{"status":"available","expires_at":"2026-10-23T00:00:00Z"}]}"#,
        )
        .unwrap();
    assert_eq!(credits.next_expiry, RateLimitResetCreditExpiry::Unavailable);
    credits.available_count = 3;
    credits.next_expiry = RateLimitResetCreditExpiry::Never;
    assert!(
        credits
            .update_next_expiry_from_details(b"invalid JSON")
            .is_err()
    );
    assert_eq!(credits.available_count, 3);
    assert_eq!(credits.next_expiry, RateLimitResetCreditExpiry::Unavailable);
}

#[test]
fn reset_credit_expiry_enrichment_does_not_change_usage_json() {
    for value in [json!({"availableCount":3}), json!({"available_count":3})] {
        let mut credits: RateLimitResetCreditsSummary = serde_json::from_value(value).unwrap();
        assert_eq!(credits.next_expiry, RateLimitResetCreditExpiry::Unavailable);
        credits.next_expiry = RateLimitResetCreditExpiry::At(42);
        assert_eq!(
            serde_json::to_value(credits).unwrap(),
            json!({"availableCount":3})
        );
    }
}
