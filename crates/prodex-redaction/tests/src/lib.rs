use super::*;

fn fake_secret(parts: &[&str]) -> String {
    parts.concat()
}

fn fake_named_secret(name: &str) -> String {
    fake_secret(&["fixture_", name, "_notreal_", "12345"])
}

fn fake_api_key(prefix: &str, name: &str) -> String {
    fake_secret(&[prefix, "fixture-", name, "-notreal-", "123456789"])
}

#[test]
fn redaction_headers_mask_sensitive_values_and_nested_tokens() {
    let authorization_token = fake_named_secret("authorization");
    let api_key = fake_api_key("sk-ant-", "header");
    let cookie_value = fake_named_secret("cookie");
    let forwarded_token = fake_named_secret("forwarded");
    let account_id = fake_secret(&["acct_", "fixture_", "12345"]);
    let nested_bearer_token = fake_named_secret("nested_bearer");
    let proxy_authorization = fake_named_secret("proxy_authorization");
    let headers = vec![
        (
            "authorization".to_string(),
            format!("Bearer {authorization_token}"),
        ),
        ("x-api-key".to_string(), api_key.clone()),
        ("cookie".to_string(), format!("session={cookie_value}")),
        ("x-forwarded-token".to_string(), forwarded_token.clone()),
        ("ChatGPT-Account-Id".to_string(), account_id.clone()),
        (
            "x-observed-value".to_string(),
            format!("Bearer {nested_bearer_token}"),
        ),
        (
            "Proxy-Authorization".to_string(),
            format!("Basic {proxy_authorization}"),
        ),
        ("anthropic-version".to_string(), "2023-06-01".to_string()),
    ];

    let redacted = redaction_redacted_headers_debug(&headers);

    assert!(redacted.contains("authorization"));
    assert!(redacted.contains("Bearer <redacted>"));
    assert!(redacted.contains("Basic <redacted>"));
    assert!(redacted.contains("anthropic-version"));
    assert!(redacted.contains("2023-06-01"));
    assert!(!redacted.contains(authorization_token.as_str()));
    assert!(!redacted.contains(api_key.as_str()));
    assert!(!redacted.contains(cookie_value.as_str()));
    assert!(!redacted.contains(forwarded_token.as_str()));
    assert!(!redacted.contains(account_id.as_str()));
    assert!(!redacted.contains(nested_bearer_token.as_str()));
    assert!(!redacted.contains(proxy_authorization.as_str()));
}

#[test]
fn redaction_masks_standalone_basic_and_token_credentials() {
    let basic = fake_named_secret("standalone_basic");
    let token = fake_named_secret("standalone_token");
    let value = format!("Basic {basic}\nToken {token}");

    let redacted = redaction_redact_secret_like_text(&value);

    assert_eq!(redacted, "Basic <redacted>\nToken <redacted>");
    assert!(!redacted.contains(&basic));
    assert!(!redacted.contains(&token));
}

#[test]
fn gateway_json_serialization_failure_is_fail_closed() {
    let error = serde_json::Error::io(std::io::Error::other("injected serialization failure"));

    assert_eq!(
        redaction_gateway_json_bytes(Err(error)),
        REDACTION_FAILED_GATEWAY_BODY
    );
}

#[test]
fn redaction_body_masks_json_fields_bearer_values_and_api_key_prefixes() {
    let api_key = fake_api_key("sk-ant-", "json");
    let access_token = fake_named_secret("access_token");
    let refresh_token = fake_named_secret("refresh_token");
    let client_secret = fake_named_secret("client_secret");
    let password = fake_named_secret("password");
    let bearer_token = fake_named_secret("body_bearer");
    let free_text_key = fake_api_key("sk-proj-", "free_text");
    let body = serde_json::to_vec(&serde_json::json!({
        "model": "claude-sonnet-4-6",
        "max_tokens": 1024,
        "api_key": api_key.clone(),
        "auth": {
            "access_token": access_token.clone(),
            "refreshToken": refresh_token.clone(),
            "client_secret": client_secret.clone(),
            "password": password.clone()
        },
        "messages": [
            {
                "role": "user",
                "content": format!(
                    "Use Authorization: Bearer {bearer_token} and {free_text_key}"
                )
            }
        ]
    }))
    .expect("test body should serialize");

    let redacted = redaction_redacted_body_snippet(&body, 4096);

    assert!(redacted.contains("claude-sonnet-4-6"));
    assert!(redacted.contains("max_tokens"));
    assert!(redacted.contains("\"api_key\":\"<redacted>\""));
    assert!(redacted.contains("\"access_token\":\"<redacted>\""));
    assert!(redacted.contains("\"refreshToken\":\"<redacted>\""));
    assert!(redacted.contains("\"client_secret\":\"<redacted>\""));
    assert!(redacted.contains("\"password\":\"<redacted>\""));
    assert!(redacted.contains("Authorization: Bearer <redacted>"));
    assert!(redacted.contains("sk-proj-<redacted>"));
    assert!(!redacted.contains(api_key.as_str()));
    assert!(!redacted.contains(access_token.as_str()));
    assert!(!redacted.contains(refresh_token.as_str()));
    assert!(!redacted.contains(client_secret.as_str()));
    assert!(!redacted.contains(password.as_str()));
    assert!(!redacted.contains(bearer_token.as_str()));
    assert!(!redacted.contains(free_text_key.as_str()));
}

#[test]
fn redaction_body_masks_plain_text_secret_assignments() {
    let api_key = fake_named_secret("plain_api_key");
    let access_token = fake_named_secret("plain_access_token");
    let bearer_token = fake_named_secret("plain_bearer");
    let prefixed_key = fake_api_key("sk-live-", "plain");
    let body = format!(
        "api_key={api_key} access_token: {access_token} \
             Authorization: Bearer {bearer_token} x={prefixed_key}"
    );

    let redacted = redaction_redacted_body_snippet(body.as_bytes(), 4096);

    assert!(redacted.contains("api_key=<redacted>"));
    assert!(redacted.contains("access_token: <redacted>"));
    assert!(redacted.contains("Authorization: Bearer <redacted>"));
    assert!(redacted.contains("sk-live-<redacted>"));
    assert!(!redacted.contains(api_key.as_str()));
    assert!(!redacted.contains(access_token.as_str()));
    assert!(!redacted.contains(bearer_token.as_str()));
    assert!(!redacted.contains(prefixed_key.as_str()));
}

#[test]
fn redaction_large_nonsecret_token_remains_unchanged() {
    let value = "A".repeat(128 * 1024);

    assert_eq!(redaction_redact_secret_like_text(&value), value);
}

#[test]
fn redaction_masks_sensitive_url_query_values_in_json_text() {
    let value = r#"{"error":"Authorization: Bearer fixture-token-123 url=https://example.test?api_key=sk-fixture-123"}"#;

    assert_eq!(
        redaction_redact_secret_like_text(value),
        r#"{"error":"Authorization: Bearer <redacted> url=https://example.test?api_key=<redacted>"}"#
    );
}

#[test]
fn redaction_gateway_body_masks_pii_and_secret_like_content() {
    let bearer_token = fake_named_secret("gateway_bearer");
    let prefixed_key = fake_api_key("sk-proj-", "gateway");
    let body = serde_json::to_vec(&serde_json::json!({
        "model": "gpt-5.4",
        "input": [
            {
                "type": "input_text",
                "text": format!(
                    "email alice@example.com card 4111-1111-1111-1111 bearer {bearer_token} key {prefixed_key}"
                )
            },
            {
                "type": "input_text",
                "text": "bob@example.org"
            }
        ]
    }))
    .expect("test body should serialize");

    let redacted = redaction_redact_gateway_body(&body).expect("body should change");
    let redacted = String::from_utf8(redacted).expect("redacted body should be utf8");

    assert!(redacted.contains("\"model\":\"gpt-5.4\""));
    assert!(redacted.contains("<redacted>"));
    assert!(!redacted.contains("alice@example.com"));
    assert!(!redacted.contains("bob@example.org"));
    assert!(!redacted.contains("4111-1111-1111-1111"));
    assert!(!redacted.contains(bearer_token.as_str()));
    assert!(!redacted.contains(prefixed_key.as_str()));
}

#[test]
fn redaction_cli_args_mask_sensitive_flags_and_inline_values() {
    let api_value = fake_named_secret("cli_flag");
    let config_token = fake_named_secret("config_token");
    let cli_bearer_token = fake_named_secret("cli_bearer");
    let cli_prefixed_key = fake_api_key("sk-proj-", "cli");
    let args = vec![
        OsString::from("--api-key"),
        OsString::from(api_value.clone()),
        OsString::from(format!("--config=access_token=\"{config_token}\"")),
        OsString::from("--header"),
        OsString::from(format!("Authorization: Bearer {cli_bearer_token}")),
        OsString::from("--prompt"),
        OsString::from(format!("Use {cli_prefixed_key} today")),
        OsString::from("--model"),
        OsString::from("gpt-5.4"),
    ];

    let redacted = redaction_redacted_cli_args(&args).join("\n");

    assert!(redacted.contains("--api-key"));
    assert!(redacted.contains("<redacted>"));
    assert!(redacted.contains("access_token=\"<redacted>\""));
    assert!(redacted.contains("Authorization: Bearer <redacted>"));
    assert!(redacted.contains("sk-proj-<redacted>"));
    assert!(redacted.contains("gpt-5.4"));
    assert!(!redacted.contains(api_value.as_str()));
    assert!(!redacted.contains(config_token.as_str()));
    assert!(!redacted.contains(cli_bearer_token.as_str()));
    assert!(!redacted.contains(cli_prefixed_key.as_str()));
}

#[test]
fn redaction_env_values_mask_sensitive_keys_and_secret_like_values() {
    let env_value = fake_named_secret("env_value");
    let env_bearer_token = fake_named_secret("env_bearer");
    assert_eq!(
        redaction_redacted_env_value(OsStr::new("ANTHROPIC_AUTH_TOKEN"), OsStr::new(&env_value),),
        REDACTED
    );
    assert_eq!(
        redaction_redacted_env_value(
            OsStr::new("VISIBLE"),
            OsStr::new(&format!("Bearer {env_bearer_token}")),
        ),
        "Bearer <redacted>"
    );
    assert_eq!(
        redaction_redacted_env_value(OsStr::new("PRODEX_VISIBLE"), OsStr::new("1")),
        "1"
    );
}
