use std::borrow::Cow;
use std::collections::BTreeMap;

use crate::{RUNTIME_DOCTOR_MARKERS, RuntimeDoctorMarker};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct RuntimeDoctorParsedLogMessage {
    event: Option<String>,
    fields: Vec<(String, String)>,
}

impl RuntimeDoctorParsedLogMessage {
    fn fields_map(&self) -> BTreeMap<String, String> {
        self.fields.iter().cloned().collect()
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

    pub(crate) fn timestamp(&self) -> Option<String> {
        if let Some(value) = self.json() {
            return value
                .get("timestamp")
                .or_else(|| value.get("ts"))
                .and_then(serde_json::Value::as_str)
                .map(ToString::to_string);
        }
        let end = self.line.find("] ")?;
        self.line
            .strip_prefix('[')
            .and_then(|trimmed| trimmed.get(..end.saturating_sub(1)))
            .map(ToString::to_string)
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

    pub(crate) fn marker_name(&self) -> Option<&'static str> {
        if let Some(event) = self
            .json()
            .and_then(|value| value.get("event"))
            .and_then(serde_json::Value::as_str)
            && let Some(marker) = runtime_doctor_known_marker(event)
        {
            return Some(marker);
        }

        let message = self.message();
        if let Some(event) = runtime_doctor_parse_log_message(&message).event
            && let Some(marker) = runtime_doctor_known_marker(&event)
        {
            return Some(marker);
        }
        RUNTIME_DOCTOR_MARKERS
            .iter()
            .copied()
            .find(|marker| message.contains(marker))
    }
}

pub(super) fn runtime_doctor_parse_message_fields(message: &str) -> BTreeMap<String, String> {
    runtime_doctor_parse_log_message(message).fields_map()
}

fn runtime_doctor_known_marker(event: &str) -> Option<&'static str> {
    RuntimeDoctorMarker::from_name(event).map(RuntimeDoctorMarker::as_str)
}

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
            Some((key.clone(), value))
        })
        .collect()
}

fn runtime_doctor_parse_log_message(message: &str) -> RuntimeDoctorParsedLogMessage {
    let mut parsed = RuntimeDoctorParsedLogMessage::default();
    let bytes = message.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        index = runtime_doctor_skip_log_whitespace(message, index);
        if index >= bytes.len() {
            break;
        }

        let token_start = index;
        while index < bytes.len() && !bytes[index].is_ascii_whitespace() && bytes[index] != b'=' {
            index += 1;
        }
        if index < bytes.len() && bytes[index] == b'=' {
            let key = &message[token_start..index];
            index += 1;
            let value_start = index;
            index = runtime_doctor_skip_log_field_value(message, index);
            let raw_value = &message[value_start..index];
            if !key.is_empty() && !raw_value.is_empty() {
                parsed.fields.push((
                    key.to_string(),
                    runtime_doctor_parse_log_field_value(raw_value),
                ));
            }
            continue;
        }

        if token_start < index && parsed.event.is_none() {
            parsed.event = Some(message[token_start..index].to_string());
        }
        while index < bytes.len() && !bytes[index].is_ascii_whitespace() {
            index += 1;
        }
    }

    parsed
}

fn runtime_doctor_skip_log_whitespace(message: &str, mut index: usize) -> usize {
    let bytes = message.as_bytes();
    while index < bytes.len() && bytes[index].is_ascii_whitespace() {
        index += 1;
    }
    index
}

fn runtime_doctor_skip_log_field_value(message: &str, mut index: usize) -> usize {
    let bytes = message.as_bytes();
    if index >= bytes.len() {
        return index;
    }
    if bytes[index] == b'"' {
        index += 1;
        let mut escaped = false;
        while index < bytes.len() {
            let byte = bytes[index];
            if escaped {
                escaped = false;
                index += 1;
                continue;
            }
            match byte {
                b'\\' => {
                    escaped = true;
                    index += 1;
                }
                b'"' => {
                    index += 1;
                    break;
                }
                _ => index += 1,
            }
        }
        return index;
    }
    while index < bytes.len() && !bytes[index].is_ascii_whitespace() {
        index += 1;
    }
    index
}

fn runtime_doctor_parse_log_field_value(raw_value: &str) -> String {
    if raw_value.starts_with('"') {
        serde_json::from_str::<String>(raw_value)
            .unwrap_or_else(|_| raw_value.trim_matches('"').to_string())
    } else {
        raw_value.trim_matches('"').to_string()
    }
}

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
            parts.push(format!("{key}={value}"));
        }
    }
    parts.join(" ")
}

pub(super) fn runtime_doctor_truncate_line(line: &str, limit: usize) -> String {
    let trimmed = line.trim();
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
