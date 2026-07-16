pub(super) use self::render::log_snapshot_items;
#[cfg(test)]
pub(super) use self::render::log_stream_tui_text;
use super::{
    FollowedLog, LOG_SNAPSHOT_TAIL_BYTES, LogStreamItem, TranscriptEvent,
    collect_new_runtime_log_stream_items, collect_new_transcript_events, latest_transcript_event,
    local_token_usage_event, print_token_usage_event, print_transcript_event,
    print_upstream_payload_event, read_new_runtime_log_events, read_new_transcript_events,
    recent_session_log_paths,
};
use crate::app_commands::collect_recent_runtime_log_paths;
use crate::app_commands::log_tui::{
    LogTuiHeaderDetail, LogTuiInput, LogTuiState, LogTuiTerminal, log_tui_header_detail,
    log_tui_header_next_refresh_at,
};
use crate::app_commands::log_upstream::{
    latest_upstream_payload_event, stream_upstream_payload_events,
};
use crate::app_commands::log_upstream_payload::UpstreamPayloadEvent;
use crate::{LogArgs, LogMode, prodex_runtime_log_paths_in_dir, runtime_proxy_log_dir};
use anyhow::{Context, Result};
use crossterm::event::{self, Event, KeyEventKind};
use prodex_app_reports::{InfoTokenUsageEvent, info_token_usage_event_from_line};
use prodex_runtime_doctor::read_runtime_log_tail;
use std::collections::{BTreeMap, VecDeque};
use std::fs;
use std::io::{self, IsTerminal};
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};

#[path = "log_tui_render.rs"]
mod render;

const LOG_STREAM_POLL_INTERVAL: Duration = Duration::from_millis(250);
const LOG_TUI_EVENT_LIMIT: usize = 200;

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
            let upstream_payload = latest_upstream_payload_event();
            let token_usage = latest_token_usage_event();
            print_log_snapshot(
                transcript.as_ref(),
                upstream_payload.as_ref(),
                token_usage.as_ref(),
            )
        }
        LogMode::Stream => stream_token_usage_events(args.json),
        LogMode::Upstream => stream_upstream_payload_events(args.json),
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
            let Some(event) = info_token_usage_event_from_line(line).map(local_token_usage_event)
            else {
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
    if !json && io::stdout().is_terminal() && io::stdin().is_terminal() {
        return stream_token_usage_events_tui();
    }

    if !json && let Some(event) = latest_transcript_event()? {
        print_transcript_event(&event)?;
    }
    if !json && let Some(event) = latest_upstream_payload_event() {
        print_upstream_payload_event(&event)?;
    }
    if let Some(event) = latest_token_usage_event() {
        print_token_usage_event(&event, json)?;
    } else {
        eprintln!("Waiting for transcript, upstream payload, or token usage events...");
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
            read_new_runtime_log_events(&path, state, json)?;
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

fn stream_token_usage_events_tui() -> Result<()> {
    let mut tui = LogTuiTerminal::stdout("log stream TUI")?;
    let mut view = LogTuiState::default();
    let mut items = VecDeque::<LogStreamItem>::new();
    if let Some(event) = latest_transcript_event()? {
        push_log_stream_item(&mut items, LogStreamItem::Transcript(event));
    }
    if let Some(event) = latest_upstream_payload_event() {
        push_log_stream_item(&mut items, LogStreamItem::UpstreamPayload(event));
    }
    if let Some(event) = latest_token_usage_event() {
        push_log_stream_item(&mut items, LogStreamItem::TokenUsage(event));
    }
    let mut header_profile = latest_log_stream_profile(&items).map(str::to_string);
    let mut header_detail = log_tui_header_detail(header_profile.as_deref());
    let mut header_refresh_at =
        log_tui_header_next_refresh_at(header_detail.as_ref(), Instant::now());

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

    loop {
        for path in prodex_runtime_log_paths_in_dir(&runtime_proxy_log_dir()) {
            let state = followed_runtime_logs.entry(path.clone()).or_default();
            for event in collect_new_runtime_log_stream_items(&path, state)? {
                push_log_stream_item(&mut items, event);
            }
        }
        for path in recent_session_log_paths()? {
            let state = followed_session_logs.entry(path.clone()).or_default();
            for event in collect_new_transcript_events(&path, state)? {
                push_log_stream_item(&mut items, LogStreamItem::Transcript(event));
            }
        }
        let latest_profile = latest_log_stream_profile(&items).map(str::to_string);
        let now = Instant::now();
        if latest_profile != header_profile || now >= header_refresh_at {
            header_profile = latest_profile;
            header_detail = log_tui_header_detail(header_profile.as_deref());
            header_refresh_at = log_tui_header_next_refresh_at(header_detail.as_ref(), now);
        }

        tui.terminal
            .draw(|frame| render_log_stream_tui(frame, &items, &view, header_detail.as_ref()))
            .context("failed to draw log stream TUI")?;

        if event::poll(LOG_STREAM_POLL_INTERVAL).context("failed to poll log stream TUI input")? {
            match event::read().context("failed to read log stream TUI input")? {
                Event::Key(key)
                    if key.kind == KeyEventKind::Press
                        && view.apply_key(key) == LogTuiInput::Quit =>
                {
                    return Ok(());
                }
                _ => {}
            }
        }
    }
}

fn print_log_snapshot(
    transcript: Option<&TranscriptEvent>,
    upstream_payload: Option<&UpstreamPayloadEvent>,
    token_usage: Option<&InfoTokenUsageEvent>,
) -> Result<()> {
    if io::stdout().is_terminal()
        && let Some(mut terminal) = crate::try_inline_stdout_terminal(
            render::log_snapshot_tui_height(transcript, upstream_payload, token_usage),
        )
    {
        let items = log_snapshot_items(transcript, upstream_payload, token_usage);
        terminal
            .draw(|frame| render::render_log_snapshot_tui(frame, &items))
            .context("failed to draw log snapshot TUI")?;
        let _ = terminal.show_cursor();
        return Ok(());
    }

    if transcript.is_none() && upstream_payload.is_none() && token_usage.is_none() {
        println!("No transcript, upstream payload, or token usage events found.");
        return Ok(());
    }
    if let Some(event) = transcript {
        print_transcript_event(event)?;
    }
    if let Some(event) = upstream_payload {
        print_upstream_payload_event(event)?;
    }
    if let Some(event) = token_usage {
        print_token_usage_event(event, false)?;
    }
    Ok(())
}

fn push_log_stream_item(items: &mut VecDeque<LogStreamItem>, item: LogStreamItem) {
    items.push_back(item);
    while items.len() > LOG_TUI_EVENT_LIMIT {
        items.pop_front();
    }
}

fn latest_log_stream_profile(items: &VecDeque<LogStreamItem>) -> Option<&str> {
    items.iter().rev().find_map(|item| match item {
        LogStreamItem::TokenUsage(event) => Some(event.profile.as_str()),
        LogStreamItem::UpstreamPayload(event) => Some(event.profile.as_str()),
        LogStreamItem::Transcript(_) => None,
    })
}

fn render_log_stream_tui(
    frame: &mut ratatui::Frame<'_>,
    items: &VecDeque<LogStreamItem>,
    state: &LogTuiState,
    header_detail: Option<&LogTuiHeaderDetail>,
) {
    render::render_log_stream_tui(frame, items, state, header_detail);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn maps_scroll_and_search_keys() {
        let mut state = LogTuiState::default();

        assert_eq!(state.apply_key(key(KeyCode::Up)), LogTuiInput::Continue);
        assert_eq!(state.scroll_from_bottom(), 1);
        assert_eq!(state.apply_key(key(KeyCode::Down)), LogTuiInput::Continue);
        assert_eq!(state.scroll_from_bottom(), 0);

        state.apply_key(key(KeyCode::Char('/')));
        state.apply_key(key(KeyCode::Char('h')));
        state.apply_key(key(KeyCode::Char('i')));
        state.apply_key(key(KeyCode::Enter));

        assert_eq!(state.query(), Some("hi"));
        assert!(state.footer_text("q quit").contains("search: /hi"));
    }
}
