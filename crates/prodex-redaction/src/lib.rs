use std::ffi::{OsStr, OsString};

const REDACTED: &str = "<redacted>";
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
    prodex_mojo_core::redaction::key_looks_sensitive(name).unwrap_or(true)
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
            *value = redaction_redact_gateway_text(value);
        }
        _ => {}
    }
}

pub fn redaction_redact_json(value: &mut serde_json::Value) {
    redaction_redact_json_value(value);
}

pub fn redaction_redact_secret_like_text(value: &str) -> String {
    prodex_mojo_core::redaction::redact_secret_like_text(value)
        .unwrap_or_else(|_| REDACTED.to_string())
}

fn redaction_redact_gateway_text(value: &str) -> String {
    prodex_mojo_core::redaction::redact_gateway_text(value).unwrap_or_else(|_| REDACTED.to_string())
}

fn redaction_cli_flag_name_looks_sensitive(value: &str) -> bool {
    let trimmed = value.trim_start_matches('-');
    let name = trimmed
        .split_once('=')
        .map(|(name, _)| name)
        .unwrap_or(trimmed);
    redaction_key_looks_sensitive(name)
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
