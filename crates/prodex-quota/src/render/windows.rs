use super::*;
use prodex_mojo_core::rich::ascii_casefold_find;

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
    let capacity = crate::capacity::quota_capacity_for_window_pair(pair)
        .expect("Mojo quota capacity classification failed");
    if !capacity.admission_allowed && !capacity.any_window_exhausted {
        return None;
    }

    let remaining_percent = remaining_percent(Some(window.used_percent?));
    let reset_at = window.reset_at.unwrap_or(i64::MAX);
    let pressure_score = crate::mojo::quota_window_pressure(remaining_percent, reset_at, now)
        .expect("Mojo quota window pressure failed");
    Some(MainWindowSnapshot {
        remaining_percent,
        reset_at,
        pressure_score,
    })
}

pub use crate::capacity::additional_rate_limit_is_usable;

pub fn window_pair_has_ready_limit(pair: &WindowPair) -> bool {
    crate::capacity::quota_capacity_for_window_pair(pair)
        .expect("Mojo quota capacity classification failed")
        .usable
}

pub(crate) fn window_pair_has_blocking_admission(pair: &WindowPair) -> bool {
    !crate::capacity::quota_capacity_for_window_pair(pair)
        .expect("Mojo quota capacity classification failed")
        .admission_allowed
}

pub fn openai_quota_runtime_window_pair(usage: &UsageResponse) -> Option<&WindowPair> {
    let candidates = quota_window_capacity_candidates_at(usage, Local::now().timestamp())
        .expect("Mojo quota capacity classification failed");
    candidates
        .iter()
        .find(|candidate| candidate.output.routing_eligible)
        .map(|candidate| candidate.pair)
        .or_else(|| {
            candidates
                .iter()
                .find(|candidate| {
                    candidate.output.lane == prodex_mojo_core::quota::QUOTA_CAPACITY_LANE_MAIN
                })
                .map(|candidate| candidate.pair)
        })
}

pub fn openai_quota_has_ready_limit(usage: &UsageResponse) -> bool {
    quota_window_capacity_candidates_at(usage, Local::now().timestamp())
        .expect("Mojo quota capacity classification failed")
        .iter()
        .any(|candidate| candidate.output.routing_eligible)
}

fn quota_window_capacity_candidates_at(
    usage: &UsageResponse,
    now: i64,
) -> Result<Vec<crate::capacity::QuotaCapacityCandidate<'_>>, prodex_mojo_core::MojoError> {
    crate::capacity::quota_capacity_candidates_for_usage_at(
        usage,
        prodex_runtime_state::RuntimeRouteKind::Standard,
        now,
    )
}

#[cfg(test)]
#[path = "../../tests/src/render/windows.rs"]
mod tests;

fn openai_quota_has_ready_runtime_limit(usage: &UsageResponse) -> bool {
    openai_quota_has_ready_limit(usage)
}

pub fn quota_window_summary(usage: &UsageResponse, label: &str) -> RuntimeQuotaWindowSummary {
    let Some(window) = required_main_window_snapshot(usage, label) else {
        return RuntimeQuotaWindowSummary {
            status: RuntimeQuotaWindowStatus::Unknown,
            remaining_percent: 0,
            reset_at: i64::MAX,
        };
    };

    let status = quota_window_status(window.remaining_percent, true);

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

pub fn quota_reset_at_from_message(message: &str) -> Option<i64> {
    if let Some(reset_at) = quota_reset_at_from_json_message(message) {
        return Some(reset_at);
    }

    let marker =
        ascii_casefold_find(message, "try again at ").expect("Mojo retry marker search failed")?;
    let candidate = message
        .get(marker + "try again at ".len()..)?
        .trim()
        .trim_end_matches('.');
    let parsed = prodex_mojo_core::quota::quota_retry_time_candidate(candidate)
        .expect("Mojo quota retry-time text parser returned invalid output")?;
    match parsed {
        prodex_mojo_core::quota::QuotaRetryTimeCandidate::LocalClock(clock) => {
            let time = chrono::NaiveTime::parse_from_str(&clock, "%I:%M %p").ok()?;
            let now = Local::now();
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
            Some(parsed.timestamp())
        }
        prodex_mojo_core::quota::QuotaRetryTimeCandidate::LocalDateTime(normalized) => {
            let naive =
                chrono::NaiveDateTime::parse_from_str(&normalized, "%b %d, %Y %I:%M %p").ok()?;
            Local
                .from_local_datetime(&naive)
                .single()
                .or_else(|| Local.from_local_datetime(&naive).earliest())
                .map(|datetime| datetime.timestamp())
        }
    }
}

fn quota_reset_at_from_json_message(message: &str) -> Option<i64> {
    let value = serde_json::from_str::<serde_json::Value>(message.trim()).ok()?;
    let canonical = serde_json::to_string(&value).ok()?;
    prodex_mojo_core::quota::reset_epoch::quota_reset_json_epoch(&canonical)
        .expect("Mojo quota reset JSON parser returned an invalid result")
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
    format_window_pair_for_display(usage.rate_limit.as_ref(), false)
}

pub fn format_main_windows_compact(usage: &UsageResponse) -> String {
    format_window_pair_for_display(usage.rate_limit.as_ref(), true)
}

pub fn format_main_reset_summary(usage: &UsageResponse) -> String {
    let five_hour_window = usage
        .rate_limit
        .as_ref()
        .and_then(|pair| find_main_window(pair, "5h"));
    let weekly_window = usage
        .rate_limit
        .as_ref()
        .and_then(|pair| find_main_window(pair, "weekly"));
    let five_hour_reset = five_hour_window.and_then(|window| {
        window
            .reset_at
            .map(|epoch| format_precise_reset_time(Some(epoch)))
    });
    let weekly_reset = weekly_window.and_then(|window| {
        window
            .reset_at
            .map(|epoch| format_precise_reset_time(Some(epoch)))
    });
    prodex_mojo_core::quota::quota_reset_summary_display(
        five_hour_window.map(|_| prodex_mojo_core::quota::QuotaResetWindowDisplayInput {
            reset: five_hour_reset.as_deref(),
        }),
        weekly_window.map(|_| prodex_mojo_core::quota::QuotaResetWindowDisplayInput {
            reset: weekly_reset.as_deref(),
        }),
    )
    .expect("Mojo quota reset summary display returned invalid output")
}

pub(super) fn format_window_pair(rate_limit: &WindowPair) -> String {
    format_window_pair_for_display(Some(rate_limit), false)
}

pub(super) fn format_window_pair_compact(rate_limit: &WindowPair) -> String {
    format_window_pair_for_display(Some(rate_limit), true)
}

pub fn format_window_status(window: &UsageWindow) -> String {
    let label = window_label(window.limit_window_seconds);
    let reset = format_reset_time(window.reset_at);
    prodex_mojo_core::quota::quota_window_status_display(
        prodex_mojo_core::quota::QuotaWindowDisplayInput {
            label: &label,
            used_percent: window.used_percent,
            reset: Some(&reset),
        },
    )
    .expect("Mojo quota window status display returned invalid output")
}

pub fn format_window_status_compact(window: &UsageWindow) -> String {
    let label = window_label(window.limit_window_seconds);
    prodex_mojo_core::quota::quota_window_status_compact_display(&label, window.used_percent)
        .expect("Mojo compact quota window display returned invalid output")
}

pub(super) fn format_window_details(window: &UsageWindow) -> String {
    let reset = format_reset_time(window.reset_at);
    prodex_mojo_core::quota::quota_window_details_display(window.used_percent, &reset)
        .expect("Mojo quota window details display returned invalid output")
}

fn format_window_pair_for_display(pair: Option<&WindowPair>, compact: bool) -> String {
    let primary = pair
        .and_then(|pair| pair.primary_window.as_ref())
        .map(|window| {
            (
                window_label(window.limit_window_seconds),
                window.used_percent,
                (!compact).then(|| format_reset_time(window.reset_at)),
            )
        });
    let secondary = pair
        .and_then(|pair| pair.secondary_window.as_ref())
        .map(|window| {
            (
                window_label(window.limit_window_seconds),
                window.used_percent,
                (!compact).then(|| format_reset_time(window.reset_at)),
            )
        });
    let primary_input = primary.as_ref().map(|(label, used_percent, reset)| {
        prodex_mojo_core::quota::QuotaWindowDisplayInput {
            label,
            used_percent: *used_percent,
            reset: reset.as_deref(),
        }
    });
    let secondary_input = secondary.as_ref().map(|(label, used_percent, reset)| {
        prodex_mojo_core::quota::QuotaWindowDisplayInput {
            label,
            used_percent: *used_percent,
            reset: reset.as_deref(),
        }
    });
    prodex_mojo_core::quota::quota_window_pair_display(primary_input, secondary_input, compact)
        .expect("Mojo quota window pair display returned invalid output")
}

pub fn collect_blocked_limits(
    usage: &UsageResponse,
    include_code_review: bool,
) -> Vec<BlockedLimit> {
    let mut blocked = Vec::new();
    let runtime_ready = openai_quota_has_ready_runtime_limit(usage);
    let main_pair_missing = !runtime_ready && usage.rate_limit.is_none();
    let mut windows = Vec::new();

    if !runtime_ready {
        collect_runtime_blocked_window_refs(usage, &mut blocked, &mut windows);
    }

    let code_review_start = windows.len();
    if include_code_review && let Some(code_review) = usage.code_review_rate_limit.as_ref() {
        for window in [
            code_review.primary_window.as_ref(),
            code_review.secondary_window.as_ref(),
        ]
        .into_iter()
        .flatten()
        {
            windows.push(BlockedWindowRef {
                kind: prodex_mojo_core::quota::QUOTA_BLOCKED_WINDOW_KIND_CODE_REVIEW,
                name: Some("code-review"),
                main_label: None,
                window,
            });
        }
    }

    let plan = prodex_mojo_core::quota::quota_blocked_window_plan(
        runtime_ready,
        include_code_review,
        &windows
            .iter()
            .map(|entry| prodex_mojo_core::quota::QuotaBlockedWindowInput {
                kind: entry.kind,
                used_percent: entry.window.used_percent,
            })
            .collect::<Vec<_>>(),
    )
    .expect("Mojo quota blocked-window planner returned invalid output");
    append_planned_blocked_windows(
        &mut blocked,
        &windows[..code_review_start],
        &plan[..code_review_start],
    );
    if !runtime_ready && !main_pair_missing && blocked.is_empty() {
        blocked.push(BlockedLimit {
            message: "quota unavailable".to_string(),
        });
    }
    append_planned_blocked_windows(
        &mut blocked,
        &windows[code_review_start..],
        &plan[code_review_start..],
    );

    blocked
}

// Project borrowed runtime windows in their existing main/additional order.
// The Mojo planner remains responsible for blocking decisions.
fn collect_runtime_blocked_window_refs<'a>(
    usage: &'a UsageResponse,
    blocked: &mut Vec<BlockedLimit>,
    windows: &mut Vec<BlockedWindowRef<'a>>,
) {
    if let Some(main) = usage.rate_limit.as_ref() {
        for label in ["5h", "weekly"] {
            if let Some(window) = find_main_window(main, label) {
                windows.push(BlockedWindowRef {
                    kind: prodex_mojo_core::quota::QUOTA_BLOCKED_WINDOW_KIND_MAIN,
                    name: None,
                    main_label: Some(label),
                    window,
                });
            }
        }
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
            .or(additional.metered_feature.as_deref())
            .or(additional.limit_id.as_deref());
        for window in [
            additional.rate_limit.primary_window.as_ref(),
            additional.rate_limit.secondary_window.as_ref(),
        ]
        .into_iter()
        .flatten()
        {
            windows.push(BlockedWindowRef {
                kind: prodex_mojo_core::quota::QUOTA_BLOCKED_WINDOW_KIND_ADDITIONAL,
                name: label,
                main_label: None,
                window,
            });
        }
    }
}

#[derive(Clone, Copy)]
struct BlockedWindowRef<'a> {
    kind: i64,
    name: Option<&'a str>,
    main_label: Option<&'static str>,
    window: &'a UsageWindow,
}

pub fn find_main_window<'a>(main: &'a WindowPair, expected_label: &str) -> Option<&'a UsageWindow> {
    [main.primary_window.as_ref(), main.secondary_window.as_ref()]
        .into_iter()
        .flatten()
        .find(|window| window_label(window.limit_window_seconds) == expected_label)
}

fn append_planned_blocked_windows(
    blocked: &mut Vec<BlockedLimit>,
    windows: &[BlockedWindowRef<'_>],
    plan: &[bool],
) {
    debug_assert_eq!(windows.len(), plan.len());
    for (entry, should_block) in windows.iter().zip(plan) {
        if !should_block {
            continue;
        }
        if entry.kind == prodex_mojo_core::quota::QUOTA_BLOCKED_WINDOW_KIND_MAIN
            && entry.window.used_percent.is_none()
        {
            blocked.push(BlockedLimit {
                message: format!(
                    "{} quota unknown",
                    entry.main_label.expect("main window label is present")
                ),
            });
            continue;
        }
        let label = match entry.name {
            Some(base) if !base.is_empty() => {
                format!("{base} {}", window_label(entry.window.limit_window_seconds))
            }
            _ => window_label(entry.window.limit_window_seconds),
        };
        blocked.push(BlockedLimit {
            message: format!(
                "{label} exhausted until {}",
                format_reset_time(entry.window.reset_at)
            ),
        });
    }
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
    let kind = blocked
        .iter()
        .map(|limit| {
            prodex_mojo_core::quota::blocked_limit_kind(&limit.message)
                .expect("Mojo blocked quota status classification failed")
        })
        .max()
        .unwrap_or(prodex_mojo_core::quota::QUOTA_BLOCKED_KIND_NONE);
    prodex_mojo_core::quota::quota_blocked_status_label(kind)
        .expect("Mojo blocked quota status label returned invalid output")
        .to_string()
}

pub fn format_quota_error_status(error: &str) -> String {
    let unauthorized = prodex_mojo_core::quota::quota_error_auth_failure(error, false)
        .expect("Mojo quota error authentication classification failed");
    if unauthorized {
        "Blocked unauthorized".to_string()
    } else {
        format!("Error {}", quota_error_summary(error))
    }
}

pub fn format_quota_error_detail(error: &str) -> String {
    let unauthorized = prodex_mojo_core::quota::quota_error_auth_failure(error, false)
        .expect("Mojo quota error authentication classification failed");
    let summary = if unauthorized {
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

    let kind = prodex_mojo_core::quota::quota_error_summary_kind(&first_line)
        .expect("Mojo quota error summary classification failed");
    prodex_mojo_core::quota::quota_error_summary_label(kind)
        .expect("Mojo quota error summary label returned invalid output")
        .unwrap_or(first_line)
}
