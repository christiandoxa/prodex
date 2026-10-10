use super::*;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[path = "watch_tui.rs"]
mod watch_tui;

#[cfg(test)]
use self::watch_tui::quota_watch_quit_key;
#[cfg(test)]
use self::watch_tui::{
    AllQuotaWatchTuiRow, AllQuotaWatchTuiTable, build_all_quota_watch_tui_frame,
    build_profile_quota_watch_tui_frame, quota_human_tui_spans, quota_watch_overview_height,
    quota_watch_table_text, quota_watch_tui_max_scroll_offset_for_snapshot,
    quota_watch_tui_table_lines,
};
pub(crate) use self::watch_tui::{
    quota_watch_enabled, render_all_quota_reports_once_tui, render_profile_quota_once_tui,
    watch_all_quotas, watch_quota,
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

fn quota_watch_command(key: KeyEvent) -> Option<QuotaWatchCommand> {
    let (key_kind, char_code) = match key.code {
        KeyCode::Esc => (1, None),
        KeyCode::Char(value) => (2, Some(value)),
        KeyCode::Down => (3, None),
        KeyCode::Up => (4, None),
        _ => (0, None),
    };
    match prodex_mojo_core::quota_watch_policy::key_action(
        key_kind,
        char_code,
        key.modifiers.contains(KeyModifiers::CONTROL),
    )
    .expect("Mojo quota-watch key classification failed")
    {
        prodex_mojo_core::quota_watch_policy::ACTION_UP => Some(QuotaWatchCommand::Up),
        prodex_mojo_core::quota_watch_policy::ACTION_DOWN => Some(QuotaWatchCommand::Down),
        prodex_mojo_core::quota_watch_policy::ACTION_SORT => Some(QuotaWatchCommand::Sort),
        prodex_mojo_core::quota_watch_policy::ACTION_FILTER => Some(QuotaWatchCommand::Filter),
        prodex_mojo_core::quota_watch_policy::ACTION_UPDATE => Some(QuotaWatchCommand::Update),
        prodex_mojo_core::quota_watch_policy::ACTION_QUIT => Some(QuotaWatchCommand::Quit),
        -1 => None,
        _ => unreachable!("validated Mojo quota-watch action"),
    }
}

fn quota_watch_provider_filter_kind(provider_filter: QuotaProviderFilter) -> i64 {
    provider_filter.policy_kind()
}

fn quota_watch_snapshot_kind(report: &QuotaReport) -> (i64, Option<&str>) {
    match report.result.as_ref() {
        Ok(ProviderQuotaSnapshot::OpenAi(_)) => {
            (prodex_mojo_core::quota_watch_policy::SNAPSHOT_OPENAI, None)
        }
        Ok(ProviderQuotaSnapshot::Gemini(_)) => {
            (prodex_mojo_core::quota_watch_policy::SNAPSHOT_GEMINI, None)
        }
        Ok(ProviderQuotaSnapshot::Copilot(_)) => {
            (prodex_mojo_core::quota_watch_policy::SNAPSHOT_COPILOT, None)
        }
        Ok(ProviderQuotaSnapshot::External(info)) => (
            prodex_mojo_core::quota_watch_policy::SNAPSHOT_EXTERNAL,
            Some(info.provider.as_str()),
        ),
        Err(_) => (prodex_mojo_core::quota_watch_policy::SNAPSHOT_NONE, None),
    }
}
#[derive(Debug, Clone)]
struct ProfileQuotaWatchSnapshot {
    updated: String,
    quota: std::result::Result<ProviderQuotaSnapshot, String>,
}

struct QuotaWatchRefresh<T> {
    receiver: Receiver<T>,
    sender: mpsc::Sender<T>,
    in_flight: bool,
}

type AllQuotaWatchRefresh = QuotaWatchRefresh<AllQuotaWatchSnapshot>;
type ProfileQuotaWatchRefresh = QuotaWatchRefresh<ProfileQuotaWatchSnapshot>;

impl<T: Send + 'static> QuotaWatchRefresh<T> {
    fn new() -> Self {
        let (sender, receiver) = mpsc::channel();
        Self {
            receiver,
            sender,
            in_flight: false,
        }
    }

    #[cfg(test)]
    fn try_start<F>(&mut self, load: F) -> bool
    where
        F: FnOnce() -> T + Send + 'static,
    {
        if self.in_flight {
            return false;
        }

        let sender = self.sender.clone();
        if thread::Builder::new()
            .name("prodex-quota-refresh".to_string())
            .spawn(move || {
                let _ = sender.send(load());
            })
            .is_err()
        {
            return false;
        }
        self.in_flight = true;
        true
    }

    fn try_start_catching_panic<F>(&mut self, load: F, fallback: T) -> bool
    where
        F: FnOnce() -> T + Send + 'static,
        T: Clone,
    {
        if self.in_flight {
            return false;
        }

        let sender = self.sender.clone();
        let panic_fallback = fallback.clone();
        if thread::Builder::new()
            .name("prodex-quota-refresh".to_string())
            .spawn(move || {
                let snapshot = crate::runtime_panic::catch_runtime_unwind_silently(load)
                    .unwrap_or(panic_fallback);
                let _ = sender.send(snapshot);
            })
            .is_err()
        {
            let _ = self.sender.send(fallback);
            return false;
        }
        self.in_flight = true;
        true
    }

    fn take_latest(&mut self) -> Option<T> {
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

pub(crate) fn render_profile_quota_watch_output(
    profile_name: &str,
    _updated: &str,
    quota_result: std::result::Result<ProviderQuotaSnapshot, String>,
    detail: bool,
) -> String {
    match quota_result {
        Ok(quota) => render_profile_quota_snapshot_with_detail(profile_name, &quota, detail),
        Err(err) => render_quota_watch_error_panel(&format!("Quota {profile_name}"), &err),
    }
}

fn collect_all_quota_watch_snapshot(
    updated: &str,
    state_result: std::result::Result<AppState, String>,
    base_url: Option<&str>,
    auth_filter: &QuotaAuthFilter,
    provider_filter: QuotaProviderFilter,
) -> AllQuotaWatchSnapshot {
    match state_result {
        Ok(state) => {
            let reports =
                collect_quota_reports_with_filters(&state, base_url, auth_filter, provider_filter);
            if reports.is_empty() {
                AllQuotaWatchSnapshot::Empty {
                    updated: updated.to_string(),
                }
            } else {
                AllQuotaWatchSnapshot::Reports {
                    updated: updated.to_string(),
                    profile_count: state.profiles.len(),
                    reports,
                }
            }
        }
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
    match prodex_mojo_core::quota_watch_policy::scroll_kind(
        window.total_profiles,
        window.shown_profiles,
        window.hidden_before,
        window.hidden_after,
    )
    .expect("Mojo quota-watch scroll visibility policy failed")
    {
        0 => None,
        1 => Some(format!(
            "0/{} visible; {} above, {} below",
            window.total_profiles, window.hidden_before, window.hidden_after
        )),
        2 => {
            let first_visible = window.start_profile.saturating_add(1);
            let last_visible = window.start_profile.saturating_add(window.shown_profiles);
            Some(format!(
                "{first_visible}-{last_visible}/{}; {} above, {} below",
                window.total_profiles, window.hidden_before, window.hidden_after
            ))
        }
        _ => unreachable!("validated Mojo quota-watch scroll kind"),
    }
}

fn filter_quota_reports_by_provider(
    reports: &[QuotaReport],
    provider_filter: QuotaProviderFilter,
) -> Vec<QuotaReport> {
    reports
        .iter()
        .filter(|report| {
            let (snapshot_kind, provider) = quota_watch_snapshot_kind(report);
            prodex_mojo_core::quota_watch_policy::filter_matches(
                quota_watch_provider_filter_kind(provider_filter),
                snapshot_kind,
                &report.auth.label,
                provider,
            )
            .expect("Mojo quota-watch provider filter policy failed")
        })
        .cloned()
        .collect()
}

fn quota_watch_available_report_lines(header: &str) -> Option<usize> {
    let terminal_height = terminal_height_lines()?;
    let reserved = header.lines().count().saturating_add(2);
    Some(
        prodex_mojo_core::quota_watch_policy::available_lines(terminal_height, reserved)
            .expect("Mojo quota-watch report viewport policy failed"),
    )
}

fn quota_watch_tui_fallback_message(err: &anyhow::Error) -> String {
    format!(
        "prodex quota TUI unavailable, falling back to plain watch: {}",
        redaction_redact_secret_like_text(&format!("{err:#}"))
    )
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
    refresh.try_start_catching_panic(
        move || {
            load_all_quota_watch_snapshot(
                &paths,
                base_url.as_deref(),
                &auth_filter,
                provider_filter,
            )
        },
        AllQuotaWatchSnapshot::Error {
            updated: quota_watch_updated_at(),
            message: "quota refresh failed unexpectedly".to_string(),
        },
    )
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
    Instant::now() + quota_watch_refresh_duration()
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
    prodex_mojo_core::quota_watch_policy::cache_is_eligible(
        detail,
        i64::from(!matches!(auth_filter, QuotaAuthFilter::All)),
        quota_watch_provider_filter_kind(provider_filter),
    )
    .expect("Mojo quota-watch cache eligibility policy failed")
}

fn all_quota_watch_refresh_interval(
    _snapshot: &AllQuotaWatchSnapshot,
    _detail: bool,
    _now: i64,
) -> Duration {
    quota_watch_refresh_duration()
}

fn quota_watch_refresh_duration() -> Duration {
    Duration::from_secs(
        prodex_mojo_core::quota_watch_policy::live_refresh_seconds()
            .expect("Mojo quota-watch live refresh policy failed"),
    )
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
    let plan = prodex_mojo_core::quota_watch_policy::merge_plan(
        all_quota_watch_snapshot_kind(previous),
        all_quota_watch_snapshot_kind(&next),
    )
    .expect("Mojo quota-watch snapshot merge policy failed");
    match plan {
        prodex_mojo_core::quota_watch_policy::SNAPSHOT_KEEP_PREVIOUS => previous.clone(),
        prodex_mojo_core::quota_watch_policy::SNAPSHOT_MERGE_REPORTS => {
            let AllQuotaWatchSnapshot::Reports {
                reports: previous_reports,
                ..
            } = previous
            else {
                unreachable!("Mojo quota-watch merge plan requires report snapshot");
            };
            let AllQuotaWatchSnapshot::Reports {
                updated,
                profile_count,
                mut reports,
            } = next
            else {
                unreachable!("Mojo quota-watch merge plan requires report snapshot");
            };
            preserve_previous_successful_quota_reports(previous_reports, &mut reports);
            AllQuotaWatchSnapshot::Reports {
                updated,
                profile_count,
                reports,
            }
        }
        prodex_mojo_core::quota_watch_policy::SNAPSHOT_USE_NEXT => next,
        _ => unreachable!("validated Mojo quota-watch merge plan"),
    }
}

fn all_quota_watch_snapshot_kind(snapshot: &AllQuotaWatchSnapshot) -> i64 {
    match snapshot {
        AllQuotaWatchSnapshot::Loading { .. } => 0,
        AllQuotaWatchSnapshot::Reports { .. } => 1,
        AllQuotaWatchSnapshot::Empty { .. } => 2,
        AllQuotaWatchSnapshot::Error { .. } => 3,
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
        if report.result.is_err() && auth_backoff_profiles.contains(&report.name) {
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
        let Some(previous) = previous_reports.iter().find(|previous| {
            previous.name == report.name
                && previous.result.is_ok()
                && previous.auth.label == report.auth.label
        }) else {
            continue;
        };
        if prodex_mojo_core::quota_watch_policy::preserve_report(
            previous.result.is_ok(),
            report.result.is_ok(),
            report
                .result
                .as_ref()
                .err()
                .is_some_and(|error| quota_watch_error_is_auth_failure(error)),
            previous.name == report.name,
            previous.auth.label == report.auth.label,
        )
        .expect("Mojo quota-watch report preservation policy failed")
        {
            *report = previous.clone();
        }
    }
}

fn quota_watch_error_is_auth_failure(error: &str) -> bool {
    prodex_mojo_core::quota::quota_error_auth_failure(error, true)
        .expect("Mojo quota-watch error category policy failed")
}

fn apply_quota_watch_command(
    command: QuotaWatchCommand,
    scroll_offset: usize,
    max_scroll_offset: usize,
) -> QuotaWatchCommandOutcome {
    let action = match command {
        QuotaWatchCommand::Up => prodex_mojo_core::quota_watch_policy::ACTION_UP,
        QuotaWatchCommand::Down => prodex_mojo_core::quota_watch_policy::ACTION_DOWN,
        QuotaWatchCommand::Sort => prodex_mojo_core::quota_watch_policy::ACTION_SORT,
        QuotaWatchCommand::Filter => prodex_mojo_core::quota_watch_policy::ACTION_FILTER,
        QuotaWatchCommand::Update => prodex_mojo_core::quota_watch_policy::ACTION_UPDATE,
        QuotaWatchCommand::Quit => prodex_mojo_core::quota_watch_policy::ACTION_QUIT,
    };
    let (outcome, next_offset) =
        prodex_mojo_core::quota_watch_policy::action(action, scroll_offset, max_scroll_offset)
            .expect("Mojo quota-watch action policy failed");
    match outcome {
        prodex_mojo_core::quota_watch_policy::OUTCOME_CONTINUE => {
            QuotaWatchCommandOutcome::Continue(next_offset)
        }
        prodex_mojo_core::quota_watch_policy::OUTCOME_SORT => QuotaWatchCommandOutcome::Sort,
        prodex_mojo_core::quota_watch_policy::OUTCOME_FILTER => QuotaWatchCommandOutcome::Filter,
        prodex_mojo_core::quota_watch_policy::OUTCOME_UPDATE => QuotaWatchCommandOutcome::Update,
        prodex_mojo_core::quota_watch_policy::OUTCOME_QUIT => QuotaWatchCommandOutcome::Quit,
        _ => unreachable!("validated Mojo quota-watch action outcome"),
    }
}

#[cfg(test)]
mod tests {
    include!("../../tests/src/quota_support/watch_unit.rs");
    include!("../../tests/src/quota_support/watch_refresh_unit.rs");
    include!("../../tests/src/quota_support/watch_tui_frame_unit.rs");
    include!("../../tests/src/quota_support/watch_tui_table_unit.rs");
}
