use super::collect_recent_runtime_log_paths;
use super::log_format::{
    current_log_width, local_log_timestamp, render_log_block, render_text_body,
};
use super::log_upstream::stream_upstream_payload_events;
use crate::{LogArgs, LogMode, prodex_runtime_log_paths_in_dir, runtime_proxy_log_dir};
use anyhow::{Context, Result};
use crossterm::cursor::{Hide, Show};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use prodex_app_reports::{InfoTokenUsageEvent, info_token_usage_event_from_line};
use prodex_core::AppPaths;
use prodex_runtime_doctor::read_runtime_log_tail;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use std::collections::{BTreeMap, VecDeque};
#[cfg(test)]
use std::env;
use std::fs;
use std::io::{self, IsTerminal, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::thread;
#[cfg(test)]
use std::time::SystemTime;
use std::time::{Duration, UNIX_EPOCH};
use terminal_ui::{tui_border_style, tui_primary_style, tui_secondary_style, tui_title_style};

const LOG_STREAM_POLL_INTERVAL: Duration = Duration::from_millis(250);
const LOG_SNAPSHOT_TAIL_BYTES: usize = 1024 * 1024;
const SESSION_SNAPSHOT_TAIL_BYTES: usize = 2 * 1024 * 1024;
const SESSION_FOLLOW_LIMIT: usize = 32;
const LOG_TUI_EVENT_LIMIT: usize = 200;

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

#[derive(Debug, Clone)]
enum LogStreamItem {
    Transcript(TranscriptEvent),
    TokenUsage(InfoTokenUsageEvent),
}

struct LogStreamTui {
    terminal: Terminal<CrosstermBackend<io::Stdout>>,
}

impl LogStreamTui {
    fn new() -> Result<Self> {
        enable_raw_mode().context("failed to enable log stream TUI raw mode")?;
        let mut stdout = io::stdout();
        if let Err(err) = crossterm::execute!(stdout, EnterAlternateScreen, Hide) {
            let _ = disable_raw_mode();
            return Err(err).context("failed to enter log stream TUI alternate screen");
        }
        let backend = CrosstermBackend::new(stdout);
        let terminal = match Terminal::new(backend) {
            Ok(terminal) => terminal,
            Err(err) => {
                let mut stdout = io::stdout();
                let _ = crossterm::execute!(stdout, Show, LeaveAlternateScreen);
                let _ = disable_raw_mode();
                return Err(err).context("failed to initialize log stream TUI terminal");
            }
        };
        Ok(Self { terminal })
    }
}

impl Drop for LogStreamTui {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = crossterm::execute!(self.terminal.backend_mut(), Show, LeaveAlternateScreen);
        let _ = self.terminal.show_cursor();
    }
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
            print_log_snapshot(transcript.as_ref(), token_usage.as_ref())
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

fn stream_token_usage_events_tui() -> Result<()> {
    let mut tui = LogStreamTui::new()?;
    let mut items = VecDeque::<LogStreamItem>::new();
    if let Some(event) = latest_transcript_event()? {
        push_log_stream_item(&mut items, LogStreamItem::Transcript(event));
    }
    if let Some(event) = latest_token_usage_event() {
        push_log_stream_item(&mut items, LogStreamItem::TokenUsage(event));
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
            for event in collect_new_token_usage_events(&path, state)? {
                push_log_stream_item(&mut items, LogStreamItem::TokenUsage(event));
            }
        }
        for path in recent_session_log_paths()? {
            let state = followed_session_logs.entry(path.clone()).or_default();
            for event in collect_new_transcript_events(&path, state)? {
                push_log_stream_item(&mut items, LogStreamItem::Transcript(event));
            }
        }

        tui.terminal
            .draw(|frame| render_log_stream_tui(frame, &items))
            .context("failed to draw log stream TUI")?;

        if event::poll(LOG_STREAM_POLL_INTERVAL).context("failed to poll log stream TUI input")? {
            match event::read().context("failed to read log stream TUI input")? {
                Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                    _ => {}
                },
                _ => {}
            }
        }
    }
}

fn print_log_snapshot(
    transcript: Option<&TranscriptEvent>,
    token_usage: Option<&InfoTokenUsageEvent>,
) -> Result<()> {
    if io::stdout().is_terminal()
        && let Some(mut terminal) =
            crate::try_inline_stdout_terminal(log_snapshot_tui_height(transcript, token_usage))
    {
        let items = log_snapshot_items(transcript, token_usage);
        terminal
            .draw(|frame| render_log_snapshot_tui(frame, &items))
            .context("failed to draw log snapshot TUI")?;
        let _ = terminal.show_cursor();
        return Ok(());
    }

    if transcript.is_none() && token_usage.is_none() {
        println!("No transcript or token usage events found.");
        return Ok(());
    }
    if let Some(event) = transcript {
        print_transcript_event(event)?;
    }
    if let Some(event) = token_usage {
        print_token_usage_event(event, false)?;
    }
    Ok(())
}

fn log_snapshot_items(
    transcript: Option<&TranscriptEvent>,
    token_usage: Option<&InfoTokenUsageEvent>,
) -> VecDeque<LogStreamItem> {
    let mut items = VecDeque::new();
    if let Some(event) = transcript {
        items.push_back(LogStreamItem::Transcript(event.clone()));
    }
    if let Some(event) = token_usage {
        items.push_back(LogStreamItem::TokenUsage(event.clone()));
    }
    items
}

fn log_snapshot_tui_height(
    transcript: Option<&TranscriptEvent>,
    token_usage: Option<&InfoTokenUsageEvent>,
) -> u16 {
    let body_rows = if transcript.is_none() && token_usage.is_none() {
        1
    } else {
        let transcript_rows = transcript
            .map(|event| render_text_body(&event.text, 100).len().saturating_add(2))
            .unwrap_or(0);
        let usage_rows = token_usage.map(|_| 3).unwrap_or(0);
        transcript_rows.saturating_add(usage_rows).saturating_add(1)
    };
    body_rows.saturating_add(4).clamp(8, 28) as u16
}

fn render_log_snapshot_tui(frame: &mut ratatui::Frame<'_>, items: &VecDeque<LogStreamItem>) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(1)])
        .split(frame.area());

    let header = Paragraph::new(Line::from(vec![
        Span::styled("Prodex Log Snapshot", log_title_style()),
        Span::raw("  "),
        Span::styled(format!("{} event(s)", items.len()), log_muted_style()),
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(log_border_style()),
    );
    frame.render_widget(header, chunks[0]);

    let body = Paragraph::new(log_stream_tui_text(
        items,
        usize::from(chunks[1].height),
        usize::from(chunks[1].width).saturating_sub(2).max(20),
    ))
    .block(
        Block::default()
            .borders(Borders::LEFT | Borders::RIGHT | Borders::BOTTOM)
            .border_style(log_border_style()),
    )
    .wrap(Wrap { trim: false });
    frame.render_widget(body, chunks[1]);
}

fn push_log_stream_item(items: &mut VecDeque<LogStreamItem>, item: LogStreamItem) {
    items.push_back(item);
    while items.len() > LOG_TUI_EVENT_LIMIT {
        items.pop_front();
    }
}

fn render_log_stream_tui(frame: &mut ratatui::Frame<'_>, items: &VecDeque<LogStreamItem>) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(3),
        ])
        .split(frame.area());

    let header = Paragraph::new(Line::from(vec![
        Span::styled("Prodex Log Stream", log_title_style()),
        Span::raw("  "),
        Span::styled(format!("{} event(s)", items.len()), log_muted_style()),
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(log_border_style()),
    );
    frame.render_widget(header, chunks[0]);

    let body = Paragraph::new(log_stream_tui_text(
        items,
        usize::from(chunks[1].height),
        usize::from(chunks[1].width).saturating_sub(2).max(20),
    ))
    .block(
        Block::default()
            .borders(Borders::LEFT | Borders::RIGHT)
            .border_style(log_border_style()),
    )
    .wrap(Wrap { trim: false });
    frame.render_widget(body, chunks[1]);

    let footer = Paragraph::new(Line::styled(
        "live transcript + token usage | q quit",
        log_footer_style(),
    ))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(log_border_style()),
    );
    frame.render_widget(footer, chunks[2]);
}

fn log_stream_tui_text(
    items: &VecDeque<LogStreamItem>,
    max_lines: usize,
    body_width: usize,
) -> Text<'_> {
    if items.is_empty() {
        return Text::from(Line::styled(
            "Waiting for transcript or token usage events...",
            log_muted_style(),
        ));
    }

    let mut lines = Vec::new();
    for item in items {
        if !lines.is_empty() {
            lines.push(Line::raw(""));
        }
        match item {
            LogStreamItem::Transcript(event) => {
                lines.push(Line::from(vec![
                    Span::styled(event.timestamp.as_str(), log_muted_style()),
                    Span::raw(" "),
                    Span::styled(
                        format!("stream {}", event.source),
                        Style::default()
                            .fg(log_stream_source_color(&event.source))
                            .add_modifier(Modifier::BOLD),
                    ),
                ]));
                for line in render_text_body(&event.text, body_width) {
                    lines.push(Line::styled(line, tui_primary_style()));
                }
            }
            LogStreamItem::TokenUsage(event) => {
                let request = event
                    .request
                    .map(|request| request.to_string())
                    .unwrap_or_else(|| "-".to_string());
                lines.push(Line::from(vec![
                    Span::styled(event.timestamp.as_str(), log_muted_style()),
                    Span::raw(" "),
                    Span::styled(
                        "stream usage",
                        Style::default()
                            .fg(Color::LightMagenta)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]));
                lines.push(Line::from(vec![
                    Span::styled("profile ", log_muted_style()),
                    Span::styled(event.profile.as_str(), tui_primary_style()),
                    Span::styled(" request ", log_muted_style()),
                    Span::styled(request, tui_primary_style()),
                    Span::styled(" transport ", log_muted_style()),
                    Span::styled(event.transport.as_str(), tui_primary_style()),
                    Span::styled(" source ", log_muted_style()),
                    Span::styled(event.source.as_str(), tui_primary_style()),
                ]));
                lines.push(Line::from(vec![
                    Span::styled("sent ", log_muted_style()),
                    Span::styled(
                        event.input_tokens.to_string(),
                        Style::default().fg(Color::Green),
                    ),
                    Span::styled(" cached ", log_muted_style()),
                    Span::styled(
                        event.cached_input_tokens.to_string(),
                        Style::default().fg(Color::LightCyan),
                    ),
                    Span::styled(" received ", log_muted_style()),
                    Span::styled(
                        event.output_tokens.to_string(),
                        Style::default().fg(Color::Green),
                    ),
                    Span::styled(" reasoning ", log_muted_style()),
                    Span::styled(
                        event.reasoning_tokens.to_string(),
                        Style::default().fg(Color::LightMagenta),
                    ),
                ]));
            }
        }
    }
    if lines.len() > max_lines {
        lines.drain(..lines.len().saturating_sub(max_lines));
    }
    Text::from(lines)
}

fn log_title_style() -> Style {
    tui_title_style()
}

fn log_border_style() -> Style {
    tui_border_style()
}

fn log_muted_style() -> Style {
    tui_secondary_style()
}

fn log_footer_style() -> Style {
    tui_title_style()
}

fn log_stream_source_color(source: &str) -> Color {
    match source {
        "user" => Color::Green,
        "assistant" => Color::Cyan,
        "tool-output" => Color::Magenta,
        source if source.starts_with("tool-call:") => Color::Magenta,
        _ => Color::Reset,
    }
}

fn read_new_token_usage_events(path: &Path, state: &mut FollowedLog, json: bool) -> Result<()> {
    for event in collect_new_token_usage_events(path, state)? {
        print_token_usage_event(&event, json)?;
    }
    Ok(())
}

fn collect_new_token_usage_events(
    path: &Path,
    state: &mut FollowedLog,
) -> Result<Vec<InfoTokenUsageEvent>> {
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
        if let Some(event) = info_token_usage_event_from_line(line) {
            events.push(local_token_usage_event(event));
        }
    }
    Ok(events)
}

fn read_new_transcript_events(path: &Path, state: &mut FollowedLog) -> Result<()> {
    for event in collect_new_transcript_events(path, state)? {
        print_transcript_event(&event)?;
    }
    Ok(())
}

fn collect_new_transcript_events(
    path: &Path,
    state: &mut FollowedLog,
) -> Result<Vec<TranscriptEvent>> {
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
        for event in transcript_events_from_session_line(line) {
            events.push(local_transcript_event(event));
        }
    }
    Ok(events)
}

fn print_token_usage_event(event: &InfoTokenUsageEvent, json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string(event)?);
    } else {
        let request = event
            .request
            .map(|request| request.to_string())
            .unwrap_or_else(|| "-".to_string());
        let meta = [
            ("profile", event.profile.clone()),
            ("request", request),
            ("transport", event.transport.clone()),
            ("source", event.source.clone()),
            ("sent", event.input_tokens.to_string()),
            ("cached", event.cached_input_tokens.to_string()),
            ("received", event.output_tokens.to_string()),
            ("reasoning", event.reasoning_tokens.to_string()),
        ];
        for line in render_log_block(
            &event.timestamp,
            "stream usage",
            &meta,
            &[],
            current_log_width(),
        ) {
            println!("{line}");
        }
    }
    io::stdout()
        .flush()
        .context("failed to flush token log output")
}

fn local_token_usage_event(mut event: InfoTokenUsageEvent) -> InfoTokenUsageEvent {
    event.timestamp = local_log_timestamp(&event.timestamp);
    event
}

fn print_transcript_event(event: &TranscriptEvent) -> Result<()> {
    let width = current_log_width();
    let body = render_text_body(&event.text, width);
    for line in render_log_block(
        &event.timestamp,
        &format!("stream {}", event.source),
        &[],
        &body,
        width,
    ) {
        println!("{line}");
    }
    io::stdout()
        .flush()
        .context("failed to flush transcript log output")
}

fn local_transcript_event(mut event: TranscriptEvent) -> TranscriptEvent {
    event.timestamp = local_log_timestamp(&event.timestamp);
    event
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
                let event = local_transcript_event(event);
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
    let timestamp = local_log_timestamp(&timestamp);
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
                timestamp: local_log_timestamp("2026-06-20T01:00:00Z"),
                source: "prompt-engineering".to_string(),
                text: "System prompt.".to_string(),
            }]
        );
        assert_eq!(
            transcript_events_from_session_line(user),
            vec![TranscriptEvent {
                timestamp: local_log_timestamp("2026-06-20T01:00:01Z"),
                source: "user".to_string(),
                text: "Hello model.".to_string(),
            }]
        );
        assert_eq!(
            transcript_events_from_session_line(assistant),
            vec![TranscriptEvent {
                timestamp: local_log_timestamp("2026-06-20T01:00:02Z"),
                source: "assistant".to_string(),
                text: "Hello user.".to_string(),
            }]
        );
        assert_eq!(
            transcript_events_from_session_line(tool),
            vec![TranscriptEvent {
                timestamp: local_log_timestamp("2026-06-20T01:00:03Z"),
                source: "tool-call:exec_command".to_string(),
                text: "{\"cmd\":\"pwd\"}".to_string(),
            }]
        );
    }

    #[test]
    fn log_snapshot_items_preserve_transcript_and_usage_order() {
        let transcript = TranscriptEvent {
            timestamp: "2026-06-20 08:00:01".to_string(),
            source: "assistant".to_string(),
            text: "Hello user.".to_string(),
        };
        let usage = InfoTokenUsageEvent {
            timestamp: "2026-06-20 08:00:02".to_string(),
            request: Some(7),
            transport: "http".to_string(),
            profile: "main".to_string(),
            source: "responses".to_string(),
            input_tokens: 12,
            cached_input_tokens: 3,
            output_tokens: 4,
            reasoning_tokens: 1,
        };

        let items = log_snapshot_items(Some(&transcript), Some(&usage));

        assert_eq!(items.len(), 2);
        assert!(matches!(items[0], LogStreamItem::Transcript(_)));
        assert!(matches!(items[1], LogStreamItem::TokenUsage(_)));
    }

    #[test]
    fn log_snapshot_tui_text_handles_empty_state() {
        let items = VecDeque::new();
        let text = log_stream_tui_text(&items, 10, 80);
        let rendered = text
            .lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");

        assert!(rendered.contains("Waiting for transcript or token usage events"));
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
