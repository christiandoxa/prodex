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
fn redaction_masks_every_cookie_on_a_plain_text_header_line() {
    let first = fake_named_secret("first_cookie");
    let second = fake_named_secret("second_cookie");
    let value = format!("Cookie: first={first}; second={second}\nvisible=ok");

    let redacted = redaction_redact_secret_like_text(&value);

    assert_eq!(redacted, "Cookie: <redacted>\nvisible=ok");
    assert!(!redacted.contains(&first));
    assert!(!redacted.contains(&second));
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
fn redaction_preserves_uuid_identifiers_while_masking_card_numbers() {
    let tenant_id = "019ffa02-3993-7e50-b331-5604955720ad";
    let mut value = serde_json::json!({
        "tenant_id": tenant_id,
        "message": format!("tenant {tenant_id} card=4111-1111-1111-1111"),
    });

    redaction_redact_json(&mut value);

    assert_eq!(value["tenant_id"], tenant_id);
    assert_eq!(
        value["message"],
        format!("tenant {tenant_id} card={REDACTED}")
    );
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
