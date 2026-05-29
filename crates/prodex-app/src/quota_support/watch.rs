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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum QuotaProviderFilter {
    All,
    OpenAi,
    Gemini,
}

impl QuotaProviderFilter {
    fn next(self) -> Self {
        match self {
            Self::All => Self::OpenAi,
            Self::OpenAi => Self::Gemini,
            Self::Gemini => Self::All,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::OpenAi => "openai",
            Self::Gemini => "gemini",
        }
    }

    fn matches(self, provider: &ProfileProvider) -> bool {
        match self {
            Self::All => true,
            Self::OpenAi => matches!(provider, ProfileProvider::Openai),
            Self::Gemini => matches!(provider, ProfileProvider::Gemini { .. }),
        }
    }
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
        &collect_all_quota_watch_snapshot(updated, state_result, base_url, &QuotaAuthFilter::All),
        detail,
        0,
        QuotaReportSort::Remaining,
        QuotaProviderFilter::All,
    )
}

fn collect_all_quota_watch_snapshot(
    updated: &str,
    state_result: std::result::Result<AppState, String>,
    base_url: Option<&str>,
    auth_filter: &QuotaAuthFilter,
) -> AllQuotaWatchSnapshot {
    match state_result {
        Ok(state) if !state.profiles.is_empty() => AllQuotaWatchSnapshot::Reports {
            updated: updated.to_string(),
            profile_count: state.profiles.len(),
            reports: collect_quota_reports_with_auth_filter(&state, base_url, auth_filter),
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
) -> String {
    let filtered_reports = filter_quota_reports_by_provider(reports, provider_filter);
    let mut output = render_quota_reports_window_with_sort(
        &filtered_reports,
        detail,
        quota_watch_available_report_lines(""),
        current_cli_width(),
        scroll_offset,
        true,
        sort,
    )
    .output;
    output.push('\n');
    output.push_str(&format!(
        "sort: {} | filter: {} | s sort | f provider | j/k/Up/Down scroll | q quit",
        sort.label(),
        provider_filter.label()
    ));
    output
}

fn filter_quota_reports_by_provider(
    reports: &[QuotaReport],
    provider_filter: QuotaProviderFilter,
) -> Vec<QuotaReport> {
    reports
        .iter()
        .filter(|report| provider_filter.matches(&report.provider))
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
) -> Result<()> {
    let mut input = QuotaWatchInput::open();
    let mut scroll_offset = 0_usize;
    let mut sort = QuotaReportSort::Remaining;
    let mut provider_filter = QuotaProviderFilter::All;
    let mut snapshot = AllQuotaWatchSnapshot::Loading {
        updated: quota_watch_updated_at(),
    };
    let mut refresh = AllQuotaWatchRefresh::new();
    let _ = start_all_quota_watch_refresh(&mut refresh, paths, base_url, &auth_filter);
    let mut redraw_needed = true;
    let mut next_refresh_at = None;

    loop {
        if let Some(next_snapshot) = refresh.take_latest() {
            snapshot = merge_all_quota_watch_snapshot(&snapshot, next_snapshot);
            redraw_needed = true;
            next_refresh_at = Some(quota_watch_next_refresh_at());
        }

        if redraw_needed {
            scroll_offset =
                scroll_offset.min(quota_watch_max_scroll_offset(&snapshot, provider_filter));
            let output = render_all_quota_watch_snapshot(
                &snapshot,
                detail,
                scroll_offset,
                sort,
                provider_filter,
            );
            redraw_quota_watch(&output)?;
            redraw_needed = false;
        }

        if next_refresh_at.is_some_and(|refresh_at| Instant::now() >= refresh_at)
            && start_all_quota_watch_refresh(&mut refresh, paths, base_url, &auth_filter)
        {
            next_refresh_at = None;
            continue;
        }

        if let Some(command) = input.as_mut().and_then(QuotaWatchInput::read_command) {
            match apply_quota_watch_command(
                command,
                scroll_offset,
                quota_watch_max_scroll_offset(&snapshot, provider_filter),
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
                    provider_filter = provider_filter.next();
                    scroll_offset = 0;
                    redraw_needed = true;
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
) -> bool {
    let paths = paths.clone();
    let base_url = base_url.map(str::to_string);
    let auth_filter = auth_filter.clone();
    refresh
        .try_start(move || load_all_quota_watch_snapshot(&paths, base_url.as_deref(), &auth_filter))
}

fn quota_watch_updated_at() -> String {
    Local::now().format("%Y-%m-%d %H:%M:%S %Z").to_string()
}

fn load_all_quota_watch_snapshot(
    paths: &AppPaths,
    base_url: Option<&str>,
    auth_filter: &QuotaAuthFilter,
) -> AllQuotaWatchSnapshot {
    collect_all_quota_watch_snapshot(
        &quota_watch_updated_at(),
        AppState::load(paths).map_err(|err| err.to_string()),
        base_url,
        auth_filter,
    )
}

fn quota_watch_next_refresh_at() -> Instant {
    Instant::now() + Duration::from_secs(DEFAULT_WATCH_INTERVAL_SECONDS)
}

fn quota_watch_max_scroll_offset(
    snapshot: &AllQuotaWatchSnapshot,
    provider_filter: QuotaProviderFilter,
) -> usize {
    match snapshot {
        AllQuotaWatchSnapshot::Reports { reports, .. } => reports
            .iter()
            .filter(|report| provider_filter.matches(&report.provider))
            .count()
            .saturating_sub(1),
        _ => 0,
    }
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
    use super::*;

    fn test_quota_report(
        name: &str,
        result: std::result::Result<ProviderQuotaSnapshot, String>,
    ) -> QuotaReport {
        test_quota_report_with_provider(name, ProfileProvider::Openai, result)
    }

    fn test_quota_report_with_provider(
        name: &str,
        provider: ProfileProvider,
        result: std::result::Result<ProviderQuotaSnapshot, String>,
    ) -> QuotaReport {
        QuotaReport {
            name: name.to_string(),
            active: false,
            auth: AuthSummary {
                label: "chatgpt".to_string(),
                quota_compatible: true,
            },
            provider,
            workspace_id: None,
            result,
            fetched_at: 1_700_000_000,
        }
    }

    fn test_usage(email: &str) -> ProviderQuotaSnapshot {
        ProviderQuotaSnapshot::OpenAi(UsageResponse {
            email: Some(email.to_string()),
            plan_type: Some("plus".to_string()),
            rate_limit: None,
            code_review_rate_limit: None,
            additional_rate_limits: Vec::new(),
        })
    }

    fn test_gemini_quota(email: &str) -> ProviderQuotaSnapshot {
        ProviderQuotaSnapshot::Gemini(GeminiQuotaInfo {
            email: Some(email.to_string()),
            project_id: Some("test-project".to_string()),
            buckets: Vec::new(),
        })
    }

    #[test]
    fn all_quota_watch_refresh_keeps_single_background_refresh_in_flight() {
        let mut refresh = AllQuotaWatchRefresh::new();
        let (release_sender, release_receiver) = mpsc::channel();

        assert!(refresh.try_start(move || {
            release_receiver
                .recv()
                .expect("test refresh should be released");
            AllQuotaWatchSnapshot::Empty {
                updated: "done".to_string(),
            }
        }));
        assert!(!refresh.try_start(|| AllQuotaWatchSnapshot::Empty {
            updated: "second".to_string(),
        }));
        assert!(refresh.take_latest().is_none());

        release_sender
            .send(())
            .expect("test refresh release should send");
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut completed = None;
        while Instant::now() < deadline {
            if let Some(snapshot) = refresh.take_latest() {
                completed = Some(snapshot);
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }

        assert!(matches!(
            completed,
            Some(AllQuotaWatchSnapshot::Empty { .. })
        ));
        assert!(refresh.try_start(|| AllQuotaWatchSnapshot::Empty {
            updated: "third".to_string(),
        }));
    }

    #[test]
    fn all_quota_watch_merge_preserves_previous_successful_report_on_refresh_error() {
        let previous = AllQuotaWatchSnapshot::Reports {
            updated: "before".to_string(),
            profile_count: 1,
            reports: vec![test_quota_report(
                "main",
                Ok(test_usage("main@example.com")),
            )],
        };
        let next = AllQuotaWatchSnapshot::Reports {
            updated: "after".to_string(),
            profile_count: 1,
            reports: vec![test_quota_report("main", Err("HTTP 401".to_string()))],
        };

        let merged = merge_all_quota_watch_snapshot(&previous, next);

        let AllQuotaWatchSnapshot::Reports {
            updated, reports, ..
        } = merged
        else {
            panic!("expected report snapshot");
        };
        assert_eq!(updated, "after");
        assert!(reports[0].result.is_ok());
    }

    #[test]
    fn all_quota_watch_merge_keeps_previous_snapshot_on_full_refresh_error() {
        let previous = AllQuotaWatchSnapshot::Reports {
            updated: "before".to_string(),
            profile_count: 1,
            reports: vec![test_quota_report(
                "main",
                Ok(test_usage("main@example.com")),
            )],
        };

        let merged = merge_all_quota_watch_snapshot(
            &previous,
            AllQuotaWatchSnapshot::Error {
                updated: "after".to_string(),
                message: "load failed".to_string(),
            },
        );

        let AllQuotaWatchSnapshot::Reports {
            updated, reports, ..
        } = merged
        else {
            panic!("expected previous report snapshot");
        };
        assert_eq!(updated, "before");
        assert!(reports[0].result.is_ok());
    }

    #[test]
    fn quota_watch_sort_command_cycles_sort_mode() {
        assert!(matches!(
            apply_quota_watch_command(QuotaWatchCommand::Sort, 3, 10),
            QuotaWatchCommandOutcome::Sort
        ));
        assert_eq!(QuotaReportSort::Remaining.next(), QuotaReportSort::Profile);
        assert_eq!(QuotaReportSort::Plan.next(), QuotaReportSort::Remaining);
    }

    #[test]
    fn quota_watch_filter_command_cycles_provider_filter() {
        assert!(matches!(
            apply_quota_watch_command(QuotaWatchCommand::Filter, 3, 10),
            QuotaWatchCommandOutcome::Filter
        ));
        assert_eq!(QuotaProviderFilter::All.next(), QuotaProviderFilter::OpenAi);
        assert_eq!(
            QuotaProviderFilter::OpenAi.next(),
            QuotaProviderFilter::Gemini
        );
        assert_eq!(QuotaProviderFilter::Gemini.next(), QuotaProviderFilter::All);
    }

    #[test]
    fn all_quota_watch_output_filters_reports_by_provider() {
        let reports = vec![
            test_quota_report_with_provider(
                "openai-main",
                ProfileProvider::Openai,
                Ok(test_usage("main@example.com")),
            ),
            test_quota_report_with_provider(
                "gemini-main",
                ProfileProvider::Gemini {
                    email: "gemini@example.com".to_string(),
                    project_id: Some("test-project".to_string()),
                },
                Ok(test_gemini_quota("gemini@example.com")),
            ),
        ];

        let output = render_all_quota_watch_report_output(
            &reports,
            true,
            0,
            QuotaReportSort::Remaining,
            QuotaProviderFilter::Gemini,
        );

        assert!(output.contains("filter: gemini"));
        assert!(output.contains("gemini-main"));
        assert!(!output.contains("openai-main"));
    }
}
