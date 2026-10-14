use std::borrow::Cow;
#[cfg(feature = "runtime-log-mojo")]
use std::collections::BTreeMap;

#[cfg(feature = "runtime-log-mojo")]
use crate::markers::runtime_doctor_marker_is_known;
#[cfg(feature = "runtime-log-mojo")]
use runtime_proxy_crate::runtime_proxy_redact_log_field_value;
#[cfg(feature = "runtime-log-mojo")]
use runtime_proxy_crate::runtime_proxy_redact_log_text;

pub(crate) struct RuntimeDoctorParsedLogLine<'a> {
    line: &'a str,
    json: Option<serde_json::Value>,
}

impl<'a> RuntimeDoctorParsedLogLine<'a> {
    pub(crate) fn new(line: &'a str) -> Self {
        let trimmed = line.trim();
        Self {
            line,
            json: if trimmed.starts_with('{') {
                serde_json::from_str(trimmed).ok()
            } else {
                None
            },
        }
    }

    pub(crate) fn json(&self) -> Option<&serde_json::Value> {
        self.json.as_ref()
    }

    #[cfg(feature = "runtime-log-mojo")]
    pub(crate) fn timestamp(&self) -> Option<String> {
        if let Some(value) = self.json() {
            return value
                .get("timestamp")
                .or_else(|| value.get("ts"))
                .and_then(serde_json::Value::as_str)
                .map(runtime_proxy_redact_log_text);
        }
        prodex_mojo_core::rich::parse_plain_log_envelope(self.line)
            .expect("compiled Mojo runtime-doctor log envelope failed")
            .timestamp
            .map(runtime_proxy_redact_log_text)
    }

    pub(crate) fn message(&self) -> Cow<'_, str> {
        if let Some(message) = self
            .json()
            .and_then(|value| value.get("message"))
            .and_then(serde_json::Value::as_str)
        {
            return Cow::Borrowed(message);
        }
        let envelope = prodex_mojo_core::rich::parse_plain_log_envelope(self.line)
            .expect("compiled Mojo runtime-doctor log envelope failed");
        Cow::Borrowed(envelope.message.trim())
    }

    #[cfg(feature = "runtime-log-mojo")]
    pub(crate) fn fields(&self) -> BTreeMap<String, String> {
        let mut fields = runtime_doctor_parse_message_fields(&self.message());
        if let Some(json_fields) = self
            .json()
            .and_then(|value| value.get("fields"))
            .and_then(serde_json::Value::as_object)
        {
            fields.extend(runtime_doctor_json_fields_map(json_fields));
        }
        fields
    }

    #[cfg(feature = "runtime-log-mojo")]
    pub(crate) fn marker_name(&self) -> Option<String> {
        if let Some(event) = self
            .json()
            .and_then(|value| value.get("event"))
            .and_then(serde_json::Value::as_str)
            && runtime_doctor_marker_is_known(event)
        {
            return Some(event.to_string());
        }

        let message = self.message();
        prodex_mojo_core::rich::runtime_doctor_parse_message_offsets(&message)
            .expect("Mojo runtime-doctor message parser returned invalid output")
            .marker
            .map(|(start, end)| message[start..end].to_string())
    }
}

#[cfg(feature = "runtime-log-mojo")]
pub(super) fn runtime_doctor_parse_message_fields(message: &str) -> BTreeMap<String, String> {
    let plan = prodex_mojo_core::rich::runtime_doctor_parse_message_offsets(message)
        .expect("Mojo runtime-doctor message parser returned invalid output");
    plan.fields
        .into_iter()
        .filter_map(|field| {
            let key = &message[field.key_start..field.key_end];
            let raw_value = &message[field.value_start..field.value_end];
            (!key.is_empty() && !raw_value.is_empty()).then(|| {
                (
                    key.to_string(),
                    runtime_proxy_redact_log_field_value(
                        key,
                        &runtime_doctor_parse_log_field_value(raw_value),
                    ),
                )
            })
        })
        .collect()
}

#[cfg(feature = "runtime-log-mojo")]
fn runtime_doctor_json_fields_map(
    json_fields: &serde_json::Map<String, serde_json::Value>,
) -> BTreeMap<String, String> {
    json_fields
        .iter()
        .filter_map(|(key, value)| {
            let value = match value {
                serde_json::Value::String(value) => value.clone(),
                serde_json::Value::Number(value) => value.to_string(),
                serde_json::Value::Bool(value) => value.to_string(),
                _ => return None,
            };
            Some((
                key.clone(),
                runtime_proxy_redact_log_field_value(key, &value),
            ))
        })
        .collect()
}

#[cfg(feature = "runtime-log-mojo")]
fn runtime_doctor_parse_log_field_value(raw_value: &str) -> String {
    if raw_value.starts_with('"') {
        serde_json::from_str::<String>(raw_value)
            .unwrap_or_else(|_| raw_value.trim_matches('"').to_string())
    } else {
        raw_value.trim_matches('"').to_string()
    }
}

#[cfg(feature = "runtime-log-mojo")]
pub(super) fn runtime_doctor_chain_event_summary(
    marker: &str,
    fields: &BTreeMap<String, String>,
) -> String {
    let keys = [
        "reason",
        "profile",
        "transport",
        "route",
        "websocket_session",
        "previous_response_id",
        "event",
        "via",
    ];
    let redacted = keys.map(|key| {
        fields
            .get(key)
            .map(|value| runtime_proxy_redact_log_field_value(key, value))
    });
    prodex_mojo_core::rich::runtime_doctor_render(
        prodex_mojo_core::rich::RuntimeDoctorRenderInput {
            operation: prodex_mojo_core::rich::RUNTIME_DOCTOR_RENDER_CHAIN_EVENT_SUMMARY,
            detail: 0,
            values: &[
                Some(marker),
                redacted[0].as_deref(),
                redacted[1].as_deref(),
                redacted[2].as_deref(),
                redacted[3].as_deref(),
                redacted[4].as_deref(),
                redacted[5].as_deref(),
                redacted[6].as_deref(),
                redacted[7].as_deref(),
            ],
        },
    )
    .expect("Mojo runtime-doctor chain-event renderer returned invalid output")
}

#[cfg(feature = "runtime-log-mojo")]
pub(super) fn runtime_doctor_truncate_line(line: &str) -> String {
    let redacted = runtime_proxy_redact_log_text(line);
    let trimmed = redacted.trim();
    prodex_mojo_core::rich::runtime_doctor_render(
        prodex_mojo_core::rich::RuntimeDoctorRenderInput {
            operation: prodex_mojo_core::rich::RUNTIME_DOCTOR_RENDER_LAST_MARKER_LINE_TRUNCATION,
            detail: 0,
            values: &[Some(trimmed)],
        },
    )
    .expect("Mojo runtime-doctor last-marker renderer returned invalid output")
}

#[cfg(all(test, feature = "runtime-log-mojo"))]
mod expected_message_parser_output {
    use super::*;

    #[test]
    fn message_parser_matches_fixed_cases() {
        let fields = runtime_doctor_parse_message_fields(
            r#"selection_pick profile="alpha beta" note="say \"yes\"" count=42"#,
        );

        assert_eq!(
            fields,
            BTreeMap::from([
                ("count".to_string(), "42".to_string()),
                ("note".to_string(), "say \"yes\"".to_string()),
                ("profile".to_string(), "alpha beta".to_string()),
            ])
        );
    }
}

#[cfg(all(test, feature = "runtime-log-mojo"))]
mod expected_last_marker_line_output {
    use super::runtime_doctor_truncate_line;

    #[test]
    fn last_marker_line_trims_before_mojo_unicode_truncation() {
        let line = format!("  {}  ", "🧭".repeat(161));

        assert_eq!(
            runtime_doctor_truncate_line(&line),
            format!("{}…", "🧭".repeat(159))
        );
    }
}

#[cfg(all(test, feature = "runtime-log-mojo"))]
mod plain_envelope_tests {
    use super::RuntimeDoctorParsedLogLine;

    #[test]
    fn compiled_mojo_envelope_owns_plain_text_while_json_keeps_precedence() {
        let text =
            RuntimeDoctorParsedLogLine::new("[2026-10-14 10:00:00Z]  selection_pick profile=🧭 ");
        assert_eq!(text.timestamp().as_deref(), Some("2026-10-14 10:00:00Z"));
        assert_eq!(text.message(), "selection_pick profile=🧭");

        let raw = RuntimeDoctorParsedLogLine::new("note]  first] second ");
        assert_eq!(raw.timestamp(), None);
        assert_eq!(raw.message(), "first] second");

        let unclosed = RuntimeDoctorParsedLogLine::new(" [missing");
        assert_eq!(unclosed.timestamp(), None);
        assert_eq!(unclosed.message(), "[missing");

        let json = RuntimeDoctorParsedLogLine::new(
            r#"{"timestamp":"2026-10-14T10:00Z","message":"JSON ] payload"}"#,
        );
        assert_eq!(json.timestamp().as_deref(), Some("2026-10-14T10:00Z"));
        assert_eq!(json.message(), "JSON ] payload");
    }
}
