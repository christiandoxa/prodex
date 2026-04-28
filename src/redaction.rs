use std::ffi::{OsStr, OsString};

const REDACTED: &str = "<redacted>";
const API_KEY_PREFIXES: &[&str] = &[
    "sk-proj-", "sk-ant-", "sk-live-", "sk_test_", "sk_live_", "sk-", "sk_",
];

pub(crate) fn redaction_text_snippet(text: &str, max_chars: usize) -> String {
    let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.is_empty() {
        return "-".to_string();
    }

    let snippet = normalized.chars().take(max_chars).collect::<String>();
    if normalized.chars().count() > max_chars {
        format!("{snippet}...")
    } else {
        snippet
    }
}

pub(crate) fn redaction_redacted_body_snippet(body: &[u8], max_chars: usize) -> String {
    let redacted = match serde_json::from_slice::<serde_json::Value>(body) {
        Ok(mut value) => {
            redaction_redact_json_value(&mut value);
            serde_json::to_string(&value).unwrap_or_else(|_| {
                redaction_redact_secret_like_text(&String::from_utf8_lossy(body))
            })
        }
        Err(_) => redaction_redact_secret_like_text(&String::from_utf8_lossy(body)),
    };
    redaction_text_snippet(&redacted, max_chars)
}

pub(crate) fn redaction_redacted_headers_debug(headers: &[(String, String)]) -> String {
    let redacted_headers = headers
        .iter()
        .map(|(name, value)| {
            let value = if redaction_key_looks_sensitive(name) {
                redaction_redact_sensitive_header_value(name, value)
            } else {
                redaction_redact_secret_like_text(value)
            };
            (name.clone(), value)
        })
        .collect::<Vec<_>>();
    format!("{redacted_headers:?}")
}

pub(crate) fn redaction_redacted_cli_args(args: &[OsString]) -> Vec<String> {
    let mut redact_next = false;
    let mut redacted = Vec::with_capacity(args.len());

    for arg in args {
        if redact_next {
            redacted.push(REDACTED.to_string());
            redact_next = false;
            continue;
        }

        let text = arg.to_string_lossy();
        let sensitive_flag_without_value = redaction_cli_flag_name_looks_sensitive(&text)
            && !text.contains('=')
            && text.starts_with('-');
        if sensitive_flag_without_value {
            redacted.push(redaction_display_os(arg));
            redact_next = true;
            continue;
        }

        let redacted_text = redaction_redact_secret_like_text(&text);
        if redacted_text != text {
            redacted.push(redacted_text);
        } else {
            redacted.push(redaction_display_os(arg));
        }
    }

    redacted
}

pub(crate) fn redaction_redacted_env_value(key: &OsStr, value: &OsStr) -> String {
    if redaction_key_looks_sensitive(&key.to_string_lossy()) {
        return REDACTED.to_string();
    }

    let text = value.to_string_lossy();
    let redacted = redaction_redact_secret_like_text(&text);
    if redacted != text {
        redacted
    } else {
        redaction_display_os(value)
    }
}

pub(crate) fn redaction_display_os(value: &OsStr) -> String {
    redaction_display_text(&value.to_string_lossy())
}

pub(crate) fn redaction_key_looks_sensitive(name: &str) -> bool {
    let normalized = name
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect::<String>();

    matches!(
        normalized.as_str(),
        "authorization"
            | "apikey"
            | "xapikey"
            | "cookie"
            | "setcookie"
            | "token"
            | "accesstoken"
            | "refreshtoken"
            | "idtoken"
            | "secret"
            | "password"
            | "credential"
            | "credentials"
            | "accountid"
            | "chatgptaccountid"
    ) || normalized.ends_with("token")
        || normalized.contains("apikey")
        || normalized.contains("secret")
        || normalized.contains("password")
        || normalized.contains("cookie")
        || normalized.contains("credential")
}

fn redaction_redact_json_value(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, value) in map.iter_mut() {
                if redaction_key_looks_sensitive(key) {
                    *value = serde_json::Value::String(REDACTED.to_string());
                } else {
                    redaction_redact_json_value(value);
                }
            }
        }
        serde_json::Value::Array(values) => {
            for value in values {
                redaction_redact_json_value(value);
            }
        }
        serde_json::Value::String(value) => {
            *value = redaction_redact_secret_like_text(value);
        }
        _ => {}
    }
}

fn redaction_redact_sensitive_header_value(name: &str, value: &str) -> String {
    if name.eq_ignore_ascii_case("authorization") {
        let trimmed = value.trim_start();
        for scheme in ["Bearer", "Basic", "Token"] {
            if trimmed.len() > scheme.len()
                && redaction_starts_with_ignore_ascii_case(trimmed.as_bytes(), 0, scheme.as_bytes())
                && trimmed.as_bytes()[scheme.len()].is_ascii_whitespace()
            {
                return format!("{scheme} {REDACTED}");
            }
        }
    }
    REDACTED.to_string()
}

fn redaction_redact_secret_like_text(value: &str) -> String {
    let value = redaction_redact_sensitive_key_value_text(value);
    let value = redaction_redact_bearer_like_values(&value);
    redaction_redact_prefixed_api_key_tokens(&value)
}

fn redaction_redact_sensitive_key_value_text(value: &str) -> String {
    let mut redacted = String::with_capacity(value.len());
    let bytes = value.as_bytes();
    let mut index = 0usize;

    while index < bytes.len() {
        if let Some((key, after_key)) = redaction_parse_potential_field_name(value, index) {
            let separator = redaction_skip_ascii_whitespace(bytes, after_key);
            if separator < bytes.len()
                && matches!(bytes[separator], b':' | b'=')
                && redaction_key_looks_sensitive(key)
            {
                let value_start = redaction_skip_ascii_whitespace(bytes, separator + 1);
                let (value_end, replacement) = redaction_redacted_field_value(value, value_start);
                redacted.push_str(&value[index..value_start]);
                redacted.push_str(&replacement);
                index = value_end;
                continue;
            }
        }

        let Some(ch) = value[index..].chars().next() else {
            break;
        };
        redacted.push(ch);
        index += ch.len_utf8();
    }

    redacted
}

fn redaction_parse_potential_field_name(value: &str, index: usize) -> Option<(&str, usize)> {
    let bytes = value.as_bytes();
    let first = *bytes.get(index)?;

    if matches!(first, b'"' | b'\'') {
        let quote = first;
        let key_start = index + 1;
        let mut cursor = key_start;
        while cursor < bytes.len() {
            match bytes[cursor] {
                byte if byte == quote => return Some((&value[key_start..cursor], cursor + 1)),
                b'\\' => cursor = cursor.saturating_add(2),
                _ => cursor += 1,
            }
        }
        return None;
    }

    if !redaction_field_name_start_byte(first) {
        return None;
    }
    let mut cursor = index + 1;
    while cursor < bytes.len() && redaction_field_name_byte(bytes[cursor]) {
        cursor += 1;
    }
    Some((&value[index..cursor], cursor))
}

fn redaction_field_name_start_byte(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || matches!(byte, b'_' | b'-')
}

fn redaction_field_name_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-')
}

fn redaction_skip_ascii_whitespace(bytes: &[u8], mut index: usize) -> usize {
    while index < bytes.len() && bytes[index].is_ascii_whitespace() {
        index += 1;
    }
    index
}

fn redaction_redacted_field_value(value: &str, value_start: usize) -> (usize, String) {
    let bytes = value.as_bytes();
    let Some(first) = bytes.get(value_start).copied() else {
        return (value_start, REDACTED.to_string());
    };

    if matches!(first, b'"' | b'\'') {
        let quote = first;
        let mut cursor = value_start + 1;
        while cursor < bytes.len() {
            match bytes[cursor] {
                byte if byte == quote => {
                    return (
                        cursor + 1,
                        format!("{}{}{}", quote as char, REDACTED, quote as char),
                    );
                }
                b'\\' => cursor = cursor.saturating_add(2),
                _ => cursor += 1,
            }
        }
        return (
            bytes.len(),
            format!("{}{}{}", quote as char, REDACTED, quote as char),
        );
    }

    for scheme in ["Bearer", "Basic", "Token"] {
        let scheme_end = value_start + scheme.len();
        if redaction_starts_with_ignore_ascii_case(bytes, value_start, scheme.as_bytes())
            && scheme_end < bytes.len()
            && bytes[scheme_end].is_ascii_whitespace()
        {
            let token_start = redaction_skip_ascii_whitespace(bytes, scheme_end);
            let token_end = redaction_secret_token_end(bytes, token_start);
            if token_end > token_start {
                let actual_scheme = &value[value_start..scheme_end];
                return (token_end, format!("{actual_scheme} {REDACTED}"));
            }
        }
    }

    let mut cursor = value_start;
    while cursor < bytes.len()
        && !bytes[cursor].is_ascii_whitespace()
        && !matches!(bytes[cursor], b',' | b'&' | b';' | b'}' | b']' | b')')
    {
        cursor += 1;
    }
    (cursor, REDACTED.to_string())
}

fn redaction_redact_bearer_like_values(value: &str) -> String {
    let mut redacted = String::with_capacity(value.len());
    let bytes = value.as_bytes();
    let mut index = 0usize;

    while index < bytes.len() {
        if redaction_starts_with_ignore_ascii_case(bytes, index, b"bearer")
            && redaction_ascii_boundary_before(bytes, index)
            && redaction_ascii_boundary_after(bytes, index + b"bearer".len())
        {
            let whitespace = redaction_skip_ascii_whitespace(bytes, index + b"bearer".len());
            if whitespace > index + b"bearer".len() {
                let token_end = redaction_secret_token_end(bytes, whitespace);
                if token_end > whitespace {
                    redacted.push_str(&value[index..whitespace]);
                    redacted.push_str(REDACTED);
                    index = token_end;
                    continue;
                }
            }
        }

        let Some(ch) = value[index..].chars().next() else {
            break;
        };
        redacted.push(ch);
        index += ch.len_utf8();
    }

    redacted
}

fn redaction_redact_prefixed_api_key_tokens(value: &str) -> String {
    let mut redacted = String::with_capacity(value.len());
    let bytes = value.as_bytes();
    let mut index = 0usize;

    while index < bytes.len() {
        if redaction_ascii_boundary_before(bytes, index)
            && let Some(prefix) = API_KEY_PREFIXES
                .iter()
                .find(|prefix| {
                    redaction_starts_with_ignore_ascii_case(bytes, index, prefix.as_bytes())
                })
                .copied()
        {
            let token_end = redaction_secret_token_end(bytes, index);
            if token_end >= index + prefix.len() + 8 {
                redacted.push_str(prefix);
                redacted.push_str(REDACTED);
                index = token_end;
                continue;
            }
        }

        let Some(ch) = value[index..].chars().next() else {
            break;
        };
        redacted.push(ch);
        index += ch.len_utf8();
    }

    redacted
}

fn redaction_secret_token_end(bytes: &[u8], mut index: usize) -> usize {
    while index < bytes.len()
        && !bytes[index].is_ascii_whitespace()
        && !matches!(
            bytes[index],
            b'"' | b'\'' | b',' | b'}' | b']' | b')' | b';' | b'&'
        )
    {
        index += 1;
    }
    index
}

fn redaction_cli_flag_name_looks_sensitive(value: &str) -> bool {
    let trimmed = value.trim_start_matches('-');
    let name = trimmed
        .split_once('=')
        .map(|(name, _)| name)
        .unwrap_or(trimmed);
    redaction_key_looks_sensitive(name)
}

fn redaction_starts_with_ignore_ascii_case(bytes: &[u8], index: usize, needle: &[u8]) -> bool {
    bytes
        .get(index..index + needle.len())
        .is_some_and(|candidate| candidate.eq_ignore_ascii_case(needle))
}

fn redaction_ascii_boundary_before(bytes: &[u8], index: usize) -> bool {
    index == 0 || !bytes[index - 1].is_ascii_alphanumeric()
}

fn redaction_ascii_boundary_after(bytes: &[u8], index: usize) -> bool {
    index >= bytes.len() || !bytes[index].is_ascii_alphanumeric()
}

fn redaction_display_text(value: &str) -> String {
    if value.is_empty() {
        return "''".to_string();
    }
    if value.chars().all(|ch| {
        ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.' | '/' | ':' | '=' | '<' | '>')
    }) {
        return value.to_string();
    }
    format!("{value:?}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redaction_headers_mask_sensitive_values_and_nested_tokens() {
        let headers = vec![
            (
                "authorization".to_string(),
                "Bearer live_authorization_secret_12345".to_string(),
            ),
            (
                "x-api-key".to_string(),
                "sk-ant-api-key-secret-12345".to_string(),
            ),
            (
                "cookie".to_string(),
                "session=secret-cookie-value".to_string(),
            ),
            (
                "x-forwarded-token".to_string(),
                "forwarded-token-secret".to_string(),
            ),
            (
                "ChatGPT-Account-Id".to_string(),
                "acct_live_12345".to_string(),
            ),
            (
                "x-observed-value".to_string(),
                "Bearer nested_bearer_secret_12345".to_string(),
            ),
            ("anthropic-version".to_string(), "2023-06-01".to_string()),
        ];

        let redacted = redaction_redacted_headers_debug(&headers);

        assert!(redacted.contains("authorization"));
        assert!(redacted.contains("Bearer <redacted>"));
        assert!(redacted.contains("anthropic-version"));
        assert!(redacted.contains("2023-06-01"));
        assert!(!redacted.contains("live_authorization_secret_12345"));
        assert!(!redacted.contains("sk-ant-api-key-secret-12345"));
        assert!(!redacted.contains("secret-cookie-value"));
        assert!(!redacted.contains("forwarded-token-secret"));
        assert!(!redacted.contains("acct_live_12345"));
        assert!(!redacted.contains("nested_bearer_secret_12345"));
    }

    #[test]
    fn redaction_body_masks_json_fields_bearer_values_and_api_key_prefixes() {
        let body = br#"{
            "model": "claude-sonnet-4-6",
            "max_tokens": 1024,
            "api_key": "sk-ant-json-secret-123456789",
            "auth": {
                "access_token": "access-token-secret",
                "refreshToken": "refresh-token-secret",
                "client_secret": "client-secret-value",
                "password": "password-secret"
            },
            "messages": [
                {
                    "role": "user",
                    "content": "Use Authorization: Bearer auth_bearer_secret_12345 and sk-proj-free-text-secret-12345"
                }
            ]
        }"#;

        let redacted = redaction_redacted_body_snippet(body, 4096);

        assert!(redacted.contains("claude-sonnet-4-6"));
        assert!(redacted.contains("max_tokens"));
        assert!(redacted.contains("\"api_key\":\"<redacted>\""));
        assert!(redacted.contains("\"access_token\":\"<redacted>\""));
        assert!(redacted.contains("\"refreshToken\":\"<redacted>\""));
        assert!(redacted.contains("\"client_secret\":\"<redacted>\""));
        assert!(redacted.contains("\"password\":\"<redacted>\""));
        assert!(redacted.contains("Authorization: Bearer <redacted>"));
        assert!(redacted.contains("sk-proj-<redacted>"));
        assert!(!redacted.contains("sk-ant-json-secret-123456789"));
        assert!(!redacted.contains("access-token-secret"));
        assert!(!redacted.contains("refresh-token-secret"));
        assert!(!redacted.contains("client-secret-value"));
        assert!(!redacted.contains("password-secret"));
        assert!(!redacted.contains("auth_bearer_secret_12345"));
        assert!(!redacted.contains("free-text-secret-12345"));
    }

    #[test]
    fn redaction_body_masks_plain_text_secret_assignments() {
        let body = concat!(
            "api_",
            "key",
            "=",
            "plain-api-key-secret-12345 access_token: plain-access-token-secret ",
            "Authorization: Bearer plain-bearer-secret-12345 x=sk-live-",
            "plain-secret-12345"
        )
        .as_bytes();

        let redacted = redaction_redacted_body_snippet(body, 4096);

        assert!(redacted.contains("api_key=<redacted>"));
        assert!(redacted.contains("access_token: <redacted>"));
        assert!(redacted.contains("Authorization: Bearer <redacted>"));
        assert!(redacted.contains("sk-live-<redacted>"));
        assert!(!redacted.contains("plain-api-key-secret-12345"));
        assert!(!redacted.contains("plain-access-token-secret"));
        assert!(!redacted.contains("plain-bearer-secret-12345"));
        assert!(!redacted.contains("plain-secret-12345"));
    }

    #[test]
    fn redaction_cli_args_mask_sensitive_flags_and_inline_values() {
        let args = vec![
            OsString::from("--api-key"),
            OsString::from("opaque-api-value"),
            OsString::from("--config=access_token=\"config-token-secret\""),
            OsString::from("--header"),
            OsString::from("Authorization: Bearer cli-bearer-secret-12345"),
            OsString::from("--prompt"),
            OsString::from("Use sk-proj-cli-secret-123456789 today"),
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
        assert!(!redacted.contains("opaque-api-value"));
        assert!(!redacted.contains("config-token-secret"));
        assert!(!redacted.contains("cli-bearer-secret-12345"));
        assert!(!redacted.contains("cli-secret-123456789"));
    }

    #[test]
    fn redaction_env_values_mask_sensitive_keys_and_secret_like_values() {
        assert_eq!(
            redaction_redacted_env_value(
                OsStr::new("ANTHROPIC_AUTH_TOKEN"),
                OsStr::new("opaque-env-value"),
            ),
            REDACTED
        );
        assert_eq!(
            redaction_redacted_env_value(
                OsStr::new("VISIBLE"),
                OsStr::new("Bearer env-bearer-secret-12345"),
            ),
            "Bearer <redacted>"
        );
        assert_eq!(
            redaction_redacted_env_value(OsStr::new("PRODEX_VISIBLE"), OsStr::new("1")),
            "1"
        );
    }
}
