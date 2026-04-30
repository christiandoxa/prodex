use chrono::{Local, TimeZone};
use terminal_ui::{
    current_cli_width, format_field_lines_with_layout, panel_label_width, section_header_with_width,
};

use super::{
    BlockedLimit, MainWindowSnapshot, RuntimeQuotaPressureBand, RuntimeQuotaSummary,
    RuntimeQuotaWindowStatus, RuntimeQuotaWindowSummary, UsageResponse, UsageWindow, WindowPair,
};

pub fn main_window_snapshots(
    usage: &UsageResponse,
) -> Option<(MainWindowSnapshot, MainWindowSnapshot)> {
    Some((
        required_main_window_snapshot(usage, "5h")?,
        required_main_window_snapshot(usage, "weekly")?,
    ))
}

pub fn required_main_window_snapshot(
    usage: &UsageResponse,
    label: &str,
) -> Option<MainWindowSnapshot> {
    required_main_window_snapshot_at(usage, label, Local::now().timestamp())
}

pub fn required_main_window_snapshot_at(
    usage: &UsageResponse,
    label: &str,
    now: i64,
) -> Option<MainWindowSnapshot> {
    let window = find_main_window(usage.rate_limit.as_ref()?, label)?;
    let remaining_percent = remaining_percent(window.used_percent);
    let reset_at = window.reset_at.unwrap_or(i64::MAX);
    let seconds_until_reset = if reset_at == i64::MAX {
        i64::MAX
    } else {
        (reset_at - now).max(0)
    };
    let pressure_score = seconds_until_reset
        .saturating_mul(1_000)
        .checked_div(remaining_percent.max(1))
        .unwrap_or(i64::MAX);

    Some(MainWindowSnapshot {
        remaining_percent,
        reset_at,
        pressure_score,
    })
}

pub fn quota_window_summary(usage: &UsageResponse, label: &str) -> RuntimeQuotaWindowSummary {
    let Some(window) = required_main_window_snapshot(usage, label) else {
        return RuntimeQuotaWindowSummary {
            status: RuntimeQuotaWindowStatus::Unknown,
            remaining_percent: 0,
            reset_at: i64::MAX,
        };
    };

    let status = if window.remaining_percent == 0 {
        RuntimeQuotaWindowStatus::Exhausted
    } else if window.remaining_percent <= 5 {
        RuntimeQuotaWindowStatus::Critical
    } else if window.remaining_percent <= 15 {
        RuntimeQuotaWindowStatus::Thin
    } else {
        RuntimeQuotaWindowStatus::Ready
    };

    RuntimeQuotaWindowSummary {
        status,
        remaining_percent: window.remaining_percent,
        reset_at: window.reset_at,
    }
}

pub fn quota_summary(usage: &UsageResponse) -> RuntimeQuotaSummary {
    let five_hour = quota_window_summary(usage, "5h");
    let weekly = quota_window_summary(usage, "weekly");
    RuntimeQuotaSummary {
        five_hour,
        weekly,
        route_band: quota_pressure_band_from_windows(five_hour, weekly),
    }
}

pub fn quota_pressure_band_from_windows(
    five_hour: RuntimeQuotaWindowSummary,
    weekly: RuntimeQuotaWindowSummary,
) -> RuntimeQuotaPressureBand {
    [five_hour.status, weekly.status]
        .into_iter()
        .map(quota_pressure_band_from_window_status)
        .max()
        .unwrap_or(RuntimeQuotaPressureBand::Unknown)
}

pub fn quota_pressure_band_from_window_status(
    status: RuntimeQuotaWindowStatus,
) -> RuntimeQuotaPressureBand {
    match status {
        RuntimeQuotaWindowStatus::Ready => RuntimeQuotaPressureBand::Healthy,
        RuntimeQuotaWindowStatus::Thin => RuntimeQuotaPressureBand::Thin,
        RuntimeQuotaWindowStatus::Critical => RuntimeQuotaPressureBand::Critical,
        RuntimeQuotaWindowStatus::Exhausted => RuntimeQuotaPressureBand::Exhausted,
        RuntimeQuotaWindowStatus::Unknown => RuntimeQuotaPressureBand::Unknown,
    }
}

pub fn earliest_required_main_reset_epoch(usage: &UsageResponse) -> Option<i64> {
    ["5h", "weekly"]
        .into_iter()
        .filter_map(|label| {
            find_main_window(usage.rate_limit.as_ref()?, label).and_then(|window| window.reset_at)
        })
        .min()
}

pub fn format_main_windows(usage: &UsageResponse) -> String {
    usage
        .rate_limit
        .as_ref()
        .map(format_window_pair)
        .unwrap_or_else(|| "-".to_string())
}

pub fn format_main_windows_compact(usage: &UsageResponse) -> String {
    usage
        .rate_limit
        .as_ref()
        .map(format_window_pair_compact)
        .unwrap_or_else(|| "-".to_string())
}

pub fn format_main_reset_summary(usage: &UsageResponse) -> String {
    usage
        .rate_limit
        .as_ref()
        .map(format_main_reset_pair)
        .unwrap_or_else(|| "5h unavailable | weekly unavailable".to_string())
}

fn format_main_reset_pair(rate_limit: &WindowPair) -> String {
    [
        format_main_reset_window(rate_limit, "5h"),
        format_main_reset_window(rate_limit, "weekly"),
    ]
    .join(" | ")
}

fn format_main_reset_window(rate_limit: &WindowPair, label: &str) -> String {
    match find_main_window(rate_limit, label) {
        Some(window) => {
            let reset = window
                .reset_at
                .map(|epoch| format_precise_reset_time(Some(epoch)))
                .unwrap_or_else(|| "unknown".to_string());
            format!("{label} {reset}")
        }
        None => format!("{label} unavailable"),
    }
}

fn format_window_pair(rate_limit: &WindowPair) -> String {
    let mut parts = Vec::new();
    if let Some(primary) = rate_limit.primary_window.as_ref() {
        parts.push(format_window_status(primary));
    }
    if let Some(secondary) = rate_limit.secondary_window.as_ref() {
        parts.push(format_window_status(secondary));
    }

    if parts.is_empty() {
        "-".to_string()
    } else {
        parts.join(" | ")
    }
}

fn format_window_pair_compact(rate_limit: &WindowPair) -> String {
    let mut parts = Vec::new();
    if let Some(primary) = rate_limit.primary_window.as_ref() {
        parts.push(format_window_status_compact(primary));
    }
    if let Some(secondary) = rate_limit.secondary_window.as_ref() {
        parts.push(format_window_status_compact(secondary));
    }

    if parts.is_empty() {
        "-".to_string()
    } else {
        parts.join(" | ")
    }
}

fn format_named_window_status(label: &str, window: &UsageWindow) -> String {
    format!("{label}: {}", format_window_details(window))
}

pub fn format_window_status(window: &UsageWindow) -> String {
    format_named_window_status(&window_label(window.limit_window_seconds), window)
}

pub fn format_window_status_compact(window: &UsageWindow) -> String {
    let label = window_label(window.limit_window_seconds);
    match window.used_percent {
        Some(used) => {
            let remaining = remaining_percent(Some(used));
            format!("{label} {remaining}% left")
        }
        None => format!("{label} ?"),
    }
}

fn format_window_details(window: &UsageWindow) -> String {
    let reset = format_reset_time(window.reset_at);
    match window.used_percent {
        Some(used) => {
            let remaining = remaining_percent(window.used_percent);
            format!("{remaining}% left ({used}% used), resets {reset}")
        }
        None => format!("usage unknown, resets {reset}"),
    }
}

pub fn collect_blocked_limits(
    usage: &UsageResponse,
    include_code_review: bool,
) -> Vec<BlockedLimit> {
    let mut blocked = Vec::new();

    if let Some(main) = usage.rate_limit.as_ref() {
        push_required_main_window(&mut blocked, main, "5h");
        push_required_main_window(&mut blocked, main, "weekly");
    } else {
        blocked.push(BlockedLimit {
            message: "5h quota unavailable".to_string(),
        });
        blocked.push(BlockedLimit {
            message: "weekly quota unavailable".to_string(),
        });
    }

    for additional in &usage.additional_rate_limits {
        let label = additional
            .limit_name
            .as_deref()
            .or(additional.metered_feature.as_deref());
        push_blocked_window(
            &mut blocked,
            label,
            additional.rate_limit.primary_window.as_ref(),
        );
        push_blocked_window(
            &mut blocked,
            label,
            additional.rate_limit.secondary_window.as_ref(),
        );
    }

    if include_code_review && let Some(code_review) = usage.code_review_rate_limit.as_ref() {
        push_blocked_window(
            &mut blocked,
            Some("code-review"),
            code_review.primary_window.as_ref(),
        );
        push_blocked_window(
            &mut blocked,
            Some("code-review"),
            code_review.secondary_window.as_ref(),
        );
    }

    blocked
}

fn push_required_main_window(
    blocked: &mut Vec<BlockedLimit>,
    main: &WindowPair,
    required_label: &str,
) {
    let Some(window) = find_main_window(main, required_label) else {
        blocked.push(BlockedLimit {
            message: format!("{required_label} quota unavailable"),
        });
        return;
    };

    match window.used_percent {
        Some(used) if used < 100 => {}
        Some(_) => blocked.push(BlockedLimit {
            message: format!(
                "{required_label} exhausted until {}",
                format_reset_time(window.reset_at)
            ),
        }),
        None => blocked.push(BlockedLimit {
            message: format!("{required_label} quota unknown"),
        }),
    }
}

pub fn find_main_window<'a>(main: &'a WindowPair, expected_label: &str) -> Option<&'a UsageWindow> {
    [main.primary_window.as_ref(), main.secondary_window.as_ref()]
        .into_iter()
        .flatten()
        .find(|window| window_label(window.limit_window_seconds) == expected_label)
}

fn push_blocked_window(
    blocked: &mut Vec<BlockedLimit>,
    name: Option<&str>,
    window: Option<&UsageWindow>,
) {
    let Some(window) = window else {
        return;
    };
    let Some(used) = window.used_percent else {
        return;
    };
    if used < 100 {
        return;
    }

    let label = match name {
        Some(base) if !base.is_empty() => {
            format!("{base} {}", window_label(window.limit_window_seconds))
        }
        _ => window_label(window.limit_window_seconds),
    };

    blocked.push(BlockedLimit {
        message: format!(
            "{label} exhausted until {}",
            format_reset_time(window.reset_at)
        ),
    });
}

pub fn format_blocked_limits(blocked: &[BlockedLimit]) -> String {
    blocked
        .iter()
        .map(|limit| limit.message.clone())
        .collect::<Vec<_>>()
        .join(", ")
}

pub fn remaining_percent(used_percent: Option<i64>) -> i64 {
    let Some(used) = used_percent else {
        return 0;
    };
    (100 - used).clamp(0, 100)
}

pub fn window_label(seconds: Option<i64>) -> String {
    let Some(seconds) = seconds else {
        return "usage".to_string();
    };

    if (17_700..=18_300).contains(&seconds) {
        return "5h".to_string();
    }
    if (601_200..=608_400).contains(&seconds) {
        return "weekly".to_string();
    }
    if (2_505_600..=2_678_400).contains(&seconds) {
        return "monthly".to_string();
    }

    format!("{seconds}s")
}

pub fn format_reset_time(epoch: Option<i64>) -> String {
    format_local_epoch(epoch, "%Y-%m-%d %H:%M %:z")
}

pub fn format_precise_reset_time(epoch: Option<i64>) -> String {
    format_local_epoch(epoch, "%Y-%m-%d %H:%M:%S %:z")
}

pub fn format_quota_snapshot_time(epoch: Option<i64>) -> String {
    format_local_epoch(epoch, "%Y-%m-%d %H:%M:%S %:z")
}

fn format_local_epoch(epoch: Option<i64>, pattern: &str) -> String {
    let Some(epoch) = epoch else {
        return "-".to_string();
    };

    Local
        .timestamp_opt(epoch, 0)
        .single()
        .map(|dt| dt.format(pattern).to_string())
        .unwrap_or_else(|| epoch.to_string())
}

fn display_optional(value: Option<&str>) -> &str {
    value.unwrap_or("-")
}

pub fn render_profile_quota(profile_name: &str, usage: &UsageResponse) -> String {
    render_profile_quota_with_width(profile_name, usage, current_cli_width())
}

pub fn render_profile_quota_with_width(
    profile_name: &str,
    usage: &UsageResponse,
    total_width: usize,
) -> String {
    let blocked = collect_blocked_limits(usage, false);
    let status = if blocked.is_empty() {
        "Ready".to_string()
    } else {
        format!("Blocked ({})", format_blocked_limits(&blocked))
    };
    let mut fields = Vec::new();
    fields.push(("Profile".to_string(), profile_name.to_string()));
    fields.push((
        "Account".to_string(),
        display_optional(usage.email.as_deref()).to_string(),
    ));
    fields.push((
        "Plan".to_string(),
        display_optional(usage.plan_type.as_deref()).to_string(),
    ));
    fields.push(("Status".to_string(), status));
    fields.push(("Main".to_string(), format_main_windows(usage)));

    if let Some(code_review) = usage.code_review_rate_limit.as_ref() {
        fields.push(("Code review".to_string(), format_window_pair(code_review)));
    }

    fields.extend(format_additional_limits(usage));
    render_panel_with_width(&format!("Quota {profile_name}"), &fields, total_width)
}

fn render_panel_with_width(title: &str, fields: &[(String, String)], total_width: usize) -> String {
    let label_width = panel_label_width(fields, total_width);
    let mut lines = vec![section_header_with_width(title, total_width)];
    for (label, value) in fields {
        lines.extend(format_field_lines_with_layout(
            label,
            value,
            total_width,
            label_width,
        ));
    }
    lines.join("\n")
}

fn format_additional_limits(usage: &UsageResponse) -> Vec<(String, String)> {
    let mut lines = Vec::new();

    for additional in &usage.additional_rate_limits {
        let name = additional
            .limit_name
            .as_deref()
            .or(additional.metered_feature.as_deref())
            .unwrap_or("Additional");

        if let Some(primary) = additional.rate_limit.primary_window.as_ref() {
            lines.push((
                additional_window_label(name, primary),
                format_window_details(primary),
            ));
        }
        if let Some(secondary) = additional.rate_limit.secondary_window.as_ref() {
            lines.push((
                additional_window_label(name, secondary),
                format_window_details(secondary),
            ));
        }
    }

    lines
}

fn additional_window_label(base: &str, window: &UsageWindow) -> String {
    format!("{base} {}", window_label(window.limit_window_seconds))
}

pub fn first_line_of_error(input: &str) -> String {
    input
        .lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("-")
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_standard_windows() {
        assert_eq!(window_label(Some(18_000)), "5h");
        assert_eq!(window_label(Some(604_800)), "weekly");
        assert_eq!(window_label(Some(2_592_000)), "monthly");
    }

    #[test]
    fn blocks_missing_required_main_window() {
        let usage = UsageResponse {
            email: None,
            plan_type: None,
            rate_limit: Some(WindowPair {
                primary_window: Some(UsageWindow {
                    used_percent: Some(20),
                    reset_at: Some(1_700_000_000),
                    limit_window_seconds: Some(18_000),
                }),
                secondary_window: None,
            }),
            code_review_rate_limit: None,
            additional_rate_limits: Vec::new(),
        };

        let blocked = collect_blocked_limits(&usage, false);
        assert_eq!(blocked.len(), 1);
        assert_eq!(blocked[0].message, "weekly quota unavailable");
    }

    #[test]
    fn compact_window_format_uses_scale_of_100() {
        let window = UsageWindow {
            used_percent: Some(37),
            reset_at: None,
            limit_window_seconds: Some(18_000),
        };

        assert_eq!(format_window_status_compact(&window), "5h 63% left");
        assert!(format_window_status(&window).contains("63% left"));
        assert!(format_window_status(&window).contains("37% used"));
    }

    #[test]
    fn quota_summary_marks_exhausted_window() {
        let usage = UsageResponse {
            email: None,
            plan_type: None,
            rate_limit: Some(WindowPair {
                primary_window: Some(UsageWindow {
                    used_percent: Some(100),
                    reset_at: Some(1_700_000_000),
                    limit_window_seconds: Some(18_000),
                }),
                secondary_window: Some(UsageWindow {
                    used_percent: Some(30),
                    reset_at: Some(1_700_000_000),
                    limit_window_seconds: Some(604_800),
                }),
            }),
            code_review_rate_limit: None,
            additional_rate_limits: Vec::new(),
        };

        let summary = quota_summary(&usage);
        assert_eq!(
            summary.five_hour.status,
            RuntimeQuotaWindowStatus::Exhausted
        );
        assert_eq!(summary.route_band, RuntimeQuotaPressureBand::Exhausted);
    }

    #[test]
    fn profile_quota_render_contains_core_fields() {
        let usage = UsageResponse {
            email: Some("me@example.com".to_string()),
            plan_type: Some("plus".to_string()),
            rate_limit: None,
            code_review_rate_limit: None,
            additional_rate_limits: Vec::new(),
        };

        let rendered = render_profile_quota_with_width("main", &usage, 80);
        assert!(rendered.contains("Quota main"));
        assert!(rendered.contains("me@example.com"));
        assert!(rendered.contains("5h quota unavailable"));
    }
}
