use anyhow::Result;
use chrono::Local;
use prodex_quota::{format_info_pool_remaining, required_main_window_snapshot_at};
use std::collections::BTreeMap;
use std::io::{self, IsTerminal, Write};
use std::thread;
use std::time::Duration;
use terminal_ui::print_panel;

use crate::{
    AppPaths, AppState, AppStateIoExt, RUNTIME_PROFILE_USAGE_CACHE_STALE_GRACE_SECONDS, StatusArgs,
    collect_run_profile_reports, load_runtime_usage_snapshots,
};

struct StatusOverview {
    updated_at: String,
    active_profile: String,
    profile_count: usize,
    quota: StatusQuotaSummary,
}

#[derive(Debug, Clone, Copy, Default)]
struct StatusQuotaWindow {
    profiles: usize,
    total_remaining: i64,
    earliest_reset_at: Option<i64>,
}

#[derive(Debug, Clone, Copy, Default)]
struct StatusQuotaSummary {
    compatible_profiles: usize,
    unavailable_profiles: usize,
    five_hour: StatusQuotaWindow,
    weekly: StatusQuotaWindow,
}

pub(crate) fn handle_status(args: StatusArgs) -> Result<()> {
    if args.once || !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return print_status_once();
    }
    watch_status(Duration::from_secs(args.interval.max(1)))
}

fn print_status_once() -> Result<()> {
    let paths = AppPaths::discover()?;
    let overview = collect_status_overview(&paths)?;
    print_panel("Status", &status_fields(&overview))?;
    Ok(())
}

fn watch_status(interval: Duration) -> Result<()> {
    let paths = AppPaths::discover()?;
    let mut stdout = io::stdout().lock();
    loop {
        let overview = collect_status_overview(&paths)?;
        write!(stdout, "\x1b[2J\x1b[H")?;
        stdout.flush()?;
        drop(stdout);
        print_panel("Status", &status_fields(&overview))?;
        stdout = io::stdout().lock();
        thread::sleep(interval);
    }
}

fn collect_status_overview(paths: &AppPaths) -> Result<StatusOverview> {
    let state = AppState::load(paths)?;
    let now = Local::now().timestamp();
    let quota = collect_status_quota(paths, &state, now);
    Ok(StatusOverview {
        updated_at: Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        active_profile: state.active_profile.unwrap_or_else(|| "-".to_string()),
        profile_count: state.profiles.len(),
        quota,
    })
}

fn collect_status_quota(paths: &AppPaths, state: &AppState, now: i64) -> StatusQuotaSummary {
    let profile_names = state
        .profiles
        .iter()
        .filter(|(_, profile)| profile.provider.supports_codex_runtime())
        .map(|(name, _)| name.clone())
        .collect::<Vec<_>>();
    let snapshots = load_runtime_usage_snapshots(paths, &state.profiles).unwrap_or_default();
    let reports = collect_run_profile_reports(state, profile_names, None, false);
    status_quota_from_reports(&reports, &snapshots, now)
}

fn status_quota_from_reports(
    reports: &[crate::RunProfileProbeReport],
    snapshots: &BTreeMap<String, crate::RuntimeProfileUsageSnapshot>,
    now: i64,
) -> StatusQuotaSummary {
    let mut summary = StatusQuotaSummary {
        compatible_profiles: reports
            .iter()
            .filter(|report| report.auth.quota_compatible)
            .count(),
        ..StatusQuotaSummary::default()
    };
    for report in reports {
        if !report.auth.quota_compatible {
            continue;
        }
        let usage = match &report.result {
            Ok(usage) => Some(usage.clone()),
            Err(_) => snapshots
                .get(&report.name)
                .filter(|snapshot| {
                    prodex_runtime_quota::runtime_usage_snapshot_is_usable(
                        snapshot,
                        now,
                        RUNTIME_PROFILE_USAGE_CACHE_STALE_GRACE_SECONDS,
                    )
                })
                .map(prodex_runtime_quota::usage_from_runtime_usage_snapshot),
        };
        let Some(usage) = usage else {
            summary.unavailable_profiles += 1;
            continue;
        };
        let five_hour = required_main_window_snapshot_at(&usage, "5h", now);
        let weekly = required_main_window_snapshot_at(&usage, "weekly", now);
        if five_hour.is_none() && weekly.is_none() {
            summary.unavailable_profiles += 1;
            continue;
        }
        if let Some(window) = five_hour {
            add_quota_window(&mut summary.five_hour, window);
        }
        if let Some(window) = weekly {
            add_quota_window(&mut summary.weekly, window);
        }
    }
    summary
}

fn add_quota_window(window: &mut StatusQuotaWindow, snapshot: crate::MainWindowSnapshot) {
    window.profiles += 1;
    window.total_remaining = window
        .total_remaining
        .saturating_add(snapshot.remaining_percent);
    if snapshot.reset_at != i64::MAX {
        window.earliest_reset_at = Some(
            window
                .earliest_reset_at
                .map_or(snapshot.reset_at, |current| current.min(snapshot.reset_at)),
        );
    }
}

fn status_fields(overview: &StatusOverview) -> Vec<(String, String)> {
    vec![
        (
            "Profile".to_string(),
            format!(
                "configured={}, pool={}, quota-compatible={}, unavailable={}",
                overview.active_profile,
                overview.profile_count,
                overview.quota.compatible_profiles,
                overview.quota.unavailable_profiles
            ),
        ),
        (
            "5h quota".to_string(),
            format_info_pool_remaining(
                overview.quota.five_hour.total_remaining,
                overview.quota.five_hour.profiles,
                overview.quota.five_hour.earliest_reset_at,
            ),
        ),
        (
            "Weekly quota".to_string(),
            format_info_pool_remaining(
                overview.quota.weekly.total_remaining,
                overview.quota.weekly.profiles,
                overview.quota.weekly.earliest_reset_at,
            ),
        ),
        ("Updated".to_string(), overview.updated_at.clone()),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quota_window_aggregates_remaining_and_earliest_reset() {
        let mut window = StatusQuotaWindow::default();
        add_quota_window(
            &mut window,
            crate::MainWindowSnapshot {
                remaining_percent: 40,
                reset_at: 100,
                pressure_score: 1,
            },
        );
        add_quota_window(
            &mut window,
            crate::MainWindowSnapshot {
                remaining_percent: 60,
                reset_at: 90,
                pressure_score: 1,
            },
        );
        assert_eq!(window.profiles, 2);
        assert_eq!(window.total_remaining, 100);
        assert_eq!(window.earliest_reset_at, Some(90));
    }
}
