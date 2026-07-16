use std::ffi::{OsStr, OsString};

const REDACTED: &str = "<redacted>";
const REDACTION_FAILED_GATEWAY_BODY: &[u8] = b"{}";
const AUTHORIZATION_SCHEMES: &[&str] = &["Bearer", "Basic", "Token"];
const API_KEY_PREFIXES: &[&str] = &[
    "sk-proj-", "sk-ant-", "sk-live-", "sk_test_", "sk_live_", "sk-", "sk_",
];

pub fn redaction_text_snippet(text: &str, max_chars: usize) -> String {
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

pub fn redaction_redacted_body_snippet(body: &[u8], max_chars: usize) -> String {
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

pub fn redaction_redact_gateway_body(body: &[u8]) -> Option<Vec<u8>> {
    match serde_json::from_slice::<serde_json::Value>(body) {
        Ok(mut value) => {
            let changed = redaction_redact_gateway_json_value(&mut value);
            changed.then(|| redaction_gateway_json_bytes(serde_json::to_vec(&value)))
        }
        Err(_) => {
            let text = String::from_utf8_lossy(body);
            let redacted = redaction_redact_gateway_text(&text);
            (redacted.as_bytes() != body).then(|| redacted.into_bytes())
        }
    }
}

pub fn redaction_redacted_headers_debug(headers: &[(String, String)]) -> String {
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

pub fn redaction_redacted_cli_args(args: &[OsString]) -> Vec<String> {
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

pub fn redaction_redacted_env_value(key: &OsStr, value: &OsStr) -> String {
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

pub fn redaction_display_os(value: &OsStr) -> String {
    redaction_display_text(&value.to_string_lossy())
}

pub fn redaction_key_looks_sensitive(name: &str) -> bool {
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
    ) || normalized == "proxyauthorization"
        || normalized.ends_with("token")
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

pub fn redaction_redact_json(value: &mut serde_json::Value) {
    redaction_redact_json_value(value);
}

fn redaction_redact_sensitive_header_value(name: &str, value: &str) -> String {
    if name.eq_ignore_ascii_case("authorization")
        || name.eq_ignore_ascii_case("proxy-authorization")
    {
        let trimmed = value.trim_start();
        for scheme in AUTHORIZATION_SCHEMES {
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

pub fn redaction_redact_secret_like_text(value: &str) -> String {
    let value = redaction_redact_sensitive_key_value_text(value);
    let value = redaction_redact_authorization_like_values(&value);
    redaction_redact_prefixed_api_key_tokens(&value)
}

fn redaction_gateway_json_bytes(result: serde_json::Result<Vec<u8>>) -> Vec<u8> {
    result.unwrap_or_else(|_| REDACTION_FAILED_GATEWAY_BODY.to_vec())
}

fn redaction_redact_gateway_json_value(value: &mut serde_json::Value) -> bool {
    match value {
        serde_json::Value::Object(map) => {
            let mut changed = false;
            for (key, value) in map.iter_mut() {
                if redaction_key_looks_sensitive(key) {
                    if value != &serde_json::Value::String(REDACTED.to_string()) {
                        *value = serde_json::Value::String(REDACTED.to_string());
                        changed = true;
                    }
                } else {
                    changed |= redaction_redact_gateway_json_value(value);
                }
            }
            changed
        }
        serde_json::Value::Array(values) => {
            let mut changed = false;
            for value in values {
                changed |= redaction_redact_gateway_json_value(value);
            }
            changed
        }
        serde_json::Value::String(value) => {
            let redacted = redaction_redact_gateway_text(value);
            if redacted != *value {
                *value = redacted;
                true
            } else {
                false
            }
        }
        _ => false,
    }
}

fn redaction_redact_gateway_text(value: &str) -> String {
    let value = redaction_redact_secret_like_text(value);
    let value = redaction_redact_email_tokens(&value);
    redaction_redact_long_digit_tokens(&value)
}

fn redaction_redact_email_tokens(value: &str) -> String {
    redaction_redact_matching_tokens(value, redaction_token_byte, |token| {
        let Some((local, domain)) = token.split_once('@') else {
            return false;
        };
        !local.is_empty() && domain.contains('.') && domain.split('.').all(|part| !part.is_empty())
    })
}

fn redaction_redact_long_digit_tokens(value: &str) -> String {
    redaction_redact_matching_tokens(value, redaction_digit_group_byte, |token| {
        let digit_count = token.bytes().filter(u8::is_ascii_digit).count();
        (13..=19).contains(&digit_count)
    })
}

fn redaction_redact_matching_tokens<F, G>(value: &str, token_byte: F, should_redact: G) -> String
where
    F: Fn(u8) -> bool,
    G: Fn(&str) -> bool,
{
    let mut redacted = String::with_capacity(value.len());
    let bytes = value.as_bytes();
    let mut index = 0usize;
    while index < bytes.len() {
        if token_byte(bytes[index]) {
            let start = index;
            while index < bytes.len() && token_byte(bytes[index]) {
                index += 1;
            }
            let token = &value[start..index];
            if should_redact(token) {
                redacted.push_str(REDACTED);
            } else {
                redacted.push_str(token);
            }
            continue;
        }
        let Some(ch) = value[index..].chars().next() else {
            break;
        };
        redacted.push(ch);
        index += ch.len_utf8();
    }
    redacted
}

fn redaction_token_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'%' | b'+' | b'-' | b'@')
}

fn redaction_digit_group_byte(byte: u8) -> bool {
    byte.is_ascii_digit() || matches!(byte, b' ' | b'-')
}

fn redaction_redact_sensitive_key_value_text(value: &str) -> String {
    let mut redacted = String::with_capacity(value.len());
    let bytes = value.as_bytes();
    let mut index = 0usize;

    while index < bytes.len() {
        if let Some((key, after_key)) = redaction_parse_potential_field_name(value, index) {
            let separator = redaction_skip_ascii_whitespace(bytes, after_key);
            if separator < bytes.len() && matches!(bytes[separator], b':' | b'=') {
                if redaction_key_looks_sensitive(key) {
                    let value_start = redaction_skip_ascii_whitespace(bytes, separator + 1);
                    let (value_end, replacement) =
                        redaction_redacted_field_value(value, value_start);
                    redacted.push_str(&value[index..value_start]);
                    redacted.push_str(&replacement);
                    index = value_end;
                    continue;
                }
                redacted.push_str(&value[index..after_key]);
                index = after_key;
                continue;
            }
            if !matches!(bytes[index], b'"' | b'\'') {
                redacted.push_str(&value[index..after_key]);
                index = after_key;
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

    for scheme in AUTHORIZATION_SCHEMES {
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
        && !matches!(
            bytes[cursor],
            b'"' | b'\'' | b',' | b'&' | b';' | b'}' | b']' | b')'
        )
    {
        cursor += 1;
    }
    (cursor, REDACTED.to_string())
}

fn redaction_redact_authorization_like_values(value: &str) -> String {
    let mut redacted = String::with_capacity(value.len());
    let bytes = value.as_bytes();
    let mut index = 0usize;

    while index < bytes.len() {
        for scheme in AUTHORIZATION_SCHEMES {
            if redaction_starts_with_ignore_ascii_case(bytes, index, scheme.as_bytes())
                && redaction_ascii_boundary_before(bytes, index)
                && redaction_ascii_boundary_after(bytes, index + scheme.len())
            {
                let whitespace = redaction_skip_ascii_whitespace(bytes, index + scheme.len());
                if whitespace > index + scheme.len() {
                    let token_end = redaction_secret_token_end(bytes, whitespace);
                    if token_end > whitespace {
                        redacted.push_str(&value[index..whitespace]);
                        redacted.push_str(REDACTED);
                        index = token_end;
                        break;
                    }
                }
            }
        }

        if index >= bytes.len() {
            break;
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
#[path = "../tests/src/lib.rs"]
mod tests;
