use super::*;
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

struct QuotaWatchInput {
    tty: fs::File,
    saved_stty_state: String,
}

impl QuotaWatchInput {
    fn open() -> Option<Self> {
        let tty = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open("/dev/tty")
            .ok()?;
        let saved_stty_state = quota_watch_stty_output(&tty, &["-g"])?.trim().to_string();
        quota_watch_stty_apply(&tty, &["-icanon", "-echo", "min", "0", "time", "1"])?;
        Some(Self {
            tty,
            saved_stty_state,
        })
    }

    fn read_command(&mut self) -> Option<QuotaWatchCommand> {
        let mut buf = [0_u8; 8];
        let bytes_read = self.tty.read(&mut buf).ok()?;
        if bytes_read == 0 {
            return None;
        }
        let bytes = &buf[..bytes_read];
        match bytes {
            [b'q' | b'Q', ..] => Some(QuotaWatchCommand::Quit),
            [b'k' | b'K', ..] => Some(QuotaWatchCommand::Up),
            [b'j' | b'J', ..] => Some(QuotaWatchCommand::Down),
            [b's' | b'S', ..] => Some(QuotaWatchCommand::Sort),
            [b'f' | b'F', ..] => Some(QuotaWatchCommand::Filter),
            [0x1b, b'[', b'A', ..] => Some(QuotaWatchCommand::Up),
            [0x1b, b'[', b'B', ..] => Some(QuotaWatchCommand::Down),
            _ => None,
        }
    }
}

impl Drop for QuotaWatchInput {
    fn drop(&mut self) {
        let _ = quota_watch_stty_apply(&self.tty, &[self.saved_stty_state.as_str()]);
    }
}

fn quota_watch_stty_output(tty: &fs::File, args: &[&str]) -> Option<String> {
    let output = Command::new("stty")
        .args(args)
        .stdin(tty.try_clone().ok()?)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8(output.stdout).ok())
        .flatten()
}

fn quota_watch_stty_apply(tty: &fs::File, args: &[&str]) -> Option<()> {
    let output = Command::new("stty")
        .args(args)
        .stdin(tty.try_clone().ok()?)
        .output()
        .ok()?;
    output.status.success().then_some(())
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
        QuotaReportSort::Remaining,
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
    let filtered_reports = filter_quota_reports_by_provider(reports, provider_filter);
    let window = render_quota_reports_window_with_sort(
        &filtered_reports,
        detail,
        quota_watch_available_report_lines(""),
        current_cli_width(),
        scroll_offset,
        true,
        sort,
    );

    let mut output = quota_watch_without_interactive_scroll_notice(&window.output);
    output.push('\n');
    let provider_hint = if provider_filter_locked {
        "provider fixed"
    } else {
        "f provider"
    };
    let range = quota_watch_scroll_range(&window)
        .map(|range| format!(" | {range}"))
        .unwrap_or_default();
    output.push_str(&format!(
        "sort: {} | filter: {}{} | s sort | {} | j/k/Up/Down scroll | q quit",
        sort.label(),
        provider_filter.label(),
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

fn redraw_quota_watch(output: &str) -> Result<()> {
    print!("\x1b[H\x1b[2J{output}");
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
    loop {
        let output = render_profile_quota_watch_output(
            profile_name,
            &quota_watch_updated_at(),
            fetch_profile_quota(provider, codex_home, base_url).map_err(|err| err.to_string()),
        );
        redraw_quota_watch(&output)?;
        thread::sleep(Duration::from_secs(DEFAULT_WATCH_INTERVAL_SECONDS));
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
    let mut input = QuotaWatchInput::open();
    let mut scroll_offset = 0_usize;
    let mut sort = QuotaReportSort::Remaining;
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
            redraw_quota_watch(&output)?;
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

        if let Some(command) = input.as_mut().and_then(QuotaWatchInput::read_command) {
            match apply_quota_watch_command(
                command,
                scroll_offset,
                quota_watch_max_scroll_offset(&snapshot, detail, provider_filter, sort),
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

        thread::sleep(Duration::from_millis(QUOTA_WATCH_INPUT_POLL_MS));
    }
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
