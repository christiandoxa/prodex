use super::log_format::{local_log_timestamp, render_text_body};
use base64::Engine;
pub(super) use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub(super) struct UpstreamPayloadEvent {
    pub(super) timestamp: String,
    pub(super) request: Option<u64>,
    pub(super) transport: String,
    pub(super) route: String,
    pub(super) profile: String,
    pub(super) bytes: usize,
    pub(super) logged_bytes: usize,
    pub(super) truncated: bool,
    pub(super) payload: String,
}

pub(super) fn upstream_payload_event_from_runtime_line(line: &str) -> Option<UpstreamPayloadEvent> {
    let parsed = parse_runtime_log_line(line)?;
    if parsed.event.as_deref() != Some("upstream_payload") {
        return None;
    }
    let payload_b64 = parsed.fields.get("payload_b64")?;
    let payload_bytes = BASE64_STANDARD.decode(payload_b64).ok()?;
    let payload = String::from_utf8_lossy(&payload_bytes).into_owned();
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
        bytes: parsed
            .fields
            .get("bytes")
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(payload_bytes.len()),
        logged_bytes: parsed
            .fields
            .get("logged_bytes")
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(payload_bytes.len()),
        truncated: parsed
            .fields
            .get("truncated")
            .is_some_and(|value| value == "true"),
        payload,
    })
}

pub(super) fn render_upstream_payload_lines(payload: &str, width: usize) -> Vec<String> {
    let Ok(value) = serde_json::from_str::<Value>(payload) else {
        return render_text_body(payload, width);
    };

    let mut lines = Vec::new();
    if let Some(object) = value.as_object() {
        let mut summary = Vec::new();
        for key in [
            "model",
            "stream",
            "store",
            "tool_choice",
            "parallel_tool_calls",
            "prompt_cache_key",
        ] {
            if let Some(value) = object.get(key).and_then(short_json_value) {
                summary.push(format!("{key}={value}"));
            }
        }
        if !summary.is_empty() {
            push_wrapped_line(&mut lines, &summary.join(" "), width);
        }

        if let Some(metadata) = object.get("client_metadata").and_then(Value::as_object) {
            let mut metadata_fields = Vec::new();
            for key in [
                "session_id",
                "thread_id",
                "turn_id",
                "x-codex-window-id",
                "x-codex-installation-id",
            ] {
                if let Some(value) = metadata.get(key).and_then(Value::as_str) {
                    metadata_fields.push(format!("{key}={value}"));
                }
            }
            if !metadata_fields.is_empty() {
                push_wrapped_line(
                    &mut lines,
                    &format!("client_metadata: {}", metadata_fields.join(" ")),
                    width,
                );
            }
        }

        if let Some(instructions) = object.get("instructions").and_then(Value::as_str) {
            push_text_block(&mut lines, "instructions", instructions, width);
        }

        if let Some(input) = object.get("input") {
            render_input_value(&mut lines, input, width);
        }

        if let Some(tools) = object.get("tools").and_then(Value::as_array) {
            let names = tools.iter().filter_map(tool_name).collect::<Vec<_>>();
            let summary = if names.is_empty() {
                format!("tools: {}", tools.len())
            } else {
                format!("tools: {} {}", tools.len(), names.join(", "))
            };
            push_wrapped_line(&mut lines, &summary, width);
        }
    }

    if lines.is_empty() {
        serde_json::to_string_pretty(&value)
            .map(|pretty| render_text_body(&pretty, width))
            .unwrap_or_else(|_| render_text_body(payload, width))
    } else {
        lines
    }
}

fn render_input_value(lines: &mut Vec<String>, input: &Value, width: usize) {
    match input {
        Value::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                render_input_item(lines, index, item, width);
            }
        }
        Value::String(text) => push_text_block(lines, "input", text, width),
        _ => push_text_block(
            lines,
            "input",
            &serde_json::to_string_pretty(input).unwrap_or_else(|_| input.to_string()),
            width,
        ),
    }
}

fn render_input_item(lines: &mut Vec<String>, index: usize, item: &Value, width: usize) {
    let Some(object) = item.as_object() else {
        push_text_block(
            lines,
            &format!("input[{index}]"),
            &readable_json_value(item),
            width,
        );
        return;
    };

    let item_type = object
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("message");
    let role = object
        .get("role")
        .and_then(Value::as_str)
        .unwrap_or(item_type);
    match item_type {
        "function_call" => {
            let name = object.get("name").and_then(Value::as_str).unwrap_or("tool");
            let text = object
                .get("arguments")
                .map(readable_json_string_or_value)
                .unwrap_or_default();
            push_text_block(
                lines,
                &format!("input[{index}] tool-call:{name}"),
                &text,
                width,
            );
        }
        "function_call_output" => {
            let text = object
                .get("output")
                .map(readable_json_string_or_value)
                .unwrap_or_default();
            push_text_block(lines, &format!("input[{index}] tool-output"), &text, width);
        }
        _ => {
            let text = object
                .get("content")
                .map(readable_content_text)
                .filter(|text| !text.trim().is_empty())
                .or_else(|| {
                    object
                        .get("text")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                })
                .unwrap_or_else(|| readable_json_value(item));
            push_text_block(lines, &format!("input[{index}] {role}"), &text, width);
        }
    }
}

fn readable_content_text(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Array(items) => items
            .iter()
            .map(readable_content_text)
            .filter(|text| !text.trim().is_empty())
            .collect::<Vec<_>>()
            .join("\n"),
        Value::Object(object) => object
            .get("text")
            .or_else(|| object.get("content"))
            .or_else(|| object.get("input_text"))
            .or_else(|| object.get("output_text"))
            .map(readable_content_text)
            .unwrap_or_else(|| readable_json_value(value)),
        _ => readable_json_value(value),
    }
}

fn readable_json_string_or_value(value: &Value) -> String {
    if let Some(text) = value.as_str() {
        return serde_json::from_str::<Value>(text)
            .ok()
            .and_then(|value| serde_json::to_string_pretty(&value).ok())
            .unwrap_or_else(|| text.to_string());
    }
    readable_json_value(value)
}

fn readable_json_value(value: &Value) -> String {
    serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
}

fn short_json_value(value: &Value) -> Option<String> {
    match value {
        Value::String(value) if value.len() <= 80 => Some(value.clone()),
        Value::String(value) => Some(format!("{}...", value.chars().take(77).collect::<String>())),
        Value::Bool(value) => Some(value.to_string()),
        Value::Number(value) => Some(value.to_string()),
        _ => None,
    }
}

fn tool_name(value: &Value) -> Option<String> {
    let object = value.as_object()?;
    object
        .get("name")
        .and_then(Value::as_str)
        .or_else(|| {
            object
                .get("function")
                .and_then(|function| function.get("name"))
                .and_then(Value::as_str)
        })
        .map(str::to_string)
}

fn push_text_block(lines: &mut Vec<String>, label: &str, text: &str, width: usize) {
    lines.push(format!("{label}:"));
    for line in render_text_body(text, width) {
        lines.push(format!("  {line}"));
    }
}

fn push_wrapped_line(lines: &mut Vec<String>, text: &str, width: usize) {
    lines.extend(render_text_body(text, width));
}

struct ParsedRuntimeLogLine {
    timestamp: String,
    event: Option<String>,
    fields: BTreeMap<String, String>,
}

fn parse_runtime_log_line(line: &str) -> Option<ParsedRuntimeLogLine> {
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
                        value.as_str().map(|value| (key.clone(), value.to_string()))
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
