use super::collect_recent_runtime_log_paths;
use super::log_format::{
    current_log_width, local_log_timestamp, render_log_block, render_text_body,
};
use super::log_upstream::stream_upstream_payload_events;
use super::log_upstream_payload::{
    UpstreamPayloadEvent, render_upstream_payload_lines, upstream_payload_event_from_runtime_line,
};
use crate::{LogArgs, LogMode, prodex_runtime_log_paths_in_dir, runtime_proxy_log_dir};
use anyhow::{Context, Result};
use crossterm::cursor::{Hide, Show};
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
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
use terminal_ui::{
    tui_accent_style, tui_border_style, tui_metric_style, tui_muted_style, tui_primary_style,
    tui_title_style, tui_tool_style,
};

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
pub(crate) struct TranscriptEvent {
    pub(crate) timestamp: String,
    pub(crate) source: String,
    pub(crate) text: String,
}

#[derive(Debug, Clone)]
enum LogStreamItem {
    Transcript(TranscriptEvent),
    TokenUsage(InfoTokenUsageEvent),
    UpstreamPayload(UpstreamPayloadEvent),
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
    let mut tui = LogStreamTui::new()?;
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

        tui.terminal
            .draw(|frame| render_log_stream_tui(frame, &items))
            .context("failed to draw log stream TUI")?;

        if event::poll(LOG_STREAM_POLL_INTERVAL).context("failed to poll log stream TUI input")? {
            match event::read().context("failed to read log stream TUI input")? {
                Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                    KeyCode::Char('c') | KeyCode::Char('z')
                        if key.modifiers.contains(KeyModifiers::CONTROL) =>
                    {
                        return Ok(());
                    }
                    _ => {}
                },
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
        && let Some(mut terminal) = crate::try_inline_stdout_terminal(log_snapshot_tui_height(
            transcript,
            upstream_payload,
            token_usage,
        ))
    {
        let items = log_snapshot_items(transcript, upstream_payload, token_usage);
        terminal
            .draw(|frame| render_log_snapshot_tui(frame, &items))
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

fn log_snapshot_items(
    transcript: Option<&TranscriptEvent>,
    upstream_payload: Option<&UpstreamPayloadEvent>,
    token_usage: Option<&InfoTokenUsageEvent>,
) -> VecDeque<LogStreamItem> {
    let mut items = VecDeque::new();
    if let Some(event) = transcript {
        items.push_back(LogStreamItem::Transcript(event.clone()));
    }
    if let Some(event) = upstream_payload {
        items.push_back(LogStreamItem::UpstreamPayload(event.clone()));
    }
    if let Some(event) = token_usage {
        items.push_back(LogStreamItem::TokenUsage(event.clone()));
    }
    items
}

fn log_snapshot_tui_height(
    transcript: Option<&TranscriptEvent>,
    upstream_payload: Option<&UpstreamPayloadEvent>,
    token_usage: Option<&InfoTokenUsageEvent>,
) -> u16 {
    let body_rows = if transcript.is_none() && upstream_payload.is_none() && token_usage.is_none() {
        1
    } else {
        let transcript_rows = transcript
            .map(|event| render_text_body(&event.text, 100).len().saturating_add(2))
            .unwrap_or(0);
        let upstream_rows = upstream_payload
            .map(|event| {
                render_upstream_payload_lines(&event.payload, 100)
                    .len()
                    .saturating_add(3)
            })
            .unwrap_or(0);
        let usage_rows = token_usage.map(|_| 3).unwrap_or(0);
        transcript_rows
            .saturating_add(upstream_rows)
            .saturating_add(usage_rows)
            .saturating_add(1)
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
                    Span::styled(event.input_tokens.to_string(), tui_metric_style()),
                    Span::styled(" cached ", log_muted_style()),
                    Span::styled(event.cached_input_tokens.to_string(), tui_accent_style()),
                    Span::styled(" received ", log_muted_style()),
                    Span::styled(event.output_tokens.to_string(), tui_metric_style()),
                    Span::styled(" reasoning ", log_muted_style()),
                    Span::styled(event.reasoning_tokens.to_string(), tui_tool_style()),
                ]));
            }
            LogStreamItem::UpstreamPayload(event) => {
                let request = event
                    .request
                    .map(|request| request.to_string())
                    .unwrap_or_else(|| "-".to_string());
                lines.push(Line::from(vec![
                    Span::styled(event.timestamp.as_str(), log_muted_style()),
                    Span::raw(" "),
                    Span::styled(
                        "stream payload",
                        Style::default()
                            .fg(Color::Yellow)
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
                    Span::styled(" route ", log_muted_style()),
                    Span::styled(event.route.as_str(), tui_primary_style()),
                ]));
                for line in render_upstream_payload_lines(&event.payload, body_width) {
                    lines.push(Line::styled(line, tui_primary_style()));
                }
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
    tui_muted_style()
}

fn log_footer_style() -> Style {
    tui_title_style()
}

fn log_stream_source_color(source: &str) -> Color {
    match source {
        "user" => Color::Green,
        "assistant" => Color::Cyan,
        "reasoning" => Color::Yellow,
        "turn-context" | "session-context" => Color::Blue,
        "prompt-engineering" => Color::LightBlue,
        "tool-output" => Color::Magenta,
        source if source.starts_with("tool-call:") => Color::Magenta,
        _ => Color::Reset,
    }
}

fn read_new_runtime_log_events(path: &Path, state: &mut FollowedLog, json: bool) -> Result<()> {
    for event in collect_new_runtime_log_stream_items(path, state)? {
        match event {
            LogStreamItem::TokenUsage(event) => print_token_usage_event(&event, json)?,
            LogStreamItem::UpstreamPayload(event) if !json => print_upstream_payload_event(&event)?,
            LogStreamItem::Transcript(_) => {}
            LogStreamItem::UpstreamPayload(_) => {}
        }
    }
    Ok(())
}

#[cfg(test)]
fn read_new_token_usage_events(path: &Path, state: &mut FollowedLog, json: bool) -> Result<()> {
    for event in collect_new_runtime_log_stream_items(path, state)? {
        if let LogStreamItem::TokenUsage(event) = event {
            print_token_usage_event(&event, json)?;
        }
    }
    Ok(())
}

fn collect_new_runtime_log_stream_items(
    path: &Path,
    state: &mut FollowedLog,
) -> Result<Vec<LogStreamItem>> {
    let mut items = Vec::new();
    for line in collect_new_followed_lines(path, state)? {
        if let Some(event) = upstream_payload_event_from_runtime_line(&line) {
            items.push(LogStreamItem::UpstreamPayload(event));
        }
        if let Some(event) = info_token_usage_event_from_line(&line) {
            items.push(LogStreamItem::TokenUsage(local_token_usage_event(event)));
        }
    }
    Ok(items)
}

fn collect_new_followed_lines(path: &Path, state: &mut FollowedLog) -> Result<Vec<String>> {
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
    Ok(complete.lines().map(str::to_string).collect())
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
    let mut events = Vec::new();
    for line in collect_new_followed_lines(path, state)? {
        for event in transcript_events_from_session_line(&line) {
            let event = local_transcript_event(event);
            if events.last().is_some_and(|last: &TranscriptEvent| {
                last.source == event.source && last.text == event.text
            }) {
                continue;
            }
            events.push(event);
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

fn print_upstream_payload_event(event: &UpstreamPayloadEvent) -> Result<()> {
    let width = current_log_width();
    let meta = [
        ("profile", event.profile.clone()),
        (
            "request",
            event
                .request
                .map(|request| request.to_string())
                .unwrap_or_else(|| "-".to_string()),
        ),
        ("transport", event.transport.clone()),
        ("route", event.route.clone()),
        ("bytes", event.bytes.to_string()),
        ("logged", event.logged_bytes.to_string()),
    ];
    let body = render_upstream_payload_lines(&event.payload, width);
    for line in render_log_block(&event.timestamp, "stream payload", &meta, &body, width) {
        println!("{line}");
    }
    io::stdout()
        .flush()
        .context("failed to flush upstream payload log output")
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

pub(crate) fn transcript_events_from_session_line(line: &str) -> Vec<TranscriptEvent> {
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
        "event_msg" => event_msg_transcript_event(timestamp, payload)
            .into_iter()
            .collect(),
        "session_meta" => session_meta_transcript_events(timestamp, payload),
        "turn_context" => turn_context_transcript_event(timestamp, payload)
            .into_iter()
            .collect(),
        "response_item" => response_item_transcript_event(timestamp, payload)
            .into_iter()
            .collect(),
        _ => Vec::new(),
    }
}

fn session_meta_transcript_events(
    timestamp: String,
    payload: &serde_json::Value,
) -> Vec<TranscriptEvent> {
    let mut events = Vec::new();
    if let Some(text) = payload
        .get("base_instructions")
        .and_then(|base| base.get("text"))
        .and_then(serde_json::Value::as_str)
        .filter(|text| !text.trim().is_empty())
    {
        events.push(TranscriptEvent {
            timestamp: timestamp.clone(),
            source: "prompt-engineering".to_string(),
            text: text.to_string(),
        });
    }

    let mut fields = Vec::new();
    if let Some(provider) = payload
        .get("model_provider")
        .or_else(|| payload.get("provider"))
        .and_then(serde_json::Value::as_str)
    {
        fields.push(format!("provider={provider}"));
    }
    if let Some(source) = payload.get("source").and_then(serde_json::Value::as_str) {
        fields.push(format!("source={source}"));
    }
    if let Some(originator) = payload
        .get("originator")
        .and_then(serde_json::Value::as_str)
    {
        fields.push(format!("originator={originator}"));
    }
    if let Some(cwd) = payload.get("cwd").and_then(serde_json::Value::as_str) {
        fields.push(format!("cwd={cwd}"));
    }
    if !fields.is_empty() {
        events.push(TranscriptEvent {
            timestamp,
            source: "session-context".to_string(),
            text: fields.join(" "),
        });
    }
    events
}

fn event_msg_transcript_event(
    timestamp: String,
    payload: &serde_json::Value,
) -> Option<TranscriptEvent> {
    let event_type = payload.get("type").and_then(serde_json::Value::as_str)?;
    let text_field = match event_type {
        "agent_reasoning" => "text",
        _ => "message",
    };
    let text = payload
        .get(text_field)
        .and_then(serde_json::Value::as_str)
        .filter(|text| !text.trim().is_empty())?;
    let source = match event_type {
        "user_message" => "user",
        "agent_message" => "assistant",
        "agent_reasoning" => "reasoning",
        _ => return None,
    };
    Some(TranscriptEvent {
        timestamp,
        source: source.to_string(),
        text: text.to_string(),
    })
}

fn turn_context_transcript_event(
    timestamp: String,
    payload: &serde_json::Value,
) -> Option<TranscriptEvent> {
    let mut fields = Vec::new();
    if let Some(model) = payload.get("model").and_then(serde_json::Value::as_str) {
        fields.push(format!("model={model}"));
    }
    if let Some(effort) = payload.get("effort").and_then(serde_json::Value::as_str) {
        fields.push(format!("effort={effort}"));
    }
    if let Some(summary) = payload.get("summary").and_then(serde_json::Value::as_str) {
        fields.push(format!("summary={summary}"));
    }
    if let Some(approval) = payload
        .get("approval_policy")
        .and_then(serde_json::Value::as_str)
    {
        fields.push(format!("approval={approval}"));
    }
    if let Some(cwd) = payload.get("cwd").and_then(serde_json::Value::as_str) {
        fields.push(format!("cwd={cwd}"));
    }
    (!fields.is_empty()).then(|| TranscriptEvent {
        timestamp,
        source: "turn-context".to_string(),
        text: fields.join(" "),
    })
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
        "custom_tool_call" => {
            let name = payload
                .get("name")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("custom-tool");
            let input = payload
                .get("input")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("");
            Some(TranscriptEvent {
                timestamp,
                source: format!("tool-call:{name}"),
                text: input.to_string(),
            })
        }
        "custom_tool_call_output" => {
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
        "reasoning" => transcript_text_from_reasoning(payload).map(|text| TranscriptEvent {
            timestamp,
            source: "reasoning".to_string(),
            text,
        }),
        _ => None,
    }
    .filter(|event| !event.text.trim().is_empty())
}

fn transcript_text_from_reasoning(payload: &serde_json::Value) -> Option<String> {
    let summary = payload.get("summary")?;
    let parts = match summary {
        serde_json::Value::Array(items) => items
            .iter()
            .filter_map(reasoning_summary_text)
            .collect::<Vec<_>>(),
        _ => reasoning_summary_text(summary).into_iter().collect(),
    };
    (!parts.is_empty()).then(|| parts.join("\n"))
}

fn reasoning_summary_text(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(text) if !text.trim().is_empty() => Some(text.to_string()),
        serde_json::Value::Object(_) => value
            .get("text")
            .or_else(|| value.get("summary_text"))
            .and_then(serde_json::Value::as_str)
            .filter(|text| !text.trim().is_empty())
            .map(str::to_string),
        _ => None,
    }
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
        let meta = r#"{"timestamp":"2026-06-20T01:00:00Z","type":"session_meta","payload":{"base_instructions":{"text":"System prompt."},"model_provider":"openai","source":"cli","originator":"codex-tui","cwd":"/repo"}}"#;
        let user = r#"{"timestamp":"2026-06-20T01:00:01Z","type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"Hello model."}]}}"#;
        let assistant = r#"{"timestamp":"2026-06-20T01:00:02Z","type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"Hello user."}]}}"#;
        let tool = r#"{"timestamp":"2026-06-20T01:00:03Z","type":"response_item","payload":{"type":"function_call","name":"exec_command","arguments":"{\"cmd\":\"pwd\"}"}}"#;

        assert_eq!(
            transcript_events_from_session_line(meta),
            vec![
                TranscriptEvent {
                    timestamp: local_log_timestamp("2026-06-20T01:00:00Z"),
                    source: "prompt-engineering".to_string(),
                    text: "System prompt.".to_string(),
                },
                TranscriptEvent {
                    timestamp: local_log_timestamp("2026-06-20T01:00:00Z"),
                    source: "session-context".to_string(),
                    text: "provider=openai source=cli originator=codex-tui cwd=/repo".to_string(),
                }
            ]
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
    fn parses_turn_context_reasoning_and_custom_tool_events() {
        let turn_context = r#"{"timestamp":"2026-01-10T11:52:46.163Z","type":"turn_context","payload":{"cwd":"/repo","approval_policy":"on-request","model":"gpt-5.2-codex","effort":"medium","summary":"auto"}}"#;
        let reasoning = r#"{"timestamp":"2026-01-10T11:53:34.029Z","type":"response_item","payload":{"type":"reasoning","summary":[{"type":"summary_text","text":"**Planning server bill synchronization**"}]}}"#;
        let event_reasoning = r#"{"timestamp":"2026-01-10T11:53:34.029Z","type":"event_msg","payload":{"type":"agent_reasoning","text":"**Planning server bill synchronization**"}}"#;
        let custom_tool = r#"{"timestamp":"2026-01-10T11:53:51.257Z","type":"response_item","payload":{"type":"custom_tool_call","name":"apply_patch","input":"*** Begin Patch"}}"#;
        let custom_tool_output = r#"{"timestamp":"2026-01-10T11:53:51.287Z","type":"response_item","payload":{"type":"custom_tool_call_output","output":"{\"output\":\"Success\"}"}}"#;

        assert_eq!(
            transcript_events_from_session_line(turn_context),
            vec![TranscriptEvent {
                timestamp: local_log_timestamp("2026-01-10T11:52:46.163Z"),
                source: "turn-context".to_string(),
                text:
                    "model=gpt-5.2-codex effort=medium summary=auto approval=on-request cwd=/repo"
                        .to_string(),
            }]
        );
        assert_eq!(
            transcript_events_from_session_line(reasoning),
            vec![TranscriptEvent {
                timestamp: local_log_timestamp("2026-01-10T11:53:34.029Z"),
                source: "reasoning".to_string(),
                text: "**Planning server bill synchronization**".to_string(),
            }]
        );
        assert_eq!(
            transcript_events_from_session_line(event_reasoning),
            vec![TranscriptEvent {
                timestamp: local_log_timestamp("2026-01-10T11:53:34.029Z"),
                source: "reasoning".to_string(),
                text: "**Planning server bill synchronization**".to_string(),
            }]
        );
        assert_eq!(
            transcript_events_from_session_line(custom_tool),
            vec![TranscriptEvent {
                timestamp: local_log_timestamp("2026-01-10T11:53:51.257Z"),
                source: "tool-call:apply_patch".to_string(),
                text: "*** Begin Patch".to_string(),
            }]
        );
        assert_eq!(
            transcript_events_from_session_line(custom_tool_output),
            vec![TranscriptEvent {
                timestamp: local_log_timestamp("2026-01-10T11:53:51.287Z"),
                source: "tool-output".to_string(),
                text: "{\"output\":\"Success\"}".to_string(),
            }]
        );
    }

    #[test]
    fn preserves_real_provider_and_effort_values() {
        for provider in [
            "openai",
            "prodex-gemini",
            "prodex-deepseek",
            "prodex-local",
            "prodex-test",
            "mock",
        ] {
            let session_meta = format!(
                "{{\"timestamp\":\"2026-07-01T13:00:00Z\",\"type\":\"session_meta\",\"payload\":{{\"model_provider\":\"{provider}\",\"source\":\"cli\",\"cwd\":\"/repo\"}}}}"
            );
            assert_eq!(
                transcript_events_from_session_line(&session_meta),
                vec![TranscriptEvent {
                    timestamp: local_log_timestamp("2026-07-01T13:00:00Z"),
                    source: "session-context".to_string(),
                    text: format!("provider={provider} source=cli cwd=/repo"),
                }]
            );
        }

        for effort in ["medium", "high", "xhigh"] {
            for model in [
                "auto",
                "deepseek-v4-pro",
                "gemini-2.5-flash",
                "gemini-2.5-pro",
                "gemini-3-pro-preview",
                "gemini-3.1-pro-preview",
                "gpt-5.2-codex",
                "gpt-5.3-codex",
                "gpt-5.4",
                "gpt-5.4-mini",
                "gpt-5.5",
                "mock-model",
                "pro",
                "unsloth/qwen3.5-35b-a3b",
            ] {
                let turn_context = format!(
                    "{{\"timestamp\":\"2026-07-01T13:00:01Z\",\"type\":\"turn_context\",\"payload\":{{\"model\":\"{model}\",\"effort\":\"{effort}\",\"summary\":\"auto\",\"approval_policy\":\"never\",\"cwd\":\"/repo\"}}}}"
                );
                assert_eq!(
                    transcript_events_from_session_line(&turn_context),
                    vec![TranscriptEvent {
                        timestamp: local_log_timestamp("2026-07-01T13:00:01Z"),
                        source: "turn-context".to_string(),
                        text: format!(
                            "model={model} effort={effort} summary=auto approval=never cwd=/repo"
                        ),
                    }]
                );
            }
        }
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

        let items = log_snapshot_items(Some(&transcript), None, Some(&usage));

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

        assert!(rendered.contains("Waiting for transcript"));
    }

    #[test]
    fn parses_event_msg_transcript_text_events() {
        let user = r#"{"timestamp":"2026-07-01T13:40:05.000Z","type":"event_msg","payload":{"type":"user_message","message":"hello from user"}}"#;
        let assistant = r#"{"timestamp":"2026-07-01T13:10:32.292Z","type":"event_msg","payload":{"type":"agent_message","message":"hello from assistant","phase":"commentary"}}"#;

        assert_eq!(
            transcript_events_from_session_line(user),
            vec![TranscriptEvent {
                timestamp: local_log_timestamp("2026-07-01T13:40:05.000Z"),
                source: "user".to_string(),
                text: "hello from user".to_string(),
            }]
        );
        assert_eq!(
            transcript_events_from_session_line(assistant),
            vec![TranscriptEvent {
                timestamp: local_log_timestamp("2026-07-01T13:10:32.292Z"),
                source: "assistant".to_string(),
                text: "hello from assistant".to_string(),
            }]
        );
    }

    #[test]
    fn collects_websocket_payload_and_usage_from_runtime_log() {
        let root = env::temp_dir().join(format!(
            "prodex-runtime-payload-follow-{}-{}",
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
            concat!(
                "[2026-07-01 21:52:36.700 +07:00] upstream_payload request=28 transport=websocket route=websocket profile=main bytes=35 logged_bytes=35 truncated=false payload_b64=eyJpbnB1dCI6ImhlbGxvIn0=\n",
                "[2026-07-01 21:52:36.729 +07:00] token_usage request=28 route=websocket transport=websocket profile=main source=responses_websocket prompt_cache_key=present prompt_cache_key_hash=sc:abc prompt_cache_owner=no_cached_tokens input_tokens=9741 uncached_input_tokens=9741 cached_input_tokens=0 output_tokens=0 reasoning_tokens=0\n"
            ),
        )
        .unwrap();

        let items =
            collect_new_runtime_log_stream_items(&path, &mut FollowedLog::default()).unwrap();

        assert_eq!(items.len(), 2);
        assert!(matches!(items[0], LogStreamItem::UpstreamPayload(_)));
        assert!(matches!(items[1], LogStreamItem::TokenUsage(_)));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn dedupes_consecutive_equivalent_transcript_events() {
        let root = env::temp_dir().join(format!(
            "prodex-transcript-dedupe-{}-{}",
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
            concat!(
                "{\"timestamp\":\"2026-07-01T13:08:43.923Z\",\"type\":\"response_item\",\"payload\":{\"type\":\"message\",\"role\":\"user\",\"content\":[{\"type\":\"input_text\",\"text\":\"same text\"}]}}\n",
                "{\"timestamp\":\"2026-07-01T13:08:43.923Z\",\"type\":\"event_msg\",\"payload\":{\"type\":\"user_message\",\"message\":\"same text\"}}\n"
            ),
        )
        .unwrap();

        let events = collect_new_transcript_events(&path, &mut FollowedLog::default()).unwrap();

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].source, "user");
        assert_eq!(events[0].text, "same text");
        fs::remove_dir_all(root).unwrap();
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
