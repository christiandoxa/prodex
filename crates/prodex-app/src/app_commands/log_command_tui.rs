pub(super) use self::render::log_snapshot_items;
#[cfg(test)]
pub(super) use self::render::log_stream_tui_text;
use super::{
    FollowedLog, LOG_SNAPSHOT_TAIL_BYTES, LogStreamItem, TranscriptEvent,
    collect_new_runtime_log_stream_items, collect_new_transcript_events,
    latest_runtime_stream_payload_event, latest_transcript_event, local_token_usage_event,
    print_log_stream_item, print_token_usage_event, print_transcript_event,
    print_upstream_payload_event, recent_session_log_paths,
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
use crate::reports::{InfoTokenUsageEvent, info_token_usage_event_from_line};
use crate::{LogArgs, LogMode, prodex_runtime_log_paths_in_dir, runtime_proxy_log_dir};
use anyhow::{Context, Result};
use crossterm::event::{self, Event, KeyEventKind};
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
            let transcript = latest_transcript_event()?;
            let upstream_payload = latest_upstream_payload_event();
            let token_usage = latest_token_usage_event();
            if args.json {
                for item in log_snapshot_items(
                    transcript.as_ref(),
                    upstream_payload.as_ref(),
                    token_usage.as_ref(),
                ) {
                    print_log_stream_item(&item, true)?;
                }
                return Ok(());
            }
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

    print_initial_token_usage_events(json)?;
    let mut followed_runtime_logs =
        followed_logs(prodex_runtime_log_paths_in_dir(&runtime_proxy_log_dir()));
    let mut followed_session_logs = followed_logs(stream_session_log_paths());
    follow_token_usage_events(json, &mut followed_runtime_logs, &mut followed_session_logs)
}

fn stream_token_usage_events_tui() -> Result<()> {
    let mut tui = LogTuiTerminal::stdout("log stream TUI")?;
    let mut view = LogTuiState::default();
    let mut items = initial_log_stream_items()?;
    let mut header_profile = latest_log_stream_profile(&items).map(str::to_string);
    let mut header_detail = log_tui_header_detail(header_profile.as_deref());
    let mut header_refresh_at =
        log_tui_header_next_refresh_at(header_detail.as_ref(), Instant::now());

    let mut followed_runtime_logs =
        followed_logs(prodex_runtime_log_paths_in_dir(&runtime_proxy_log_dir()));
    let mut followed_session_logs = followed_logs(stream_session_log_paths());

    loop {
        collect_log_stream_items(
            &mut items,
            &mut followed_runtime_logs,
            &mut followed_session_logs,
        )?;
        update_log_stream_header(
            &items,
            &mut header_profile,
            &mut header_detail,
            &mut header_refresh_at,
        );

        tui.terminal
            .draw(|frame| render_log_stream_tui(frame, &items, &view, header_detail.as_ref()))
            .context("failed to draw log stream TUI")?;

        if log_stream_tui_should_quit(&mut view)? {
            return Ok(());
        }
    }
}

fn print_initial_token_usage_events(json: bool) -> Result<()> {
    let items = initial_log_stream_items()?;
    if items.is_empty() {
        eprintln!("Waiting for transcript, upstream payload, or token usage events...");
        return Ok(());
    }
    for item in items {
        print_log_stream_item(&item, json)?;
    }
    Ok(())
}

fn followed_logs(paths: impl IntoIterator<Item = PathBuf>) -> BTreeMap<PathBuf, FollowedLog> {
    paths
        .into_iter()
        .map(|path| {
            let offset = fs::metadata(&path)
                .map(|metadata| metadata.len())
                .unwrap_or(0);
            (
                path,
                FollowedLog {
                    offset,
                    ..Default::default()
                },
            )
        })
        .collect()
}

fn stream_best_effort<T>(result: Result<Vec<T>>) -> Vec<T> {
    result.unwrap_or_default()
}

fn stream_session_log_paths() -> Vec<PathBuf> {
    stream_best_effort(recent_session_log_paths())
}

fn follow_token_usage_events(
    json: bool,
    followed_runtime_logs: &mut BTreeMap<PathBuf, FollowedLog>,
    followed_session_logs: &mut BTreeMap<PathBuf, FollowedLog>,
) -> Result<()> {
    loop {
        read_token_usage_events_tick(json, followed_runtime_logs, followed_session_logs)?;
        thread::sleep(LOG_STREAM_POLL_INTERVAL);
    }
}

fn read_token_usage_events_tick(
    json: bool,
    followed_runtime_logs: &mut BTreeMap<PathBuf, FollowedLog>,
    followed_session_logs: &mut BTreeMap<PathBuf, FollowedLog>,
) -> Result<()> {
    for path in prodex_runtime_log_paths_in_dir(&runtime_proxy_log_dir()) {
        let state = followed_runtime_logs.entry(path.clone()).or_default();
        for event in stream_best_effort(collect_new_runtime_log_stream_items(&path, state)) {
            print_log_stream_item(&event, json)?;
        }
    }
    for path in stream_session_log_paths() {
        let state = followed_session_logs.entry(path.clone()).or_default();
        for event in stream_best_effort(collect_new_transcript_events(&path, state)) {
            if json {
                print_log_stream_item(&LogStreamItem::Transcript(event), true)?;
            } else {
                print_transcript_event(&event)?;
            }
        }
    }
    Ok(())
}

fn initial_log_stream_items() -> Result<VecDeque<LogStreamItem>> {
    let mut items = VecDeque::new();
    if let Ok(Some(event)) = latest_transcript_event() {
        push_log_stream_item(&mut items, LogStreamItem::Transcript(event));
    }
    if let Some(event) = latest_runtime_stream_payload_event() {
        push_log_stream_item(&mut items, LogStreamItem::Transcript(event));
    }
    if let Some(event) = latest_upstream_payload_event() {
        push_log_stream_item(&mut items, LogStreamItem::UpstreamPayload(event));
    }
    if let Some(event) = latest_token_usage_event() {
        push_log_stream_item(&mut items, LogStreamItem::TokenUsage(event));
    }
    Ok(items)
}

fn collect_log_stream_items(
    items: &mut VecDeque<LogStreamItem>,
    followed_runtime_logs: &mut BTreeMap<PathBuf, FollowedLog>,
    followed_session_logs: &mut BTreeMap<PathBuf, FollowedLog>,
) -> Result<()> {
    for path in prodex_runtime_log_paths_in_dir(&runtime_proxy_log_dir()) {
        let state = followed_runtime_logs.entry(path.clone()).or_default();
        for event in stream_best_effort(collect_new_runtime_log_stream_items(&path, state)) {
            push_log_stream_item(items, event);
        }
    }
    for path in stream_session_log_paths() {
        let state = followed_session_logs.entry(path.clone()).or_default();
        for event in stream_best_effort(collect_new_transcript_events(&path, state)) {
            push_log_stream_item(items, LogStreamItem::Transcript(event));
        }
    }
    Ok(())
}

fn update_log_stream_header(
    items: &VecDeque<LogStreamItem>,
    header_profile: &mut Option<String>,
    header_detail: &mut Option<LogTuiHeaderDetail>,
    header_refresh_at: &mut Instant,
) {
    let latest_profile = latest_log_stream_profile(items).map(str::to_string);
    let now = Instant::now();
    if latest_profile != *header_profile || now >= *header_refresh_at {
        *header_profile = latest_profile;
        *header_detail = log_tui_header_detail(header_profile.as_deref());
        *header_refresh_at = log_tui_header_next_refresh_at(header_detail.as_ref(), now);
    }
}

fn log_stream_tui_should_quit(view: &mut LogTuiState) -> Result<bool> {
    if !event::poll(LOG_STREAM_POLL_INTERVAL).context("failed to poll log stream TUI input")? {
        return Ok(false);
    }
    let Event::Key(key) = event::read().context("failed to read log stream TUI input")? else {
        return Ok(false);
    };
    Ok(key.kind == KeyEventKind::Press && view.apply_key(key) == LogTuiInput::Quit)
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

    #[test]
    fn transient_log_discovery_and_read_errors_do_not_end_streaming() {
        let paths = stream_best_effort::<PathBuf>(Err(anyhow::anyhow!("transient discovery")));
        let events = stream_best_effort::<LogStreamItem>(Err(anyhow::anyhow!("transient read")));

        assert!(paths.is_empty());
        assert!(events.is_empty());
    }

    #[test]
    fn stream_tick_survives_temporarily_unreadable_session_root() {
        let _runtime_lock = crate::acquire_test_runtime_lock();
        let root = std::env::temp_dir().join(format!(
            "prodex-log-stream-transient-root-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let shared_file = root.join("shared-file");
        std::fs::write(&shared_file, "temporarily unavailable").unwrap();
        let _home = crate::TestEnvVarGuard::set("PRODEX_HOME", root.to_str().unwrap());
        let _shared =
            crate::TestEnvVarGuard::set("PRODEX_SHARED_CODEX_HOME", shared_file.to_str().unwrap());
        let _logs = crate::TestEnvVarGuard::set(
            "PRODEX_RUNTIME_LOG_DIR",
            root.join("logs").to_str().unwrap(),
        );

        assert!(initial_log_stream_items().is_ok());
        assert!(
            read_token_usage_events_tick(true, &mut BTreeMap::new(), &mut BTreeMap::new()).is_ok()
        );
        let _ = std::fs::remove_dir_all(root);
    }
}
