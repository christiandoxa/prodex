use anyhow::{Context, Result};
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use super::*;

pub(crate) fn read_runtime_log_tail(path: &Path, max_bytes: usize) -> Result<Vec<u8>> {
    let mut file =
        fs::File::open(path).with_context(|| format!("failed to open {}", path.display()))?;
    let len = file
        .metadata()
        .with_context(|| format!("failed to inspect {}", path.display()))?
        .len();
    let start = len.saturating_sub(max_bytes as u64);
    file.seek(SeekFrom::Start(start))
        .with_context(|| format!("failed to seek {}", path.display()))?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)
        .with_context(|| format!("failed to read {}", path.display()))?;
    if start > 0
        && let Some(position) = buffer.iter().position(|byte| *byte == b'\n')
    {
        buffer.drain(..=position);
    }
    Ok(buffer)
}

pub(crate) fn summarize_runtime_log_tail(tail: &[u8]) -> RuntimeDoctorSummary {
    let text = String::from_utf8_lossy(tail);
    let mut summary = RuntimeDoctorSummary::default();
    for line in text.lines() {
        summary.line_count += 1;
        if let Some(timestamp) = runtime_doctor_line_timestamp(line) {
            if summary.first_timestamp.is_none() {
                summary.first_timestamp = Some(timestamp.clone());
            }
            summary.last_timestamp = Some(timestamp);
        }
        if let Some(marker) = runtime_doctor_marker_name(line) {
            *summary.marker_counts.entry(marker).or_insert(0) += 1;
            summary.last_marker_line = Some(runtime_doctor_truncate_line(line, 160));
            let fields = runtime_doctor_parse_fields(line);
            if matches!(
                marker,
                "chain_retried_owner" | "chain_dead_upstream_confirmed" | "stale_continuation"
            ) {
                summary.latest_chain_event =
                    Some(runtime_doctor_chain_event_summary(marker, &fields));
            }
            if let Some(reason) = fields.get("reason").cloned() {
                match marker {
                    "chain_retried_owner" => {
                        *summary
                            .chain_retried_owner_by_reason
                            .entry(reason)
                            .or_insert(0) += 1;
                    }
                    "chain_dead_upstream_confirmed" => {
                        *summary
                            .chain_dead_upstream_confirmed_by_reason
                            .entry(reason)
                            .or_insert(0) += 1;
                    }
                    "stale_continuation" => {
                        summary.latest_stale_continuation_reason = Some(reason.clone());
                        *summary
                            .stale_continuation_by_reason
                            .entry(reason)
                            .or_insert(0) += 1;
                    }
                    _ => {}
                }
            }
            if marker == "previous_response_not_found" {
                if let Some(route) = fields.get("route").cloned() {
                    *summary
                        .previous_response_not_found_by_route
                        .entry(route)
                        .or_insert(0) += 1;
                }
                if let Some(transport) = fields.get("transport").cloned() {
                    *summary
                        .previous_response_not_found_by_transport
                        .entry(transport)
                        .or_insert(0) += 1;
                }
            }
            for facet in RUNTIME_DOCTOR_FACETS {
                if let Some(value) = fields.get(*facet).cloned() {
                    *summary
                        .facet_counts
                        .entry((*facet).to_string())
                        .or_default()
                        .entry(value)
                        .or_insert(0) += 1;
                }
            }
            if !fields.is_empty() {
                summary.marker_last_fields.insert(marker, fields);
            }
        }
    }
    diagnosis::runtime_doctor_finalize_log_summary(&mut summary);
    summary
}

fn runtime_doctor_line_timestamp(line: &str) -> Option<String> {
    if let Some(value) = runtime_doctor_json_line_value(line) {
        return value
            .get("timestamp")
            .or_else(|| value.get("ts"))
            .and_then(serde_json::Value::as_str)
            .map(ToString::to_string);
    }
    let end = line.find("] ")?;
    line.strip_prefix('[')
        .and_then(|trimmed| trimmed.get(..end.saturating_sub(1)))
        .map(ToString::to_string)
}

fn runtime_doctor_json_line_value(line: &str) -> Option<serde_json::Value> {
    let trimmed = line.trim();
    if !trimmed.starts_with('{') {
        return None;
    }
    serde_json::from_str(trimmed).ok()
}

fn runtime_doctor_line_message<'a>(line: &'a str) -> Cow<'a, str> {
    if let Some(value) = runtime_doctor_json_line_value(line)
        && let Some(message) = value.get("message").and_then(serde_json::Value::as_str)
    {
        return Cow::Owned(message.to_string());
    }
    Cow::Borrowed(
        line.split_once("] ")
            .map(|(_, message)| message)
            .unwrap_or(line)
            .trim(),
    )
}

fn runtime_doctor_parse_fields(line: &str) -> BTreeMap<String, String> {
    if let Some(value) = runtime_doctor_json_line_value(line)
        && let Some(fields) = value.get("fields").and_then(serde_json::Value::as_object)
    {
        let mut parsed = BTreeMap::new();
        for (key, value) in fields {
            let string_value = match value {
                serde_json::Value::String(value) => value.clone(),
                serde_json::Value::Number(value) => value.to_string(),
                serde_json::Value::Bool(value) => value.to_string(),
                _ => continue,
            };
            parsed.insert(key.clone(), string_value);
        }
        if !parsed.is_empty() {
            return parsed;
        }
    }

    let message = runtime_doctor_line_message(line);
    let mut fields = BTreeMap::new();
    for token in message.split_whitespace() {
        let Some((key, value)) = token.split_once('=') else {
            continue;
        };
        if key.is_empty() || value.is_empty() {
            continue;
        }
        fields.insert(key.to_string(), value.trim_matches('"').to_string());
    }
    fields
}

fn runtime_doctor_marker_name(line: &str) -> Option<&'static str> {
    let message = runtime_doctor_line_message(line);
    for token in message.split_whitespace() {
        if token.contains('=') {
            continue;
        }
        if let Some(marker) = RUNTIME_DOCTOR_MARKERS
            .iter()
            .copied()
            .find(|marker| *marker == token)
        {
            return Some(marker);
        }
    }
    RUNTIME_DOCTOR_MARKERS
        .iter()
        .copied()
        .find(|marker| message.contains(marker))
}

fn runtime_doctor_chain_event_summary(marker: &str, fields: &BTreeMap<String, String>) -> String {
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

fn runtime_doctor_truncate_line(line: &str, limit: usize) -> String {
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
