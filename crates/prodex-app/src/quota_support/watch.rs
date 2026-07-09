use super::*;
use crossterm::cursor::{Hide, Show};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use std::io::IsTerminal;

#[path = "watch_tui.rs"]
mod watch_tui;

#[cfg(test)]
use self::watch_tui::{
    AllQuotaWatchTuiRow, AllQuotaWatchTuiTable, quota_human_tui_spans, quota_watch_overview_height,
    quota_watch_separator_line, quota_watch_table_text,
};
use self::watch_tui::{
    build_all_quota_watch_tui_frame, build_profile_quota_watch_tui_frame,
    quota_watch_snapshot_overview_field_count, quota_watch_tui_max_scroll_offset_for_snapshot,
    quota_watch_tui_table_lines, render_all_quota_watch_tui,
};
pub(crate) use self::watch_tui::{
    render_all_quota_reports_once_tui, render_profile_quota_once_tui,
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
const ALL_QUOTA_WATCH_AUTH_BACKOFF_POLL_SECONDS: u64 = 1;
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
    let mut auth_backoff_profiles = std::collections::BTreeSet::new();
    let mut next_auth_backoff_poll_at = Instant::now();

    loop {
        if Instant::now() >= next_auth_backoff_poll_at {
            let next_auth_backoff_profiles = quota_runtime_auth_backoff_profiles();
            if next_auth_backoff_profiles != auth_backoff_profiles {
                auth_backoff_profiles = next_auth_backoff_profiles;
                redraw_needed = true;
            }
            next_auth_backoff_poll_at =
                Instant::now() + Duration::from_secs(ALL_QUOTA_WATCH_AUTH_BACKOFF_POLL_SECONDS);
        }

        if let Some(next_snapshot) = refresh.take_latest() {
            snapshot = merge_all_quota_watch_snapshot(&snapshot, next_snapshot);
            redraw_needed = true;
            next_refresh_at = Some(all_quota_watch_next_refresh_at(
                paths,
                &snapshot,
                detail,
                &auth_filter,
                collection_provider_filter,
            ));
        }

        if redraw_needed {
            let render_snapshot =
                quota_watch_snapshot_with_auth_backoff(&snapshot, &auth_backoff_profiles);
            scroll_offset = scroll_offset.min(quota_watch_max_scroll_offset(
                &render_snapshot,
                detail,
                provider_filter,
                sort,
            ));
            let output = render_all_quota_watch_snapshot(
                &render_snapshot,
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
    let mut auth_backoff_profiles = std::collections::BTreeSet::new();
    let mut next_auth_backoff_poll_at = Instant::now();

    loop {
        if Instant::now() >= next_auth_backoff_poll_at {
            let next_auth_backoff_profiles = quota_runtime_auth_backoff_profiles();
            if next_auth_backoff_profiles != auth_backoff_profiles {
                auth_backoff_profiles = next_auth_backoff_profiles;
                redraw_needed = true;
            }
            next_auth_backoff_poll_at =
                Instant::now() + Duration::from_secs(ALL_QUOTA_WATCH_AUTH_BACKOFF_POLL_SECONDS);
        }

        if let Some(next_snapshot) = refresh.take_latest() {
            snapshot = merge_all_quota_watch_snapshot(&snapshot, next_snapshot);
            redraw_needed = true;
            next_refresh_at = Some(all_quota_watch_next_refresh_at(
                paths,
                &snapshot,
                detail,
                &auth_filter,
                collection_provider_filter,
            ));
        }

        if redraw_needed {
            let size = tui
                .terminal
                .size()
                .context("failed to read quota TUI terminal size")?;
            let render_snapshot =
                quota_watch_snapshot_with_auth_backoff(&snapshot, &auth_backoff_profiles);
            let max_lines = quota_watch_tui_table_lines(
                size.height,
                quota_watch_snapshot_overview_field_count(&render_snapshot, provider_filter),
            );
            scroll_offset = scroll_offset.min(quota_watch_tui_max_scroll_offset_for_snapshot(
                &render_snapshot,
                detail,
                provider_filter,
                sort,
                max_lines,
            ));
            let frame = build_all_quota_watch_tui_frame(
                &render_snapshot,
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
                        quota_watch_tui_max_scroll_offset_for_snapshot(
                            &snapshot,
                            detail,
                            provider_filter,
                            sort,
                            quota_watch_tui_table_lines(
                                tui.terminal.size().map(|size| size.height).unwrap_or(24),
                                quota_watch_snapshot_overview_field_count(
                                    &snapshot,
                                    provider_filter,
                                ),
                            ),
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

fn all_quota_watch_next_refresh_at(
    paths: &AppPaths,
    snapshot: &AllQuotaWatchSnapshot,
    detail: bool,
    auth_filter: &QuotaAuthFilter,
    provider_filter: QuotaProviderFilter,
) -> Instant {
    let interval = all_quota_watch_refresh_interval(snapshot, detail, Local::now().timestamp());
    maybe_save_all_quota_watch_runtime_usage_cache(
        paths,
        snapshot,
        detail,
        auth_filter,
        provider_filter,
        interval,
    );
    Instant::now() + interval
}

fn maybe_save_all_quota_watch_runtime_usage_cache(
    paths: &AppPaths,
    snapshot: &AllQuotaWatchSnapshot,
    detail: bool,
    auth_filter: &QuotaAuthFilter,
    provider_filter: QuotaProviderFilter,
    refresh_interval: Duration,
) {
    if !quota_watch_runtime_usage_cache_enabled(detail, auth_filter, provider_filter) {
        return;
    }
    if let AllQuotaWatchSnapshot::Reports { reports, .. } = snapshot {
        save_quota_watch_runtime_usage_cache(paths, reports, refresh_interval);
    }
}

fn quota_watch_runtime_usage_cache_enabled(
    detail: bool,
    auth_filter: &QuotaAuthFilter,
    provider_filter: QuotaProviderFilter,
) -> bool {
    detail
        && matches!(auth_filter, QuotaAuthFilter::All)
        && matches!(
            provider_filter,
            QuotaProviderFilter::All | QuotaProviderFilter::OpenAi
        )
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

    quota_watch_refresh_interval_for_signal(signal, detail, reports.len(), now)
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

fn quota_watch_snapshot_with_auth_backoff(
    snapshot: &AllQuotaWatchSnapshot,
    auth_backoff_profiles: &std::collections::BTreeSet<String>,
) -> AllQuotaWatchSnapshot {
    if auth_backoff_profiles.is_empty() {
        return snapshot.clone();
    }
    let AllQuotaWatchSnapshot::Reports {
        updated,
        profile_count,
        reports,
    } = snapshot
    else {
        return snapshot.clone();
    };
    let mut reports = reports.clone();
    for report in &mut reports {
        if auth_backoff_profiles.contains(&report.name) {
            report.result = Err(format!(
                "unauthorized: runtime saw token invalidated for {}; run `prodex login {}` again",
                report.name, report.name
            ));
        }
    }
    AllQuotaWatchSnapshot::Reports {
        updated: updated.clone(),
        profile_count: *profile_count,
        reports,
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
        if quota_watch_error_is_auth_failure(&report.result) {
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

fn quota_watch_error_is_auth_failure(
    result: &std::result::Result<ProviderQuotaSnapshot, String>,
) -> bool {
    let Err(error) = result else {
        return false;
    };
    let lower = error.to_ascii_lowercase();
    lower.contains("401") || lower.contains("unauthorized") || lower.contains("token invalidated")
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
    include!("../../tests/src/quota_support/watch_tui_frame_unit.rs");
    include!("../../tests/src/quota_support/watch_tui_table_unit.rs");
}
