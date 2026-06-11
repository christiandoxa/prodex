use super::*;

#[test]
fn usage_url_matches_backend_shape() {
    assert_eq!(
        usage_url("https://chatgpt.com/backend-api"),
        "https://chatgpt.com/backend-api/wham/usage"
    );
    assert_eq!(
        usage_url("https://chatgpt.com"),
        "https://chatgpt.com/api/codex/usage"
    );
}

#[test]
fn response_body_pretty_prints_json() {
    let formatted = format_response_body(br#"{"error":{"message":"quota"}}"#);
    assert!(formatted.contains("\"message\": \"quota\""));
}

#[test]
fn plan_capacity_pressure_scale_normalizes_known_plan_names() {
    assert_eq!(plan_capacity_pressure_scale_bps("plus"), 10_000);
    assert_eq!(plan_capacity_pressure_scale_bps("Pro Lite"), 5_000);
    assert_eq!(plan_capacity_pressure_scale_bps("pro-20x"), 2_000);
    assert_eq!(plan_capacity_pressure_scale_bps("free"), 12_000);
    assert_eq!(plan_capacity_pressure_scale_bps("enterprise"), 10_000);
}

#[test]
fn quota_auth_filter_matches_labels_and_compatibility() {
    let no_auth = AuthSummary {
        label: "no-auth".to_string(),
        quota_compatible: false,
    };
    let chatgpt = AuthSummary {
        label: "chatgpt".to_string(),
        quota_compatible: true,
    };

    assert!(QuotaAuthFilter::parse("no-auth").unwrap().matches(&no_auth));
    assert!(!QuotaAuthFilter::parse("no-auth").unwrap().matches(&chatgpt));
    assert!(
        QuotaAuthFilter::parse("quota-compatible")
            .unwrap()
            .matches(&chatgpt)
    );
    assert!(
        QuotaAuthFilter::parse("non-quota-compatible")
            .unwrap()
            .matches(&no_auth)
    );
}
