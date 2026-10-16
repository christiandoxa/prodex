use super::log_format::{local_log_timestamp, render_text_body};
use base64::Engine;
pub(super) use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use prodex_mojo_core::json::LogPayloadJsonPresentation;
use serde_json::Value;
use std::collections::BTreeMap;

const MAX_DECODED_UPSTREAM_PAYLOAD_BYTES: usize = 128 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub(crate) struct UpstreamPayloadEvent {
    pub(crate) timestamp: String,
    pub(crate) request: Option<u64>,
    pub(crate) transport: String,
    pub(crate) route: String,
    pub(crate) profile: String,
    pub(crate) bytes: usize,
    pub(crate) logged_bytes: usize,
    pub(crate) truncated: bool,
    pub(crate) payload: String,
}

pub(crate) fn upstream_payload_event_from_runtime_line(line: &str) -> Option<UpstreamPayloadEvent> {
    if !line.contains("upstream_payload") {
        return None;
    }
    let parsed = parse_runtime_log_line(line)?;
    if parsed.event.as_deref() != Some("upstream_payload") {
        return None;
    }
    let payload_b64 = parsed.fields.get("payload_b64")?;
    let payload_bytes = BASE64_STANDARD.decode(payload_b64).ok()?;
    let bytes = parsed
        .fields
        .get("bytes")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(payload_bytes.len());
    let logged_bytes = parsed
        .fields
        .get("logged_bytes")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(payload_bytes.len());
    let truncated = parsed
        .fields
        .get("truncated")
        .is_some_and(|value| value == "true");
    let payload = readable_upstream_payload(&payload_bytes, &parsed.fields);
    Some(UpstreamPayloadEvent {
        timestamp: local_log_timestamp(&parsed.timestamp),
        request: parsed
            .fields
            .get("request")
            .and_then(|value| value.parse::<u64>().ok()),
        transport: parsed
            .fields
            .get("transport")
            .cloned()
            .unwrap_or_else(|| "-".to_string()),
        route: parsed
            .fields
            .get("route")
            .cloned()
            .unwrap_or_else(|| "-".to_string()),
        profile: parsed
            .fields
            .get("profile")
            .cloned()
            .unwrap_or_else(|| "-".to_string()),
        bytes,
        logged_bytes,
        truncated,
        payload,
    })
}

fn readable_upstream_payload(payload_bytes: &[u8], fields: &BTreeMap<String, String>) -> String {
    let classification = prodex_mojo_core::log::classify_upstream_payload(payload_bytes)
        .expect("Mojo upstream-payload classifier returned invalid output");
    if classification.readable_text {
        return String::from_utf8(payload_bytes.to_vec())
            .expect("Mojo readable-text classification validates UTF-8");
    }
    if matches!(
        classification.binary_kind,
        prodex_mojo_core::log::UpstreamPayloadBinaryKind::Zstd
    ) && let Ok(decoded) =
        runtime_log::decode_zstd_bounded(payload_bytes, MAX_DECODED_UPSTREAM_PAYLOAD_BYTES)
        && prodex_mojo_core::log::classify_upstream_payload(&decoded)
            .expect("Mojo decoded upstream-payload classifier returned invalid output")
            .readable_text
    {
        return String::from_utf8(decoded)
            .expect("Mojo readable decoded-text classification validates UTF-8");
    }

    if let Some(path) = upstream_payload_binary_path(fields) {
        return format!("[binary payload: path={path}]");
    }
    format!("[binary payload: {}]", classification.binary_kind.label())
}

fn upstream_payload_binary_path(fields: &BTreeMap<String, String>) -> Option<&str> {
    [
        "path",
        "file_path",
        "local_path",
        "absolute_path",
        "image_path",
        "attachment_path",
        "uri",
    ]
    .into_iter()
    .find_map(|key| fields.get(key).map(String::as_str))
    .map(|path| path.trim())
    .filter(|path| !path.is_empty())
}

pub(crate) fn render_upstream_payload_lines(payload: &str, width: usize) -> Vec<String> {
    let Ok(value) = serde_json::from_str::<Value>(payload) else {
        return render_text_body(payload, width);
    };
    let tree = crate::json_boundary::app_json_tree(&value)
        .expect("log payload exceeded the JSON ABI boundary");
    let mut serialized = Vec::with_capacity(tree.nodes.len());
    serialize_payload_values(&value, &mut serialized);
    let raw = std::str::from_utf8(&tree.raw).expect("Serde emits UTF-8 JSON");
    let projected = prodex_mojo_core::json::log_payload_projection(&tree.nodes, raw, &serialized)
        .expect("Mojo log payload projection returned invalid output");
    let records: Vec<(u8, String)> = serde_json::from_slice(&projected)
        .expect("Mojo log payload projection returned invalid records");
    let mut lines = Vec::new();
    for (kind, text) in records {
        match kind {
            0 => lines.extend(render_text_body(&text, width)),
            1 => lines.push(text),
            2 => lines.extend(
                render_text_body(&text, width)
                    .into_iter()
                    .map(|line| format!("  {line}")),
            ),
            _ => panic!("Mojo log payload projection returned an invalid line kind"),
        }
    }
    lines
}

// Serde owns JSON parsing and pretty serialization, including JSON in strings.
// Preorder matches the shared borrowed JSON tree; Mojo chooses all display fields.
fn serialize_payload_values(value: &Value, output: &mut Vec<LogPayloadJsonPresentation>) {
    let pretty = serde_json::to_string_pretty(value).expect("in-memory JSON serialization");
    let parsed = value
        .as_str()
        .and_then(|text| serde_json::from_str::<Value>(text).ok())
        .map(|value| serde_json::to_string_pretty(&value).expect("in-memory JSON serialization"));
    output.push(LogPayloadJsonPresentation {
        pretty,
        parsed_string_json: parsed,
    });
    match value {
        Value::Array(items) => {
            for item in items {
                serialize_payload_values(item, output);
            }
        }
        Value::Object(fields) => {
            for item in fields.values() {
                serialize_payload_values(item, output);
            }
        }
        _ => {}
    }
}

pub(super) struct ParsedRuntimeLogLine {
    pub(super) timestamp: String,
    pub(super) event: Option<String>,
    pub(super) fields: BTreeMap<String, String>,
}

pub(super) fn parse_runtime_log_line(line: &str) -> Option<ParsedRuntimeLogLine> {
    if line.trim_start().starts_with('{') {
        let value = serde_json::from_str::<serde_json::Value>(line).ok()?;
        let timestamp = value
            .get("timestamp")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("-")
            .to_string();
        let event = value
            .get("event")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string);
        let fields = value
            .get("fields")
            .and_then(serde_json::Value::as_object)
            .map(|fields| {
                fields
                    .iter()
                    .filter_map(|(key, value)| {
                        runtime_json_log_field_value(value).map(|value| (key.clone(), value))
                    })
                    .collect()
            })
            .unwrap_or_default();
        return Some(ParsedRuntimeLogLine {
            timestamp,
            event,
            fields,
        });
    }

    let rest = line.strip_prefix('[')?;
    let (timestamp, message) = rest.split_once("] ")?;
    let event = runtime_proxy_crate::runtime_proxy_log_event(message).map(str::to_string);
    let fields = runtime_proxy_crate::runtime_proxy_log_fields(message);
    Some(ParsedRuntimeLogLine {
        timestamp: timestamp.to_string(),
        event,
        fields,
    })
}

fn runtime_json_log_field_value(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(value) => Some(value.clone()),
        serde_json::Value::Number(value) => Some(value.to_string()),
        serde_json::Value::Bool(value) => Some(value.to_string()),
        serde_json::Value::Null | serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        BASE64_STANDARD, local_log_timestamp, render_upstream_payload_lines,
        upstream_payload_event_from_runtime_line,
    };
    use base64::Engine;

    #[test]
    fn parses_upstream_payload_events() {
        let payload = br#"{"input":"hello <EMAIL_ADDRESS>"}"#;
        let encoded = BASE64_STANDARD.encode(payload);
        let line = format!(
            "[2026-06-20 12:00:00.000 +07:00] upstream_payload request=9 transport=websocket route=websocket profile=main bytes=35 logged_bytes=35 truncated=false payload_b64={encoded}"
        );

        let event = upstream_payload_event_from_runtime_line(&line).unwrap();
        assert_eq!(
            event.timestamp,
            local_log_timestamp("2026-06-20 12:00:00.000 +07:00")
        );
        assert_eq!(event.request, Some(9));
        assert_eq!(event.transport, "websocket");
        assert_eq!(event.route, "websocket");
        assert_eq!(event.profile, "main");
        assert_eq!(event.payload, "{\"input\":\"hello <EMAIL_ADDRESS>\"}");
        assert!(!event.truncated);
    }

    #[test]
    fn parses_json_format_upstream_payload_events() {
        let payload = br#"{"input":"hello <EMAIL_ADDRESS>"}"#;
        let encoded = BASE64_STANDARD.encode(payload);
        let line = serde_json::json!({
            "timestamp": "2026-06-20 12:00:00.000 +07:00",
            "event": "upstream_payload",
            "fields": {
                "request": "9",
                "transport": "websocket",
                "route": "websocket",
                "profile": "main",
                "bytes": "35",
                "logged_bytes": "35",
                "truncated": "false",
                "payload_b64": encoded,
            }
        })
        .to_string();

        let event = upstream_payload_event_from_runtime_line(&line).unwrap();
        assert_eq!(event.request, Some(9));
        assert_eq!(event.payload, "{\"input\":\"hello <EMAIL_ADDRESS>\"}");
    }

    #[test]
    fn decodes_zstd_upstream_payload_events() {
        let payload = br#"{"model":"gpt-5-codex","input":"hello"}"#;
        let compressed = zstd::stream::encode_all(payload.as_slice(), 0).unwrap();
        let encoded = BASE64_STANDARD.encode(&compressed);
        let line = format!(
            "[2026-06-20 12:00:00.000 +07:00] upstream_payload request=9 transport=http route=responses profile=main bytes={} logged_bytes={} truncated=false payload_b64={encoded}",
            compressed.len(),
            compressed.len()
        );

        let event = upstream_payload_event_from_runtime_line(&line).unwrap();

        assert_eq!(event.payload, String::from_utf8_lossy(payload));
        assert_eq!(
            render_upstream_payload_lines(&event.payload, 80)[0],
            "model=gpt-5-codex"
        );
    }

    #[test]
    fn summarizes_binary_payloads_instead_of_lossy_text() {
        let payload = b"\x89PNG\r\n\x1a\n\0\x01\x02\x03";
        let encoded = BASE64_STANDARD.encode(payload);
        let line = format!(
            "[2026-06-20 12:00:00.000 +07:00] upstream_payload request=9 transport=websocket route=websocket profile=main bytes={} logged_bytes={} truncated=false payload_b64={encoded}",
            payload.len(),
            payload.len()
        );

        let event = upstream_payload_event_from_runtime_line(&line).unwrap();

        assert_eq!(event.payload, "[binary payload: PNG image]");
        assert!(!event.payload.contains('\u{fffd}'));
        assert!(
            !render_upstream_payload_lines(&event.payload, 80)
                .join("\n")
                .contains('\u{fffd}')
        );
    }

    #[test]
    fn summarizes_binary_payloads_with_path_when_available() {
        let payload = b"\xff\xd8\xff\xe0\0\x10JFIF";
        let encoded = BASE64_STANDARD.encode(payload);
        let line = format!(
            "[2026-06-20 12:00:00.000 +07:00] upstream_payload request=9 transport=websocket route=websocket profile=main bytes={} logged_bytes={} truncated=false path=/tmp/screenshot.jpg payload_b64={encoded}",
            payload.len(),
            payload.len()
        );

        let event = upstream_payload_event_from_runtime_line(&line).unwrap();

        assert_eq!(event.payload, "[binary payload: path=/tmp/screenshot.jpg]");
        assert!(!event.payload.contains('\u{fffd}'));
    }

    #[test]
    fn renders_upstream_payload_as_readable_blocks() {
        let payload = serde_json::json!({
            "model": "gpt-5-codex",
            "stream": true,
            "input": [
                {
                    "type": "message",
                    "role": "user",
                    "content": [
                        {
                            "type": "input_text",
                            "text": "Please inspect the latest runtime log and make upstream output readable."
                        }
                    ]
                },
                {
                    "type": "function_call",
                    "name": "exec_command",
                    "arguments": "{\"cmd\":\"pwd\",\"workdir\":\"/repo\"}"
                }
            ],
            "tools": [
                { "type": "function", "name": "exec_command" },
                { "type": "function", "name": "view_image" }
            ],
            "client_metadata": {
                "session_id": "session-1",
                "thread_id": "thread-1",
                "turn_id": "turn-1"
            }
        })
        .to_string();

        let lines = render_upstream_payload_lines(&payload, 88);
        let rendered = lines.join("\n");

        assert!(rendered.contains("model=gpt-5-codex stream=true"));
        assert!(rendered.contains("client_metadata: session_id=session-1 thread_id=thread-1"));
        assert!(rendered.contains("input[0] user:"));
        assert!(rendered.contains("Please inspect the latest runtime log"));
        assert!(rendered.contains("input[1] tool-call:exec_command:"));
        assert!(rendered.contains("\"cmd\": \"pwd\""));
        assert!(rendered.contains("tools: 2 exec_command, view_image"));
    }

    #[test]
    fn pretty_prints_unknown_json_payloads() {
        let lines = render_upstream_payload_lines(r#"{"outer":{"inner":"value"}}"#, 80);
        let rendered = lines.join("\n");

        assert!(rendered.contains("\"outer\": {"));
        assert!(rendered.contains("\"inner\": \"value\""));
    }
}
