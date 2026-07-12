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

#[test]
fn credential_debug_is_redacted_through_nested_dtos() {
    const ACCESS: &str = "debug-access-sentinel";
    const ACCOUNT: &str = "debug-account-sentinel";
    const REFRESH: &str = "debug-refresh-sentinel";
    const ID: &str = "debug-id-sentinel";
    const OPENAI_KEY: &str = "debug-openai-key-sentinel";
    const BEDROCK_KEY: &str = "debug-bedrock-key-sentinel";

    let auth = UsageAuth {
        access_token: ACCESS.to_string(),
        account_id: Some(ACCOUNT.to_string()),
        refresh_token: Some(REFRESH.to_string()),
        expires_at: Some(123),
        last_refresh: Some(100),
    };
    let outcome = UsageAuthSyncOutcome {
        auth: auth.clone(),
        source: UsageAuthSyncSource::Refreshed,
        auth_changed: true,
    };
    let stored = StoredAuth {
        auth_mode: Some("chatgpt".to_string()),
        tokens: Some(StoredTokens {
            access_token: Some(ACCESS.to_string()),
            account_id: Some(ACCOUNT.to_string()),
            id_token: Some(ID.to_string()),
            refresh_token: Some(REFRESH.to_string()),
        }),
        openai_api_key: Some(OPENAI_KEY.to_string()),
        bedrock_api_key: Some(BedrockApiKeyAuth {
            api_key: Some(BEDROCK_KEY.to_string()),
            region: Some("us-east-1".to_string()),
        }),
        last_refresh: Some("2026-01-01T00:00:00Z".to_string()),
    };
    let refreshed = ChatgptRefreshResponse {
        id_token: Some(ID.to_string()),
        access_token: Some(ACCESS.to_string()),
        refresh_token: Some(REFRESH.to_string()),
    };

    let rendered = format!("{auth:?}\n{outcome:?}\n{stored:?}\n{refreshed:?}");
    assert!(rendered.contains("<redacted>"), "{rendered}");
    for secret in [ACCESS, ACCOUNT, REFRESH, ID, OPENAI_KEY, BEDROCK_KEY] {
        assert!(!rendered.contains(secret), "{rendered}");
    }

    let stored_json = serde_json::to_value(&stored).unwrap();
    assert_eq!(stored_json["OPENAI_API_KEY"], OPENAI_KEY);
    assert_eq!(stored_json["tokens"]["id_token"], ID);
    assert_eq!(stored_json["bedrock_api_key"]["api_key"], BEDROCK_KEY);
    let refreshed_json = serde_json::to_value(&refreshed).unwrap();
    assert_eq!(refreshed_json["access_token"], ACCESS);
}

#[test]
fn credential_dtos_zeroize() {
    use zeroize::{Zeroize, ZeroizeOnDrop};

    fn assert_zeroize_on_drop<T: ZeroizeOnDrop>() {}

    assert_zeroize_on_drop::<UsageAuth>();
    assert_zeroize_on_drop::<StoredAuth>();
    assert_zeroize_on_drop::<StoredTokens>();
    assert_zeroize_on_drop::<BedrockApiKeyAuth>();
    assert_zeroize_on_drop::<ChatgptRefreshResponse>();

    let mut auth = UsageAuth {
        access_token: "access".to_string(),
        account_id: Some("account".to_string()),
        refresh_token: Some("refresh".to_string()),
        expires_at: None,
        last_refresh: None,
    };
    auth.zeroize();
    assert!(auth.access_token.is_empty());
    assert!(auth.account_id.is_none());
    assert!(auth.refresh_token.is_none());
}
