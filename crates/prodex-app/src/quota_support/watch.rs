use super::*;
use crossterm::cursor::{Hide, Show};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph};
use std::io::IsTerminal;
use terminal_ui::{
    tui_border_style, tui_detail_style, tui_error_style, tui_muted_style, tui_success_style,
    tui_title_style,
};

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
const ALL_QUOTA_WATCH_IMMINENT_INTERVAL_SECONDS: u64 = DEFAULT_WATCH_INTERVAL_SECONDS;
const ALL_QUOTA_WATCH_FAST_INTERVAL_SECONDS: u64 = 10;
const ALL_QUOTA_WATCH_DETAIL_STABLE_INTERVAL_SECONDS: u64 = 45;
const ALL_QUOTA_WATCH_STABLE_INTERVAL_SECONDS: u64 = 90;
const ALL_QUOTA_WATCH_PROFILE_SCALE_SECONDS: u64 = 2;
const ALL_QUOTA_WATCH_IMMINENT_RESET_SECONDS: i64 = 2 * 60;
const ALL_QUOTA_WATCH_NEAR_RESET_SECONDS: i64 = 15 * 60;
enum QuotaWatchCommand {
    Up,
    Down,
    Sort,
    Filter,
    Update,
    Quit,
}
enum QuotaWatchCommandOutcome {
    Continue(usize),
    Sort,
    Filter,
    Update,
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
        "sort: {} | filter: {}{} | u update | s sort | {} | j/k/Up/Down scroll | q quit",
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
                    Event::Key(key) if key.kind == KeyEventKind::Press => {
                        if quota_watch_quit_key(key) {
                            return Ok(());
                        }
                    }
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
            next_refresh_at = Some(all_quota_watch_next_refresh_at(&snapshot, detail));
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
            next_refresh_at = Some(all_quota_watch_next_refresh_at(&snapshot, detail));
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
                        _ if quota_watch_quit_key(key) => Some(QuotaWatchCommand::Quit),
                        KeyCode::Char('j') | KeyCode::Down => Some(QuotaWatchCommand::Down),
                        KeyCode::Char('k') | KeyCode::Up => Some(QuotaWatchCommand::Up),
                        KeyCode::Char('s') => Some(QuotaWatchCommand::Sort),
                        KeyCode::Char('f') => Some(QuotaWatchCommand::Filter),
                        KeyCode::Char('u') | KeyCode::Char('U') => Some(QuotaWatchCommand::Update),
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
                        QuotaWatchCommandOutcome::Update => {
                            if start_all_quota_watch_refresh(
                                &mut refresh,
                                paths,
                                base_url,
                                &auth_filter,
                                collection_provider_filter,
                            ) {
                                next_refresh_at = None;
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

fn quota_watch_quit_key(key: KeyEvent) -> bool {
    matches!(key.code, KeyCode::Esc | KeyCode::Char('q'))
        || (key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('c') | KeyCode::Char('z')))
}

struct AllQuotaWatchTuiFrame {
    title: String,
    body: String,
    overview_fields: Vec<(String, String)>,
    table: Option<AllQuotaWatchTuiTable>,
    footer: String,
}

struct AllQuotaWatchTuiTable {
    rows: Vec<AllQuotaWatchTuiRow>,
}

struct AllQuotaWatchTuiRow {
    profile: Vec<String>,
    current: Vec<String>,
    auth: Vec<String>,
    account: Vec<String>,
    plan: Vec<String>,
    status: Vec<String>,
    remaining: Vec<String>,
    detail: Vec<String>,
}

fn build_all_quota_watch_tui_frame(
    snapshot: &AllQuotaWatchSnapshot,
    layout: AllQuotaWatchLayout,
) -> AllQuotaWatchTuiFrame {
    let (body, overview_fields, table) = match snapshot {
        AllQuotaWatchSnapshot::Reports { reports, .. } => {
            let filtered_reports =
                filter_quota_reports_by_provider(reports, layout.provider_filter);
            let window = render_quota_reports_window_with_sort(
                &filtered_reports,
                layout.detail,
                layout.max_lines,
                layout.total_width,
                layout.scroll_offset,
                true,
                layout.sort,
            );
            (
                "Quota Overview".to_string(),
                quota_pool_summary_fields_for_reports(&filtered_reports),
                Some(build_all_quota_watch_tui_table(
                    &filtered_reports,
                    layout,
                    window.start_profile,
                    window.shown_profiles,
                )),
            )
        }
        AllQuotaWatchSnapshot::Loading { updated } => (
            "Quota".to_string(),
            quota_watch_status_fields("Loading quota data...", updated),
            None,
        ),
        AllQuotaWatchSnapshot::Empty { updated } => (
            "Quota".to_string(),
            quota_watch_status_fields("No profiles configured", updated),
            None,
        ),
        AllQuotaWatchSnapshot::Error { updated, message } => (
            "Quota".to_string(),
            quota_watch_status_fields(message, updated),
            None,
        ),
    };
    let provider_hint = if layout.provider_filter_locked {
        "provider fixed"
    } else {
        "f provider"
    };
    AllQuotaWatchTuiFrame {
        title: "Prodex Quota".to_string(),
        body,
        overview_fields,
        table,
        footer: format!(
            "u update | s sort {} | filter {} | {} | j/k scroll | q quit",
            layout.sort.label(),
            layout.provider_filter.label(),
            provider_hint
        ),
    }
}

fn quota_watch_status_fields(message: &str, updated: &str) -> Vec<(String, String)> {
    vec![
        ("Status".to_string(), message.to_string()),
        ("Last Updated".to_string(), updated.to_string()),
    ]
}

fn build_all_quota_watch_tui_table(
    reports: &[QuotaReport],
    layout: AllQuotaWatchLayout,
    start_profile: usize,
    shown_profiles: usize,
) -> AllQuotaWatchTuiTable {
    let rows = sorted_quota_report_indexes_by_sort(reports, layout.sort)
        .into_iter()
        .skip(start_profile)
        .take(shown_profiles)
        .map(|index| build_all_quota_watch_tui_row(&reports[index], layout.detail))
        .collect::<Vec<_>>();
    AllQuotaWatchTuiTable { rows }
}

fn build_all_quota_watch_tui_row(report: &QuotaReport, detail: bool) -> AllQuotaWatchTuiRow {
    let view = quota_watch_report_view(report);
    let account = vec![view.account];
    let mut detail_line = Vec::new();
    if detail && quota_watch_should_show_workspace(report) {
        detail_line.push(format!(
            "workspace: {}",
            quota_watch_workspace_label(report)
        ));
    }
    if detail && let Some(resets) = view.resets {
        let reset = quota_watch_reset_detail(&resets);
        detail_line.insert(0, reset.windows);
        if let Some(credits) = reset.credits {
            detail_line.push(credits);
        }
    }
    AllQuotaWatchTuiRow {
        profile: vec![report.name.clone()],
        current: vec![if report.active { "*" } else { "" }.to_string()],
        auth: vec![report.auth.label.clone()],
        account,
        plan: vec![view.plan],
        status: vec![view.status],
        remaining: vec![view.main],
        detail: (!detail_line.is_empty())
            .then(|| detail_line.join(" | "))
            .into_iter()
            .collect(),
    }
}

struct QuotaWatchReportView {
    account: String,
    plan: String,
    main: String,
    status: String,
    resets: Option<String>,
}

fn quota_watch_report_view(report: &QuotaReport) -> QuotaWatchReportView {
    match &report.result {
        Ok(ProviderQuotaSnapshot::OpenAi(usage)) => QuotaWatchReportView {
            account: quota_watch_optional(usage.email.as_deref()).to_string(),
            plan: quota_watch_optional(usage.plan_type.as_deref()).to_string(),
            main: format_main_windows_compact(usage),
            status: format_openai_quota_status(usage),
            resets: Some(quota_watch_openai_reset_summary(usage)),
        },
        Ok(ProviderQuotaSnapshot::Copilot(info)) => QuotaWatchReportView {
            account: quota_watch_optional(info.login.as_deref()).to_string(),
            plan: quota_watch_optional(
                info.copilot_plan
                    .as_deref()
                    .or(info.access_type_sku.as_deref()),
            )
            .to_string(),
            main: format_copilot_main_quota(info),
            status: format_copilot_quota_status(info),
            resets: Some(format_copilot_reset_summary(info).map_or_else(
                || "resets: unavailable".to_string(),
                |value| format!("resets: {value}"),
            )),
        },
        Ok(ProviderQuotaSnapshot::Gemini(info)) => QuotaWatchReportView {
            account: quota_watch_optional(info.email.as_deref()).to_string(),
            plan: quota_watch_optional(info.plan.as_deref()).to_string(),
            main: format_gemini_main_quota(info),
            status: format_gemini_quota_status(info),
            resets: Some(format_gemini_reset_summary(info).map_or_else(
                || "resets: unavailable".to_string(),
                |value| format!("resets: {value}"),
            )),
        },
        Ok(ProviderQuotaSnapshot::External(info)) => QuotaWatchReportView {
            account: quota_watch_optional(info.account.as_deref()).to_string(),
            plan: quota_watch_optional(info.plan.as_deref()).to_string(),
            main: info.main.clone(),
            status: info.status.clone(),
            resets: Some(info.reset.as_ref().map_or_else(
                || "resets: unavailable".to_string(),
                |value| format!("resets: {value}"),
            )),
        },
        Err(err) => QuotaWatchReportView {
            account: "-".to_string(),
            plan: "-".to_string(),
            main: "-".to_string(),
            status: format_quota_error_status(err),
            resets: Some(prodex_quota::format_quota_error_detail(err)),
        },
    }
}

fn quota_watch_openai_reset_summary(usage: &UsageResponse) -> String {
    let reset_summary = prodex_quota::format_main_reset_summary(usage);
    match usage.rate_limit_reset_credits.as_ref() {
        Some(credits) => format!(
            "resets: {reset_summary}; reset credits: {} available",
            credits.available_count
        ),
        None => format!("resets: {reset_summary}"),
    }
}

struct QuotaWatchResetDetail {
    windows: String,
    credits: Option<String>,
}

fn quota_watch_reset_detail(resets: &str) -> QuotaWatchResetDetail {
    if resets.trim_start().starts_with("error:") {
        return QuotaWatchResetDetail {
            windows: resets.to_string(),
            credits: None,
        };
    }
    let (windows, credits) = resets
        .strip_prefix("resets: ")
        .unwrap_or(resets)
        .split_once("; reset credits:")
        .map_or(
            (resets.strip_prefix("resets: ").unwrap_or(resets), None),
            |(left, right)| (left, Some(right.trim())),
        );
    QuotaWatchResetDetail {
        windows: format!("resets: {windows}"),
        credits: credits.map(|credits| format!("reset credits: {credits}")),
    }
}

fn quota_watch_optional(value: Option<&str>) -> &str {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("-")
}

fn quota_watch_openai_email(report: &QuotaReport) -> Option<&str> {
    match report.result.as_ref().ok()? {
        ProviderQuotaSnapshot::OpenAi(usage) => usage
            .email
            .as_deref()
            .map(str::trim)
            .filter(|email| !email.is_empty()),
        ProviderQuotaSnapshot::Copilot(_)
        | ProviderQuotaSnapshot::Gemini(_)
        | ProviderQuotaSnapshot::External(_) => None,
    }
}

fn quota_watch_should_show_workspace(report: &QuotaReport) -> bool {
    quota_watch_openai_email(report).is_some()
        && (report
            .workspace_name
            .as_deref()
            .map(str::trim)
            .is_some_and(|name| !name.is_empty())
            || report
                .workspace_id
                .as_deref()
                .map(str::trim)
                .is_some_and(|id| !id.is_empty()))
}

fn quota_watch_workspace_label(report: &QuotaReport) -> String {
    report
        .workspace_name
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| report.name.trim())
        .to_string()
}

fn quota_human_tui_compact_label(line: &str) -> Option<&'static str> {
    const LABELS: [&str; 23] = [
        "Available",
        "Last Updated",
        "5h remaining pool",
        "Weekly remaining pool",
        "Remaining pool",
        "Account",
        "Plan",
        "Status",
        "Main",
        "5h",
        "Weekly",
        "Reset credits",
        "Auth",
        "Provider",
        "Reset",
        "Project",
        "Bucket",
        "Quota",
        "Rate groups",
        "Model groups",
        "Admin API",
        "Model provider",
        "Source",
    ];
    LABELS
        .into_iter()
        .find(|label| line.trim_start().starts_with(&format!("{label}:")))
}

fn build_profile_quota_watch_tui_frame(
    profile_name: &str,
    updated: &str,
    quota_result: std::result::Result<ProviderQuotaSnapshot, String>,
) -> AllQuotaWatchTuiFrame {
    let (body, overview_fields) = match quota_result {
        Ok(snapshot) => (
            format!("Quota {profile_name}"),
            quota_watch_profile_fields(updated, &snapshot),
        ),
        Err(err) => (
            format!("Quota {profile_name}"),
            quota_watch_status_fields(&err, updated),
        ),
    };
    AllQuotaWatchTuiFrame {
        title: format!("Prodex Quota {profile_name}"),
        body,
        overview_fields,
        table: None,
        footer: "refresh 5s | q quit".to_string(),
    }
}

fn quota_watch_profile_fields(
    updated: &str,
    snapshot: &ProviderQuotaSnapshot,
) -> Vec<(String, String)> {
    match snapshot {
        ProviderQuotaSnapshot::OpenAi(usage) => {
            vec![
                (
                    "Account".to_string(),
                    quota_watch_optional(usage.email.as_deref()).to_string(),
                ),
                (
                    "Plan".to_string(),
                    quota_watch_optional(usage.plan_type.as_deref()).to_string(),
                ),
                ("Status".to_string(), format_openai_quota_status(usage)),
                ("Main".to_string(), format_main_windows(usage)),
                (
                    "Reset".to_string(),
                    quota_watch_openai_reset_summary(usage)
                        .trim_start_matches("resets: ")
                        .to_string(),
                ),
                ("Last Updated".to_string(), updated.to_string()),
            ]
        }
        ProviderQuotaSnapshot::Copilot(info) => vec![
            (
                "Account".to_string(),
                quota_watch_optional(info.login.as_deref()).to_string(),
            ),
            (
                "Plan".to_string(),
                quota_watch_optional(
                    info.copilot_plan
                        .as_deref()
                        .or(info.access_type_sku.as_deref()),
                )
                .to_string(),
            ),
            ("Status".to_string(), format_copilot_quota_status(info)),
            ("Main".to_string(), format_copilot_main_quota(info)),
            (
                "Reset".to_string(),
                format_copilot_reset_summary(info).unwrap_or_else(|| "unavailable".to_string()),
            ),
            ("Last Updated".to_string(), updated.to_string()),
        ],
        ProviderQuotaSnapshot::Gemini(info) => vec![
            (
                "Account".to_string(),
                quota_watch_optional(info.email.as_deref()).to_string(),
            ),
            (
                "Plan".to_string(),
                quota_watch_optional(info.plan.as_deref()).to_string(),
            ),
            ("Status".to_string(), format_gemini_quota_status(info)),
            ("Main".to_string(), format_gemini_main_quota(info)),
            (
                "Reset".to_string(),
                format_gemini_reset_summary(info).unwrap_or_else(|| "unavailable".to_string()),
            ),
            ("Last Updated".to_string(), updated.to_string()),
        ],
        ProviderQuotaSnapshot::External(info) => vec![
            (
                "Account".to_string(),
                quota_watch_optional(info.account.as_deref()).to_string(),
            ),
            (
                "Plan".to_string(),
                quota_watch_optional(info.plan.as_deref()).to_string(),
            ),
            ("Status".to_string(), info.status.clone()),
            ("Main".to_string(), info.main.clone()),
            (
                "Reset".to_string(),
                info.reset
                    .clone()
                    .unwrap_or_else(|| "unavailable".to_string()),
            ),
            ("Last Updated".to_string(), updated.to_string()),
        ],
    }
}

fn render_all_quota_watch_tui(frame: &mut ratatui::Frame<'_>, data: &AllQuotaWatchTuiFrame) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(3)])
        .split(frame.area());

    let body_block = Block::default()
        .title(Line::styled(data.title.as_str(), quota_watch_title_style()))
        .borders(Borders::ALL)
        .border_style(quota_watch_border_style());
    let body_area = body_block.inner(chunks[0]);
    frame.render_widget(body_block, chunks[0]);

    if let Some(table) = &data.table {
        let overview_height =
            quota_watch_overview_height(data.overview_fields.len(), body_area.height);
        let body_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(overview_height),
                Constraint::Length(1),
                Constraint::Min(1),
            ])
            .split(body_area);
        let overview = Paragraph::new(quota_watch_fields_text(&data.body, &data.overview_fields))
            .wrap(ratatui::widgets::Wrap { trim: false });
        frame.render_widget(overview, body_chunks[0]);
        frame.render_widget(
            Block::default()
                .borders(Borders::TOP)
                .border_style(quota_watch_border_style()),
            body_chunks[1],
        );
        render_all_quota_watch_tui_table(frame, body_chunks[2], table);
    } else if !data.overview_fields.is_empty() {
        let body = Paragraph::new(quota_watch_fields_text(&data.body, &data.overview_fields))
            .wrap(ratatui::widgets::Wrap { trim: false });
        frame.render_widget(body, body_area);
    } else {
        let body = Paragraph::new(quota_human_tui_text(&data.body))
            .wrap(ratatui::widgets::Wrap { trim: false });
        frame.render_widget(body, body_area);
    }

    let footer = Paragraph::new(Line::styled(
        data.footer.as_str(),
        quota_watch_footer_style(),
    ))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(quota_watch_border_style()),
    );
    frame.render_widget(footer, chunks[1]);
}

pub(crate) fn render_all_quota_reports_once_tui(
    frame: &mut ratatui::Frame<'_>,
    reports: &[QuotaReport],
    detail: bool,
) {
    let area = frame.area();
    let snapshot = AllQuotaWatchSnapshot::Reports {
        updated: quota_watch_updated_at(),
        profile_count: reports.len(),
        reports: reports.to_vec(),
    };
    let data = build_all_quota_watch_tui_frame(
        &snapshot,
        AllQuotaWatchLayout {
            detail,
            scroll_offset: 0,
            sort: QuotaReportSort::Remaining,
            provider_filter: QuotaProviderFilter::All,
            provider_filter_locked: false,
            total_width: usize::from(area.width).saturating_sub(4),
            max_lines: quota_watch_tui_report_lines(area.height),
        },
    );
    render_all_quota_watch_tui(frame, &data);
}

pub(crate) fn render_profile_quota_once_tui(
    frame: &mut ratatui::Frame<'_>,
    profile_name: &str,
    quota: ProviderQuotaSnapshot,
) {
    let data =
        build_profile_quota_watch_tui_frame(profile_name, &quota_watch_updated_at(), Ok(quota));
    render_all_quota_watch_tui(frame, &data);
}

fn render_all_quota_watch_tui_table(
    frame: &mut ratatui::Frame<'_>,
    area: ratatui::layout::Rect,
    table: &AllQuotaWatchTuiTable,
) {
    let text = quota_watch_table_text(table);
    let widget = Paragraph::new(text).wrap(ratatui::widgets::Wrap { trim: false });
    frame.render_widget(widget, area);
}

fn quota_watch_overview_height(field_count: usize, max_height: u16) -> u16 {
    u16::try_from(field_count)
        .unwrap_or(max_height)
        .min(max_height)
}

fn quota_watch_table_text(table: &AllQuotaWatchTuiTable) -> Text<'static> {
    let mut lines = vec![Line::styled(
        format!(
            "{:<24} {:<3} {:<7} {:<24} {:<8} {:<15} {}",
            "PROFILE", "CUR", "AUTH", "ACCOUNT", "PLAN", "STATUS", "REMAINING"
        ),
        quota_watch_title_style(),
    )];
    for row in &table.rows {
        lines.push(quota_watch_table_main_line(row));
        for detail in &row.detail {
            lines.push(Line::styled(detail.clone(), quota_watch_detail_style()));
        }
        lines.push(Line::raw(""));
    }
    Text::from(lines)
}

fn quota_watch_table_main_line(row: &AllQuotaWatchTuiRow) -> Line<'static> {
    let status = quota_watch_first_cell(&row.status);
    let cell = quota_watch_table_cell_text;
    Line::from(vec![
        Span::raw(cell(quota_watch_first_cell(&row.profile), 24)),
        Span::raw(" "),
        Span::raw(cell(quota_watch_first_cell(&row.current), 3)),
        Span::raw(" "),
        Span::raw(cell(quota_watch_first_cell(&row.auth), 7)),
        Span::raw(" "),
        Span::raw(cell(quota_watch_first_cell(&row.account), 24)),
        Span::raw(" "),
        Span::raw(cell(quota_watch_first_cell(&row.plan), 8)),
        Span::raw(" "),
        Span::styled(cell(status, 15), quota_watch_status_style(status)),
        Span::raw(" "),
        Span::raw(quota_watch_first_cell(&row.remaining).to_string()),
    ])
}

fn quota_watch_table_cell_text(value: &str, width: usize) -> String {
    let mut value = value.trim().to_string();
    if value.chars().count() > width {
        value = value
            .chars()
            .take(width.saturating_sub(1))
            .collect::<String>();
        value.push('~');
    }
    format!("{value:<width$}")
}

fn quota_watch_status_style(status: &str) -> Style {
    if status.contains("Blocked") || status.contains("Error") {
        tui_error_style()
    } else if status.contains("Ready") || status.contains("healthy") {
        tui_success_style()
    } else {
        Style::default()
    }
}

fn quota_watch_first_cell(lines: &[String]) -> &str {
    lines.first().map(String::as_str).unwrap_or("")
}

fn quota_watch_fields_text(title: &str, fields: &[(String, String)]) -> Text<'static> {
    let mut lines = vec![Line::styled(title.to_string(), quota_watch_title_style())];
    lines.extend(fields.iter().map(|(label, value)| {
        Line::from(vec![
            Span::styled(format!("{label}:"), tui_title_style()),
            Span::raw(" "),
            Span::raw(value.clone()),
        ])
    }));
    Text::from(lines)
}

pub(crate) fn quota_human_tui_text(output: &str) -> Text<'_> {
    Text::from(
        output
            .lines()
            .map(|line| Line::from(quota_human_tui_spans(line)))
            .collect::<Vec<_>>(),
    )
}

fn quota_human_tui_spans(line: &str) -> Vec<Span<'_>> {
    if line.starts_with("== ")
        || line == "Quota Overview"
        || line.starts_with("Quota ")
        || line.ends_with("profiles")
    {
        return vec![Span::styled(line, quota_watch_title_style())];
    }
    if quota_human_tui_compact_label(line).is_some() {
        let Some((label, value)) = line.split_once(':') else {
            return vec![Span::raw(line)];
        };
        return vec![
            Span::styled(format!("{label}:"), tui_title_style()),
            Span::raw(" "),
            Span::raw(value.trim_start().to_string()),
        ];
    }
    if line.chars().all(|ch| ch == '-' || ch.is_whitespace()) {
        return vec![Span::styled(line, quota_watch_muted_style())];
    }
    if line.contains("Blocked") || line.contains("Error") {
        return vec![Span::styled(line, tui_error_style())];
    }
    if line.contains("Ready") || line.contains("healthy") {
        return vec![Span::styled(line, tui_success_style())];
    }
    if line.contains("thin") || line.contains("critical") || line.contains("exhausted") {
        return vec![Span::styled(line, tui_error_style())];
    }
    if line.starts_with("PROFILE") {
        return vec![Span::styled(
            line,
            Style::default().add_modifier(Modifier::BOLD),
        )];
    }
    if line.starts_with("workspace:")
        || line.starts_with("error:")
        || line.starts_with("resets:")
        || line.starts_with("status:")
        || line.starts_with("reset credits:")
        || line.trim_start().starts_with("workspace:")
        || line.trim_start().starts_with("error:")
        || line.trim_start().starts_with("resets:")
        || line.trim_start().starts_with("status:")
        || line.trim_start().starts_with("reset credits:")
    {
        return vec![Span::styled(line, quota_watch_detail_style())];
    }
    if line.starts_with("press ") || line.trim_start().starts_with("press ") {
        return vec![Span::styled(line, quota_watch_muted_style())];
    }
    vec![Span::raw(line)]
}

fn quota_watch_detail_style() -> Style {
    tui_detail_style()
}

fn quota_watch_title_style() -> Style {
    tui_title_style()
}

fn quota_watch_border_style() -> Style {
    tui_border_style()
}

fn quota_watch_muted_style() -> Style {
    tui_muted_style()
}

fn quota_watch_footer_style() -> Style {
    tui_title_style()
}

fn quota_watch_tui_report_lines(terminal_height: u16) -> Option<usize> {
    Some(usize::from(terminal_height).saturating_sub(5).max(1))
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
    Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
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

fn all_quota_watch_next_refresh_at(snapshot: &AllQuotaWatchSnapshot, detail: bool) -> Instant {
    Instant::now() + all_quota_watch_refresh_interval(snapshot, detail, Local::now().timestamp())
}

fn all_quota_watch_refresh_interval(
    snapshot: &AllQuotaWatchSnapshot,
    detail: bool,
    now: i64,
) -> Duration {
    match snapshot {
        AllQuotaWatchSnapshot::Reports { reports, .. } => {
            all_quota_watch_reports_refresh_interval(reports, detail, now)
        }
        AllQuotaWatchSnapshot::Empty { .. } => {
            Duration::from_secs(ALL_QUOTA_WATCH_DETAIL_STABLE_INTERVAL_SECONDS)
        }
        AllQuotaWatchSnapshot::Loading { .. } | AllQuotaWatchSnapshot::Error { .. } => {
            Duration::from_secs(ALL_QUOTA_WATCH_FAST_INTERVAL_SECONDS)
        }
    }
}

fn all_quota_watch_reports_refresh_interval(
    reports: &[QuotaReport],
    detail: bool,
    now: i64,
) -> Duration {
    let signal = reports
        .iter()
        .map(|report| report_quota_refresh_signal(report, now))
        .max()
        .unwrap_or(QuotaRefreshSignal::Stable);

    let base = match signal {
        QuotaRefreshSignal::Imminent => ALL_QUOTA_WATCH_IMMINENT_INTERVAL_SECONDS,
        QuotaRefreshSignal::Watch => ALL_QUOTA_WATCH_FAST_INTERVAL_SECONDS,
        QuotaRefreshSignal::Stable => {
            if detail {
                ALL_QUOTA_WATCH_DETAIL_STABLE_INTERVAL_SECONDS
            } else {
                ALL_QUOTA_WATCH_STABLE_INTERVAL_SECONDS
            }
        }
    };
    let scaled = base.max(
        u64::try_from(reports.len())
            .unwrap_or(u64::MAX)
            .saturating_mul(ALL_QUOTA_WATCH_PROFILE_SCALE_SECONDS),
    );
    Duration::from_secs(quota_watch_jittered_interval_seconds(scaled, now))
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum QuotaRefreshSignal {
    Stable,
    Watch,
    Imminent,
}

fn report_quota_refresh_signal(report: &QuotaReport, now: i64) -> QuotaRefreshSignal {
    let Ok(snapshot) = &report.result else {
        return QuotaRefreshSignal::Watch;
    };
    match snapshot {
        ProviderQuotaSnapshot::OpenAi(usage) => openai_usage_quota_refresh_signal(usage, now),
        ProviderQuotaSnapshot::Copilot(info) => info
            .limited_user_reset_date
            .as_deref()
            .and_then(parse_quota_reset_epoch)
            .map(|reset_at| quota_reset_refresh_signal(reset_at, now))
            .unwrap_or(QuotaRefreshSignal::Stable),
        ProviderQuotaSnapshot::Gemini(info) => info
            .buckets
            .iter()
            .filter_map(|bucket| {
                bucket
                    .reset_time
                    .as_deref()
                    .and_then(parse_quota_reset_epoch)
                    .map(|reset_at| quota_reset_refresh_signal(reset_at, now))
            })
            .max()
            .unwrap_or(QuotaRefreshSignal::Stable),
        ProviderQuotaSnapshot::External(_) => QuotaRefreshSignal::Stable,
    }
}

fn openai_usage_quota_refresh_signal(usage: &UsageResponse, now: i64) -> QuotaRefreshSignal {
    if !collect_blocked_limits(usage, false).is_empty() {
        return QuotaRefreshSignal::Watch;
    }
    if usage
        .rate_limit_reset_credits
        .as_ref()
        .is_some_and(|credits| credits.available_count > 0)
    {
        return QuotaRefreshSignal::Watch;
    }
    main_quota_reset_refresh_signal(usage, now)
}

fn main_quota_reset_refresh_signal(usage: &UsageResponse, now: i64) -> QuotaRefreshSignal {
    usage
        .rate_limit
        .as_ref()
        .into_iter()
        .flat_map(|pair| [&pair.primary_window, &pair.secondary_window])
        .flatten()
        .filter_map(|window| window.reset_at)
        .map(|reset_at| quota_reset_refresh_signal(reset_at, now))
        .max()
        .unwrap_or(QuotaRefreshSignal::Stable)
}

fn quota_reset_refresh_signal(reset_at: i64, now: i64) -> QuotaRefreshSignal {
    if reset_at <= now + ALL_QUOTA_WATCH_IMMINENT_RESET_SECONDS {
        QuotaRefreshSignal::Imminent
    } else if reset_at <= now + ALL_QUOTA_WATCH_NEAR_RESET_SECONDS {
        QuotaRefreshSignal::Watch
    } else {
        QuotaRefreshSignal::Stable
    }
}

fn quota_watch_jittered_interval_seconds(seconds: u64, now: i64) -> u64 {
    if seconds <= ALL_QUOTA_WATCH_IMMINENT_INTERVAL_SECONDS {
        return seconds;
    }
    let jitter_span = (seconds / 10).max(1);
    let bucket = now.unsigned_abs() % (jitter_span.saturating_mul(2).saturating_add(1));
    let delta = i64::try_from(bucket).unwrap_or(0) - i64::try_from(jitter_span).unwrap_or(0);
    seconds.saturating_add_signed(delta).max(1)
}

fn parse_quota_reset_epoch(value: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(value)
        .map(|datetime| datetime.timestamp())
        .ok()
        .or_else(|| value.trim().parse::<i64>().ok())
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
        QuotaWatchCommand::Sort => QuotaWatchCommandOutcome::Sort,
        QuotaWatchCommand::Filter => QuotaWatchCommandOutcome::Filter,
        QuotaWatchCommand::Update => QuotaWatchCommandOutcome::Update,
        QuotaWatchCommand::Quit => QuotaWatchCommandOutcome::Quit,
    }
}

#[cfg(test)]
mod tests {
    include!("../../tests/src/quota_support/watch_unit.rs");
    include!("../../tests/src/quota_support/watch_tui_table_unit.rs");
}
