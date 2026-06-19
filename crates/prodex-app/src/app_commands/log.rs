use super::collect_recent_runtime_log_paths;
use crate::{LogArgs, LogMode, prodex_runtime_log_paths_in_dir, runtime_proxy_log_dir};
use anyhow::{Context, Result};
use prodex_app_reports::{InfoTokenUsageEvent, info_token_usage_event_from_line};
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

pub(crate) fn handle_log(args: LogArgs) -> Result<()> {
    match args.mode {
        LogMode::Last => {
            let Some(event) = latest_token_usage_event() else {
                println!("No token usage events found.");
                return Ok(());
            };
            print_token_usage_event(&event, args.json)
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
    if let Some(event) = latest_token_usage_event() {
        print_token_usage_event(&event, json)?;
    } else {
        eprintln!("Waiting for token usage events...");
    }

    let mut followed = BTreeMap::<PathBuf, FollowedLog>::new();
    for path in prodex_runtime_log_paths_in_dir(&runtime_proxy_log_dir()) {
        let offset = fs::metadata(&path)
            .map(|metadata| metadata.len())
            .unwrap_or(0);
        followed.insert(
            path,
            FollowedLog {
                offset,
                pending: String::new(),
            },
        );
    }

    loop {
        for path in prodex_runtime_log_paths_in_dir(&runtime_proxy_log_dir()) {
            let state = followed.entry(path.clone()).or_default();
            read_new_token_usage_events(&path, state, json)?;
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
}
