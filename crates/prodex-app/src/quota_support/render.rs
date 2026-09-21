use super::{ProviderQuotaSnapshot, QuotaReport};
#[cfg(test)]
use prodex_quota::UsageWindow;
use prodex_quota::{BlockedLimit, UsageResponse};

pub(crate) use prodex_quota::{QuotaReportSort, RenderedQuotaReportWindow};

fn render_quota_report_inputs(reports: &[QuotaReport]) -> Vec<prodex_quota::QuotaReport> {
    reports.to_vec()
}

pub(crate) fn render_quota_reports(reports: &[QuotaReport], detail: bool) -> String {
    prodex_quota::render_quota_reports(&render_quota_report_inputs(reports), detail)
}

pub(crate) fn render_quota_reports_window_with_sort(
    reports: &[QuotaReport],
    detail: bool,
    max_lines: Option<usize>,
    total_width: usize,
    start_profile: usize,
    interactive_scroll_hint: bool,
    sort: QuotaReportSort,
) -> RenderedQuotaReportWindow {
    prodex_quota::render_quota_reports_window_with_sort(
        &render_quota_report_inputs(reports),
        detail,
        max_lines,
        total_width,
        start_profile,
        interactive_scroll_hint,
        sort,
    )
}

pub(crate) fn sorted_quota_report_indexes_by_sort(
    reports: &[QuotaReport],
    sort: QuotaReportSort,
) -> Vec<usize> {
    prodex_quota::sorted_quota_report_indexes_by(&render_quota_report_inputs(reports), sort)
}

pub(crate) fn quota_pool_summary_fields_for_reports(
    reports: &[QuotaReport],
) -> Vec<(String, String)> {
    prodex_quota::quota_pool_summary_fields(&render_quota_report_inputs(reports))
}

pub(crate) fn format_main_windows(usage: &UsageResponse) -> String {
    prodex_quota::format_main_windows(usage)
}

pub(crate) fn format_main_windows_compact(usage: &UsageResponse) -> String {
    prodex_quota::format_main_windows_compact(usage)
}

#[cfg(test)]
pub(crate) fn format_main_reset_summary(usage: &UsageResponse) -> String {
    prodex_quota::format_main_reset_summary(usage)
}

#[cfg(test)]
pub(crate) fn format_window_status(window: &UsageWindow) -> String {
    prodex_quota::format_window_status(window)
}

#[cfg(test)]
pub(crate) fn format_window_status_compact(window: &UsageWindow) -> String {
    prodex_quota::format_window_status_compact(window)
}

pub(crate) fn collect_blocked_limits(
    usage: &UsageResponse,
    include_code_review: bool,
) -> Vec<BlockedLimit> {
    prodex_quota::collect_blocked_limits(usage, include_code_review)
}

pub(crate) fn format_blocked_limits(blocked: &[BlockedLimit]) -> String {
    prodex_quota::format_blocked_limits(blocked)
}

pub(crate) fn format_openai_quota_status(usage: &UsageResponse) -> String {
    prodex_quota::format_openai_quota_status(usage)
}

pub(crate) fn format_quota_error_status(error: &str) -> String {
    prodex_quota::format_quota_error_status(error)
}

#[cfg(test)]
pub(crate) fn window_label(seconds: Option<i64>) -> String {
    prodex_quota::window_label(seconds)
}

#[cfg(test)]
pub(crate) fn format_precise_reset_time(epoch: Option<i64>) -> String {
    prodex_quota::format_precise_reset_time(epoch)
}

pub(crate) fn format_copilot_quota_status(info: &prodex_quota::CopilotQuotaInfo) -> String {
    prodex_quota::format_copilot_quota_status(info)
}

pub(crate) fn format_copilot_main_quota(info: &prodex_quota::CopilotQuotaInfo) -> String {
    prodex_quota::format_copilot_main_quota(info)
}

pub(crate) fn format_copilot_reset_summary(
    info: &prodex_quota::CopilotQuotaInfo,
) -> Option<String> {
    prodex_quota::format_copilot_reset_summary(info)
}

pub(crate) fn format_gemini_quota_status(info: &prodex_quota::GeminiQuotaInfo) -> String {
    prodex_quota::format_gemini_quota_status(info)
}

pub(crate) fn format_gemini_main_quota(info: &prodex_quota::GeminiQuotaInfo) -> String {
    prodex_quota::format_gemini_main_quota(info)
}

pub(crate) fn format_gemini_reset_summary(info: &prodex_quota::GeminiQuotaInfo) -> Option<String> {
    prodex_quota::format_gemini_reset_summary(info)
}

pub(crate) fn render_profile_quota_snapshot_with_detail(
    profile_name: &str,
    snapshot: &ProviderQuotaSnapshot,
    detail: bool,
) -> String {
    prodex_quota::render_profile_quota_snapshot_with_detail(profile_name, snapshot, detail)
}

pub(crate) fn first_line_of_error(input: &str) -> String {
    prodex_quota::first_line_of_error(input)
}

pub(super) fn render_quota_watch_error_panel(title: &str, message: &str) -> String {
    prodex_quota::render_quota_error_panel_with_width(
        title,
        message,
        terminal_ui::current_cli_width(),
    )
}
