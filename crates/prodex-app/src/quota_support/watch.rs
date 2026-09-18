use super::*;
use std::io::{self, IsTerminal};
use std::thread;
use std::time::Duration;

#[derive(Debug, Clone)]
enum AllQuotaWatchSnapshot {
    Reports(Vec<QuotaReport>),
    Empty,
    Error(String),
}

#[derive(Debug, Clone)]
struct ProfileQuotaWatchSnapshot {
    quota: std::result::Result<ProviderQuotaSnapshot, String>,
}

pub(crate) fn quota_watch_enabled(args: &QuotaArgs) -> bool {
    !args.raw && !args.once
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

#[cfg(test)]
pub(crate) fn render_all_quota_watch_output(
    _updated: &str,
    state_result: std::result::Result<AppState, String>,
    base_url: Option<&str>,
    detail: bool,
) -> String {
    let snapshot = load_all_quota_watch_snapshot(
        state_result,
        base_url,
        &QuotaAuthFilter::All,
        QuotaProviderFilter::All,
    );
    render_all_quota_watch_snapshot(&snapshot, detail, QuotaReportSort::Current)
}

pub(crate) fn watch_quota(
    profile_name: &str,
    provider: &ProfileProvider,
    codex_home: &Path,
    detail: bool,
    base_url: Option<&str>,
) -> Result<()> {
    let mut previous: Option<ProfileQuotaWatchSnapshot> = None;
    loop {
        let next = ProfileQuotaWatchSnapshot {
            quota: fetch_profile_quota(provider, codex_home, base_url)
                .map_err(|err| quota_error_message(&err)),
        };
        let snapshot = merge_profile_quota_watch_snapshot(previous.as_ref(), next);
        let output =
            render_profile_quota_watch_output(profile_name, "", snapshot.quota.clone(), detail);
        print_quota_watch_snapshot(&output)?;
        previous = Some(snapshot);
        thread::sleep(Duration::from_secs(DEFAULT_WATCH_INTERVAL_SECONDS));
    }
}

pub(crate) fn watch_all_quotas(
    paths: &AppPaths,
    base_url: Option<&str>,
    detail: bool,
    auth_filter: QuotaAuthFilter,
    provider_filter: QuotaProviderFilter,
    _provider_filter_locked: bool,
) -> Result<()> {
    let mut previous: Option<AllQuotaWatchSnapshot> = None;
    loop {
        let next = load_all_quota_watch_snapshot(
            AppState::load(paths).map_err(|err| err.to_string()),
            base_url,
            &auth_filter,
            provider_filter,
        );
        let snapshot = merge_all_quota_watch_snapshot(previous.as_ref(), next);
        if let AllQuotaWatchSnapshot::Reports(reports) = &snapshot
            && quota_watch_runtime_usage_cache_enabled(detail, &auth_filter, provider_filter)
        {
            save_quota_watch_runtime_usage_cache(
                paths,
                reports,
                Duration::from_secs(DEFAULT_WATCH_INTERVAL_SECONDS),
            );
        }
        let output = render_all_quota_watch_snapshot(&snapshot, detail, QuotaReportSort::Current);
        print_quota_watch_snapshot(&output)?;
        previous = Some(snapshot);
        thread::sleep(Duration::from_secs(DEFAULT_WATCH_INTERVAL_SECONDS));
    }
}

fn load_all_quota_watch_snapshot(
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
                AllQuotaWatchSnapshot::Empty
            } else {
                AllQuotaWatchSnapshot::Reports(reports)
            }
        }
        Err(err) => AllQuotaWatchSnapshot::Error(err),
    }
}

fn merge_profile_quota_watch_snapshot(
    previous: Option<&ProfileQuotaWatchSnapshot>,
    next: ProfileQuotaWatchSnapshot,
) -> ProfileQuotaWatchSnapshot {
    if next.quota.is_err()
        && let Some(previous) = previous
        && previous.quota.is_ok()
    {
        return previous.clone();
    }
    next
}

fn merge_all_quota_watch_snapshot(
    previous: Option<&AllQuotaWatchSnapshot>,
    next: AllQuotaWatchSnapshot,
) -> AllQuotaWatchSnapshot {
    match (previous, next) {
        (
            Some(AllQuotaWatchSnapshot::Reports(previous_reports)),
            AllQuotaWatchSnapshot::Reports(mut reports),
        ) => {
            preserve_previous_successful_quota_reports(previous_reports, &mut reports);
            AllQuotaWatchSnapshot::Reports(reports)
        }
        (Some(previous @ AllQuotaWatchSnapshot::Reports(_)), AllQuotaWatchSnapshot::Error(_)) => {
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
        if report.result.is_ok() || quota_watch_error_is_auth_failure(&report.result) {
            continue;
        }
        if let Some(previous) = previous_reports.iter().find(|previous| {
            previous.name == report.name
                && previous.result.is_ok()
                && previous.auth.label == report.auth.label
        }) {
            *report = previous.clone();
        }
    }
}

fn quota_watch_error_is_auth_failure(
    result: &std::result::Result<ProviderQuotaSnapshot, String>,
) -> bool {
    let Err(error) = result else {
        return false;
    };
    let lower = error.to_ascii_lowercase();
    lower.contains("401") || lower.contains("unauthorized") || lower.contains("token <redacted>")
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

fn render_all_quota_watch_snapshot(
    snapshot: &AllQuotaWatchSnapshot,
    detail: bool,
    sort: QuotaReportSort,
) -> String {
    match snapshot {
        AllQuotaWatchSnapshot::Reports(reports) => {
            let max_lines = terminal_height_lines().map(|height| height.saturating_sub(2));
            render_quota_reports_window_with_sort(
                reports,
                detail,
                max_lines,
                current_cli_width(),
                0,
                false,
                sort,
            )
            .output
        }
        AllQuotaWatchSnapshot::Empty => {
            render_quota_watch_error_panel("Quota", "No profiles configured")
        }
        AllQuotaWatchSnapshot::Error(message) => render_quota_watch_error_panel("Quota", message),
    }
}

fn render_quota_watch_error_panel(title: &str, message: &str) -> String {
    format!(
        "{title}
Status: {}",
        redaction_redact_secret_like_text(message)
    )
}

fn print_quota_watch_snapshot(output: &str) -> Result<()> {
    let mut stdout = io::stdout().lock();
    if io::stdout().is_terminal() {
        write!(stdout, "\x1b[2J\x1b[H")?;
    }
    writeln!(stdout, "{output}")?;
    stdout.flush().context("failed to flush quota watch output")
}
