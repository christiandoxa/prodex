use super::{ProviderQuotaSnapshot, QuotaReport};
use prodex_profile_export::CopilotUserInfo;
#[cfg(test)]
use prodex_quota::UsageWindow;
use prodex_quota::{BlockedLimit, UsageResponse};

pub(crate) use prodex_quota::{QuotaReportSort, RenderedQuotaReportWindow};

fn render_quota_report_inputs(reports: &[QuotaReport]) -> Vec<prodex_quota::QuotaReport> {
    reports.iter().map(render_quota_report_input).collect()
}

fn render_quota_report_input(report: &QuotaReport) -> prodex_quota::QuotaReport {
    prodex_quota::QuotaReport {
        name: report.name.clone(),
        active: report.active,
        auth: report.auth.clone(),
        workspace_id: report.workspace_id.clone(),
        result: report
            .result
            .as_ref()
            .map(render_quota_snapshot)
            .map_err(Clone::clone),
        fetched_at: report.fetched_at,
    }
}

fn render_quota_snapshot(snapshot: &ProviderQuotaSnapshot) -> prodex_quota::ProviderQuotaSnapshot {
    match snapshot {
        ProviderQuotaSnapshot::OpenAi(usage) => {
            prodex_quota::ProviderQuotaSnapshot::OpenAi(usage.clone())
        }
        ProviderQuotaSnapshot::Copilot(info) => {
            prodex_quota::ProviderQuotaSnapshot::Copilot(render_copilot_quota_info(info))
        }
        ProviderQuotaSnapshot::Gemini(info) => {
            prodex_quota::ProviderQuotaSnapshot::Gemini(info.clone())
        }
    }
}

fn render_copilot_quota_info(info: &CopilotUserInfo) -> prodex_quota::CopilotQuotaInfo {
    prodex_quota::CopilotQuotaInfo {
        login: info.login.clone(),
        access_type_sku: info.access_type_sku.clone(),
        copilot_plan: info.copilot_plan.clone(),
        limited_user_quotas: info.limited_user_quotas.clone(),
        monthly_quotas: info.monthly_quotas.clone(),
        limited_user_reset_date: info.limited_user_reset_date.clone(),
    }
}

pub(crate) fn render_quota_reports(reports: &[QuotaReport], detail: bool) -> String {
    prodex_quota::render_quota_reports(&render_quota_report_inputs(reports), detail)
}

#[cfg(test)]
pub(crate) fn render_quota_reports_with_line_limit(
    reports: &[QuotaReport],
    detail: bool,
    max_lines: Option<usize>,
) -> String {
    prodex_quota::render_quota_reports_with_line_limit(
        &render_quota_report_inputs(reports),
        detail,
        max_lines,
    )
}

#[cfg(test)]
pub(crate) fn render_quota_reports_with_layout(
    reports: &[QuotaReport],
    detail: bool,
    max_lines: Option<usize>,
    total_width: usize,
) -> String {
    prodex_quota::render_quota_reports_with_layout(
        &render_quota_report_inputs(reports),
        detail,
        max_lines,
        total_width,
    )
}

#[cfg(test)]
pub(crate) fn render_quota_reports_window_with_layout(
    reports: &[QuotaReport],
    detail: bool,
    max_lines: Option<usize>,
    total_width: usize,
    start_profile: usize,
    interactive_scroll_hint: bool,
) -> RenderedQuotaReportWindow {
    prodex_quota::render_quota_reports_window_with_layout(
        &render_quota_report_inputs(reports),
        detail,
        max_lines,
        total_width,
        start_profile,
        interactive_scroll_hint,
    )
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

#[cfg(test)]
pub(crate) fn sort_quota_reports_for_display(reports: &[QuotaReport]) -> Vec<&QuotaReport> {
    let render_reports = render_quota_report_inputs(reports);
    prodex_quota::sorted_quota_report_indexes(&render_reports)
        .into_iter()
        .map(|index| &reports[index])
        .collect()
}

pub(crate) fn format_main_windows(usage: &UsageResponse) -> String {
    prodex_quota::format_main_windows(usage)
}

#[cfg(test)]
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

#[cfg(test)]
pub(crate) fn window_label(seconds: Option<i64>) -> String {
    prodex_quota::window_label(seconds)
}

#[cfg(test)]
pub(crate) fn format_precise_reset_time(epoch: Option<i64>) -> String {
    prodex_quota::format_precise_reset_time(epoch)
}

pub(crate) fn format_copilot_quota_status(info: &CopilotUserInfo) -> String {
    prodex_quota::format_copilot_quota_status(&render_copilot_quota_info(info))
}

pub(crate) fn format_copilot_main_quota(info: &CopilotUserInfo) -> String {
    prodex_quota::format_copilot_main_quota(&render_copilot_quota_info(info))
}

pub(crate) fn format_copilot_reset_summary(info: &CopilotUserInfo) -> Option<String> {
    prodex_quota::format_copilot_reset_summary(&render_copilot_quota_info(info))
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

pub(crate) fn render_profile_quota_snapshot(
    profile_name: &str,
    snapshot: &ProviderQuotaSnapshot,
) -> String {
    prodex_quota::render_profile_quota_snapshot(profile_name, &render_quota_snapshot(snapshot))
}

pub(crate) fn first_line_of_error(input: &str) -> String {
    prodex_quota::first_line_of_error(input)
}

pub(super) fn render_quota_watch_error_panel(title: &str, message: &str) -> String {
    prodex_quota::render_quota_error_panel(title, message)
}
