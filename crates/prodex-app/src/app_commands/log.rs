use super::collect_recent_runtime_log_paths;
use crate::{LogArgs, LogMode, prodex_runtime_log_paths_in_dir, runtime_proxy_log_dir};
use anyhow::{Context, Result};
use prodex_app_reports::{InfoTokenUsageEvent, info_token_usage_event_from_line};
use prodex_core::AppPaths;
use prodex_runtime_doctor::read_runtime_log_tail;
use std::collections::BTreeMap;
#[cfg(test)]
use std::env;
use std::fs;
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::thread;
#[cfg(test)]
use std::time::SystemTime;
use std::time::{Duration, UNIX_EPOCH};

const LOG_STREAM_POLL_INTERVAL: Duration = Duration::from_millis(250);
const LOG_SNAPSHOT_TAIL_BYTES: usize = 1024 * 1024;
const SESSION_SNAPSHOT_TAIL_BYTES: usize = 2 * 1024 * 1024;
const SESSION_FOLLOW_LIMIT: usize = 32;

#[derive(Default)]
struct FollowedLog {
    offset: u64,
    pending: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TranscriptEvent {
    timestamp: String,
    source: String,
    text: String,
}

pub(crate) fn handle_log(args: LogArgs) -> Result<()> {
    match args.mode {
        LogMode::Last => {
            if args.json {
                let Some(event) = latest_token_usage_event() else {
                    println!("No token usage events found.");
                    return Ok(());
                };
                return print_token_usage_event(&event, true);
            }
            let transcript = latest_transcript_event()?;
            let token_usage = latest_token_usage_event();
            if transcript.is_none() && token_usage.is_none() {
                println!("No transcript or token usage events found.");
                return Ok(());
            }
            if let Some(event) = transcript.as_ref() {
                print_transcript_event(event)?;
            }
            if let Some(event) = token_usage.as_ref() {
                print_token_usage_event(event, false)?;
            }
            Ok(())
        }
        LogMode::Stream => stream_token_usage_events(args.json),
    }
}

fn latest_token_usage_event() -> Option<InfoTokenUsageEvent> {
    let mut latest = None;
    for path in collect_recent_runtime_log_paths(32) {
        let tail = match read_runtime_log_tail(&path, LOG_SNAPSHOT_TAIL_BYTES) {
            Ok(tail) => tail,
            Err(_) => continue,
        };
        for line in String::from_utf8_lossy(&tail).lines() {
            let Some(event) = info_token_usage_event_from_line(line) else {
                continue;
            };
            if latest
                .as_ref()
                .is_none_or(|current: &InfoTokenUsageEvent| event.timestamp >= current.timestamp)
            {
                latest = Some(event);
            }
        }
    }
    latest
}

fn stream_token_usage_events(json: bool) -> Result<()> {
    if !json && let Some(event) = latest_transcript_event()? {
        print_transcript_event(&event)?;
    }
    if let Some(event) = latest_token_usage_event() {
        print_token_usage_event(&event, json)?;
    } else {
        eprintln!("Waiting for transcript or token usage events...");
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
    let mut followed_session_logs = BTreeMap::<PathBuf, FollowedLog>::new();
    if !json {
        for path in recent_session_log_paths()? {
            let offset = fs::metadata(&path)
                .map(|metadata| metadata.len())
                .unwrap_or(0);
            followed_session_logs.insert(
                path,
                FollowedLog {
                    offset,
                    pending: String::new(),
                },
            );
        }
    }

    loop {
        for path in prodex_runtime_log_paths_in_dir(&runtime_proxy_log_dir()) {
            let state = followed_runtime_logs.entry(path.clone()).or_default();
            read_new_token_usage_events(&path, state, json)?;
        }
        if !json {
            for path in recent_session_log_paths()? {
                let state = followed_session_logs.entry(path.clone()).or_default();
                read_new_transcript_events(&path, state)?;
            }
        }
        thread::sleep(LOG_STREAM_POLL_INTERVAL);
    }
}

fn read_new_token_usage_events(path: &Path, state: &mut FollowedLog, json: bool) -> Result<()> {
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
        if let Some(event) = info_token_usage_event_from_line(line) {
            print_token_usage_event(&event, json)?;
        }
    }
    Ok(())
}

fn read_new_transcript_events(path: &Path, state: &mut FollowedLog) -> Result<()> {
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
        for event in transcript_events_from_session_line(line) {
            print_transcript_event(&event)?;
        }
    }
    Ok(())
}

fn print_token_usage_event(event: &InfoTokenUsageEvent, json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string(event)?);
    } else {
        let request = event
            .request
            .map(|request| request.to_string())
            .unwrap_or_else(|| "-".to_string());
        println!(
            "{} profile={} request={} transport={} source={} sent={} cached={} received={} reasoning={}",
            event.timestamp,
            event.profile,
            request,
            event.transport,
            event.source,
            event.input_tokens,
            event.cached_input_tokens,
            event.output_tokens,
            event.reasoning_tokens,
        );
    }
    io::stdout()
        .flush()
        .context("failed to flush token log output")
}

fn print_transcript_event(event: &TranscriptEvent) -> Result<()> {
    println!("{} {}:", event.timestamp, event.source);
    for line in event.text.lines() {
        println!("  {line}");
    }
    io::stdout()
        .flush()
        .context("failed to flush transcript log output")
}

fn latest_transcript_event() -> Result<Option<TranscriptEvent>> {
    let mut latest = None;
    for path in recent_session_log_paths()? {
        let tail = match read_runtime_log_tail(&path, SESSION_SNAPSHOT_TAIL_BYTES) {
            Ok(tail) => tail,
            Err(_) => continue,
        };
        for line in String::from_utf8_lossy(&tail).lines() {
            for event in transcript_events_from_session_line(line) {
                if latest
                    .as_ref()
                    .is_none_or(|current: &TranscriptEvent| event.timestamp >= current.timestamp)
                {
                    latest = Some(event);
                }
            }
        }
    }
    Ok(latest)
}

fn recent_session_log_paths() -> Result<Vec<PathBuf>> {
    let sessions_root = AppPaths::discover()?.shared_codex_root.join("sessions");
    let mut paths = Vec::new();
    collect_session_log_paths(&sessions_root, &mut paths)?;
    paths.sort_by(|left, right| {
        let left_modified = path_modified_key(left);
        let right_modified = path_modified_key(right);
        right_modified
            .cmp(&left_modified)
            .then_with(|| left.cmp(right))
    });
    paths.truncate(SESSION_FOLLOW_LIMIT);
    Ok(paths)
}

fn collect_session_log_paths(root: &Path, paths: &mut Vec<PathBuf>) -> Result<()> {
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(err).with_context(|| format!("failed to read {}", root.display())),
    };
    for entry in entries {
        let entry = entry.with_context(|| format!("failed to read entry in {}", root.display()))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .with_context(|| format!("failed to inspect {}", path.display()))?;
        if file_type.is_dir() {
            collect_session_log_paths(&path, paths)?;
        } else if file_type.is_file()
            && path
                .extension()
                .is_some_and(|extension| extension == "jsonl")
        {
            paths.push(path);
        }
    }
    Ok(())
}

fn path_modified_key(path: &Path) -> u128 {
    fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .ok()
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

fn transcript_events_from_session_line(line: &str) -> Vec<TranscriptEvent> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
        return Vec::new();
    };
    let timestamp = value
        .get("timestamp")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("-")
        .to_string();
    let Some(record_type) = value.get("type").and_then(serde_json::Value::as_str) else {
        return Vec::new();
    };
    let Some(payload) = value.get("payload") else {
        return Vec::new();
    };

    match record_type {
        "session_meta" => payload
            .get("base_instructions")
            .and_then(|base| base.get("text"))
            .and_then(serde_json::Value::as_str)
            .map(|text| TranscriptEvent {
                timestamp,
                source: "prompt-engineering".to_string(),
                text: text.to_string(),
            })
            .into_iter()
            .collect(),
        "response_item" => response_item_transcript_event(timestamp, payload)
            .into_iter()
            .collect(),
        _ => Vec::new(),
    }
}

fn response_item_transcript_event(
    timestamp: String,
    payload: &serde_json::Value,
) -> Option<TranscriptEvent> {
    match payload.get("type").and_then(serde_json::Value::as_str)? {
        "message" => {
            let source = payload
                .get("role")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("message")
                .to_string();
            let text = transcript_text_from_content(payload.get("content")?)?;
            Some(TranscriptEvent {
                timestamp,
                source,
                text,
            })
        }
        "function_call" => {
            let name = payload
                .get("name")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("tool");
            let arguments = payload
                .get("arguments")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("");
            Some(TranscriptEvent {
                timestamp,
                source: format!("tool-call:{name}"),
                text: arguments.to_string(),
            })
        }
        "function_call_output" => {
            let output = payload
                .get("output")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("");
            Some(TranscriptEvent {
                timestamp,
                source: "tool-output".to_string(),
                text: output.to_string(),
            })
        }
        _ => None,
    }
    .filter(|event| !event.text.trim().is_empty())
}

fn transcript_text_from_content(content: &serde_json::Value) -> Option<String> {
    let values = content.as_array()?;
    let mut parts = Vec::new();
    for value in values {
        if let Some(text) = value
            .get("text")
            .or_else(|| value.get("content"))
            .and_then(serde_json::Value::as_str)
            .filter(|text| !text.trim().is_empty())
        {
            parts.push(text.to_string());
        }
    }
    (!parts.is_empty()).then(|| parts.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn follows_only_complete_token_usage_lines() {
        let root = env::temp_dir().join(format!(
            "prodex-log-follow-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("runtime.log");
        fs::write(
            &path,
            "[2026-06-19 20:00:00.000 +07:00] token_usage request=7 transport=http profile=main source=responses input_tokens=12 cached_input_tokens=3 output_tokens=4 reasoning_tokens=1",
        )
        .unwrap();
        let mut state = FollowedLog::default();
        read_new_token_usage_events(&path, &mut state, true).unwrap();
        assert!(!state.pending.is_empty());
        fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap()
            .write_all(b"\n")
            .unwrap();
        read_new_token_usage_events(&path, &mut state, true).unwrap();
        assert!(state.pending.is_empty());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn parses_session_transcript_text_events() {
        let meta = r#"{"timestamp":"2026-06-20T01:00:00Z","type":"session_meta","payload":{"base_instructions":{"text":"System prompt."}}}"#;
        let user = r#"{"timestamp":"2026-06-20T01:00:01Z","type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"Hello model."}]}}"#;
        let assistant = r#"{"timestamp":"2026-06-20T01:00:02Z","type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"Hello user."}]}}"#;
        let tool = r#"{"timestamp":"2026-06-20T01:00:03Z","type":"response_item","payload":{"type":"function_call","name":"exec_command","arguments":"{\"cmd\":\"pwd\"}"}}"#;

        assert_eq!(
            transcript_events_from_session_line(meta),
            vec![TranscriptEvent {
                timestamp: "2026-06-20T01:00:00Z".to_string(),
                source: "prompt-engineering".to_string(),
                text: "System prompt.".to_string(),
            }]
        );
        assert_eq!(
            transcript_events_from_session_line(user),
            vec![TranscriptEvent {
                timestamp: "2026-06-20T01:00:01Z".to_string(),
                source: "user".to_string(),
                text: "Hello model.".to_string(),
            }]
        );
        assert_eq!(
            transcript_events_from_session_line(assistant),
            vec![TranscriptEvent {
                timestamp: "2026-06-20T01:00:02Z".to_string(),
                source: "assistant".to_string(),
                text: "Hello user.".to_string(),
            }]
        );
        assert_eq!(
            transcript_events_from_session_line(tool),
            vec![TranscriptEvent {
                timestamp: "2026-06-20T01:00:03Z".to_string(),
                source: "tool-call:exec_command".to_string(),
                text: "{\"cmd\":\"pwd\"}".to_string(),
            }]
        );
    }

    #[test]
    fn follows_only_complete_transcript_lines() {
        let root = env::temp_dir().join(format!(
            "prodex-transcript-follow-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("session.jsonl");
        fs::write(
            &path,
            r#"{"timestamp":"2026-06-20T01:00:01Z","type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"Hello"}]}}"#,
        )
        .unwrap();
        let mut state = FollowedLog::default();
        read_new_transcript_events(&path, &mut state).unwrap();
        assert!(!state.pending.is_empty());
        fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap()
            .write_all(b"\n")
            .unwrap();
        read_new_transcript_events(&path, &mut state).unwrap();
        assert!(state.pending.is_empty());
        fs::remove_dir_all(root).unwrap();
    }
}
