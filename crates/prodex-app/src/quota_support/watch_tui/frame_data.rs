use super::frame::AllQuotaWatchTuiRow;
use super::*;

pub(super) fn build_all_quota_watch_tui_row(
    report: &QuotaReport,
    detail: bool,
) -> AllQuotaWatchTuiRow {
    let view = quota_watch_report_view(report);
    let account = vec![view.account];
    let mut detail_line = Vec::new();
    if detail && quota_watch_should_show_workspace(report) {
        detail_line.push(format!(
            "workspace: {}",
            quota_watch_workspace_label(report)
        ));
    }
    if detail && let Some(resets) = view.resets {
        let reset = quota_watch_reset_detail(&resets);
        detail_line.insert(0, reset.windows);
        if let Some(credits) = reset.credits {
            detail_line.push(credits);
        }
    }
    if detail && let Ok(ProviderQuotaSnapshot::OpenAi(usage)) = &report.result {
        detail_line.extend(prodex_quota::format_openai_additional_limit_summaries(
            usage,
        ));
    }
    AllQuotaWatchTuiRow {
        profile: vec![report.name.clone()],
        current: vec![if report.active { "*" } else { "" }.to_string()],
        auth: vec![report.auth.label.clone()],
        account,
        plan: vec![view.plan],
        status: vec![view.status],
        remaining: vec![view.main],
        detail: (!detail_line.is_empty())
            .then(|| detail_line.join(" | "))
            .into_iter()
            .collect(),
    }
}

pub(super) struct QuotaWatchReportView {
    pub(super) account: String,
    pub(super) plan: String,
    pub(super) main: String,
    pub(super) status: String,
    pub(super) resets: Option<String>,
}

pub(super) fn quota_watch_report_view(report: &QuotaReport) -> QuotaWatchReportView {
    match &report.result {
        Ok(ProviderQuotaSnapshot::OpenAi(usage)) => QuotaWatchReportView {
            account: quota_watch_optional(usage.email.as_deref()).to_string(),
            plan: quota_watch_optional(usage.plan_type.as_deref()).to_string(),
            main: format_main_windows_compact(usage),
            status: format_openai_quota_status(usage),
            resets: Some(quota_watch_openai_reset_summary(usage)),
        },
        Ok(ProviderQuotaSnapshot::Copilot(info)) => QuotaWatchReportView {
            account: quota_watch_optional(info.login.as_deref()).to_string(),
            plan: quota_watch_optional(
                info.copilot_plan
                    .as_deref()
                    .or(info.access_type_sku.as_deref()),
            )
            .to_string(),
            main: format_copilot_main_quota(info),
            status: format_copilot_quota_status(info),
            resets: Some(format_copilot_reset_summary(info).map_or_else(
                || "resets: unavailable".to_string(),
                |value| format!("resets: {value}"),
            )),
        },
        Ok(ProviderQuotaSnapshot::Gemini(info)) => QuotaWatchReportView {
            account: quota_watch_optional(info.email.as_deref()).to_string(),
            plan: quota_watch_optional(info.plan.as_deref()).to_string(),
            main: format_gemini_main_quota(info),
            status: format_gemini_quota_status(info),
            resets: Some(format_gemini_reset_summary(info).map_or_else(
                || "resets: unavailable".to_string(),
                |value| format!("resets: {value}"),
            )),
        },
        Ok(ProviderQuotaSnapshot::External(info)) => QuotaWatchReportView {
            account: quota_watch_optional(info.account.as_deref()).to_string(),
            plan: quota_watch_optional(info.plan.as_deref()).to_string(),
            main: info.main.clone(),
            status: info.status.clone(),
            resets: Some(info.reset.as_ref().map_or_else(
                || "resets: unavailable".to_string(),
                |value| format!("resets: {value}"),
            )),
        },
        Err(err) => QuotaWatchReportView {
            account: "-".to_string(),
            plan: "-".to_string(),
            main: "-".to_string(),
            status: format_quota_error_status(err),
            resets: Some(prodex_quota::format_quota_error_detail(err)),
        },
    }
}

fn quota_watch_openai_reset_summary(usage: &UsageResponse) -> String {
    let reset_summary = prodex_quota::format_main_reset_summary(usage);
    match usage.rate_limit_reset_credits.as_ref() {
        Some(credits) => format!(
            "resets: {reset_summary}; reset credits: {} available",
            credits.available_count
        ),
        None => format!("resets: {reset_summary}"),
    }
}

struct QuotaWatchResetDetail {
    windows: String,
    credits: Option<String>,
}

fn quota_watch_reset_detail(resets: &str) -> QuotaWatchResetDetail {
    if resets.trim_start().starts_with("error:") {
        return QuotaWatchResetDetail {
            windows: resets.to_string(),
            credits: None,
        };
    }
    let (windows, credits) = resets
        .strip_prefix("resets: ")
        .unwrap_or(resets)
        .split_once("; reset credits:")
        .map_or(
            (resets.strip_prefix("resets: ").unwrap_or(resets), None),
            |(left, right)| (left, Some(right.trim())),
        );
    QuotaWatchResetDetail {
        windows: format!("resets: {windows}"),
        credits: credits.map(|credits| format!("reset credits: {credits}")),
    }
}

fn quota_watch_optional(value: Option<&str>) -> &str {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("-")
}

fn quota_watch_openai_email(report: &QuotaReport) -> Option<&str> {
    match report.result.as_ref().ok()? {
        ProviderQuotaSnapshot::OpenAi(usage) => usage
            .email
            .as_deref()
            .map(str::trim)
            .filter(|email| !email.is_empty()),
        ProviderQuotaSnapshot::Copilot(_)
        | ProviderQuotaSnapshot::Gemini(_)
        | ProviderQuotaSnapshot::External(_) => None,
    }
}

fn quota_watch_should_show_workspace(report: &QuotaReport) -> bool {
    quota_watch_openai_email(report).is_some()
        && (report
            .workspace_name
            .as_deref()
            .map(str::trim)
            .is_some_and(|name| !name.is_empty())
            || report
                .workspace_id
                .as_deref()
                .map(str::trim)
                .is_some_and(|id| !id.is_empty()))
}

fn quota_watch_workspace_label(report: &QuotaReport) -> String {
    report
        .workspace_name
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| report.name.trim())
        .to_string()
}

pub(super) fn quota_human_tui_compact_label(line: &str) -> Option<&'static str> {
    const LABELS: [&str; 23] = [
        "Available",
        "Last Updated",
        "5h remaining pool",
        "Weekly remaining pool",
        "Remaining pool",
        "Account",
        "Plan",
        "Status",
        "Main",
        "5h",
        "Weekly",
        "Reset credits",
        "Auth",
        "Provider",
        "Reset",
        "Project",
        "Bucket",
        "Quota",
        "Rate groups",
        "Model groups",
        "Admin API",
        "Model provider",
        "Source",
    ];
    LABELS
        .into_iter()
        .find(|label| line.trim_start().starts_with(&format!("{label}:")))
}

pub(super) fn quota_watch_profile_fields(
    updated: &str,
    snapshot: &ProviderQuotaSnapshot,
) -> Vec<(String, String)> {
    match snapshot {
        ProviderQuotaSnapshot::OpenAi(usage) => {
            vec![
                (
                    "Account".to_string(),
                    quota_watch_optional(usage.email.as_deref()).to_string(),
                ),
                (
                    "Plan".to_string(),
                    quota_watch_optional(usage.plan_type.as_deref()).to_string(),
                ),
                ("Status".to_string(), format_openai_quota_status(usage)),
                ("Main".to_string(), format_main_windows(usage)),
                (
                    "Reset".to_string(),
                    quota_watch_openai_reset_summary(usage)
                        .trim_start_matches("resets: ")
                        .to_string(),
                ),
                ("Last Updated".to_string(), updated.to_string()),
            ]
        }
        ProviderQuotaSnapshot::Copilot(info) => vec![
            (
                "Account".to_string(),
                quota_watch_optional(info.login.as_deref()).to_string(),
            ),
            (
                "Plan".to_string(),
                quota_watch_optional(
                    info.copilot_plan
                        .as_deref()
                        .or(info.access_type_sku.as_deref()),
                )
                .to_string(),
            ),
            ("Status".to_string(), format_copilot_quota_status(info)),
            ("Main".to_string(), format_copilot_main_quota(info)),
            (
                "Reset".to_string(),
                format_copilot_reset_summary(info).unwrap_or_else(|| "unavailable".to_string()),
            ),
            ("Last Updated".to_string(), updated.to_string()),
        ],
        ProviderQuotaSnapshot::Gemini(info) => vec![
            (
                "Account".to_string(),
                quota_watch_optional(info.email.as_deref()).to_string(),
            ),
            (
                "Plan".to_string(),
                quota_watch_optional(info.plan.as_deref()).to_string(),
            ),
            ("Status".to_string(), format_gemini_quota_status(info)),
            ("Main".to_string(), format_gemini_main_quota(info)),
            (
                "Reset".to_string(),
                format_gemini_reset_summary(info).unwrap_or_else(|| "unavailable".to_string()),
            ),
            ("Last Updated".to_string(), updated.to_string()),
        ],
        ProviderQuotaSnapshot::External(info) => vec![
            (
                "Account".to_string(),
                quota_watch_optional(info.account.as_deref()).to_string(),
            ),
            (
                "Plan".to_string(),
                quota_watch_optional(info.plan.as_deref()).to_string(),
            ),
            ("Status".to_string(), info.status.clone()),
            ("Main".to_string(), info.main.clone()),
            (
                "Reset".to_string(),
                info.reset
                    .clone()
                    .unwrap_or_else(|| "unavailable".to_string()),
            ),
            ("Last Updated".to_string(), updated.to_string()),
        ],
    }
}
