use anyhow::{Context, Result};
use std::env;
use std::fs;
use std::io::{self, Write};
use std::process::Command;

pub const CLI_WIDTH: usize = 110;
pub const CLI_MIN_WIDTH: usize = 60;
pub const CLI_LABEL_WIDTH: usize = 16;
pub const CLI_MIN_LABEL_WIDTH: usize = 10;
pub const CLI_MAX_LABEL_WIDTH: usize = 24;
pub const CLI_TABLE_GAP: &str = "  ";

pub fn section_header(title: &str) -> String {
    section_header_with_width(title, current_cli_width())
}

pub fn section_header_with_width(title: &str, total_width: usize) -> String {
    let prefix = format!("[ {title} ] ");
    let width = text_width(&prefix);
    if width >= total_width {
        return prefix;
    }

    format!("{prefix}{}", "=".repeat(total_width - width))
}

pub fn text_width(value: &str) -> usize {
    value.chars().count()
}

pub fn fit_cell(value: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }

    if text_width(value) <= width {
        return value.to_string();
    }

    if width <= 3 {
        return ".".repeat(width);
    }

    let mut output = String::new();
    for ch in value.chars().take(width - 3) {
        output.push(ch);
    }
    output.push_str("...");
    output
}

pub fn chunk_token(token: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![String::new()];
    }
    if token.is_empty() {
        return vec![String::new()];
    }

    let mut chunks = Vec::new();
    let mut current = String::new();
    for ch in token.chars() {
        current.push(ch);
        if text_width(&current) >= width {
            chunks.push(std::mem::take(&mut current));
        }
    }

    if !current.is_empty() {
        chunks.push(current);
    }

    chunks
}

pub fn wrap_text(input: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![String::new()];
    }

    let mut lines = Vec::new();
    for paragraph in input.lines() {
        if paragraph.trim().is_empty() {
            lines.push(String::new());
            continue;
        }

        let mut current = String::new();
        for word in paragraph.split_whitespace() {
            for piece in chunk_token(word, width) {
                if current.is_empty() {
                    current.push_str(&piece);
                } else if text_width(&current) + 1 + text_width(&piece) <= width {
                    current.push(' ');
                    current.push_str(&piece);
                } else {
                    lines.push(std::mem::take(&mut current));
                    current.push_str(&piece);
                }
            }
        }

        if !current.is_empty() {
            lines.push(current);
        }
    }

    if lines.is_empty() {
        lines.push(String::new());
    }

    lines
}

pub fn current_cli_width() -> usize {
    terminal_width_chars()
        .unwrap_or(CLI_WIDTH)
        .max(CLI_MIN_WIDTH)
}

pub fn terminal_size_override_usize(env_key: &str) -> Option<usize> {
    env::var(env_key)
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .filter(|value| *value > 0)
}

pub fn terminal_dimensions_from_tty() -> Option<(usize, usize)> {
    let tty = fs::File::open("/dev/tty").ok()?;
    let output = Command::new("stty").arg("size").stdin(tty).output().ok()?;
    if !output.status.success() {
        return None;
    }

    let text = String::from_utf8(output.stdout).ok()?;
    let mut parts = text.split_whitespace();
    let rows = parts.next()?.parse::<usize>().ok()?;
    let cols = parts.next()?.parse::<usize>().ok()?;
    Some((rows, cols))
}

pub fn terminal_width_chars() -> Option<usize> {
    terminal_size_override_usize("PRODEX_TERM_COLUMNS")
        .or_else(|| terminal_dimensions_from_tty().map(|(_, cols)| cols))
}

pub fn terminal_height_lines() -> Option<usize> {
    terminal_size_override_usize("PRODEX_TERM_LINES")
        .or_else(|| terminal_size_override_usize("LINES"))
        .or_else(|| terminal_dimensions_from_tty().map(|(rows, _)| rows))
}

pub struct FieldRowsBuilder {
    rows: Vec<(String, String)>,
}

impl FieldRowsBuilder {
    pub fn new() -> Self {
        Self { rows: Vec::new() }
    }

    pub fn push(&mut self, label: impl Into<String>, value: impl Into<String>) -> &mut Self {
        self.rows.push((label.into(), value.into()));
        self
    }

    pub fn extend(&mut self, rows: impl IntoIterator<Item = (String, String)>) -> &mut Self {
        self.rows.extend(rows);
        self
    }

    pub fn build(self) -> Vec<(String, String)> {
        self.rows
    }
}

impl Default for FieldRowsBuilder {
    fn default() -> Self {
        Self::new()
    }
}

pub struct PanelBuilder {
    title: String,
    fields: FieldRowsBuilder,
}

impl PanelBuilder {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            fields: FieldRowsBuilder::new(),
        }
    }

    pub fn push(&mut self, label: impl Into<String>, value: impl Into<String>) -> &mut Self {
        self.fields.push(label, value);
        self
    }

    pub fn extend(&mut self, rows: impl IntoIterator<Item = (String, String)>) -> &mut Self {
        self.fields.extend(rows);
        self
    }

    pub fn render(self) -> String {
        let fields = self.fields.build();
        render_panel(&self.title, &fields)
    }
}

pub fn panel_label_width(fields: &[(String, String)], total_width: usize) -> usize {
    let longest = fields
        .iter()
        .map(|(label, _)| text_width(label) + 1)
        .max()
        .unwrap_or(CLI_LABEL_WIDTH);
    let max_by_width = total_width
        .saturating_sub(20)
        .clamp(CLI_MIN_LABEL_WIDTH, CLI_MAX_LABEL_WIDTH);
    let preferred_cap = (total_width / 4).clamp(CLI_MIN_LABEL_WIDTH, CLI_MAX_LABEL_WIDTH);
    longest.clamp(CLI_MIN_LABEL_WIDTH, max_by_width.min(preferred_cap))
}

pub fn format_field_lines_with_layout(
    label: &str,
    value: &str,
    total_width: usize,
    label_width: usize,
) -> Vec<String> {
    let label = format!("{label}:");
    let value_width = total_width.saturating_sub(label_width + 1).max(1);
    let wrapped = wrap_text(value, value_width);
    let mut lines = Vec::new();

    for (index, line) in wrapped.into_iter().enumerate() {
        let field_label = if index == 0 { label.as_str() } else { "" };
        lines.push(format!(
            "{field_label:<label_w$} {line}",
            label_w = label_width
        ));
    }

    lines
}

fn panel_lines_with_layout(
    title: &str,
    fields: &[(String, String)],
    total_width: usize,
) -> Vec<String> {
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
    lines
}

pub fn print_panel(title: &str, fields: &[(String, String)]) {
    for line in panel_lines_with_layout(title, fields, current_cli_width()) {
        print_stdout_line(&line);
    }
}

pub fn render_panel(title: &str, fields: &[(String, String)]) -> String {
    panel_lines_with_layout(title, fields, current_cli_width()).join("\n")
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct InfoLoadSummaryDisplay {
    pub log_count: usize,
    pub active_inflight_units: usize,
    pub recent_selection_events: usize,
    pub recent_first_timestamp: Option<i64>,
    pub recent_last_timestamp: Option<i64>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TokenUsageCounts {
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
    pub output_tokens: u64,
    pub reasoning_tokens: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TokenUsageProfileDisplay<'a> {
    pub profile: &'a str,
    pub total: TokenUsageCounts,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InfoRunwayEstimateDisplay<'a> {
    pub burn_per_hour: f64,
    pub observed_profiles: usize,
    pub observed_span_seconds: i64,
    pub exhaust_at: i64,
    pub exhaust_text: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InfoRunwayResetDisplay<'a> {
    pub reset_at: i64,
    pub reset_text: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeTuningWorkersDisplay {
    pub worker_count: usize,
    pub long_lived_worker_count: usize,
    pub async_worker_count: usize,
    pub probe_refresh_worker_count: usize,
    pub active_request_limit: usize,
    pub long_lived_queue_capacity: usize,
    pub lane_responses: usize,
    pub lane_compact: usize,
    pub lane_websocket: usize,
    pub lane_standard: usize,
    pub websocket_connect_worker_count: usize,
    pub websocket_connect_queue_capacity: usize,
    pub websocket_connect_overflow_capacity: usize,
    pub websocket_dns_worker_count: usize,
    pub websocket_dns_queue_capacity: usize,
    pub websocket_dns_overflow_capacity: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeTuningBudgetsDisplay {
    pub precommit_attempt_limit: usize,
    pub precommit_budget_ms: u64,
    pub pressure_precommit_attempt_limit: usize,
    pub pressure_precommit_budget_ms: u64,
    pub continuation_precommit_attempt_limit: usize,
    pub continuation_precommit_budget_ms: u64,
    pub admission_wait_budget_ms: u64,
    pub pressure_admission_wait_budget_ms: u64,
    pub long_lived_queue_wait_budget_ms: u64,
    pub pressure_long_lived_queue_wait_budget_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeTuningTransportDisplay {
    pub http_connect_timeout_ms: u64,
    pub stream_idle_timeout_ms: u64,
    pub sse_lookahead_timeout_ms: u64,
    pub websocket_connect_timeout_ms: u64,
    pub websocket_precommit_progress_timeout_ms: u64,
    pub websocket_happy_eyeballs_delay_ms: u64,
    pub websocket_previous_response_reuse_stale_ms: u64,
    pub profile_inflight_soft_limit: usize,
    pub profile_inflight_hard_limit: usize,
}

pub fn format_info_process_summary_display(
    total_count: usize,
    runtime_count: usize,
    pids: impl IntoIterator<Item = u32>,
    max_visible_pids: usize,
) -> String {
    if total_count == 0 {
        return "No".to_string();
    }

    let pid_list = pids
        .into_iter()
        .take(max_visible_pids)
        .map(|pid| pid.to_string())
        .collect::<Vec<_>>()
        .join(", ");
    let remaining = total_count.saturating_sub(max_visible_pids);
    let extra = if remaining > 0 {
        format!(" (+{remaining} more)")
    } else {
        String::new()
    };

    format!("Yes ({total_count} total, {runtime_count} runtime; pids: {pid_list}{extra})")
}

pub fn format_info_load_summary_display(
    summary: InfoLoadSummaryDisplay,
    runtime_process_count: usize,
    recent_load_window_seconds: i64,
) -> String {
    if runtime_process_count == 0 {
        return "No active prodex runtime detected".to_string();
    }
    if summary.log_count == 0 {
        return "Runtime process detected, but no matching runtime log was found".to_string();
    }
    if summary.recent_selection_events == 0 {
        return format!(
            "{} active runtime log(s); no selection activity observed in the sampled window; inflight units {}",
            summary.log_count, summary.active_inflight_units
        );
    }
    if summary.recent_selection_events == 1 {
        return format!(
            "1 selection event observed in the sampled window; inflight units {}; {} active runtime log(s)",
            summary.active_inflight_units, summary.log_count
        );
    }

    let activity_span = summary
        .recent_first_timestamp
        .zip(summary.recent_last_timestamp)
        .map(|(start, end)| format_relative_duration(end.saturating_sub(start)))
        .unwrap_or_else(|| format!("{}m", recent_load_window_seconds / 60));

    format!(
        "{} selection event(s) over {}; inflight units {}; {} active runtime log(s)",
        summary.recent_selection_events,
        activity_span,
        summary.active_inflight_units,
        summary.log_count
    )
}

pub fn format_info_quota_data_summary_display(
    quota_compatible_profiles: usize,
    live_profiles: usize,
    snapshot_profiles: usize,
    unavailable_profiles: usize,
) -> String {
    if quota_compatible_profiles == 0 {
        return "No quota-compatible profiles".to_string();
    }

    format!(
        "{quota_compatible_profiles} quota-compatible profile(s): live={live_profiles}, snapshot={snapshot_profiles}, unavailable={unavailable_profiles}"
    )
}

pub fn format_info_token_usage_summary_display<'a>(
    event_count: usize,
    log_count: usize,
    total: TokenUsageCounts,
    by_profile: impl IntoIterator<Item = TokenUsageProfileDisplay<'a>>,
) -> String {
    if event_count == 0 {
        return format!("No token_usage events found in {log_count} recent runtime log(s)");
    }

    let top_profiles = by_profile
        .into_iter()
        .take(4)
        .map(|entry| {
            format!(
                "{}:{} in/{} cached/{} out/{} reasoning",
                entry.profile,
                entry.total.input_tokens,
                entry.total.cached_input_tokens,
                entry.total.output_tokens,
                entry.total.reasoning_tokens
            )
        })
        .collect::<Vec<_>>()
        .join("; ");
    let suffix = if top_profiles.is_empty() {
        String::new()
    } else {
        format!("; by profile: {top_profiles}")
    };

    format!(
        "{} event(s), logs={}: input={}, cached_input={}, output={}, reasoning={}{}",
        event_count,
        log_count,
        total.input_tokens,
        total.cached_input_tokens,
        total.output_tokens,
        total.reasoning_tokens,
        suffix
    )
}

pub fn format_runtime_policy_summary_display(path: Option<&str>, version: Option<u32>) -> String {
    path.zip(version)
        .map(|(path, version)| format!("{path} (v{version})"))
        .unwrap_or_else(|| "disabled".to_string())
}

pub fn format_runtime_logs_summary_display(directory: &str, format: &str) -> String {
    format!("{directory} ({format})")
}

pub fn format_runtime_tuning_workers_display(snapshot: RuntimeTuningWorkersDisplay) -> String {
    format!(
        "workers proxy={}, long-lived={}, async={}, probe-refresh={}; active={}, queue={}; lanes responses={}, compact={}, websocket={}, standard={}; ws-connect workers={}, queue={}, overflow={}; ws-dns workers={}, queue={}, overflow={}",
        snapshot.worker_count,
        snapshot.long_lived_worker_count,
        snapshot.async_worker_count,
        snapshot.probe_refresh_worker_count,
        snapshot.active_request_limit,
        snapshot.long_lived_queue_capacity,
        snapshot.lane_responses,
        snapshot.lane_compact,
        snapshot.lane_websocket,
        snapshot.lane_standard,
        snapshot.websocket_connect_worker_count,
        snapshot.websocket_connect_queue_capacity,
        snapshot.websocket_connect_overflow_capacity,
        snapshot.websocket_dns_worker_count,
        snapshot.websocket_dns_queue_capacity,
        snapshot.websocket_dns_overflow_capacity
    )
}

pub fn format_runtime_tuning_budgets_display(snapshot: RuntimeTuningBudgetsDisplay) -> String {
    format!(
        "precommit={}x/{}ms, pressure-precommit={}x/{}ms, continuation={}x/{}ms; admission={}ms, pressure-admission={}ms, long-lived={}ms, pressure-long-lived={}ms",
        snapshot.precommit_attempt_limit,
        snapshot.precommit_budget_ms,
        snapshot.pressure_precommit_attempt_limit,
        snapshot.pressure_precommit_budget_ms,
        snapshot.continuation_precommit_attempt_limit,
        snapshot.continuation_precommit_budget_ms,
        snapshot.admission_wait_budget_ms,
        snapshot.pressure_admission_wait_budget_ms,
        snapshot.long_lived_queue_wait_budget_ms,
        snapshot.pressure_long_lived_queue_wait_budget_ms
    )
}

pub fn format_runtime_tuning_transport_display(snapshot: RuntimeTuningTransportDisplay) -> String {
    format!(
        "http-connect={}ms, stream-idle={}ms, sse-lookahead={}ms; ws-connect={}ms, ws-progress={}ms, ws-happy={}ms, ws-stale-reuse={}ms; inflight soft/hard={}/{}",
        snapshot.http_connect_timeout_ms,
        snapshot.stream_idle_timeout_ms,
        snapshot.sse_lookahead_timeout_ms,
        snapshot.websocket_connect_timeout_ms,
        snapshot.websocket_precommit_progress_timeout_ms,
        snapshot.websocket_happy_eyeballs_delay_ms,
        snapshot.websocket_previous_response_reuse_stale_ms,
        snapshot.profile_inflight_soft_limit,
        snapshot.profile_inflight_hard_limit
    )
}

pub fn format_info_pool_remaining_display(
    total_remaining: i64,
    profiles_with_data: usize,
    earliest_reset_text: Option<&str>,
) -> String {
    if profiles_with_data == 0 {
        return "Unavailable".to_string();
    }

    let mut value = format!("{total_remaining}% across {profiles_with_data} profile(s)");
    if let Some(reset_text) = earliest_reset_text {
        value.push_str(&format!("; earliest reset {reset_text}"));
    }
    value
}

pub fn format_info_runway_display(
    profiles_with_data: usize,
    current_remaining: i64,
    earliest_reset: Option<InfoRunwayResetDisplay<'_>>,
    estimate: Option<InfoRunwayEstimateDisplay<'_>>,
    now: i64,
) -> String {
    if profiles_with_data == 0 {
        return "Unavailable".to_string();
    }
    if current_remaining <= 0 {
        return "Exhausted".to_string();
    }

    let Some(estimate) = estimate else {
        return "Unavailable (no recent quota decay observed in active runtime logs)".to_string();
    };

    let observed = format_relative_duration(estimate.observed_span_seconds);
    let burn = format!("{:.1}", estimate.burn_per_hour);
    if let Some(reset) = earliest_reset
        && reset.reset_at <= estimate.exhaust_at
    {
        return format!(
            "Earliest reset {} arrives before the no-reset runway (~{} at {} aggregated-%/h, {} profile(s), observed over {})",
            reset.reset_text,
            format_relative_duration(estimate.exhaust_at.saturating_sub(now)),
            burn,
            estimate.observed_profiles,
            observed
        );
    }

    format!(
        "{} (~{}) at {} aggregated-%/h from {} profile(s), observed over {}, no-reset estimate",
        estimate.exhaust_text,
        format_relative_duration(estimate.exhaust_at.saturating_sub(now)),
        burn,
        estimate.observed_profiles,
        observed
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionReportDisplay<'a> {
    pub id: &'a str,
    pub updated_at: Option<&'a str>,
    pub thread_name: Option<&'a str>,
    pub cwd: Option<&'a str>,
    pub profile: Option<&'a str>,
    pub path: &'a str,
}

pub fn render_session_reports(reports: &[SessionReportDisplay<'_>]) -> String {
    render_session_reports_with_width(reports, current_cli_width())
}

pub fn render_session_reports_with_width(
    reports: &[SessionReportDisplay<'_>],
    total_width: usize,
) -> String {
    let widths = session_report_column_widths(total_width);
    let mut lines = vec![section_header_with_width("Sessions", total_width)];
    lines.push(format_session_report_row(
        "ID", "UPDATED", "THREAD", "CWD", "PATH", widths,
    ));
    lines.push("-".repeat(text_width(lines.last().map(String::as_str).unwrap_or(""))));

    for report in reports {
        lines.push(format_session_report_row(
            report.id,
            report.updated_at.unwrap_or("-"),
            report.thread_name.unwrap_or("-"),
            report.cwd.unwrap_or("-"),
            report.path,
            widths,
        ));
        if let Some(profile) = report.profile {
            lines.push(format!("  profile: {profile}"));
        }
    }

    lines.join("\n")
}

#[derive(Clone, Copy)]
struct SessionReportColumnWidths {
    id: usize,
    updated: usize,
    thread: usize,
    cwd: usize,
    path: usize,
}

fn session_report_column_widths(total_width: usize) -> SessionReportColumnWidths {
    let gap_width = text_width(CLI_TABLE_GAP) * 4;
    let available = total_width.saturating_sub(gap_width).max(60);
    let id = (available / 5).clamp(12, 26);
    let updated = (available / 5).clamp(12, 22);
    let thread = (available / 5).clamp(12, 24);
    let remaining = available.saturating_sub(id + updated + thread);
    let cwd = (remaining / 2).max(12);
    let path = remaining.saturating_sub(cwd).max(12);
    SessionReportColumnWidths {
        id,
        updated,
        thread,
        cwd,
        path,
    }
}

fn format_session_report_row(
    id: &str,
    updated: &str,
    thread_name: &str,
    cwd: &str,
    path: &str,
    widths: SessionReportColumnWidths,
) -> String {
    format!(
        "{:<id_w$}{gap}{:<updated_w$}{gap}{:<thread_w$}{gap}{:<cwd_w$}{gap}{:<path_w$}",
        fit_cell(id, widths.id),
        fit_cell(updated, widths.updated),
        fit_cell(thread_name, widths.thread),
        fit_cell(cwd, widths.cwd),
        fit_cell(path, widths.path),
        gap = CLI_TABLE_GAP,
        id_w = widths.id,
        updated_w = widths.updated,
        thread_w = widths.thread,
        cwd_w = widths.cwd,
        path_w = widths.path,
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeLaunchCandidateDisplay<'a> {
    pub name: &'a str,
    pub quota_summary: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeLaunchSelectedProfileStatus<'a> {
    Ready,
    Blocked { blocked_summary: &'a str },
    ProbeFailed { error: &'a str },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeLaunchScoredCandidateMessage<'a> {
    pub initial_profile_name: &'a str,
    pub candidate: RuntimeLaunchCandidateDisplay<'a>,
    pub selected_profile_status: Option<RuntimeLaunchSelectedProfileStatus<'a>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeLaunchScoredCandidateOutput {
    pub warning: Option<String>,
    pub selection: String,
}

pub fn format_runtime_launch_scored_candidate_message(
    message: RuntimeLaunchScoredCandidateMessage<'_>,
) -> RuntimeLaunchScoredCandidateOutput {
    let RuntimeLaunchCandidateDisplay {
        name,
        quota_summary,
    } = message.candidate;
    let mut output = RuntimeLaunchScoredCandidateOutput {
        warning: None,
        selection: format!("Using profile '{name}' ({quota_summary})"),
    };

    match message.selected_profile_status {
        Some(RuntimeLaunchSelectedProfileStatus::Blocked { blocked_summary }) => {
            output.warning = Some(format!(
                "Quota preflight blocked profile '{}': {}",
                message.initial_profile_name, blocked_summary
            ));
            output.selection = format!(
                "Auto-rotating to profile '{name}' using quota-pressure scoring ({quota_summary})."
            );
        }
        Some(RuntimeLaunchSelectedProfileStatus::Ready) => {
            output.selection = format!(
                "Auto-selecting profile '{name}' over active profile '{}' using quota-pressure scoring ({quota_summary}).",
                message.initial_profile_name
            );
        }
        Some(RuntimeLaunchSelectedProfileStatus::ProbeFailed { error }) => {
            output.warning = Some(format!(
                "Warning: quota preflight failed for '{}': {error}",
                message.initial_profile_name
            ));
            output.selection = format!(
                "Using ready profile '{name}' after quota preflight failed ({quota_summary})"
            );
        }
        None => {}
    }

    output
}

pub fn format_runtime_provider_direct_launch_message(provider_id: &str, source: &str) -> String {
    format!(
        "Detected model_provider '{provider_id}' from {source}. Launching directly without prodex quota preflight or auto-rotate proxy."
    )
}

pub fn format_runtime_launch_quota_inspect_hint(profile_name: &str) -> String {
    format!(
        "Inspect with `prodex quota --profile {profile_name}` or bypass with `prodex run --skip-quota-check`."
    )
}

pub fn format_relative_duration(seconds: i64) -> String {
    let seconds = seconds.max(0);
    if seconds == 0 {
        return "now".to_string();
    }

    let days = seconds / 86_400;
    let hours = (seconds % 86_400) / 3_600;
    let minutes = (seconds % 3_600) / 60;

    if days > 0 {
        if hours > 0 {
            format!("{days}d {hours}h")
        } else {
            format!("{days}d")
        }
    } else if hours > 0 {
        if minutes > 0 {
            format!("{hours}h {minutes}m")
        } else {
            format!("{hours}h")
        }
    } else if minutes > 0 {
        format!("{minutes}m")
    } else {
        "<1m".to_string()
    }
}

pub fn print_stdout_text(message: &str) {
    print!("{message}");
}

pub fn print_stdout_line(message: &str) {
    println!("{message}");
}

pub fn print_blank_line() {
    println!();
}

pub fn print_stderr_line(message: &str) {
    eprintln!("{message}");
}

pub fn print_stderr_prompt(prompt: &str) -> Result<()> {
    eprint!("{prompt}");
    io::stderr().flush().context("failed to flush prompt")
}

pub fn print_wrapped_stderr(message: &str) {
    for line in wrap_text(message, current_cli_width()) {
        print_stderr_line(&line);
    }
}

#[cfg(test)]
#[path = "../tests/src/lib.rs"]
mod tests;
