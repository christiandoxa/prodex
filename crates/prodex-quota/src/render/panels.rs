use super::*;
use prodex_mojo_core::json::{
    QuotaPanelExtra, QuotaPanelInput, QuotaPanelKind, quota_profile_panel_json,
};

#[cfg(test)]
#[path = "../../tests/src/render/profile_panels.rs"]
mod tests;

pub(super) fn display_optional(value: Option<&str>) -> &str {
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
    render_openai_profile_panel(profile_name, usage, false, total_width)
}

pub fn render_profile_quota_snapshot(
    profile_name: &str,
    snapshot: &ProviderQuotaSnapshot,
) -> String {
    match snapshot {
        ProviderQuotaSnapshot::OpenAi(usage) => render_profile_quota(profile_name, usage),
        ProviderQuotaSnapshot::Copilot(info) => render_profile_copilot_quota(profile_name, info),
        ProviderQuotaSnapshot::Gemini(info) => render_profile_gemini_quota(profile_name, info),
        ProviderQuotaSnapshot::External(info) => render_profile_external_quota(profile_name, info),
    }
}

pub fn render_profile_quota_snapshot_with_detail(
    profile_name: &str,
    snapshot: &ProviderQuotaSnapshot,
    detail: bool,
) -> String {
    match snapshot {
        ProviderQuotaSnapshot::OpenAi(usage) => {
            render_profile_quota_with_detail(profile_name, usage, detail)
        }
        _ => render_profile_quota_snapshot(profile_name, snapshot),
    }
}

fn render_profile_quota_with_detail(
    profile_name: &str,
    usage: &UsageResponse,
    detail: bool,
) -> String {
    render_openai_profile_panel(profile_name, usage, detail, current_cli_width())
}

fn render_openai_profile_panel(
    profile_name: &str,
    usage: &UsageResponse,
    detail: bool,
    total_width: usize,
) -> String {
    let observations = usage
        .additional_rate_limits
        .iter()
        .flat_map(|additional| {
            [
                additional.rate_limit.primary_window.as_ref(),
                additional.rate_limit.secondary_window.as_ref(),
            ]
            .into_iter()
            .flatten()
            .map(move |window| {
                (
                    additional,
                    window_label(window.limit_window_seconds),
                    format_window_details(window),
                )
            })
        })
        .collect::<Vec<_>>();
    let extras = observations
        .iter()
        .map(|(additional, label, value)| QuotaPanelExtra {
            names: [
                additional.limit_name.as_deref(),
                additional.metered_feature.as_deref(),
                additional.limit_id.as_deref(),
            ],
            window_label: label,
            value,
        })
        .collect::<Vec<_>>();
    let code_review = usage
        .code_review_rate_limit
        .as_ref()
        .map(format_window_pair);
    let credits = usage
        .rate_limit_reset_credits
        .as_ref()
        .map(format_reset_credits);
    render_compiled_profile_panel(
        QuotaPanelKind::OpenAi,
        QuotaPanelInput {
            profile: profile_name,
            account: usage.email.as_deref(),
            plan: usage.plan_type.as_deref(),
            status: &format_openai_quota_status(usage),
            main: &format_main_windows(usage),
            reset: Some(&format_main_reset_summary(usage)),
            code_review: code_review.as_deref(),
            reset_credits: credits.as_deref(),
            extras: &extras,
            ..Default::default()
        },
        detail,
        total_width,
    )
}

pub fn render_profile_copilot_quota(profile_name: &str, info: &CopilotQuotaInfo) -> String {
    render_profile_copilot_quota_with_width(profile_name, info, current_cli_width())
}

pub fn render_profile_copilot_quota_with_width(
    profile_name: &str,
    info: &CopilotQuotaInfo,
    total_width: usize,
) -> String {
    let reset = format_copilot_reset_summary(info);
    render_compiled_profile_panel(
        QuotaPanelKind::Copilot,
        QuotaPanelInput {
            profile: profile_name,
            account: info.login.as_deref(),
            plan: info.copilot_plan.as_deref(),
            access: info.access_type_sku.as_deref(),
            status: &format_copilot_quota_status(info),
            main: &format_copilot_main_quota(info),
            reset: reset.as_deref(),
            ..Default::default()
        },
        false,
        total_width,
    )
}

pub fn render_profile_gemini_quota(profile_name: &str, info: &GeminiQuotaInfo) -> String {
    render_profile_gemini_quota_with_width(profile_name, info, current_cli_width())
}

pub fn render_profile_gemini_quota_with_width(
    profile_name: &str,
    info: &GeminiQuotaInfo,
    total_width: usize,
) -> String {
    let reset = format_gemini_reset_summary(info);
    let buckets = format_gemini_bucket_summaries(info);
    let extras = buckets
        .iter()
        .map(|value| QuotaPanelExtra {
            value,
            ..Default::default()
        })
        .collect::<Vec<_>>();
    render_compiled_profile_panel(
        QuotaPanelKind::Gemini,
        QuotaPanelInput {
            profile: profile_name,
            account: info.email.as_deref(),
            plan: info.plan.as_deref(),
            project: info.project_id.as_deref(),
            status: &format_gemini_quota_status(info),
            main: &format_gemini_main_quota(info),
            reset: reset.as_deref(),
            extras: &extras,
            ..Default::default()
        },
        false,
        total_width,
    )
}

pub fn render_profile_external_quota(profile_name: &str, info: &ExternalQuotaInfo) -> String {
    render_profile_external_quota_with_width(profile_name, info, current_cli_width())
}

pub fn render_profile_external_quota_with_width(
    profile_name: &str,
    info: &ExternalQuotaInfo,
    total_width: usize,
) -> String {
    let extras = info
        .details
        .iter()
        .map(|detail| QuotaPanelExtra {
            names: [Some(detail.label.as_str()), None, None],
            value: &detail.value,
            ..Default::default()
        })
        .collect::<Vec<_>>();
    render_compiled_profile_panel(
        QuotaPanelKind::External,
        QuotaPanelInput {
            profile: profile_name,
            provider: Some(&info.provider),
            account: info.account.as_deref(),
            plan: info.plan.as_deref(),
            status: &info.status,
            main: &info.main,
            reset: info.reset.as_deref(),
            extras: &extras,
            ..Default::default()
        },
        false,
        total_width,
    )
}

fn render_compiled_profile_panel(
    kind: QuotaPanelKind,
    input: QuotaPanelInput<'_>,
    detail: bool,
    total_width: usize,
) -> String {
    let json =
        quota_profile_panel_json(kind, input, detail).expect("Mojo quota panel schema failed");
    let (title, fields): (String, Vec<(String, String)>) =
        serde_json::from_str(&json).expect("Mojo quota panel schema returned invalid rows");
    render_panel_with_width(&title, &fields, total_width)
}

pub fn render_quota_error_panel_with_width(
    title: &str,
    message: &str,
    total_width: usize,
) -> String {
    render_panel_with_width(
        title,
        &[("Error".to_string(), first_line_of_error(message))],
        total_width,
    )
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

pub fn first_line_of_error(input: &str) -> String {
    let line = prodex_mojo_core::quota::quota_panel_first_line(input)
        .expect("Mojo quota panel first-line normalization failed");
    if line.is_empty() {
        "-".to_string()
    } else {
        line
    }
}
