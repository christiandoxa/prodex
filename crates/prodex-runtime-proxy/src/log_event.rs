use std::borrow::Cow;
use std::collections::BTreeMap;

use redaction::{redaction_key_looks_sensitive, redaction_redact_secret_like_text};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeProxyLogField<'a> {
    key: Cow<'a, str>,
    value: Cow<'a, str>,
}

impl<'a> RuntimeProxyLogField<'a> {
    pub fn key(&self) -> &str {
        &self.key
    }

    pub fn value(&self) -> &str {
        &self.value
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeProxyLogEvent<'a> {
    event: Cow<'a, str>,
    fields: Vec<RuntimeProxyLogField<'a>>,
}

impl<'a> RuntimeProxyLogEvent<'a> {
    pub fn new(
        event: impl Into<Cow<'a, str>>,
        fields: impl IntoIterator<Item = RuntimeProxyLogField<'a>>,
    ) -> Self {
        Self {
            event: event.into(),
            fields: fields.into_iter().collect(),
        }
    }

    pub fn event(&self) -> &str {
        &self.event
    }

    pub fn fields(&self) -> &[RuntimeProxyLogField<'a>] {
        &self.fields
    }

    pub fn fields_map(&self) -> BTreeMap<String, String> {
        runtime_proxy_log_fields_to_map(&self.fields)
    }

    pub fn render_message(&self) -> String {
        let capacity = self.event.len()
            + self
                .fields
                .iter()
                .map(|field| field.key().len() + field.value().len() + 2)
                .sum::<usize>();
        let mut message = String::with_capacity(capacity);
        message.push_str(&runtime_proxy_redact_log_text(&self.event));
        for field in &self.fields {
            let key = runtime_proxy_sanitize_log_fragment(field.key());
            if key.is_empty() || runtime_proxy_log_key_needs_skip(&key) {
                continue;
            }
            if !message.is_empty() {
                message.push(' ');
            }
            message.push_str(&key);
            message.push('=');
            message.push_str(&runtime_proxy_format_log_field_value(
                field.key(),
                field.value(),
            ));
        }
        message
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeProxyParsedLogMessage {
    event: Option<String>,
    fields: Vec<RuntimeProxyLogField<'static>>,
}

impl RuntimeProxyParsedLogMessage {
    pub fn event(&self) -> Option<&str> {
        self.event.as_deref()
    }

    pub fn fields(&self) -> &[RuntimeProxyLogField<'static>] {
        &self.fields
    }

    pub fn fields_map(&self) -> BTreeMap<String, String> {
        runtime_proxy_log_fields_to_map(&self.fields)
    }

    pub fn into_event(self) -> Option<RuntimeProxyLogEvent<'static>> {
        self.event.map(|event| RuntimeProxyLogEvent {
            event: Cow::Owned(event),
            fields: self.fields,
        })
    }
}

pub fn runtime_proxy_log_field<'a>(
    key: &'a str,
    value: impl Into<Cow<'a, str>>,
) -> RuntimeProxyLogField<'a> {
    RuntimeProxyLogField {
        key: Cow::Borrowed(key),
        value: value.into(),
    }
}

pub fn runtime_proxy_identifier_hash(value: Option<&str>) -> String {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(crate::smart_context_hash_text)
        .unwrap_or_else(|| "none".to_string())
}

pub fn runtime_proxy_structured_log_message<'a>(
    event: &str,
    fields: impl IntoIterator<Item = RuntimeProxyLogField<'a>>,
) -> String {
    RuntimeProxyLogEvent::new(Cow::Owned(event.to_string()), fields).render_message()
}

pub fn runtime_proxy_parse_log_message(message: &str) -> RuntimeProxyParsedLogMessage {
    let plan = prodex_mojo_core::rich::parse_log_message(message)
        .expect("Mojo runtime log parser returned invalid spans");
    let event = plan
        .event
        .map(|(start, end)| message[start..end].to_string());
    let fields = plan
        .fields
        .into_iter()
        .map(|span| RuntimeProxyLogField {
            key: Cow::Owned(message[span.key_start..span.key_end].to_string()),
            value: Cow::Owned(runtime_proxy_parse_log_field_value(
                &message[span.value_start..span.value_end],
            )),
        })
        .collect();
    RuntimeProxyParsedLogMessage { event, fields }
}

pub fn runtime_proxy_parse_log_event(message: &str) -> Option<RuntimeProxyLogEvent<'static>> {
    runtime_proxy_parse_log_message(message).into_event()
}

pub fn runtime_proxy_log_fields(message: &str) -> BTreeMap<String, String> {
    runtime_proxy_parse_log_message(message).fields_map()
}

/// Replaces terminal control characters and redacts secret-like text in a log fragment.
pub fn runtime_proxy_redact_log_text(value: &str) -> String {
    let sanitized = runtime_proxy_sanitize_log_fragment(value);
    redaction_redact_secret_like_text(sanitized.as_ref())
}

/// Applies the structured-log redaction policy without adding field-value quotes.
pub fn runtime_proxy_redact_log_field_value(key: &str, value: &str) -> String {
    let value = if !runtime_proxy_log_key_is_known_safe(key) && redaction_key_looks_sensitive(key) {
        "<redacted>".to_string()
    } else if runtime_proxy_log_key_is_location(key) {
        runtime_proxy_strip_log_location_secrets(value)
    } else {
        value.to_string()
    };
    runtime_proxy_redact_log_text(&value)
}

pub fn runtime_proxy_log_event(message: &str) -> Option<&str> {
    runtime_proxy_log_event_span(message).map(|(start, end)| &message[start..end])
}

fn runtime_proxy_log_event_span(message: &str) -> Option<(usize, usize)> {
    prodex_mojo_core::rich::parse_log_message(message)
        .expect("Mojo runtime log parser returned invalid event span")
        .event
}

fn runtime_proxy_log_fields_to_map(
    fields: &[RuntimeProxyLogField<'_>],
) -> BTreeMap<String, String> {
    fields
        .iter()
        .map(|field| (field.key().to_string(), field.value().to_string()))
        .collect()
}

fn runtime_proxy_log_key_needs_skip(key: &str) -> bool {
    key.bytes()
        .any(|byte| byte == b'=' || byte.is_ascii_whitespace())
}

fn runtime_proxy_format_log_field_value(key: &str, value: &str) -> String {
    let value = if runtime_proxy_log_key_is_free_form(key)
        && !runtime_proxy_log_value_is_stable_code(value)
    {
        "<redacted>".to_string()
    } else {
        runtime_proxy_redact_log_field_value(key, value)
    };
    runtime_proxy_quote_log_field_value(&value)
}

fn runtime_proxy_quote_log_field_value(value: &str) -> String {
    let sanitized = runtime_proxy_sanitize_log_fragment(value);
    if runtime_proxy_log_field_value_needs_quotes(&sanitized) {
        serde_json::to_string(sanitized.as_ref()).unwrap_or_else(|_| "\"\"".to_string())
    } else {
        sanitized.into_owned()
    }
}

fn runtime_proxy_log_key_is_known_safe(key: &str) -> bool {
    matches!(
        key,
        "affinity"
            | "acceptance_state"
            | "balance"
            | "cold_start_jobs"
            | "excluded_count"
            | "effective_model"
            | "eligible_profiles_remaining"
            | "fallback"
            | "failure_class"
            | "health"
            | "inflight"
            | "mode"
            | "order"
            | "outcome"
            | "performance"
            | "pressure_mode"
            | "profile"
            | "profile_hash"
            | "prompt_cache_bound"
            | "ready"
            | "reason"
            | "reports"
            | "recovery_generation"
            | "recovery_outcome"
            | "requeue_reason"
            | "request"
            | "response_id"
            | "route"
            | "retry_layer"
            | "schema_version"
            | "side_effect_state"
            | "signaled"
            | "soft_limit"
            | "sync_probe_jobs"
            | "payload_b64"
            | "stream"
            | "stream_committed"
            | "trace"
            | "transport"
            | "useful"
            | "wait_ms"
            | "waited_ms"
            | "last_prompt_requeued"
            | "requested_model"
    )
}

fn runtime_proxy_log_key_is_free_form(key: &str) -> bool {
    [
        "error", "message", "detail", "body", "response", "stderr", "panic",
    ]
    .iter()
    .any(|candidate| key.eq_ignore_ascii_case(candidate))
}

fn runtime_proxy_log_value_is_stable_code(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && !value.contains("sk-")
        && !value.contains("sk_")
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || matches!(byte, b'_' | b'-' | b'.' | b':')
        })
}

fn runtime_proxy_log_key_is_location(key: &str) -> bool {
    key.eq_ignore_ascii_case("path")
        || key.ends_with("_path")
        || key.eq_ignore_ascii_case("url")
        || key.ends_with("_url")
        || key.eq_ignore_ascii_case("endpoint")
        || key.ends_with("_endpoint")
}

fn runtime_proxy_strip_log_location_secrets(value: &str) -> String {
    let value = value.split(['?', '#']).next().unwrap_or(value);
    let Some((scheme, remainder)) = value.split_once("://") else {
        return value.to_string();
    };
    let Some((_, location)) = remainder.rsplit_once('@') else {
        return value.to_string();
    };
    format!("{scheme}://<redacted>@{location}")
}

fn runtime_proxy_sanitize_log_fragment(value: &str) -> Cow<'_, str> {
    if value
        .chars()
        .any(|character| character.is_control() || character == '\u{7f}')
    {
        Cow::Owned(
            value
                .chars()
                .map(|character| {
                    if character.is_control() || character == '\u{7f}' {
                        ' '
                    } else {
                        character
                    }
                })
                .collect(),
        )
    } else {
        Cow::Borrowed(value)
    }
}

fn runtime_proxy_log_field_value_needs_quotes(value: &str) -> bool {
    value.is_empty()
        || value.bytes().any(|byte| byte.is_ascii_whitespace())
        || value.contains('"')
        || value.contains('\\')
}

fn runtime_proxy_parse_log_field_value(raw_value: &str) -> String {
    if raw_value.starts_with('"') {
        serde_json::from_str::<String>(raw_value)
            .unwrap_or_else(|_| raw_value.trim_matches('"').to_string())
    } else {
        raw_value.trim_matches('"').to_string()
    }
}

#[cfg(test)]
mod log_parser_tests {
    use super::*;

    #[test]
    fn preserves_event_and_quoted_fields() {
        let parsed = runtime_proxy_parse_log_message(
            "  stream_read_error request=7 profile=\"alpha beta\" empty=\"\" note=\"bad \\\"quote\\\" \\\\ slash\" ",
        );
        assert_eq!(parsed.event(), Some("stream_read_error"));
        assert_eq!(
            parsed
                .fields()
                .iter()
                .map(|field| (field.key(), field.value()))
                .collect::<Vec<_>>(),
            vec![
                ("request", "7"),
                ("profile", "alpha beta"),
                ("empty", ""),
                ("note", "bad \"quote\" \\ slash"),
            ]
        );
    }

    #[test]
    fn handles_unicode_and_malformed_values() {
        let parsed =
            runtime_proxy_parse_log_message("évent clé=valeur another=\"é space\" key= tail");
        assert_eq!(parsed.event(), Some("évent"));
        assert_eq!(
            parsed
                .fields()
                .iter()
                .map(|field| (field.key(), field.value()))
                .collect::<Vec<_>>(),
            vec![("clé", "valeur"), ("another", "é space")]
        );
        assert_eq!(runtime_proxy_log_event("  event key=1"), Some("event"));
        assert_eq!(runtime_proxy_log_event("key=value"), None);
    }

    #[test]
    fn parses_form_feed_log_separators_without_changing_quoted_values() {
        let message = "\u{c}event\u{c}first=one\u{c}second=\"two\u{c}part\"";
        let parsed = runtime_proxy_parse_log_message(message);

        assert_eq!(parsed.event(), Some("event"));
        assert_eq!(
            parsed
                .fields()
                .iter()
                .map(|field| (field.key(), field.value()))
                .collect::<Vec<_>>(),
            vec![("first", "one"), ("second", "two\u{c}part")]
        );
        assert_eq!(runtime_proxy_log_event(message), Some("event"));
    }
}
