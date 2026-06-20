use super::collect_recent_runtime_log_paths;
use crate::{prodex_runtime_log_paths_in_dir, runtime_proxy_log_dir};
use anyhow::{Context, Result};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use prodex_runtime_doctor::read_runtime_log_tail;
use std::collections::BTreeMap;
#[cfg(test)]
use std::env;
use std::fs;
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;
#[cfg(test)]
use std::time::{SystemTime, UNIX_EPOCH};

const LOG_STREAM_POLL_INTERVAL: Duration = Duration::from_millis(250);
const LOG_SNAPSHOT_TAIL_BYTES: usize = 1024 * 1024;

#[derive(Default)]
struct FollowedLog {
    offset: u64,
    pending: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
struct UpstreamPayloadEvent {
    timestamp: String,
    request: Option<u64>,
    transport: String,
    route: String,
    profile: String,
    bytes: usize,
    logged_bytes: usize,
    truncated: bool,
    payload: String,
}

pub(super) fn stream_upstream_payload_events(json: bool) -> Result<()> {
    if let Some(event) = latest_upstream_payload_event() {
        print_upstream_payload_event(&event, json)?;
    } else {
        eprintln!("Waiting for processed upstream payload events...");
    }

    let mut followed_runtime_logs = BTreeMap::<PathBuf, FollowedLog>::new();
    for path in prodex_runtime_log_paths_in_dir(&runtime_proxy_log_dir()) {
        let offset = fs::metadata(&path)
            .map(|metadata| metadata.len())
            .unwrap_or(0);
        followed_runtime_logs.insert(
            path,
            FollowedLog {
                offset,
                pending: String::new(),
            },
        );
    }

    loop {
        for path in prodex_runtime_log_paths_in_dir(&runtime_proxy_log_dir()) {
            let state = followed_runtime_logs.entry(path.clone()).or_default();
            read_new_upstream_payload_events(&path, state, json)?;
        }
        thread::sleep(LOG_STREAM_POLL_INTERVAL);
    }
}

fn read_new_upstream_payload_events(
    path: &Path,
    state: &mut FollowedLog,
    json: bool,
) -> Result<()> {
    let mut file = match fs::File::open(path) {
        Ok(file) => file,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(err).with_context(|| format!("failed to open {}", path.display())),
    };
    let len = file.metadata()?.len();
    if len < state.offset {
        state.offset = 0;
        state.pending.clear();
    }
    file.seek(SeekFrom::Start(state.offset))?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    state.offset = state.offset.saturating_add(bytes.len() as u64);
    if bytes.is_empty() {
        return Ok(());
    }

    state.pending.push_str(&String::from_utf8_lossy(&bytes));
    let complete_len = state
        .pending
        .rfind('\n')
        .map(|index| index + 1)
        .unwrap_or_default();
    if complete_len == 0 {
        return Ok(());
    }
    let complete = state.pending[..complete_len].to_string();
    state.pending.drain(..complete_len);
    for line in complete.lines() {
        if let Some(event) = upstream_payload_event_from_runtime_line(line) {
            print_upstream_payload_event(&event, json)?;
        }
    }
    Ok(())
}

fn print_upstream_payload_event(event: &UpstreamPayloadEvent, json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string(event)?);
    } else {
        let request = event
            .request
            .map(|request| request.to_string())
            .unwrap_or_else(|| "-".to_string());
        println!(
            "{} upstream profile={} request={} transport={} route={} bytes={} logged={} truncated={}",
            event.timestamp,
            event.profile,
            request,
            event.transport,
            event.route,
            event.bytes,
            event.logged_bytes,
            event.truncated,
        );
        for line in event.payload.lines() {
            println!("  {line}");
        }
    }
    io::stdout()
        .flush()
        .context("failed to flush upstream log output")
}

fn latest_upstream_payload_event() -> Option<UpstreamPayloadEvent> {
    let mut latest = None;
    for path in collect_recent_runtime_log_paths(32) {
        let tail = match read_runtime_log_tail(&path, LOG_SNAPSHOT_TAIL_BYTES) {
            Ok(tail) => tail,
            Err(_) => continue,
        };
        for line in String::from_utf8_lossy(&tail).lines() {
            let Some(event) = upstream_payload_event_from_runtime_line(line) else {
                continue;
            };
            if latest
                .as_ref()
                .is_none_or(|current: &UpstreamPayloadEvent| event.timestamp >= current.timestamp)
            {
                latest = Some(event);
            }
        }
    }
    latest
}

fn upstream_payload_event_from_runtime_line(line: &str) -> Option<UpstreamPayloadEvent> {
    let parsed = parse_runtime_log_line(line)?;
    if parsed.event.as_deref() != Some("upstream_payload") {
        return None;
    }
    let payload_b64 = parsed.fields.get("payload_b64")?;
    let payload_bytes = BASE64_STANDARD.decode(payload_b64).ok()?;
    let payload = String::from_utf8_lossy(&payload_bytes).into_owned();
    Some(UpstreamPayloadEvent {
        timestamp: parsed.timestamp,
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
        BASE64_STANDARD, FollowedLog, SystemTime, UNIX_EPOCH, env, fs,
        read_new_upstream_payload_events, upstream_payload_event_from_runtime_line,
    };
    use base64::Engine;
    use std::io::Write;

    #[test]
    fn parses_upstream_payload_events() {
        let payload = br#"{"input":"hello <EMAIL_ADDRESS>"}"#;
        let encoded = BASE64_STANDARD.encode(payload);
        let line = format!(
            "[2026-06-20 12:00:00.000 +07:00] upstream_payload request=9 transport=websocket route=websocket profile=main bytes=35 logged_bytes=35 truncated=false payload_b64={encoded}"
        );

        let event = upstream_payload_event_from_runtime_line(&line).unwrap();
        assert_eq!(event.timestamp, "2026-06-20 12:00:00.000 +07:00");
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
    fn follows_only_complete_upstream_payload_lines() {
        let root = env::temp_dir().join(format!(
            "prodex-upstream-follow-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("runtime.log");
        let encoded = BASE64_STANDARD.encode(br#"{"input":"hello"}"#);
        fs::write(
            &path,
            format!(
                "[2026-06-20 12:00:00.000 +07:00] upstream_payload request=9 transport=http route=responses profile=main bytes=17 logged_bytes=17 truncated=false payload_b64={encoded}"
            ),
        )
        .unwrap();
        let mut state = FollowedLog::default();
        read_new_upstream_payload_events(&path, &mut state, true).unwrap();
        assert!(!state.pending.is_empty());
        fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap()
            .write_all(b"\n")
            .unwrap();
        read_new_upstream_payload_events(&path, &mut state, true).unwrap();
        assert!(state.pending.is_empty());
        fs::remove_dir_all(root).unwrap();
    }
}
