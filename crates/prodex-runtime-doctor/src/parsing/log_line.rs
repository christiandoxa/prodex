use std::borrow::Cow;
#[cfg(feature = "runtime-log-mojo")]
use std::collections::BTreeMap;

#[cfg(feature = "runtime-log-mojo")]
use crate::markers::runtime_doctor_marker_is_known;
#[cfg(feature = "runtime-log-mojo")]
use runtime_proxy_crate::runtime_proxy_redact_log_field_value;
#[cfg(feature = "runtime-log-mojo")]
use runtime_proxy_crate::runtime_proxy_redact_log_text;

#[cfg(feature = "runtime-log-mojo")]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct RuntimeDoctorParsedLogMessage {
    event: Option<String>,
    fields: Vec<(String, String)>,
}

#[cfg(feature = "runtime-log-mojo")]
impl RuntimeDoctorParsedLogMessage {
    fn fields_map(&self) -> BTreeMap<String, String> {
        self.fields
            .iter()
            .map(|(key, value)| {
                (
                    key.clone(),
                    runtime_proxy_redact_log_field_value(key, value),
                )
            })
            .collect()
    }
}

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
        let end = self.line.find("] ")?;
        self.line
            .strip_prefix('[')
            .and_then(|trimmed| trimmed.get(..end.saturating_sub(1)))
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
        Cow::Borrowed(
            self.line
                .split_once("] ")
                .map(|(_, message)| message)
                .unwrap_or(self.line)
                .trim(),
        )
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
        if let Some(event) = runtime_doctor_parse_log_message(&message).event
            && runtime_doctor_marker_is_known(&event)
        {
            return Some(event);
        }
        message
            .split(|character: char| !(character.is_ascii_alphanumeric() || character == '_'))
            .find(|token| !token.is_empty() && runtime_doctor_marker_is_known(token))
            .map(str::to_string)
    }
}

#[cfg(feature = "runtime-log-mojo")]
pub(super) fn runtime_doctor_parse_message_fields(message: &str) -> BTreeMap<String, String> {
    runtime_doctor_parse_log_message(message).fields_map()
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
fn runtime_doctor_parse_log_message(message: &str) -> RuntimeDoctorParsedLogMessage {
    let plan = prodex_mojo_core::rich::runtime_doctor_parse_message_offsets(message)
        .expect("Mojo runtime-doctor message parser returned invalid output");
    RuntimeDoctorParsedLogMessage {
        event: plan
            .event
            .map(|(start, end)| message[start..end].to_string()),
        fields: plan
            .fields
            .into_iter()
            .filter_map(|field| {
                let key = &message[field.key_start..field.key_end];
                let raw_value = &message[field.value_start..field.value_end];
                (!key.is_empty() && !raw_value.is_empty()).then(|| {
                    (
                        key.to_string(),
                        runtime_doctor_parse_log_field_value(raw_value),
                    )
                })
            })
            .collect(),
    }
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
    let mut parts = vec![marker.to_string()];
    for key in [
        "reason",
        "profile",
        "transport",
        "route",
        "websocket_session",
        "previous_response_id",
        "event",
        "via",
    ] {
        if let Some(value) = fields.get(key) {
            parts.push(format!(
                "{key}={}",
                runtime_proxy_redact_log_field_value(key, value)
            ));
        }
    }
    parts.join(" ")
}

#[cfg(feature = "runtime-log-mojo")]
pub(super) fn runtime_doctor_truncate_line(line: &str, limit: usize) -> String {
    let redacted = runtime_proxy_redact_log_text(line);
    let trimmed = redacted.trim();
    let count = trimmed.chars().count();
    if count <= limit {
        return trimmed.to_string();
    }
    trimmed
        .chars()
        .take(limit.saturating_sub(1))
        .collect::<String>()
        + "…"
}

#[cfg(all(test, feature = "runtime-log-mojo"))]
mod expected_message_parser_output {
    use super::*;

    #[test]
    fn message_parser_matches_fixed_cases() {
        let parsed = runtime_doctor_parse_log_message(
            r#"selection_pick profile="alpha beta" note="say \"yes\"" count=42"#,
        );

        assert_eq!(parsed.event.as_deref(), Some("selection_pick"));
        assert_eq!(
            parsed.fields,
            vec![
                ("profile".to_string(), "alpha beta".to_string()),
                ("note".to_string(), "say \"yes\"".to_string()),
                ("count".to_string(), "42".to_string()),
            ]
        );
    }
}
