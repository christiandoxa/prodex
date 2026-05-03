use super::*;

#[test]
fn auth_summary_detects_chatgpt_token() {
    let summary = auth_summary_from_auth_text(r#"{"tokens":{"access_token":" token "}}"#);
    assert_eq!(summary.label, "chatgpt");
    assert!(summary.quota_compatible);
}

#[test]
fn auth_summary_detects_api_key() {
    let summary =
        auth_summary_from_auth_text(r#"{"auth_mode":"api_key","OPENAI_API_KEY":"sk-test"}"#);
    assert_eq!(summary.label, "api-key");
    assert!(!summary.quota_compatible);
}

#[test]
fn usage_auth_prefers_jwt_account_id() {
    let jwt =
        test_jwt(r#"{"https://api.openai.com/auth":{"chatgpt_account_id":"acct_jwt"},"exp":200}"#);
    let auth = usage_auth_from_auth_text(&format!(
        r#"{{
                "tokens": {{
                    "access_token": "{jwt}",
                    "account_id": "acct_stored",
                    "refresh_token": "refresh"
                }},
                "last_refresh": "1970-01-01T00:00:05Z"
            }}"#
    ))
    .unwrap();

    assert_eq!(auth.account_id.as_deref(), Some("acct_jwt"));
    assert_eq!(auth.expires_at, Some(200));
    assert_eq!(auth.last_refresh, Some(5));
}

#[test]
fn refresh_mutates_tokens_and_timestamp() {
    let jwt = test_jwt(r#"{"https://api.openai.com/auth.chatgpt_account_id":"acct_refreshed"}"#);
    let mut value = serde_json::json!({"tokens":{"refresh_token":"old"}});

    apply_chatgpt_refresh(
        &mut value,
        ChatgptRefreshResponse {
            id_token: Some("id".to_string()),
            access_token: Some(jwt),
            refresh_token: Some("new".to_string()),
        },
        "2026-04-30T00:00:00Z".to_string(),
    )
    .unwrap();

    assert_eq!(value["tokens"]["id_token"], "id");
    assert_eq!(value["tokens"]["account_id"], "acct_refreshed");
    assert_eq!(value["tokens"]["refresh_token"], "new");
    assert_eq!(value["last_refresh"], "2026-04-30T00:00:00Z");
}

fn test_jwt(payload_json: &str) -> String {
    let header = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(br#"{"alg":"none"}"#);
    let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(payload_json.as_bytes());
    format!("{header}.{payload}.sig")
}
