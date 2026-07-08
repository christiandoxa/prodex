use super::collect_recent_runtime_log_paths;
use super::log_format::{current_log_width, render_log_block};
use super::log_tui::{
    LOG_TUI_HEADER_REFRESH_INTERVAL, LogTuiInput, LogTuiState, contains_ignore_ascii_case,
    log_tui_header_detail, visible_text,
};
use super::log_upstream_payload::{
    UpstreamPayloadEvent, render_upstream_payload_lines, upstream_payload_event_from_runtime_line,
};
use crate::{prodex_runtime_log_paths_in_dir, runtime_proxy_log_dir};
use anyhow::{Context, Result};
use crossterm::cursor::{Hide, Show};
use crossterm::event::{self, Event, KeyEventKind};
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use prodex_runtime_doctor::read_runtime_log_tail;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use std::collections::{BTreeMap, VecDeque};
#[cfg(test)]
use std::env;
use std::fs;
use std::io::{self, IsTerminal, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};
#[cfg(test)]
use std::time::{SystemTime, UNIX_EPOCH};
use terminal_ui::{
    tui_border_style, tui_hint_style, tui_primary_style, tui_secondary_style, tui_success_style,
    tui_title_style,
};

const LOG_STREAM_POLL_INTERVAL: Duration = Duration::from_millis(250);
const LOG_SNAPSHOT_TAIL_BYTES: usize = 1024 * 1024;
const UPSTREAM_TUI_EVENT_LIMIT: usize = 100;

#[derive(Default)]
struct FollowedLog {
    offset: u64,
    pending: String,
}

struct UpstreamPayloadTui {
    terminal: Terminal<CrosstermBackend<io::Stdout>>,
}

impl UpstreamPayloadTui {
    fn new() -> Result<Self> {
        enable_raw_mode().context("failed to enable upstream payload TUI raw mode")?;
        let mut stdout = io::stdout();
        if let Err(err) = crossterm::execute!(stdout, EnterAlternateScreen, Hide) {
            let _ = disable_raw_mode();
            return Err(err).context("failed to enter upstream payload TUI alternate screen");
        }
        let backend = CrosstermBackend::new(stdout);
        let terminal = match Terminal::new(backend) {
            Ok(terminal) => terminal,
            Err(err) => {
                let mut stdout = io::stdout();
                let _ = crossterm::execute!(stdout, Show, LeaveAlternateScreen);
                let _ = disable_raw_mode();
                return Err(err).context("failed to initialize upstream payload TUI terminal");
            }
        };
        Ok(Self { terminal })
    }
}

impl Drop for UpstreamPayloadTui {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = crossterm::execute!(self.terminal.backend_mut(), Show, LeaveAlternateScreen);
        let _ = self.terminal.show_cursor();
    }
}

pub(super) fn stream_upstream_payload_events(json: bool) -> Result<()> {
    if !json && io::stdout().is_terminal() && io::stdin().is_terminal() {
        return stream_upstream_payload_events_tui();
    }

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
            for event in collect_new_upstream_payload_events(&path, state)? {
                print_upstream_payload_event(&event, json)?;
            }
        }
        thread::sleep(LOG_STREAM_POLL_INTERVAL);
    }
}

fn stream_upstream_payload_events_tui() -> Result<()> {
    let mut tui = UpstreamPayloadTui::new()?;
    let mut view = LogTuiState::default();
    let mut events = VecDeque::<UpstreamPayloadEvent>::new();
    if let Some(event) = latest_upstream_payload_event() {
        push_upstream_payload_event(&mut events, event);
    }
    let mut header_profile = latest_upstream_payload_profile(&events).map(str::to_string);
    let mut header_detail = log_tui_header_detail(header_profile.as_deref());
    let mut header_refresh_at = Instant::now() + LOG_TUI_HEADER_REFRESH_INTERVAL;

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
            for event in collect_new_upstream_payload_events(&path, state)? {
                push_upstream_payload_event(&mut events, event);
            }
        }
        let latest_profile = latest_upstream_payload_profile(&events).map(str::to_string);
        let now = Instant::now();
        if latest_profile != header_profile || now >= header_refresh_at {
            header_profile = latest_profile;
            header_detail = log_tui_header_detail(header_profile.as_deref());
            header_refresh_at = now + LOG_TUI_HEADER_REFRESH_INTERVAL;
        }
        tui.terminal.draw(|frame| {
            render_upstream_payload_tui(frame, &events, &view, header_detail.as_deref());
        })?;
        if event::poll(LOG_STREAM_POLL_INTERVAL)?
            && let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
            && view.apply_key(key) == LogTuiInput::Quit
        {
            return Ok(());
        }
    }
}

fn push_upstream_payload_event(
    events: &mut VecDeque<UpstreamPayloadEvent>,
    event: UpstreamPayloadEvent,
) {
    events.push_back(event);
    while events.len() > UPSTREAM_TUI_EVENT_LIMIT {
        events.pop_front();
    }
}

fn latest_upstream_payload_profile(events: &VecDeque<UpstreamPayloadEvent>) -> Option<&str> {
    events.back().map(|event| event.profile.as_str())
}

fn collect_new_upstream_payload_events(
    path: &Path,
    state: &mut FollowedLog,
) -> Result<Vec<UpstreamPayloadEvent>> {
    let mut file = match fs::File::open(path) {
        Ok(file) => file,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
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
        return Ok(Vec::new());
    }

    state.pending.push_str(&String::from_utf8_lossy(&bytes));
    let complete_len = state
        .pending
        .rfind('\n')
        .map(|index| index + 1)
        .unwrap_or_default();
    if complete_len == 0 {
        return Ok(Vec::new());
    }
    let complete = state.pending[..complete_len].to_string();
    state.pending.drain(..complete_len);
    let mut events = Vec::new();
    for line in complete.lines() {
        if let Some(event) = upstream_payload_event_from_runtime_line(line) {
            events.push(event);
        }
    }
    Ok(events)
}

fn print_upstream_payload_event(event: &UpstreamPayloadEvent, json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string(event)?);
    } else {
        let request = event
            .request
            .map(|request| request.to_string())
            .unwrap_or_else(|| "-".to_string());
        let width = current_log_width();
        let meta = [
            ("profile", event.profile.clone()),
            ("request", request),
            ("transport", event.transport.clone()),
            ("route", event.route.clone()),
            ("bytes", event.bytes.to_string()),
            ("logged", event.logged_bytes.to_string()),
            ("truncated", event.truncated.to_string()),
        ];
        let body = render_upstream_payload_lines(&event.payload, width);
        for line in render_log_block(&event.timestamp, "upstream payload", &meta, &body, width) {
            println!("{line}");
        }
    }
    io::stdout()
        .flush()
        .context("failed to flush upstream log output")
}

fn render_upstream_payload_tui(
    frame: &mut ratatui::Frame<'_>,
    events: &VecDeque<UpstreamPayloadEvent>,
    state: &LogTuiState,
    header_detail: Option<&str>,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(3),
        ])
        .split(frame.area());
    let matches = matching_upstream_payload_events(events, state.query()).len();
    let count = match state.query() {
        Some(_) => format!("{matches}/{} match(es)", events.len()),
        None => format!("{} event(s)", events.len()),
    };
    let mut header_spans = vec![
        Span::styled("Prodex Upstream Payloads", tui_title_style()),
        Span::raw("  "),
        Span::styled(count, tui_secondary_style()),
    ];
    if let Some(detail) = header_detail {
        header_spans.push(Span::raw("  "));
        header_spans.push(Span::styled(detail.to_string(), tui_primary_style()));
    }
    let header = Paragraph::new(Line::from(header_spans)).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(tui_border_style()),
    );
    frame.render_widget(header, chunks[0]);

    let width = chunks[1].width.saturating_sub(4).max(24) as usize;
    let body = Paragraph::new(upstream_payload_tui_text(
        events,
        width,
        usize::from(chunks[1].height),
        state.query(),
        state.scroll_from_bottom(),
    ))
    .block(
        Block::default()
            .borders(Borders::LEFT | Borders::RIGHT)
            .border_style(tui_border_style()),
    )
    .wrap(Wrap { trim: false });
    frame.render_widget(body, chunks[1]);

    let footer = Paragraph::new(Line::styled(
        state.footer_text("q quit esc close"),
        tui_title_style(),
    ))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(tui_border_style()),
    );
    frame.render_widget(footer, chunks[2]);
}

fn upstream_payload_tui_text(
    events: &VecDeque<UpstreamPayloadEvent>,
    width: usize,
    max_lines: usize,
    query: Option<&str>,
    scroll_from_bottom: usize,
) -> Text<'static> {
    if events.is_empty() {
        return Text::from(Line::from(Span::styled(
            "Waiting for processed upstream payload events...",
            tui_secondary_style(),
        )));
    }

    let matching = matching_upstream_payload_events(events, query);
    if matching.is_empty() {
        return Text::from(Line::from(Span::styled(
            "No upstream payload events match search.",
            tui_secondary_style(),
        )));
    }

    let mut lines = Vec::new();
    for (index, event) in matching.iter().enumerate() {
        if index > 0 {
            lines.push(Line::raw(""));
        }
        let request = event
            .request
            .map(|request| request.to_string())
            .unwrap_or_else(|| "-".to_string());
        lines.push(Line::from(vec![
            Span::styled(event.timestamp.clone(), tui_secondary_style()),
            Span::raw(" "),
            Span::styled("upstream payload", tui_title_style()),
        ]));
        lines.push(Line::from(vec![
            Span::styled("profile=", tui_secondary_style()),
            Span::styled(event.profile.clone(), tui_success_style()),
            Span::raw(" "),
            Span::styled("request=", tui_secondary_style()),
            Span::raw(request),
            Span::raw(" "),
            Span::styled("transport=", tui_secondary_style()),
            Span::raw(event.transport.clone()),
            Span::raw(" "),
            Span::styled("route=", tui_secondary_style()),
            Span::raw(event.route.clone()),
        ]));
        lines.push(Line::from(vec![
            Span::styled("bytes=", tui_secondary_style()),
            Span::raw(event.bytes.to_string()),
            Span::raw(" "),
            Span::styled("logged=", tui_secondary_style()),
            Span::raw(event.logged_bytes.to_string()),
            Span::raw(" "),
            Span::styled("truncated=", tui_secondary_style()),
            Span::styled(
                event.truncated.to_string(),
                if event.truncated {
                    tui_hint_style()
                } else {
                    tui_primary_style()
                },
            ),
        ]));
        for line in render_upstream_payload_lines(&event.payload, width) {
            lines.push(Line::raw(line));
        }
    }
    visible_text(lines, max_lines, scroll_from_bottom)
}

fn matching_upstream_payload_events<'a>(
    events: &'a VecDeque<UpstreamPayloadEvent>,
    query: Option<&str>,
) -> Vec<&'a UpstreamPayloadEvent> {
    events
        .iter()
        .filter(|event| query.is_none_or(|query| upstream_payload_event_matches(event, query)))
        .collect()
}

fn upstream_payload_event_matches(event: &UpstreamPayloadEvent, query: &str) -> bool {
    let request = event
        .request
        .map(|request| request.to_string())
        .unwrap_or_default();
    contains_ignore_ascii_case(
        &format!(
            "{} {} {} {} {} {} {} {} {}",
            event.timestamp,
            event.profile,
            request,
            event.transport,
            event.route,
            event.bytes,
            event.logged_bytes,
            event.truncated,
            event.payload
        ),
        query,
    )
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

#[cfg(test)]
mod tests {
    use super::{
        FollowedLog, SystemTime, UNIX_EPOCH, UpstreamPayloadEvent,
        collect_new_upstream_payload_events, env, fs, upstream_payload_tui_text,
    };
    use crate::app_commands::log_upstream_payload::BASE64_STANDARD;
    use base64::Engine;
    use ratatui::text::Text;
    use std::collections::VecDeque;
    use std::io::Write;

    fn rendered(text: Text<'static>) -> String {
        text.lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
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
        let events = collect_new_upstream_payload_events(&path, &mut state).unwrap();
        assert!(events.is_empty());
        assert!(!state.pending.is_empty());
        fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap()
            .write_all(b"\n")
            .unwrap();
        let events = collect_new_upstream_payload_events(&path, &mut state).unwrap();
        assert_eq!(events.len(), 1);
        assert!(state.pending.is_empty());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn upstream_tui_text_can_filter_and_scroll() {
        let events = VecDeque::from([
            UpstreamPayloadEvent {
                timestamp: "2026-07-06 10:00:00".to_string(),
                request: Some(1),
                transport: "http".to_string(),
                route: "responses".to_string(),
                profile: "main".to_string(),
                bytes: 5,
                logged_bytes: 5,
                truncated: false,
                payload: "alpha payload".to_string(),
            },
            UpstreamPayloadEvent {
                timestamp: "2026-07-06 10:00:01".to_string(),
                request: Some(2),
                transport: "websocket".to_string(),
                route: "websocket".to_string(),
                profile: "second".to_string(),
                bytes: 4,
                logged_bytes: 4,
                truncated: false,
                payload: "beta payload".to_string(),
            },
        ]);

        let filtered = rendered(upstream_payload_tui_text(&events, 80, 20, Some("beta"), 0));
        assert!(!filtered.contains("alpha payload"));
        assert!(filtered.contains("beta payload"));

        let scrolled = rendered(upstream_payload_tui_text(&events, 80, 4, None, usize::MAX));
        assert!(scrolled.contains("alpha payload"));
        assert!(!scrolled.contains("beta payload"));
    }
}
