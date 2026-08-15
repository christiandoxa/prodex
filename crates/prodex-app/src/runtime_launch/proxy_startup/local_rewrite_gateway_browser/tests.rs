use std::collections::BTreeMap;

use super::{
    BACKCHANNEL_LOGOUT_EVENT, LOGOUT_INDEX_KEY_PREFIX, RuntimeGatewayBrowserRoute,
    RuntimeGatewayBrowserSession, RuntimeGatewayBrowserTransaction, SESSION_COOKIE,
    browser_backchannel_logout_keys, browser_protect_id_token, browser_route,
    browser_session_cookie_parts, browser_unprotect_id_token, cookie_from_headers, parse_query,
};

#[test]
fn browser_routes_queries_and_cookies_are_exact() {
    assert!(matches!(
        browser_route("/v1/prodex/gateway/auth/login"),
        Some(RuntimeGatewayBrowserRoute::Login)
    ));
    assert!(browser_route("/v1/prodex/gateway/auth/login/extra").is_none());
    assert!(matches!(
        browser_route("/v1/prodex/gateway/auth/backchannel-logout"),
        Some(RuntimeGatewayBrowserRoute::BackchannelLogout)
    ));
    assert!(parse_query("code=a&state=b").is_ok());
    assert!(parse_query("state=a&state=b").is_err());
    let headers = vec![(
        "Cookie".to_string(),
        "other=x; prodex_gateway_session=session_1".to_string(),
    )];
    assert_eq!(
        cookie_from_headers(&headers, SESSION_COOKIE),
        Some("session_1")
    );
}

#[test]
fn browser_shared_records_round_trip_with_protected_authentication_state() {
    let transaction = RuntimeGatewayBrowserTransaction {
        nonce: "nonce".to_string(),
        code_verifier: "verifier".to_string(),
        expires_at_unix_ms: 123,
    };
    let transaction: RuntimeGatewayBrowserTransaction =
        serde_json::from_str(&serde_json::to_string(&transaction).unwrap()).unwrap();
    assert_eq!(transaction.nonce, "nonce");

    let raw_id_token = "fixture.id.token";
    let protection_key = [9; 32];
    let session = RuntimeGatewayBrowserSession {
        protected_id_token: browser_protect_id_token(
            "fixture-session",
            &protection_key,
            raw_id_token,
        )
        .ok()
        .unwrap(),
        csrf_digest: [7; 32],
        logout_keys: vec![format!("{LOGOUT_INDEX_KEY_PREFIX}fixture")],
        expires_at_unix_ms: 456,
    };
    assert_ne!(session.protected_id_token, raw_id_token);
    let serialized = serde_json::to_string(&session).unwrap();
    assert!(!serialized.contains(raw_id_token));
    let session: RuntimeGatewayBrowserSession = serde_json::from_str(&serialized).unwrap();
    let decrypted = browser_unprotect_id_token(
        "fixture-session",
        &protection_key,
        &session.protected_id_token,
    )
    .ok()
    .unwrap();
    assert_eq!(std::str::from_utf8(&decrypted).unwrap(), raw_id_token);
    assert!(browser_session_cookie_parts("session_1").is_none());
}

#[test]
fn backchannel_logout_claims_are_recent_event_bound_and_hashed() {
    let claims = BTreeMap::from([
        ("iat".to_string(), serde_json::json!(1_000)),
        (
            "jti".to_string(),
            serde_json::json!("logout-token-identifier"),
        ),
        (
            "sid".to_string(),
            serde_json::json!("private-session-identifier"),
        ),
        (
            "sub".to_string(),
            serde_json::json!("private-subject-identifier"),
        ),
        (
            "events".to_string(),
            serde_json::json!({BACKCHANNEL_LOGOUT_EVENT: {}}),
        ),
    ]);
    let keys = browser_backchannel_logout_keys(
        &claims,
        "https://identity.example.com",
        "prodex-gateway",
        1_001,
    )
    .ok()
    .unwrap();
    assert_eq!(keys.len(), 2);
    assert!(
        keys.iter()
            .all(|key| key.starts_with(LOGOUT_INDEX_KEY_PREFIX))
    );
    assert!(keys.iter().all(|key| {
        !key.contains("private-session-identifier") && !key.contains("private-subject-identifier")
    }));

    let mut invalid = claims;
    invalid.insert("nonce".to_string(), serde_json::json!("not-allowed"));
    assert!(
        browser_backchannel_logout_keys(
            &invalid,
            "https://identity.example.com",
            "prodex-gateway",
            1_001,
        )
        .is_err()
    );
}
