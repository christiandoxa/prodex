use crate::{LogArgs, LogMode};
use anyhow::{Context, Result};
use std::collections::BTreeMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

const LOG_INITIAL_TAIL_BYTES: u64 = 64 * 1024;
const LOG_LAST_TAIL_BYTES: usize = 256 * 1024;
const LOG_FOLLOW_FILE_LIMIT: usize = 16;
const LOG_POLL_INTERVAL: Duration = Duration::from_millis(250);

#[derive(Debug, Default)]
struct LogCursor {
    offset: u64,
    pending: Vec<u8>,
    initialized: bool,
}

pub(crate) fn handle_log(args: LogArgs) -> Result<()> {
    match args.mode {
        LogMode::Last => print_latest_log_line(args.json),
        LogMode::Stream | LogMode::Upstream => follow_runtime_logs(args.mode, args.json),
    }
}

fn print_latest_log_line(json: bool) -> Result<()> {
    let Some(line) = latest_matching_line(LogMode::Stream)? else {
        println!("No runtime logs found.");
        return Ok(());
    };
    print_log_line(&line, json)
}

fn latest_matching_line(mode: LogMode) -> Result<Option<String>> {
    for path in super::collect_recent_runtime_log_paths(LOG_FOLLOW_FILE_LIMIT) {
        let tail = prodex_runtime_doctor::read_runtime_log_tail(&path, LOG_LAST_TAIL_BYTES)
            .with_context(|| format!("failed to read runtime log {}", path.display()))?;
        if let Some(line) = String::from_utf8_lossy(&tail)
            .lines()
            .rev()
            .find(|line| log_line_matches(mode, line))
        {
            return Ok(Some(line.to_string()));
        }
    }
    Ok(None)
}

fn follow_runtime_logs(mode: LogMode, json: bool) -> Result<()> {
    let mut cursors = BTreeMap::<PathBuf, LogCursor>::new();
    let mut announced_wait = false;
    loop {
        let paths = super::collect_recent_runtime_log_paths(LOG_FOLLOW_FILE_LIMIT);
        if paths.is_empty() {
            if !announced_wait {
                eprintln!("Waiting for Prodex runtime logs...");
                announced_wait = true;
            }
            thread::sleep(LOG_POLL_INTERVAL);
            continue;
        }
        announced_wait = false;
        cursors.retain(|path, _| paths.contains(path));
        for path in paths.iter().rev() {
            let cursor = cursors.entry(path.clone()).or_default();
            for line in read_new_log_lines(path, cursor)? {
                if log_line_matches(mode, &line) {
                    print_log_line(&line, json)?;
                }
            }
        }
        thread::sleep(LOG_POLL_INTERVAL);
    }
}

fn read_new_log_lines(path: &Path, cursor: &mut LogCursor) -> Result<Vec<String>> {
    let mut file = File::open(path)
        .with_context(|| format!("failed to open runtime log {}", path.display()))?;
    let len = file
        .metadata()
        .with_context(|| format!("failed to inspect runtime log {}", path.display()))?
        .len();

    if !cursor.initialized || len < cursor.offset {
        cursor.offset = len.saturating_sub(LOG_INITIAL_TAIL_BYTES);
        cursor.pending.clear();
        cursor.initialized = true;
    }

    let started_mid_file = cursor.offset > 0 && cursor.pending.is_empty();
    file.seek(SeekFrom::Start(cursor.offset))
        .with_context(|| format!("failed to seek runtime log {}", path.display()))?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .with_context(|| format!("failed to read runtime log {}", path.display()))?;
    cursor.offset = cursor.offset.saturating_add(bytes.len() as u64);

    if bytes.is_empty() {
        return Ok(Vec::new());
    }
    if started_mid_file {
        if let Some(newline) = bytes.iter().position(|byte| *byte == b'\n') {
            bytes.drain(..=newline);
        } else {
            return Ok(Vec::new());
        }
    }

    if !cursor.pending.is_empty() {
        let mut combined = std::mem::take(&mut cursor.pending);
        combined.extend_from_slice(&bytes);
        bytes = combined;
    }

    let complete_len = bytes
        .iter()
        .rposition(|byte| *byte == b'\n')
        .map(|index| index + 1)
        .unwrap_or(0);
    if complete_len < bytes.len() {
        cursor.pending.extend_from_slice(&bytes[complete_len..]);
    }

    Ok(String::from_utf8_lossy(&bytes[..complete_len])
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(str::to_string)
        .collect())
}

fn log_line_matches(mode: LogMode, line: &str) -> bool {
    match mode {
        LogMode::Stream | LogMode::Last => true,
        LogMode::Upstream => {
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(line)
                && value
                    .get("event")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|event| event.contains("upstream"))
            {
                return true;
            }
            line.to_ascii_lowercase().contains("upstream")
        }
    }
}

fn print_log_line(line: &str, json: bool) -> Result<()> {
    let rendered = if json {
        match serde_json::from_str::<serde_json::Value>(line) {
            Ok(value) => serde_json::to_string(&value),
            Err(_) => serde_json::to_string(&serde_json::json!({"line": line})),
        }
        .context("failed to render runtime log JSON")?
    } else {
        line.to_string()
    };
    let mut stdout = std::io::stdout().lock();
    writeln!(stdout, "{rendered}")?;
    stdout.flush().context("failed to flush runtime log output")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upstream_mode_filters_text_and_json_events() {
        assert!(log_line_matches(
            LogMode::Upstream,
            "[2026-01-01] upstream_response status=200"
        ));
        assert!(log_line_matches(
            LogMode::Upstream,
            r#"{"event":"upstream_payload","message":"redacted"}"#
        ));
        assert!(!log_line_matches(
            LogMode::Upstream,
            r#"{"event":"selection_plan"}"#
        ));
    }
}
