use super::*;

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
    required_window_snapshot_at(usage.rate_limit.as_ref()?, label, now)
}

pub(super) fn required_window_snapshot_at(
    pair: &WindowPair,
    label: &str,
    now: i64,
) -> Option<MainWindowSnapshot> {
    let window = find_main_window(pair, label)?;
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

pub fn spark_window_snapshots(
    usage: &UsageResponse,
) -> Option<(MainWindowSnapshot, MainWindowSnapshot)> {
    spark_window_snapshots_at(usage, Local::now().timestamp())
}

pub fn usage_has_spark_limit(usage: &UsageResponse) -> bool {
    usage
        .additional_rate_limits
        .iter()
        .any(additional_rate_limit_is_spark)
}

pub fn spark_window_snapshots_at(
    usage: &UsageResponse,
    now: i64,
) -> Option<(MainWindowSnapshot, MainWindowSnapshot)> {
    let rate_limit = &usage
        .additional_rate_limits
        .iter()
        .find(|additional| additional_rate_limit_is_spark(additional))?
        .rate_limit;
    Some((
        required_window_snapshot_at(rate_limit, "5h", now)?,
        required_window_snapshot_at(rate_limit, "weekly", now)?,
    ))
}

fn additional_rate_limit_is_spark(additional: &AdditionalRateLimit) -> bool {
    [
        additional.limit_name.as_deref(),
        additional.metered_feature.as_deref(),
    ]
    .into_iter()
    .flatten()
    .any(|value| {
        let normalized = value.to_ascii_lowercase();
        normalized.contains("spark") || normalized.contains("bengalfox")
    })
}

pub fn openai_quota_has_ready_limit(usage: &UsageResponse) -> bool {
    collect_blocked_limits(usage, false).is_empty()
        || spark_window_snapshots(usage).is_some_and(|(five_hour, weekly)| {
            five_hour.remaining_percent > 0 && weekly.remaining_percent > 0
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

pub fn quota_reset_at_from_message(message: &str) -> Option<i64> {
    if let Some(reset_at) = quota_reset_at_from_json_message(message) {
        return Some(reset_at);
    }

    let marker = message.to_ascii_lowercase().find("try again at ")?;
    let candidate = message
        .get(marker + "try again at ".len()..)?
        .trim()
        .trim_end_matches('.');
    let now = Local::now();
    if let Some((time_text, meridiem)) = candidate
        .split_whitespace()
        .collect::<Vec<_>>()
        .get(..2)
        .and_then(|parts| (parts.len() == 2).then_some((parts[0], parts[1])))
        && let Ok(time) =
            chrono::NaiveTime::parse_from_str(&format!("{time_text} {meridiem}"), "%I:%M %p")
    {
        let mut naive = now.date_naive().and_time(time);
        let mut parsed = Local
            .from_local_datetime(&naive)
            .single()
            .or_else(|| Local.from_local_datetime(&naive).earliest())?;
        if parsed.timestamp() <= now.timestamp() {
            naive = naive.checked_add_signed(chrono::Duration::days(1))?;
            parsed = Local
                .from_local_datetime(&naive)
                .single()
                .or_else(|| Local.from_local_datetime(&naive).earliest())?;
        }
        return Some(parsed.timestamp());
    }

    let mut parts = candidate
        .split_whitespace()
        .map(|part| part.to_string())
        .collect::<Vec<_>>();
    if parts.len() < 5 {
        return None;
    }
    let day_digits = parts[1]
        .trim_end_matches(',')
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .collect::<String>();
    if day_digits.is_empty() {
        return None;
    }
    parts[1] = format!("{day_digits},");
    let normalized = parts[..5].join(" ");
    let naive = chrono::NaiveDateTime::parse_from_str(&normalized, "%b %d, %Y %I:%M %p").ok()?;
    Local
        .from_local_datetime(&naive)
        .single()
        .or_else(|| Local.from_local_datetime(&naive).earliest())
        .map(|datetime| datetime.timestamp())
}

fn quota_reset_at_from_json_message(message: &str) -> Option<i64> {
    let value = serde_json::from_str::<serde_json::Value>(message.trim()).ok()?;
    for path in [
        &["resets_at"][..],
        &["reset_at"][..],
        &["error", "resets_at"][..],
        &["error", "reset_at"][..],
    ] {
        if let Some(reset_at) = quota_json_i64_path(&value, path) {
            return Some(reset_at);
        }
    }

    let headers = quota_json_path(&value, &["headers"])?;
    let primary_reset = quota_json_object_i64(headers, "X-Codex-Primary-Reset-At");
    let secondary_reset = quota_json_object_i64(headers, "X-Codex-Secondary-Reset-At");
    let primary_used = quota_json_object_i64(headers, "X-Codex-Primary-Used-Percent")
        .is_some_and(|used| used >= 100);
    let secondary_used = quota_json_object_i64(headers, "X-Codex-Secondary-Used-Percent")
        .is_some_and(|used| used >= 100);

    if primary_used {
        return primary_reset;
    }
    if secondary_used {
        return secondary_reset;
    }
    primary_reset.or(secondary_reset)
}

fn quota_json_i64_path(value: &serde_json::Value, path: &[&str]) -> Option<i64> {
    quota_json_path(value, path).and_then(quota_json_i64)
}

fn quota_json_path<'a>(
    value: &'a serde_json::Value,
    path: &[&str],
) -> Option<&'a serde_json::Value> {
    let mut current = value;
    for segment in path {
        current = current.get(*segment)?;
    }
    Some(current)
}

fn quota_json_object_i64(value: &serde_json::Value, key: &str) -> Option<i64> {
    let object = value.as_object()?;
    object
        .iter()
        .find(|(candidate, _)| candidate.eq_ignore_ascii_case(key))
        .and_then(|(_, value)| quota_json_i64(value))
}

fn quota_json_i64(value: &serde_json::Value) -> Option<i64> {
    value
        .as_i64()
        .or_else(|| value.as_str()?.trim().parse::<i64>().ok())
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

pub(super) fn format_window_pair(rate_limit: &WindowPair) -> String {
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

pub(super) fn format_window_pair_compact(rate_limit: &WindowPair) -> String {
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
            format!("{label} {remaining}%")
        }
        None => format!("{label} ?"),
    }
}

pub(super) fn format_window_details(window: &UsageWindow) -> String {
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

pub fn format_openai_quota_status(usage: &UsageResponse) -> String {
    if openai_quota_has_ready_limit(usage) {
        return "Ready".to_string();
    }

    let blocked = collect_blocked_limits(usage, false);
    format_blocked_quota_status(&blocked)
}

pub fn format_blocked_quota_status(blocked: &[BlockedLimit]) -> String {
    if blocked
        .iter()
        .any(|limit| limit.message.to_ascii_lowercase().contains("5h"))
    {
        "Blocked 5h".to_string()
    } else if blocked
        .iter()
        .any(|limit| limit.message.to_ascii_lowercase().contains("weekly"))
    {
        "Blocked weekly".to_string()
    } else {
        "Blocked".to_string()
    }
}

pub fn format_quota_error_status(error: &str) -> String {
    let lower = error.to_ascii_lowercase();
    if lower.contains("401") || lower.contains("unauthorized") {
        "Blocked unauthorized".to_string()
    } else {
        format!("Error {}", quota_error_summary(error))
    }
}

pub fn format_quota_error_detail(error: &str) -> String {
    let lower = error.to_ascii_lowercase();
    let summary = if lower.contains("401") || lower.contains("unauthorized") {
        "unauthorized".to_string()
    } else {
        quota_error_summary(error)
    };
    format!("error: {summary}")
}

fn quota_error_summary(error: &str) -> String {
    let first_line = first_line_of_error(error);
    if first_line.is_empty() || first_line == "-" {
        return "unknown".to_string();
    }
    let lower = first_line.to_ascii_lowercase();
    if lower.contains("unavailable") {
        "unavailable".to_string()
    } else if lower.contains("missing")
        || lower.contains("not configured")
        || lower.contains("config")
    {
        "config".to_string()
    } else if lower.contains("500")
        || lower.contains("502")
        || lower.contains("503")
        || lower.contains("504")
        || lower.contains("server")
    {
        "server".to_string()
    } else if lower.contains("timeout") || lower.contains("timed out") {
        "timeout".to_string()
    } else if lower.contains("dns")
        || lower.contains("tls")
        || lower.contains("certificate")
        || lower.contains("network")
    {
        "network".to_string()
    } else if lower.contains("proxy") {
        "proxy".to_string()
    } else if lower.contains("refused") || lower.contains("connect") {
        "connection".to_string()
    } else if lower.contains("invalid auth")
        || lower.contains("invalid token")
        || lower.contains("bad credentials")
        || lower.contains("credential")
    {
        "invalid auth".to_string()
    } else if lower.contains("429") || lower.contains("rate limit") {
        "rate limit".to_string()
    } else if lower.contains("parse")
        || lower.contains("deserialize")
        || lower.contains("decode")
        || lower.contains("invalid json")
    {
        "parse".to_string()
    } else if lower.contains("empty") {
        "empty".to_string()
    } else if lower.contains("cancel") {
        "cancelled".to_string()
    } else if lower.contains("403") || lower.contains("forbidden") {
        "forbidden".to_string()
    } else if lower.contains("404") || lower.contains("not found") {
        "not found".to_string()
    } else {
        first_line
    }
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
    format_local_epoch(epoch, "%Y-%m-%d %H:%M")
}

pub fn format_precise_reset_time(epoch: Option<i64>) -> String {
    format_local_epoch(epoch, "%Y-%m-%d %H:%M:%S")
}

pub fn format_quota_snapshot_time(epoch: Option<i64>) -> String {
    format_local_epoch(epoch, "%Y-%m-%d %H:%M:%S")
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
