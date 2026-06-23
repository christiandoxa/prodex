use super::*;

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
    if let Some(reset_credits) = usage.rate_limit_reset_credits.as_ref() {
        fields.push((
            "Reset credits".to_string(),
            format!("{} available", reset_credits.available_count),
        ));
    }

    fields.extend(format_additional_limits(usage));
    render_panel_with_width(&format!("Quota {profile_name}"), &fields, total_width)
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

pub fn render_profile_copilot_quota(profile_name: &str, info: &CopilotQuotaInfo) -> String {
    render_profile_copilot_quota_with_width(profile_name, info, current_cli_width())
}

pub fn render_profile_copilot_quota_with_width(
    profile_name: &str,
    info: &CopilotQuotaInfo,
    total_width: usize,
) -> String {
    let mut fields = vec![
        ("Profile".to_string(), profile_name.to_string()),
        (
            "Account".to_string(),
            display_optional(info.login.as_deref()).to_string(),
        ),
        (
            "Plan".to_string(),
            display_optional(
                info.copilot_plan
                    .as_deref()
                    .or(info.access_type_sku.as_deref()),
            )
            .to_string(),
        ),
    ];
    if let Some(access_type) = info.access_type_sku.as_deref()
        && info.copilot_plan.as_deref() != Some(access_type)
    {
        fields.push(("Access".to_string(), access_type.to_string()));
    }
    fields.push(("Status".to_string(), format_copilot_quota_status(info)));
    fields.push(("Main".to_string(), format_copilot_main_quota(info)));
    if let Some(reset) = format_copilot_reset_summary(info) {
        fields.push(("Reset".to_string(), reset));
    }
    render_panel_with_width(&format!("Quota {profile_name}"), &fields, total_width)
}

pub fn render_profile_gemini_quota(profile_name: &str, info: &GeminiQuotaInfo) -> String {
    render_profile_gemini_quota_with_width(profile_name, info, current_cli_width())
}

pub fn render_profile_gemini_quota_with_width(
    profile_name: &str,
    info: &GeminiQuotaInfo,
    total_width: usize,
) -> String {
    let mut fields = vec![
        ("Profile".to_string(), profile_name.to_string()),
        (
            "Account".to_string(),
            display_optional(info.email.as_deref()).to_string(),
        ),
        (
            "Plan".to_string(),
            display_optional(info.plan.as_deref()).to_string(),
        ),
        (
            "Project".to_string(),
            display_optional(info.project_id.as_deref()).to_string(),
        ),
        ("Status".to_string(), format_gemini_quota_status(info)),
        ("Main".to_string(), format_gemini_main_quota(info)),
    ];
    if let Some(reset) = format_gemini_reset_summary(info) {
        fields.push(("Reset".to_string(), reset));
    }
    for (index, bucket) in info.buckets.iter().enumerate() {
        fields.push((
            format!("Bucket {}", index + 1),
            format_gemini_bucket_summary(bucket),
        ));
    }
    render_panel_with_width(&format!("Quota {profile_name}"), &fields, total_width)
}

pub fn render_profile_external_quota(profile_name: &str, info: &ExternalQuotaInfo) -> String {
    render_profile_external_quota_with_width(profile_name, info, current_cli_width())
}

pub fn render_profile_external_quota_with_width(
    profile_name: &str,
    info: &ExternalQuotaInfo,
    total_width: usize,
) -> String {
    let mut fields = vec![
        ("Profile".to_string(), profile_name.to_string()),
        ("Provider".to_string(), info.provider.clone()),
        (
            "Account".to_string(),
            display_optional(info.account.as_deref()).to_string(),
        ),
        (
            "Plan".to_string(),
            display_optional(info.plan.as_deref()).to_string(),
        ),
        ("Status".to_string(), info.status.clone()),
        ("Main".to_string(), info.main.clone()),
    ];
    if let Some(reset) = info.reset.as_deref() {
        fields.push(("Reset".to_string(), reset.to_string()));
    }
    for detail in &info.details {
        fields.push((detail.label.clone(), detail.value.clone()));
    }
    render_panel_with_width(&format!("Quota {profile_name}"), &fields, total_width)
}

pub fn render_quota_error_panel(title: &str, message: &str) -> String {
    render_quota_error_panel_with_width(title, message, current_cli_width())
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
