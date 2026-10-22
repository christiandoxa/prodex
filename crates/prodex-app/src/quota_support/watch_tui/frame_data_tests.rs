use super::*;

#[test]
fn compiled_quota_row_assembly_preserves_workspace_credits_and_provider_order() {
    let facts = serde_json::json!({
        "kind": "openai", "profile": " profile ", "active": true, "auth": "chatgpt",
        "account": "\u{2003}test@example.com\u{3000}", "plan": " plus ",
        "main": "5h full", "compact_main": "5h compact", "status": "Ready",
        "reset": "5h 1h", "reset_credit_summary": "2 available | next expiry: 2026-10-23 01:23:45 +07:00",
        "workspace_name": " \u{2003} ", "workspace_id": " fake-workspace ",
        "additional": ["extra-one", "extra-東京"], "updated": "now",
    });
    let row: serde_json::Value = quota_watch_projection(&facts, QuotaWatchPresentation::Row, true);
    assert_eq!(
        row,
        serde_json::json!({
            "profile": [" profile "], "current": ["*"], "auth": ["chatgpt"],
            "account": ["test@example.com"], "plan": ["plus"], "status": ["Ready"],
            "remaining": ["5h compact"],
            "detail": ["resets: 5h 1h | workspace: profile | reset credits: 2 available | next expiry: 2026-10-23 01:23:45 +07:00 | extra-one | extra-東京"],
        })
    );
    let fields: Vec<(String, String)> =
        quota_watch_projection(&facts, QuotaWatchPresentation::ProfileFields, false);
    assert_eq!(
        fields,
        vec![
            ("Account".to_string(), "test@example.com".to_string()),
            ("Plan".to_string(), "plus".to_string()),
            ("Status".to_string(), "Ready".to_string()),
            ("Main".to_string(), "5h full".to_string()),
            (
                "Reset credits".to_string(),
                "2 available | next expiry: 2026-10-23 01:23:45 +07:00".to_string()
            ),
            ("Last Updated".to_string(), "now".to_string()),
        ]
    );
    let fields: serde_json::Value =
        quota_watch_projection(&facts, QuotaWatchPresentation::ProfileFields, true);
    assert_eq!(
        fields[4],
        serde_json::json!([
            "Reset",
            "5h 1h; reset credits: 2 available | next expiry: 2026-10-23 01:23:45 +07:00"
        ])
    );
    let compact: serde_json::Value =
        quota_watch_projection(&facts, QuotaWatchPresentation::Row, false);
    assert_eq!(compact["detail"], serde_json::json!([]));
}

#[test]
fn compiled_quota_rows_preserve_empty_present_plan_and_error_passthrough() {
    for (plan, expected) in [
        (serde_json::Value::Null, "fallback"),
        (serde_json::json!(" \u{2003} "), "-"),
    ] {
        let facts = serde_json::json!({
            "kind": "copilot", "plan": plan, "fallback_plan": "fallback",
            "account": null, "status": "Ready", "main": "90%", "reset": null,
            "updated": "now", "profile": "main", "auth": "oauth", "active": false,
        });
        let row: serde_json::Value =
            quota_watch_projection(&facts, QuotaWatchPresentation::Row, true);
        assert_eq!(row["plan"], serde_json::json!([expected]));
        assert_eq!(row["account"], serde_json::json!(["-"]));
        assert_eq!(row["detail"], serde_json::json!(["resets: unavailable"]));
        let fields: serde_json::Value =
            quota_watch_projection(&facts, QuotaWatchPresentation::ProfileFields, false);
        assert_eq!(fields[4], serde_json::json!(["Reset", "unavailable"]));
    }
    let facts = serde_json::json!({"kind":"error", "status":"Blocked unauthorized",
        "reset":"error: bad \"credential\" 🧭", "profile":"broken", "auth":"oauth"});
    let row: serde_json::Value = quota_watch_projection(&facts, QuotaWatchPresentation::Row, true);
    assert_eq!(
        row["detail"],
        serde_json::json!(["error: bad \"credential\" 🧭"])
    );
    assert_eq!(row["remaining"], serde_json::json!(["-"]));
    assert!(quota_watch_projection::<bool>(
        &serde_json::json!("\u{2003}Model provider: example"),
        QuotaWatchPresentation::CompactLabel,
        false,
    ));
    assert!(!quota_watch_projection::<bool>(
        &serde_json::json!("model provider: example"),
        QuotaWatchPresentation::CompactLabel,
        false,
    ));
}

#[test]
fn quota_live_profile_preserves_expiry_in_both_detail_modes() {
    use prodex_quota::{RateLimitResetCreditExpiry as Expiry, RateLimitResetCreditsSummary};
    for (count, expiry, suffix) in [
        (3, Expiry::At(1_792_693_425), None),
        (2, Expiry::Never, Some("2 available | next expiry: never")),
        (
            1,
            Expiry::Unavailable,
            Some("1 available | next expiry: unavailable"),
        ),
        (0, Expiry::At(1_792_693_425), Some("0 available")),
    ] {
        let credits = RateLimitResetCreditsSummary {
            available_count: count,
            next_expiry: expiry,
        };
        let expected = prodex_quota::format_reset_credits(&credits);
        if let Some(suffix) = suffix {
            assert_eq!(expected, suffix);
        }
        let mut usage: prodex_quota::UsageResponse =
            serde_json::from_value(serde_json::json!({})).unwrap();
        usage.rate_limit_reset_credits = Some(credits);
        let snapshot = ProviderQuotaSnapshot::OpenAi(usage);
        for detail in [false, true] {
            let fields = quota_watch_profile_fields("now", &snapshot, detail);
            let values = fields
                .iter()
                .map(|(_, value)| value.as_str())
                .collect::<Vec<_>>()
                .join(" ");
            assert_eq!(values.matches(&expected).count(), 1);
            assert_eq!(values.contains("next expiry:"), count > 0);
        }
    }
    let usage = serde_json::from_value(serde_json::json!({})).unwrap();
    let fields = quota_watch_profile_fields("now", &ProviderQuotaSnapshot::OpenAi(usage), false);
    assert!(!fields.iter().any(|(label, _)| label == "Reset credits"));
}
