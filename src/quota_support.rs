use super::*;

#[derive(Debug)]
pub(crate) struct BlockedLimit {
    pub(crate) message: String,
}

#[derive(Debug, Clone)]
pub(crate) struct AuthSummary {
    pub(crate) label: String,
    pub(crate) quota_compatible: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct UsageAuth {
    pub(crate) access_token: String,
    pub(crate) account_id: Option<String>,
}

#[derive(Debug)]
pub(crate) struct QuotaReport {
    pub(crate) name: String,
    pub(crate) active: bool,
    pub(crate) auth: AuthSummary,
    pub(crate) result: std::result::Result<UsageResponse, String>,
    pub(crate) fetched_at: i64,
}

#[derive(Debug)]
struct QuotaFetchJob {
    name: String,
    active: bool,
    codex_home: PathBuf,
}

#[derive(Debug)]
struct ProfileSummaryJob {
    name: String,
    active: bool,
    managed: bool,
    email: Option<String>,
    codex_home: PathBuf,
}

#[derive(Debug)]
pub(crate) struct ProfileSummaryReport {
    pub(crate) name: String,
    pub(crate) active: bool,
    pub(crate) managed: bool,
    pub(crate) auth: AuthSummary,
    pub(crate) email: Option<String>,
    pub(crate) codex_home: PathBuf,
}

#[derive(Debug)]
pub(crate) struct DoctorProfileReport {
    pub(crate) summary: ProfileSummaryReport,
    pub(crate) quota: Option<std::result::Result<UsageResponse, String>>,
}

pub(crate) fn collect_quota_reports(state: &AppState, base_url: Option<&str>) -> Vec<QuotaReport> {
    let jobs = state
        .profiles
        .iter()
        .map(|(name, profile)| QuotaFetchJob {
            name: name.clone(),
            active: state.active_profile.as_deref() == Some(name.as_str()),
            codex_home: profile.codex_home.clone(),
        })
        .collect();
    let base_url = base_url.map(str::to_owned);

    map_parallel(jobs, |job| {
        let auth = read_auth_summary(&job.codex_home);
        let result =
            fetch_usage(&job.codex_home, base_url.as_deref()).map_err(|err| err.to_string());
        QuotaReport {
            name: job.name,
            active: job.active,
            auth,
            result,
            fetched_at: Local::now().timestamp(),
        }
    })
}

pub(crate) fn collect_profile_summaries(state: &AppState) -> Vec<ProfileSummaryReport> {
    let jobs = state
        .profiles
        .iter()
        .map(|(name, profile)| ProfileSummaryJob {
            name: name.clone(),
            active: state.active_profile.as_deref() == Some(name.as_str()),
            managed: profile.managed,
            email: profile.email.clone(),
            codex_home: profile.codex_home.clone(),
        })
        .collect();

    map_parallel(jobs, |job| ProfileSummaryReport {
        name: job.name,
        active: job.active,
        managed: job.managed,
        auth: read_auth_summary(&job.codex_home),
        email: job.email,
        codex_home: job.codex_home,
    })
}

pub(crate) fn collect_doctor_profile_reports(
    state: &AppState,
    include_quota: bool,
) -> Vec<DoctorProfileReport> {
    map_parallel(collect_profile_summaries(state), |summary| {
        DoctorProfileReport {
            quota: include_quota
                .then(|| fetch_usage(&summary.codex_home, None).map_err(|err| err.to_string())),
            summary,
        }
    })
}

pub(crate) fn fetch_usage(codex_home: &Path, base_url: Option<&str>) -> Result<UsageResponse> {
    let usage: UsageResponse = serde_json::from_value(fetch_usage_json(codex_home, base_url)?)
        .with_context(|| {
            format!(
                "invalid JSON returned by quota backend for {}",
                codex_home.display()
            )
        })?;
    Ok(usage)
}

pub(crate) fn fetch_usage_json(
    codex_home: &Path,
    base_url: Option<&str>,
) -> Result<serde_json::Value> {
    let auth = read_usage_auth(codex_home)?;
    let usage_url = usage_url(&quota_base_url(base_url));
    let client = Client::builder()
        .connect_timeout(Duration::from_millis(QUOTA_HTTP_CONNECT_TIMEOUT_MS))
        .timeout(Duration::from_millis(QUOTA_HTTP_READ_TIMEOUT_MS))
        .build()
        .context("failed to build quota HTTP client")?;

    let mut request = client
        .get(&usage_url)
        .header("Authorization", format!("Bearer {}", auth.access_token))
        .header("User-Agent", "codex-cli");

    if let Some(account_id) = auth.account_id.as_deref() {
        request = request.header("ChatGPT-Account-Id", account_id);
    }

    let response = request
        .send()
        .with_context(|| format!("failed to request quota endpoint {}", usage_url))?;
    let status = response.status();
    let body = response
        .bytes()
        .context("failed to read quota response body")?;

    if !status.is_success() {
        let body_text = format_response_body(&body);
        if body_text.is_empty() {
            bail!("request failed (HTTP {}) to {}", status.as_u16(), usage_url);
        }
        bail!(
            "request failed (HTTP {}) to {}: {}",
            status.as_u16(),
            usage_url,
            body_text
        );
    }

    let usage = serde_json::from_slice(&body).with_context(|| {
        format!(
            "invalid JSON returned by quota backend for {}",
            codex_home.display()
        )
    })?;

    Ok(usage)
}

pub(crate) fn print_quota_reports(reports: &[QuotaReport], detail: bool) {
    print!("{}", render_quota_reports(reports, detail));
}

pub(crate) fn render_quota_reports(reports: &[QuotaReport], detail: bool) -> String {
    render_quota_reports_with_layout(reports, detail, None, current_cli_width())
}

#[derive(Clone, Copy)]
struct QuotaReportColumnWidths {
    profile: usize,
    current: usize,
    auth: usize,
    account: usize,
    plan: usize,
    remaining: usize,
}

#[derive(Debug, Default, Clone, Copy)]
struct QuotaPoolAggregate {
    total_profiles: usize,
    available_profiles: usize,
    profiles_with_data: usize,
    five_hour_pool_remaining: i64,
    weekly_pool_remaining: i64,
    earliest_five_hour_reset_at: Option<i64>,
    earliest_weekly_reset_at: Option<i64>,
    last_updated_at: Option<i64>,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub(crate) struct RenderedQuotaReportWindow {
    pub(crate) output: String,
    pub(crate) shown_profiles: usize,
    pub(crate) total_profiles: usize,
    pub(crate) start_profile: usize,
    pub(crate) hidden_before: usize,
    pub(crate) hidden_after: usize,
}

#[derive(Debug)]
enum AllQuotaWatchSnapshot {
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

fn quota_report_column_widths(total_width: usize) -> QuotaReportColumnWidths {
    const MIN_WIDTHS: [usize; 6] = [12, 3, 4, 14, 4, 13];
    const EXTRA_WEIGHTS: [usize; 6] = [12, 1, 3, 13, 4, 18];
    const DISTRIBUTION_ORDER: [usize; 6] = [5, 3, 0, 4, 2, 1];

    let gap_width = text_width(CLI_TABLE_GAP) * 5;
    let min_total = MIN_WIDTHS.iter().sum::<usize>();
    let available = total_width.saturating_sub(gap_width).max(min_total);

    let mut widths = MIN_WIDTHS;
    let mut remaining_extra = available.saturating_sub(min_total);
    let total_weight = EXTRA_WEIGHTS.iter().sum::<usize>().max(1);

    for (width, weight) in widths.iter_mut().zip(EXTRA_WEIGHTS) {
        let extra = remaining_extra * weight / total_weight;
        *width += extra;
    }

    let assigned = widths.iter().sum::<usize>().saturating_sub(min_total);
    remaining_extra = remaining_extra.saturating_sub(assigned);
    for index in DISTRIBUTION_ORDER.into_iter().cycle().take(remaining_extra) {
        widths[index] += 1;
    }

    QuotaReportColumnWidths {
        profile: widths[0],
        current: widths[1],
        auth: widths[2],
        account: widths[3],
        plan: widths[4],
        remaining: widths[5],
    }
}

#[allow(dead_code)]
pub(crate) fn render_quota_reports_with_line_limit(
    reports: &[QuotaReport],
    detail: bool,
    max_lines: Option<usize>,
) -> String {
    render_quota_reports_with_layout(reports, detail, max_lines, current_cli_width())
}

pub(crate) fn render_quota_reports_with_layout(
    reports: &[QuotaReport],
    detail: bool,
    max_lines: Option<usize>,
    total_width: usize,
) -> String {
    render_quota_reports_window_with_layout(reports, detail, max_lines, total_width, 0, false)
        .output
}

pub(crate) fn render_quota_reports_window_with_layout(
    reports: &[QuotaReport],
    detail: bool,
    max_lines: Option<usize>,
    total_width: usize,
    start_profile: usize,
    interactive_scroll_hint: bool,
) -> RenderedQuotaReportWindow {
    let column_widths = quota_report_column_widths(total_width);
    let pool_summary =
        render_quota_pool_summary_lines(&collect_quota_pool_aggregate(reports), total_width);

    let mut sections = Vec::new();

    for report in sort_quota_reports_for_display(reports) {
        let active = if report.active { "*" } else { "" }.to_string();
        let auth = report.auth.label.clone();

        let (email, plan, main, status, resets) = match &report.result {
            Ok(usage) => {
                let blocked = collect_blocked_limits(usage, false);
                let status = if blocked.is_empty() {
                    "Ready".to_string()
                } else {
                    format!("Blocked: {}", format_blocked_limits(&blocked))
                };
                (
                    display_optional(usage.email.as_deref()).to_string(),
                    display_optional(usage.plan_type.as_deref()).to_string(),
                    format_main_windows_compact(usage),
                    status,
                    Some(format!("resets: {}", format_main_reset_summary(usage))),
                )
            }
            Err(err) => (
                "-".to_string(),
                "-".to_string(),
                "-".to_string(),
                format!("Error: {}", first_line_of_error(err)),
                Some("resets: unavailable".to_string()),
            ),
        };

        let mut section = Vec::new();
        section.push(format!(
            "{:<name_w$}{}{:<act_w$}{}{:<auth_w$}{}{:<email_w$}{}{:<plan_w$}{}{:<main_w$}",
            fit_cell(&report.name, column_widths.profile),
            CLI_TABLE_GAP,
            fit_cell(&active, column_widths.current),
            CLI_TABLE_GAP,
            fit_cell(&auth, column_widths.auth),
            CLI_TABLE_GAP,
            fit_cell(&email, column_widths.account),
            CLI_TABLE_GAP,
            fit_cell(&plan, column_widths.plan),
            CLI_TABLE_GAP,
            fit_cell(&main, column_widths.remaining),
            name_w = column_widths.profile,
            act_w = column_widths.current,
            auth_w = column_widths.auth,
            email_w = column_widths.account,
            plan_w = column_widths.plan,
            main_w = column_widths.remaining,
        ));
        section.extend(
            wrap_text(
                &format!("status: {status}"),
                total_width.saturating_sub(2).max(1),
            )
            .into_iter()
            .map(|line| format!("  {line}")),
        );
        if detail {
            if let Some(resets) = resets.as_deref() {
                section.extend(
                    wrap_text(resets, total_width.saturating_sub(2).max(1))
                        .into_iter()
                        .map(|line| format!("  {line}")),
                );
            }
        }
        section.push(String::new());
        sections.push(section);
    }

    let header = format!(
        "{:<name_w$}  {:<act_w$}  {:<auth_w$}  {:<email_w$}  {:<plan_w$}  {:<main_w$}",
        "PROFILE",
        "CUR",
        "AUTH",
        "ACCOUNT",
        "PLAN",
        "REMAINING",
        name_w = column_widths.profile,
        act_w = column_widths.current,
        auth_w = column_widths.auth,
        email_w = column_widths.account,
        plan_w = column_widths.plan,
        main_w = column_widths.remaining,
    );
    let has_pool_summary = !pool_summary.is_empty();
    let mut output = vec![section_header_with_width("Quota Overview", total_width)];
    output.extend(pool_summary);
    if has_pool_summary {
        output.push(String::new());
    }
    output.push(header.clone());
    output.push("-".repeat(text_width(&header)));
    let total_profiles = sections.len();
    let start_profile = if total_profiles == 0 {
        0
    } else {
        start_profile.min(total_profiles.saturating_sub(1))
    };
    let mut shown_profiles = 0_usize;
    if let Some(max_lines) = max_lines {
        let mut remaining = max_lines.saturating_sub(output.len());

        for section in sections.iter().skip(start_profile) {
            if section.len() > remaining {
                break;
            }
            remaining = remaining.saturating_sub(section.len());
            output.extend(section.iter().cloned());
            shown_profiles += 1;
        }

        let hidden_before = start_profile;
        let hidden_profiles = total_profiles.saturating_sub(shown_profiles);
        let hidden_after = hidden_profiles.saturating_sub(hidden_before);
        if hidden_before > 0 || hidden_after > 0 {
            let notice = if interactive_scroll_hint {
                let first_visible = start_profile.saturating_add(1);
                let last_visible = start_profile.saturating_add(shown_profiles);
                format!(
                    "press Up/Down to scroll profiles ({first_visible}-{last_visible} of {total_profiles}; {hidden_before} above, {hidden_after} below)"
                )
            } else {
                format!(
                    "showing top {shown_profiles} of {total_profiles} profiles due to terminal height"
                )
            };
            if remaining == 0 && !output.is_empty() {
                output.pop();
            }
            output.push(String::new());
            output.push(notice);
        }
    } else {
        for section in sections {
            output.extend(section);
            shown_profiles += 1;
        }
    }
    let hidden_before = start_profile;
    let hidden_after = total_profiles.saturating_sub(start_profile.saturating_add(shown_profiles));
    RenderedQuotaReportWindow {
        output: output.join("\n"),
        shown_profiles,
        total_profiles,
        start_profile,
        hidden_before,
        hidden_after,
    }
}

fn collect_quota_pool_aggregate(reports: &[QuotaReport]) -> QuotaPoolAggregate {
    let mut aggregate = QuotaPoolAggregate {
        total_profiles: reports.len(),
        ..QuotaPoolAggregate::default()
    };

    for report in reports {
        aggregate.last_updated_at = Some(
            aggregate
                .last_updated_at
                .map_or(report.fetched_at, |current| current.max(report.fetched_at)),
        );
        if !report.auth.quota_compatible {
            continue;
        }

        let Ok(usage) = &report.result else {
            continue;
        };
        let Some((five_hour, weekly)) = info_main_window_snapshots(usage) else {
            continue;
        };

        aggregate.available_profiles += 1;
        aggregate.profiles_with_data += 1;
        aggregate.five_hour_pool_remaining += five_hour.remaining_percent;
        aggregate.weekly_pool_remaining += weekly.remaining_percent;
        if five_hour.reset_at != i64::MAX {
            aggregate.earliest_five_hour_reset_at = Some(
                aggregate
                    .earliest_five_hour_reset_at
                    .map_or(five_hour.reset_at, |current| {
                        current.min(five_hour.reset_at)
                    }),
            );
        }
        if weekly.reset_at != i64::MAX {
            aggregate.earliest_weekly_reset_at = Some(
                aggregate
                    .earliest_weekly_reset_at
                    .map_or(weekly.reset_at, |current| current.min(weekly.reset_at)),
            );
        }
    }

    aggregate
}

fn render_quota_pool_summary_lines(
    aggregate: &QuotaPoolAggregate,
    total_width: usize,
) -> Vec<String> {
    let fields = vec![
        (
            "Available".to_string(),
            format!(
                "{}/{} profile",
                aggregate.available_profiles, aggregate.total_profiles
            ),
        ),
        (
            "Last Updated".to_string(),
            format_quota_snapshot_time(aggregate.last_updated_at),
        ),
        (
            "5h remaining pool".to_string(),
            format_info_pool_remaining(
                aggregate.five_hour_pool_remaining,
                aggregate.profiles_with_data,
                aggregate.earliest_five_hour_reset_at,
            ),
        ),
        (
            "Weekly remaining pool".to_string(),
            format_info_pool_remaining(
                aggregate.weekly_pool_remaining,
                aggregate.profiles_with_data,
                aggregate.earliest_weekly_reset_at,
            ),
        ),
    ];
    let label_width = fields
        .iter()
        .map(|(label, _)| text_width(label) + 1)
        .max()
        .unwrap_or(CLI_LABEL_WIDTH)
        .min(total_width.saturating_sub(2).max(1));
    let mut lines = Vec::new();

    for (label, value) in fields {
        lines.extend(format_field_lines_with_layout(
            &label,
            &value,
            total_width,
            label_width,
        ));
    }

    lines
}

pub(crate) fn sort_quota_reports_for_display(reports: &[QuotaReport]) -> Vec<&QuotaReport> {
    let mut sorted = reports.iter().collect::<Vec<_>>();
    sorted.sort_by(|left, right| {
        quota_report_sort_key(left)
            .cmp(&quota_report_sort_key(right))
            .then_with(|| left.name.cmp(&right.name))
    });
    sorted
}

fn quota_report_sort_key(report: &QuotaReport) -> (usize, i64) {
    (
        quota_report_status_rank(report),
        quota_report_earliest_main_reset_epoch(report).unwrap_or(i64::MAX),
    )
}

fn quota_report_status_rank(report: &QuotaReport) -> usize {
    match &report.result {
        Ok(usage) if collect_blocked_limits(usage, false).is_empty() => 0,
        Ok(_) => 1,
        Err(_) => 2,
    }
}

fn quota_report_earliest_main_reset_epoch(report: &QuotaReport) -> Option<i64> {
    earliest_required_main_reset_epoch(report.result.as_ref().ok()?)
}

fn earliest_required_main_reset_epoch(usage: &UsageResponse) -> Option<i64> {
    ["5h", "weekly"]
        .into_iter()
        .filter_map(|label| {
            find_main_window(usage.rate_limit.as_ref()?, label).and_then(|window| window.reset_at)
        })
        .min()
}

pub(crate) fn format_main_windows(usage: &UsageResponse) -> String {
    usage
        .rate_limit
        .as_ref()
        .map(format_window_pair)
        .unwrap_or_else(|| "-".to_string())
}

pub(crate) fn format_main_windows_compact(usage: &UsageResponse) -> String {
    usage
        .rate_limit
        .as_ref()
        .map(format_window_pair_compact)
        .unwrap_or_else(|| "-".to_string())
}

pub(crate) fn format_main_reset_summary(usage: &UsageResponse) -> String {
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

pub(crate) fn format_window_status(window: &UsageWindow) -> String {
    format_named_window_status(&window_label(window.limit_window_seconds), window)
}

pub(crate) fn format_window_status_compact(window: &UsageWindow) -> String {
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

pub(crate) fn collect_blocked_limits(
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

pub(crate) fn find_main_window<'a>(
    main: &'a WindowPair,
    expected_label: &str,
) -> Option<&'a UsageWindow> {
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

pub(crate) fn format_blocked_limits(blocked: &[BlockedLimit]) -> String {
    blocked
        .iter()
        .map(|limit| limit.message.clone())
        .collect::<Vec<_>>()
        .join(", ")
}

pub(crate) fn remaining_percent(used_percent: Option<i64>) -> i64 {
    let Some(used) = used_percent else {
        return 0;
    };
    (100 - used).clamp(0, 100)
}

pub(crate) fn window_label(seconds: Option<i64>) -> String {
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

pub(crate) fn format_reset_time(epoch: Option<i64>) -> String {
    format_local_epoch(epoch, "%Y-%m-%d %H:%M %:z")
}

pub(crate) fn format_precise_reset_time(epoch: Option<i64>) -> String {
    format_local_epoch(epoch, "%Y-%m-%d %H:%M:%S %:z")
}

fn format_quota_snapshot_time(epoch: Option<i64>) -> String {
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

pub(crate) fn render_profile_quota(profile_name: &str, usage: &UsageResponse) -> String {
    let blocked = collect_blocked_limits(usage, false);
    let status = if blocked.is_empty() {
        "Ready".to_string()
    } else {
        format!("Blocked ({})", format_blocked_limits(&blocked))
    };
    let mut fields = vec![
        ("Profile".to_string(), profile_name.to_string()),
        (
            "Account".to_string(),
            display_optional(usage.email.as_deref()).to_string(),
        ),
        (
            "Plan".to_string(),
            display_optional(usage.plan_type.as_deref()).to_string(),
        ),
        ("Status".to_string(), status),
        ("Main".to_string(), format_main_windows(usage)),
    ];

    if let Some(code_review) = usage.code_review_rate_limit.as_ref() {
        fields.push(("Code review".to_string(), format_window_pair(code_review)));
    }

    for (name, value) in format_additional_limits(usage) {
        fields.push((name, value));
    }

    render_panel(&format!("Quota {profile_name}"), &fields)
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

pub(crate) fn first_line_of_error(input: &str) -> String {
    input
        .lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("-")
        .trim()
        .to_string()
}

pub(crate) fn quota_watch_enabled(args: &QuotaArgs) -> bool {
    !args.raw && !args.once
}

pub(crate) fn render_profile_quota_watch_output(
    profile_name: &str,
    _updated: &str,
    usage_result: std::result::Result<UsageResponse, String>,
) -> String {
    match usage_result {
        Ok(usage) => render_profile_quota(profile_name, &usage),
        Err(err) => render_panel(
            &format!("Quota {profile_name}"),
            &[("Error".to_string(), first_line_of_error(&err))],
        ),
    }
}

#[allow(dead_code)]
pub(crate) fn render_all_quota_watch_output(
    updated: &str,
    state_result: std::result::Result<AppState, String>,
    base_url: Option<&str>,
    detail: bool,
) -> String {
    render_all_quota_watch_snapshot(
        &collect_all_quota_watch_snapshot(updated, state_result, base_url),
        detail,
        0,
    )
}

fn collect_all_quota_watch_snapshot(
    updated: &str,
    state_result: std::result::Result<AppState, String>,
    base_url: Option<&str>,
) -> AllQuotaWatchSnapshot {
    match state_result {
        Ok(state) if !state.profiles.is_empty() => AllQuotaWatchSnapshot::Reports {
            updated: updated.to_string(),
            profile_count: state.profiles.len(),
            reports: collect_quota_reports(&state, base_url),
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
) -> String {
    match snapshot {
        AllQuotaWatchSnapshot::Reports {
            updated: _updated,
            profile_count: _profile_count,
            reports,
        } => {
            let available_report_lines = quota_watch_available_report_lines("");
            let window = render_quota_reports_window_with_layout(
                reports,
                detail,
                available_report_lines,
                current_cli_width(),
                scroll_offset,
                true,
            );
            window.output
        }
        AllQuotaWatchSnapshot::Empty { updated: _updated } => render_panel(
            "Quota",
            &[("Error".to_string(), "No profiles configured".to_string())],
        ),
        AllQuotaWatchSnapshot::Error {
            updated: _updated,
            message,
        } => render_panel(
            "Quota",
            &[("Error".to_string(), first_line_of_error(message))],
        ),
    }
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
    let reserved = header.lines().count().saturating_add(1);
    Some(terminal_height.saturating_sub(reserved))
}

const QUOTA_WATCH_INPUT_POLL_MS: u64 = 100;

enum QuotaWatchCommand {
    Up,
    Down,
    Quit,
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

pub(crate) fn watch_quota(
    profile_name: &str,
    codex_home: &Path,
    base_url: Option<&str>,
) -> Result<()> {
    loop {
        let updated = Local::now().format("%Y-%m-%d %H:%M:%S %Z").to_string();
        let output = render_profile_quota_watch_output(
            profile_name,
            &updated,
            fetch_usage(codex_home, base_url).map_err(|err| err.to_string()),
        );
        redraw_quota_watch(&output)?;
        thread::sleep(Duration::from_secs(DEFAULT_WATCH_INTERVAL_SECONDS));
    }
}

pub(crate) fn watch_all_quotas(
    paths: &AppPaths,
    base_url: Option<&str>,
    detail: bool,
) -> Result<()> {
    let mut input = QuotaWatchInput::open();
    let mut scroll_offset = 0_usize;
    let mut snapshot = collect_all_quota_watch_snapshot(
        &Local::now().format("%Y-%m-%d %H:%M:%S %Z").to_string(),
        AppState::load(paths).map_err(|err| err.to_string()),
        base_url,
    );
    let mut redraw_needed = true;
    let mut next_refresh_at = Instant::now() + Duration::from_secs(DEFAULT_WATCH_INTERVAL_SECONDS);

    loop {
        if redraw_needed {
            let max_scroll_offset = match &snapshot {
                AllQuotaWatchSnapshot::Reports { reports, .. } => reports.len().saturating_sub(1),
                _ => 0,
            };
            scroll_offset = scroll_offset.min(max_scroll_offset);
            let output = render_all_quota_watch_snapshot(&snapshot, detail, scroll_offset);
            redraw_quota_watch(&output)?;
            redraw_needed = false;
        }

        if Instant::now() >= next_refresh_at {
            snapshot = collect_all_quota_watch_snapshot(
                &Local::now().format("%Y-%m-%d %H:%M:%S %Z").to_string(),
                AppState::load(paths).map_err(|err| err.to_string()),
                base_url,
            );
            redraw_needed = true;
            next_refresh_at = Instant::now() + Duration::from_secs(DEFAULT_WATCH_INTERVAL_SECONDS);
            continue;
        }

        if let Some(command) = input.as_mut().and_then(QuotaWatchInput::read_command) {
            let max_scroll_offset = match &snapshot {
                AllQuotaWatchSnapshot::Reports { reports, .. } => reports.len().saturating_sub(1),
                _ => 0,
            };
            match command {
                QuotaWatchCommand::Up => {
                    let next_offset = scroll_offset.saturating_sub(1);
                    if next_offset != scroll_offset {
                        scroll_offset = next_offset;
                        redraw_needed = true;
                    }
                }
                QuotaWatchCommand::Down => {
                    let next_offset = scroll_offset.saturating_add(1).min(max_scroll_offset);
                    if next_offset != scroll_offset {
                        scroll_offset = next_offset;
                        redraw_needed = true;
                    }
                }
                QuotaWatchCommand::Quit => return Ok(()),
            }
        }

        thread::sleep(Duration::from_millis(QUOTA_WATCH_INPUT_POLL_MS));
    }
}

pub(crate) fn read_auth_summary(codex_home: &Path) -> AuthSummary {
    let auth_path = secret_store::auth_json_path(codex_home);
    if !auth_path.is_file() {
        return AuthSummary {
            label: "no-auth".to_string(),
            quota_compatible: false,
        };
    }

    let content = match read_auth_json_text(codex_home) {
        Ok(Some(content)) => content,
        Err(_) => {
            return AuthSummary {
                label: "unreadable-auth".to_string(),
                quota_compatible: false,
            };
        }
        Ok(None) => {
            return AuthSummary {
                label: "no-auth".to_string(),
                quota_compatible: false,
            };
        }
    };

    let stored_auth: StoredAuth = match serde_json::from_str(&content) {
        Ok(auth) => auth,
        Err(_) => {
            return AuthSummary {
                label: "invalid-auth".to_string(),
                quota_compatible: false,
            };
        }
    };

    let has_chatgpt_token = stored_auth
        .tokens
        .as_ref()
        .and_then(|tokens| tokens.access_token.as_deref())
        .is_some_and(|token| !token.trim().is_empty());
    let has_api_key = stored_auth
        .openai_api_key
        .as_deref()
        .is_some_and(|key| !key.trim().is_empty());

    if has_chatgpt_token {
        return AuthSummary {
            label: "chatgpt".to_string(),
            quota_compatible: true,
        };
    }

    if matches!(stored_auth.auth_mode.as_deref(), Some("api_key")) || has_api_key {
        return AuthSummary {
            label: "api-key".to_string(),
            quota_compatible: false,
        };
    }

    AuthSummary {
        label: stored_auth
            .auth_mode
            .unwrap_or_else(|| "auth-present".to_string()),
        quota_compatible: false,
    }
}

pub(crate) fn read_usage_auth(codex_home: &Path) -> Result<UsageAuth> {
    let auth_path = secret_store::auth_json_path(codex_home);
    if !auth_path.is_file() {
        bail!(
            "auth file not found at {}. Run `codex login` first.",
            auth_path.display()
        );
    }

    let content = read_auth_json_text(codex_home)
        .with_context(|| format!("failed to read {}", auth_path.display()))?
        .with_context(|| format!("failed to read {}", auth_path.display()))?;
    let stored_auth: StoredAuth = serde_json::from_str(&content)
        .with_context(|| format!("failed to parse {}", auth_path.display()))?;

    let has_api_key = stored_auth
        .openai_api_key
        .as_deref()
        .is_some_and(|key| !key.trim().is_empty());
    if matches!(stored_auth.auth_mode.as_deref(), Some("api_key")) || has_api_key {
        bail!("quota endpoint requires a ChatGPT access token. Run `codex login` first.");
    }

    let tokens = stored_auth
        .tokens
        .as_ref()
        .context("auth tokens are missing from auth.json")?;
    let access_token = tokens
        .access_token
        .as_deref()
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .context("access token not found in auth.json")?
        .to_string();
    let account_id = tokens
        .account_id
        .as_deref()
        .map(str::trim)
        .filter(|account_id| !account_id.is_empty())
        .map(ToOwned::to_owned);

    Ok(UsageAuth {
        access_token,
        account_id,
    })
}

pub(crate) fn quota_base_url(explicit: Option<&str>) -> String {
    explicit
        .map(ToOwned::to_owned)
        .or_else(|| env::var("CODEX_CHATGPT_BASE_URL").ok())
        .unwrap_or_else(|| DEFAULT_CHATGPT_BASE_URL.to_string())
        .trim_end_matches('/')
        .to_string()
}

pub(crate) fn usage_url(base_url: &str) -> String {
    let base_url = base_url.trim_end_matches('/');
    if base_url.contains("/backend-api") {
        format!("{base_url}/wham/usage")
    } else {
        format!("{base_url}/api/codex/usage")
    }
}

pub(crate) fn format_response_body(body: &[u8]) -> String {
    if body.is_empty() {
        return String::new();
    }

    if let Ok(value) = serde_json::from_slice::<serde_json::Value>(body) {
        return serde_json::to_string_pretty(&value)
            .unwrap_or_else(|_| String::from_utf8_lossy(body).trim().to_string());
    }

    String::from_utf8_lossy(body).trim().to_string()
}

pub(crate) fn format_binary_resolution(binary: &OsString) -> String {
    let configured = binary.to_string_lossy();
    match resolve_binary_path(binary) {
        Some(path) => format!("{configured} ({})", path.display()),
        None => format!("{configured} (not found)"),
    }
}

pub(crate) fn resolve_binary_path(binary: &OsString) -> Option<PathBuf> {
    let candidate = PathBuf::from(binary);
    if candidate.components().count() > 1 {
        if candidate.is_file() {
            return Some(fs::canonicalize(&candidate).unwrap_or(candidate));
        }
        return None;
    }

    let path_var = env::var_os("PATH")?;
    for directory in env::split_paths(&path_var) {
        let full_path = directory.join(&candidate);
        if full_path.is_file() {
            return Some(full_path);
        }
    }

    None
}
