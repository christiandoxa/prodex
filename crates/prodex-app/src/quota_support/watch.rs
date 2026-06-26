use super::*;
use crossterm::cursor::{Hide, Show};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph};
use std::io::IsTerminal;

#[derive(Debug, Clone)]
enum AllQuotaWatchSnapshot {
    Loading {
        updated: String,
    },
    Reports {
        updated: String,
        profile_count: usize,
        reports: Vec<QuotaReport>,
    },
    Empty {
        updated: String,
    },
    Error {
        updated: String,
        message: String,
    },
}

#[derive(Clone, Copy)]
struct AllQuotaWatchLayout {
    detail: bool,
    scroll_offset: usize,
    sort: QuotaReportSort,
    provider_filter: QuotaProviderFilter,
    provider_filter_locked: bool,
    total_width: usize,
    max_lines: Option<usize>,
}

const QUOTA_WATCH_INPUT_POLL_MS: u64 = 100;
enum QuotaWatchCommand {
    Up,
    Down,
    Sort,
    Filter,
    Quit,
}
enum QuotaWatchCommandOutcome {
    Continue(usize),
    Sort,
    Filter,
    Quit,
}
struct AllQuotaWatchRefresh {
    receiver: Receiver<AllQuotaWatchSnapshot>,
    sender: mpsc::Sender<AllQuotaWatchSnapshot>,
    in_flight: bool,
}
impl AllQuotaWatchRefresh {
    fn new() -> Self {
        let (sender, receiver) = mpsc::channel();
        Self {
            receiver,
            sender,
            in_flight: false,
        }
    }

    fn try_start<F>(&mut self, load: F) -> bool
    where
        F: FnOnce() -> AllQuotaWatchSnapshot + Send + 'static,
    {
        if self.in_flight {
            return false;
        }

        self.in_flight = true;
        let sender = self.sender.clone();
        thread::spawn(move || {
            let _ = sender.send(load());
        });
        true
    }

    fn take_latest(&mut self) -> Option<AllQuotaWatchSnapshot> {
        let mut latest = None;
        loop {
            match self.receiver.try_recv() {
                Ok(snapshot) => {
                    self.in_flight = false;
                    latest = Some(snapshot);
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.in_flight = false;
                    break;
                }
            }
        }
        latest
    }
}

struct QuotaWatchTui {
    terminal: Terminal<CrosstermBackend<io::Stdout>>,
}

impl QuotaWatchTui {
    fn new() -> Result<Self> {
        enable_raw_mode().context("failed to enable quota TUI raw mode")?;
        let mut stdout = io::stdout();
        if let Err(err) = crossterm::execute!(stdout, EnterAlternateScreen, Hide) {
            let _ = disable_raw_mode();
            return Err(err).context("failed to enter quota TUI alternate screen");
        }
        let backend = CrosstermBackend::new(stdout);
        let terminal = match Terminal::new(backend) {
            Ok(terminal) => terminal,
            Err(err) => {
                let mut stdout = io::stdout();
                let _ = crossterm::execute!(stdout, Show, LeaveAlternateScreen);
                let _ = disable_raw_mode();
                return Err(err).context("failed to initialize quota TUI terminal");
            }
        };
        Ok(Self { terminal })
    }
}

impl Drop for QuotaWatchTui {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = crossterm::execute!(self.terminal.backend_mut(), Show, LeaveAlternateScreen);
        let _ = self.terminal.show_cursor();
    }
}

pub(crate) fn quota_watch_enabled(args: &QuotaArgs) -> bool {
    !args.raw && !args.once
}

pub(crate) fn render_profile_quota_watch_output(
    profile_name: &str,
    _updated: &str,
    quota_result: std::result::Result<ProviderQuotaSnapshot, String>,
) -> String {
    match quota_result {
        Ok(quota) => render_profile_quota_snapshot(profile_name, &quota),
        Err(err) => render_quota_watch_error_panel(&format!("Quota {profile_name}"), &err),
    }
}

#[cfg(test)]
pub(crate) fn render_all_quota_watch_output(
    updated: &str,
    state_result: std::result::Result<AppState, String>,
    base_url: Option<&str>,
    detail: bool,
) -> String {
    render_all_quota_watch_snapshot(
        &collect_all_quota_watch_snapshot(
            updated,
            state_result,
            base_url,
            &QuotaAuthFilter::All,
            QuotaProviderFilter::All,
        ),
        detail,
        0,
        QuotaReportSort::Current,
        QuotaProviderFilter::All,
        false,
    )
}

fn collect_all_quota_watch_snapshot(
    updated: &str,
    state_result: std::result::Result<AppState, String>,
    base_url: Option<&str>,
    auth_filter: &QuotaAuthFilter,
    provider_filter: QuotaProviderFilter,
) -> AllQuotaWatchSnapshot {
    match state_result {
        Ok(state) if !state.profiles.is_empty() => AllQuotaWatchSnapshot::Reports {
            updated: updated.to_string(),
            profile_count: state.profiles.len(),
            reports: collect_quota_reports_with_filters(
                &state,
                base_url,
                auth_filter,
                provider_filter,
            ),
        },
        Ok(_) => AllQuotaWatchSnapshot::Empty {
            updated: updated.to_string(),
        },
        Err(err) => AllQuotaWatchSnapshot::Error {
            updated: updated.to_string(),
            message: err,
        },
    }
}

fn render_all_quota_watch_snapshot(
    snapshot: &AllQuotaWatchSnapshot,
    detail: bool,
    scroll_offset: usize,
    sort: QuotaReportSort,
    provider_filter: QuotaProviderFilter,
    provider_filter_locked: bool,
) -> String {
    match snapshot {
        AllQuotaWatchSnapshot::Loading { updated: _updated } => {
            render_quota_watch_error_panel("Quota", "Loading quota data...")
        }
        AllQuotaWatchSnapshot::Reports {
            updated: _updated,
            profile_count: _profile_count,
            reports,
        } => render_all_quota_watch_report_output(
            reports,
            detail,
            scroll_offset,
            sort,
            provider_filter,
            provider_filter_locked,
        ),
        AllQuotaWatchSnapshot::Empty { updated: _updated } => {
            render_quota_watch_error_panel("Quota", "No profiles configured")
        }
        AllQuotaWatchSnapshot::Error {
            updated: _updated,
            message,
        } => render_quota_watch_error_panel("Quota", message),
    }
}

fn render_all_quota_watch_snapshot_with_layout(
    snapshot: &AllQuotaWatchSnapshot,
    layout: AllQuotaWatchLayout,
) -> String {
    match snapshot {
        AllQuotaWatchSnapshot::Loading { updated: _updated } => {
            render_quota_watch_error_panel("Quota", "Loading quota data...")
        }
        AllQuotaWatchSnapshot::Reports {
            updated: _updated,
            profile_count: _profile_count,
            reports,
        } => render_all_quota_watch_report_output_with_layout(reports, layout),
        AllQuotaWatchSnapshot::Empty { updated: _updated } => {
            render_quota_watch_error_panel("Quota", "No profiles configured")
        }
        AllQuotaWatchSnapshot::Error {
            updated: _updated,
            message,
        } => render_quota_watch_error_panel("Quota", message),
    }
}

fn quota_watch_snapshot_updated(snapshot: &AllQuotaWatchSnapshot) -> &str {
    match snapshot {
        AllQuotaWatchSnapshot::Loading { updated }
        | AllQuotaWatchSnapshot::Reports { updated, .. }
        | AllQuotaWatchSnapshot::Empty { updated }
        | AllQuotaWatchSnapshot::Error { updated, .. } => updated,
    }
}

fn render_all_quota_watch_report_output(
    reports: &[QuotaReport],
    detail: bool,
    scroll_offset: usize,
    sort: QuotaReportSort,
    provider_filter: QuotaProviderFilter,
    provider_filter_locked: bool,
) -> String {
    render_all_quota_watch_report_output_with_layout(
        reports,
        AllQuotaWatchLayout {
            detail,
            scroll_offset,
            sort,
            provider_filter,
            provider_filter_locked,
            total_width: current_cli_width(),
            max_lines: quota_watch_available_report_lines(""),
        },
    )
}

fn render_all_quota_watch_report_output_with_layout(
    reports: &[QuotaReport],
    layout: AllQuotaWatchLayout,
) -> String {
    let filtered_reports = filter_quota_reports_by_provider(reports, layout.provider_filter);
    let window = render_quota_reports_window_with_sort(
        &filtered_reports,
        layout.detail,
        layout.max_lines,
        layout.total_width,
        layout.scroll_offset,
        true,
        layout.sort,
    );

    let mut output = quota_watch_without_interactive_scroll_notice(&window.output);
    output.push_str("\n\n");
    let provider_hint = if layout.provider_filter_locked {
        "provider fixed"
    } else {
        "f provider"
    };
    let range = quota_watch_scroll_range(&window)
        .map(|range| format!(" | {range}"))
        .unwrap_or_default();
    output.push_str(&format!(
        "sort: {} | filter: {}{} | s sort | {} | j/k/Up/Down scroll | q quit",
        layout.sort.label(),
        layout.provider_filter.label(),
        range,
        provider_hint
    ));
    output
}

fn quota_watch_without_interactive_scroll_notice(output: &str) -> String {
    let mut lines = output.lines().map(str::to_string).collect::<Vec<_>>();
    if lines
        .last()
        .is_some_and(|line| line.starts_with("press Up/Down to scroll profiles "))
    {
        lines.pop();
        if lines.last().is_some_and(|line| line.is_empty()) {
            lines.pop();
        }
    }
    lines.join("\n")
}

fn quota_watch_scroll_range(window: &RenderedQuotaReportWindow) -> Option<String> {
    if window.total_profiles == 0
        || window.shown_profiles == 0
        || (window.hidden_before == 0 && window.hidden_after == 0)
    {
        return None;
    }
    let first_visible = window.start_profile.saturating_add(1);
    let last_visible = window.start_profile.saturating_add(window.shown_profiles);
    Some(format!(
        "{first_visible}-{last_visible}/{}; {} above, {} below",
        window.total_profiles, window.hidden_before, window.hidden_after
    ))
}

fn filter_quota_reports_by_provider(
    reports: &[QuotaReport],
    provider_filter: QuotaProviderFilter,
) -> Vec<QuotaReport> {
    reports
        .iter()
        .filter(|report| provider_filter.matches_report(report))
        .cloned()
        .collect()
}

fn print_quota_watch_plain_snapshot(output: &str) -> Result<()> {
    print!("{output}\n\n");
    io::stdout()
        .flush()
        .context("failed to flush quota watch output")?;
    Ok(())
}

fn quota_watch_available_report_lines(header: &str) -> Option<usize> {
    let terminal_height = terminal_height_lines()?;
    let reserved = header.lines().count().saturating_add(2);
    Some(terminal_height.saturating_sub(reserved))
}

pub(crate) fn watch_quota(
    profile_name: &str,
    provider: &ProfileProvider,
    codex_home: &Path,
    base_url: Option<&str>,
) -> Result<()> {
    if io::stdout().is_terminal() && io::stdin().is_terminal() {
        match watch_profile_quota_tui(profile_name, provider, codex_home, base_url) {
            Ok(()) => return Ok(()),
            Err(err) if std::env::var_os("PRODEX_TUI_STRICT").is_none() => {
                eprintln!("prodex quota TUI unavailable, falling back to plain watch: {err:#}");
            }
            Err(err) => return Err(err),
        }
    }

    loop {
        let output = render_profile_quota_watch_output(
            profile_name,
            &quota_watch_updated_at(),
            fetch_profile_quota(provider, codex_home, base_url).map_err(|err| err.to_string()),
        );
        print_quota_watch_plain_snapshot(&output)?;
        thread::sleep(Duration::from_secs(DEFAULT_WATCH_INTERVAL_SECONDS));
    }
}

fn watch_profile_quota_tui(
    profile_name: &str,
    provider: &ProfileProvider,
    codex_home: &Path,
    base_url: Option<&str>,
) -> Result<()> {
    let mut tui = QuotaWatchTui::new()?;
    loop {
        let updated = quota_watch_updated_at();
        let frame = build_profile_quota_watch_tui_frame(
            profile_name,
            &updated,
            fetch_profile_quota(provider, codex_home, base_url).map_err(|err| err.to_string()),
        );
        tui.terminal
            .draw(|area| render_all_quota_watch_tui(area, &frame))
            .context("failed to draw quota TUI")?;

        let refresh_at = quota_watch_next_refresh_at();
        while Instant::now() < refresh_at {
            if event::poll(Duration::from_millis(QUOTA_WATCH_INPUT_POLL_MS))
                .context("failed to poll quota TUI input")?
            {
                match event::read().context("failed to read quota TUI input")? {
                    Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                        _ => {}
                    },
                    Event::Resize(_, _) => break,
                    _ => {}
                }
            }
        }
    }
}

pub(crate) fn watch_all_quotas(
    paths: &AppPaths,
    base_url: Option<&str>,
    detail: bool,
    auth_filter: QuotaAuthFilter,
    provider_filter: QuotaProviderFilter,
    provider_filter_locked: bool,
) -> Result<()> {
    if io::stdout().is_terminal() && io::stdin().is_terminal() {
        match watch_all_quotas_tui(
            paths,
            base_url,
            detail,
            auth_filter.clone(),
            provider_filter,
            provider_filter_locked,
        ) {
            Ok(()) => return Ok(()),
            Err(err) if std::env::var_os("PRODEX_TUI_STRICT").is_none() => {
                eprintln!("prodex quota TUI unavailable, falling back to plain watch: {err:#}");
            }
            Err(err) => return Err(err),
        }
    }

    watch_all_quotas_plain(
        paths,
        base_url,
        detail,
        auth_filter,
        provider_filter,
        provider_filter_locked,
    )
}

fn watch_all_quotas_plain(
    paths: &AppPaths,
    base_url: Option<&str>,
    detail: bool,
    auth_filter: QuotaAuthFilter,
    provider_filter: QuotaProviderFilter,
    provider_filter_locked: bool,
) -> Result<()> {
    let mut scroll_offset = 0_usize;
    let sort = QuotaReportSort::Current;
    let collection_provider_filter = if provider_filter_locked {
        provider_filter
    } else {
        QuotaProviderFilter::All
    };
    let mut snapshot = AllQuotaWatchSnapshot::Loading {
        updated: quota_watch_updated_at(),
    };
    let mut refresh = AllQuotaWatchRefresh::new();
    let _ = start_all_quota_watch_refresh(
        &mut refresh,
        paths,
        base_url,
        &auth_filter,
        collection_provider_filter,
    );
    let mut redraw_needed = true;
    let mut next_refresh_at = None;

    loop {
        if let Some(next_snapshot) = refresh.take_latest() {
            snapshot = merge_all_quota_watch_snapshot(&snapshot, next_snapshot);
            redraw_needed = true;
            next_refresh_at = Some(quota_watch_next_refresh_at());
        }

        if redraw_needed {
            scroll_offset = scroll_offset.min(quota_watch_max_scroll_offset(
                &snapshot,
                detail,
                provider_filter,
                sort,
            ));
            let output = render_all_quota_watch_snapshot(
                &snapshot,
                detail,
                scroll_offset,
                sort,
                provider_filter,
                provider_filter_locked,
            );
            print_quota_watch_plain_snapshot(&output)?;
            redraw_needed = false;
        }

        if next_refresh_at.is_some_and(|refresh_at| Instant::now() >= refresh_at)
            && start_all_quota_watch_refresh(
                &mut refresh,
                paths,
                base_url,
                &auth_filter,
                collection_provider_filter,
            )
        {
            next_refresh_at = None;
            continue;
        }

        thread::sleep(Duration::from_millis(QUOTA_WATCH_INPUT_POLL_MS));
    }
}

fn watch_all_quotas_tui(
    paths: &AppPaths,
    base_url: Option<&str>,
    detail: bool,
    auth_filter: QuotaAuthFilter,
    provider_filter: QuotaProviderFilter,
    provider_filter_locked: bool,
) -> Result<()> {
    let mut tui = QuotaWatchTui::new()?;
    let mut scroll_offset = 0_usize;
    let mut sort = QuotaReportSort::Current;
    let mut provider_filter = provider_filter;
    let collection_provider_filter = if provider_filter_locked {
        provider_filter
    } else {
        QuotaProviderFilter::All
    };
    let mut snapshot = AllQuotaWatchSnapshot::Loading {
        updated: quota_watch_updated_at(),
    };
    let mut refresh = AllQuotaWatchRefresh::new();
    let _ = start_all_quota_watch_refresh(
        &mut refresh,
        paths,
        base_url,
        &auth_filter,
        collection_provider_filter,
    );
    let mut redraw_needed = true;
    let mut next_refresh_at = None;

    loop {
        if let Some(next_snapshot) = refresh.take_latest() {
            snapshot = merge_all_quota_watch_snapshot(&snapshot, next_snapshot);
            redraw_needed = true;
            next_refresh_at = Some(quota_watch_next_refresh_at());
        }

        if redraw_needed {
            let size = tui
                .terminal
                .size()
                .context("failed to read quota TUI terminal size")?;
            let max_lines = quota_watch_tui_report_lines(size.height);
            scroll_offset =
                scroll_offset.min(quota_watch_max_scroll_offset_for_snapshot_with_layout(
                    &snapshot,
                    detail,
                    provider_filter,
                    sort,
                    max_lines,
                    usize::from(size.width).saturating_sub(4),
                ));
            let frame = build_all_quota_watch_tui_frame(
                &snapshot,
                AllQuotaWatchLayout {
                    detail,
                    scroll_offset,
                    sort,
                    provider_filter,
                    provider_filter_locked,
                    total_width: usize::from(size.width).saturating_sub(4),
                    max_lines,
                },
            );
            tui.terminal
                .draw(|area| render_all_quota_watch_tui(area, &frame))
                .context("failed to draw quota TUI")?;
            redraw_needed = false;
        }

        if next_refresh_at.is_some_and(|refresh_at| Instant::now() >= refresh_at)
            && start_all_quota_watch_refresh(
                &mut refresh,
                paths,
                base_url,
                &auth_filter,
                collection_provider_filter,
            )
        {
            next_refresh_at = None;
            continue;
        }

        if event::poll(Duration::from_millis(QUOTA_WATCH_INPUT_POLL_MS))
            .context("failed to poll quota TUI input")?
        {
            match event::read().context("failed to read quota TUI input")? {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    let command = match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => Some(QuotaWatchCommand::Quit),
                        KeyCode::Char('j') | KeyCode::Down => Some(QuotaWatchCommand::Down),
                        KeyCode::Char('k') | KeyCode::Up => Some(QuotaWatchCommand::Up),
                        KeyCode::Char('s') => Some(QuotaWatchCommand::Sort),
                        KeyCode::Char('f') => Some(QuotaWatchCommand::Filter),
                        _ => None,
                    };
                    let Some(command) = command else {
                        continue;
                    };
                    match apply_quota_watch_command(
                        command,
                        scroll_offset,
                        quota_watch_max_scroll_offset_for_snapshot_with_layout(
                            &snapshot,
                            detail,
                            provider_filter,
                            sort,
                            quota_watch_tui_report_lines(
                                tui.terminal.size().map(|size| size.height).unwrap_or(24),
                            ),
                            tui.terminal
                                .size()
                                .map(|size| usize::from(size.width).saturating_sub(4))
                                .unwrap_or_else(|_| current_cli_width()),
                        ),
                    ) {
                        QuotaWatchCommandOutcome::Continue(next_offset) => {
                            if next_offset != scroll_offset {
                                scroll_offset = next_offset;
                                redraw_needed = true;
                            }
                        }
                        QuotaWatchCommandOutcome::Sort => {
                            sort = sort.next();
                            scroll_offset = 0;
                            redraw_needed = true;
                        }
                        QuotaWatchCommandOutcome::Filter => {
                            if !provider_filter_locked {
                                provider_filter = provider_filter.next();
                                scroll_offset = 0;
                                redraw_needed = true;
                            }
                        }
                        QuotaWatchCommandOutcome::Quit => return Ok(()),
                    }
                }
                Event::Resize(_, _) => {
                    redraw_needed = true;
                }
                _ => {}
            }
        }
    }
}

struct AllQuotaWatchTuiFrame {
    updated: String,
    title: String,
    body: String,
    footer: String,
}

fn build_all_quota_watch_tui_frame(
    snapshot: &AllQuotaWatchSnapshot,
    layout: AllQuotaWatchLayout,
) -> AllQuotaWatchTuiFrame {
    let body = quota_watch_without_control_footer(&render_all_quota_watch_snapshot_with_layout(
        snapshot, layout,
    ));
    let provider_hint = if layout.provider_filter_locked {
        "provider fixed"
    } else {
        "f provider"
    };
    AllQuotaWatchTuiFrame {
        updated: quota_watch_snapshot_updated(snapshot).to_string(),
        title: "Prodex Quota".to_string(),
        body,
        footer: format!(
            "s sort {} | filter {} | {} | j/k scroll | q quit",
            layout.sort.label(),
            layout.provider_filter.label(),
            provider_hint
        ),
    }
}

fn quota_watch_without_control_footer(output: &str) -> String {
    let mut lines = output.lines().map(str::to_string).collect::<Vec<_>>();
    if lines.last().is_some_and(|line| line.starts_with("sort: ")) {
        lines.pop();
        if lines.last().is_some_and(|line| line.is_empty()) {
            lines.pop();
        }
    }
    lines.join("\n")
}

fn build_profile_quota_watch_tui_frame(
    profile_name: &str,
    updated: &str,
    quota_result: std::result::Result<ProviderQuotaSnapshot, String>,
) -> AllQuotaWatchTuiFrame {
    AllQuotaWatchTuiFrame {
        updated: updated.to_string(),
        title: format!("Prodex Quota {profile_name}"),
        body: render_profile_quota_watch_output(profile_name, updated, quota_result),
        footer: "refresh 5s | q quit".to_string(),
    }
}

fn render_all_quota_watch_tui(frame: &mut ratatui::Frame<'_>, data: &AllQuotaWatchTuiFrame) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(3),
        ])
        .split(frame.area());

    let header = Paragraph::new(Line::from(vec![
        Span::styled(data.title.as_str(), quota_watch_title_style()),
        Span::raw("  "),
        Span::styled(data.updated.as_str(), quota_watch_muted_style()),
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(quota_watch_border_style()),
    );
    frame.render_widget(header, chunks[0]);

    let body = Paragraph::new(quota_watch_tui_text(&data.body))
        .block(
            Block::default()
                .borders(Borders::LEFT | Borders::RIGHT)
                .border_style(quota_watch_border_style()),
        )
        .wrap(ratatui::widgets::Wrap { trim: false });
    frame.render_widget(body, chunks[1]);

    let footer = Paragraph::new(Line::styled(
        data.footer.as_str(),
        quota_watch_footer_style(),
    ))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(quota_watch_border_style()),
    );
    frame.render_widget(footer, chunks[2]);
}

fn quota_watch_tui_text(output: &str) -> Text<'_> {
    Text::from(
        output
            .lines()
            .map(|line| Line::from(quota_watch_tui_spans(line)))
            .collect::<Vec<_>>(),
    )
}

fn quota_watch_tui_spans(line: &str) -> Vec<Span<'_>> {
    if line.starts_with("== ") || line.starts_with("Quota Overview") {
        return vec![Span::styled(line, quota_watch_title_style())];
    }
    if line.chars().all(|ch| ch == '-' || ch.is_whitespace()) {
        return vec![Span::styled(line, quota_watch_muted_style())];
    }
    if line.contains("Blocked:") || line.contains("Error:") {
        return vec![Span::styled(
            line,
            Style::default().fg(Color::Rgb(255, 118, 118)),
        )];
    }
    if line.contains("Ready") || line.contains("healthy") {
        return vec![Span::styled(
            line,
            Style::default().fg(Color::Rgb(105, 214, 143)),
        )];
    }
    if line.contains("thin") || line.contains("critical") || line.contains("exhausted") {
        return vec![Span::styled(
            line,
            Style::default().fg(Color::Rgb(235, 196, 109)),
        )];
    }
    if line.starts_with("PROFILE") {
        return vec![Span::styled(
            line,
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )];
    }
    if line.starts_with("workspace:")
        || line.starts_with("resets:")
        || line.starts_with("status:")
        || line.trim_start().starts_with("workspace:")
        || line.trim_start().starts_with("resets:")
        || line.trim_start().starts_with("status:")
        || line.starts_with("press ")
    {
        return vec![Span::styled(line, quota_watch_muted_style())];
    }
    vec![Span::raw(line)]
}

fn quota_watch_title_style() -> Style {
    Style::default()
        .fg(Color::Rgb(92, 221, 229))
        .add_modifier(Modifier::BOLD)
}

fn quota_watch_border_style() -> Style {
    Style::default().fg(Color::Rgb(74, 103, 123))
}

fn quota_watch_muted_style() -> Style {
    Style::default().fg(Color::Rgb(150, 165, 176))
}

fn quota_watch_footer_style() -> Style {
    Style::default()
        .fg(Color::Rgb(235, 196, 109))
        .add_modifier(Modifier::BOLD)
}

fn quota_watch_tui_report_lines(terminal_height: u16) -> Option<usize> {
    Some(usize::from(terminal_height).saturating_sub(6).max(1))
}

fn start_all_quota_watch_refresh(
    refresh: &mut AllQuotaWatchRefresh,
    paths: &AppPaths,
    base_url: Option<&str>,
    auth_filter: &QuotaAuthFilter,
    provider_filter: QuotaProviderFilter,
) -> bool {
    let paths = paths.clone();
    let base_url = base_url.map(str::to_string);
    let auth_filter = auth_filter.clone();
    refresh.try_start(move || {
        load_all_quota_watch_snapshot(&paths, base_url.as_deref(), &auth_filter, provider_filter)
    })
}

fn quota_watch_updated_at() -> String {
    Local::now().format("%Y-%m-%d %H:%M:%S %Z").to_string()
}

fn load_all_quota_watch_snapshot(
    paths: &AppPaths,
    base_url: Option<&str>,
    auth_filter: &QuotaAuthFilter,
    provider_filter: QuotaProviderFilter,
) -> AllQuotaWatchSnapshot {
    collect_all_quota_watch_snapshot(
        &quota_watch_updated_at(),
        AppState::load(paths).map_err(|err| err.to_string()),
        base_url,
        auth_filter,
        provider_filter,
    )
}

fn quota_watch_next_refresh_at() -> Instant {
    Instant::now() + Duration::from_secs(DEFAULT_WATCH_INTERVAL_SECONDS)
}

fn quota_watch_max_scroll_offset(
    snapshot: &AllQuotaWatchSnapshot,
    detail: bool,
    provider_filter: QuotaProviderFilter,
    sort: QuotaReportSort,
) -> usize {
    match snapshot {
        AllQuotaWatchSnapshot::Reports { reports, .. } => {
            quota_watch_max_scroll_offset_for_reports(reports, detail, provider_filter, sort)
        }
        _ => 0,
    }
}

fn quota_watch_max_scroll_offset_for_snapshot_with_layout(
    snapshot: &AllQuotaWatchSnapshot,
    detail: bool,
    provider_filter: QuotaProviderFilter,
    sort: QuotaReportSort,
    max_lines: Option<usize>,
    total_width: usize,
) -> usize {
    match snapshot {
        AllQuotaWatchSnapshot::Reports { reports, .. } => {
            quota_watch_max_scroll_offset_for_reports_with_layout(
                reports,
                detail,
                provider_filter,
                sort,
                max_lines,
                total_width,
            )
        }
        _ => 0,
    }
}

fn quota_watch_max_scroll_offset_for_reports(
    reports: &[QuotaReport],
    detail: bool,
    provider_filter: QuotaProviderFilter,
    sort: QuotaReportSort,
) -> usize {
    quota_watch_max_scroll_offset_for_reports_with_layout(
        reports,
        detail,
        provider_filter,
        sort,
        quota_watch_available_report_lines(""),
        current_cli_width(),
    )
}

fn quota_watch_max_scroll_offset_for_reports_with_layout(
    reports: &[QuotaReport],
    detail: bool,
    provider_filter: QuotaProviderFilter,
    sort: QuotaReportSort,
    max_lines: Option<usize>,
    total_width: usize,
) -> usize {
    let filtered_reports = filter_quota_reports_by_provider(reports, provider_filter);
    if filtered_reports.is_empty() {
        return 0;
    }
    for scroll_offset in 0..filtered_reports.len() {
        let window = render_quota_reports_window_with_sort(
            &filtered_reports,
            detail,
            max_lines,
            total_width,
            scroll_offset,
            true,
            sort,
        );
        if window.hidden_after == 0 {
            return window.start_profile;
        }
    }
    filtered_reports.len().saturating_sub(1)
}

fn merge_all_quota_watch_snapshot(
    previous: &AllQuotaWatchSnapshot,
    next: AllQuotaWatchSnapshot,
) -> AllQuotaWatchSnapshot {
    match (previous, next) {
        (
            AllQuotaWatchSnapshot::Reports {
                reports: previous_reports,
                ..
            },
            AllQuotaWatchSnapshot::Reports {
                updated,
                profile_count,
                mut reports,
            },
        ) => {
            preserve_previous_successful_quota_reports(previous_reports, &mut reports);
            AllQuotaWatchSnapshot::Reports {
                updated,
                profile_count,
                reports,
            }
        }
        (AllQuotaWatchSnapshot::Reports { .. }, AllQuotaWatchSnapshot::Error { .. }) => {
            previous.clone()
        }
        (_, next) => next,
    }
}

fn preserve_previous_successful_quota_reports(
    previous_reports: &[QuotaReport],
    reports: &mut [QuotaReport],
) {
    for report in reports {
        if report.result.is_ok() {
            continue;
        }
        let Some(previous) = previous_reports.iter().find(|previous| {
            previous.name == report.name
                && previous.result.is_ok()
                && previous.auth.label == report.auth.label
        }) else {
            continue;
        };
        *report = previous.clone();
    }
}

fn apply_quota_watch_command(
    command: QuotaWatchCommand,
    scroll_offset: usize,
    max_scroll_offset: usize,
) -> QuotaWatchCommandOutcome {
    match command {
        QuotaWatchCommand::Up => {
            QuotaWatchCommandOutcome::Continue(scroll_offset.saturating_sub(1))
        }
        QuotaWatchCommand::Down => QuotaWatchCommandOutcome::Continue(
            scroll_offset.saturating_add(1).min(max_scroll_offset),
        ),
        QuotaWatchCommand::Filter => QuotaWatchCommandOutcome::Filter,
        QuotaWatchCommand::Sort => QuotaWatchCommandOutcome::Sort,
        QuotaWatchCommand::Quit => QuotaWatchCommandOutcome::Quit,
    }
}

#[cfg(test)]
mod tests {
    include!("../../tests/src/quota_support/watch_unit.rs");
}
