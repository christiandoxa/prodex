use super::*;

fn copilot_quota_feature_labels() -> [(&'static str, &'static str); 2] {
    [("chat", "chat"), ("completions", "comp")]
}

fn copilot_remaining_quota(info: &CopilotQuotaInfo, feature: &str) -> Option<i64> {
    info.limited_user_quotas
        .get(feature)
        .copied()
        .or_else(|| info.monthly_quotas.get(feature).copied())
}

fn copilot_total_quota(info: &CopilotQuotaInfo, feature: &str) -> Option<i64> {
    info.monthly_quotas.get(feature).copied()
}

fn copilot_blocked_features(info: &CopilotQuotaInfo) -> Vec<String> {
    [("chat", "chat"), ("completions", "completions")]
        .into_iter()
        .filter_map(|(feature, label)| {
            (copilot_remaining_quota(info, feature).unwrap_or(1) <= 0).then_some(label.to_string())
        })
        .collect()
}

pub fn copilot_quota_is_ready(info: &CopilotQuotaInfo) -> bool {
    copilot_blocked_features(info).is_empty()
}

pub fn format_copilot_quota_status(info: &CopilotQuotaInfo) -> String {
    let blocked = copilot_blocked_features(info);
    if blocked.is_empty() {
        "Ready".to_string()
    } else {
        "Blocked".to_string()
    }
}

pub fn format_copilot_main_quota(info: &CopilotQuotaInfo) -> String {
    let parts = copilot_quota_feature_labels()
        .into_iter()
        .filter_map(|(feature, label)| {
            let remaining = copilot_remaining_quota(info, feature)?;
            Some(match copilot_total_quota(info, feature) {
                Some(total) => format!("{label} {remaining}/{total}"),
                None => format!("{label} {remaining}"),
            })
        })
        .collect::<Vec<_>>();

    if parts.is_empty() {
        "-".to_string()
    } else {
        parts.join(" | ")
    }
}

pub(super) fn copilot_reset_epoch(info: &CopilotQuotaInfo) -> Option<i64> {
    let date =
        chrono::NaiveDate::parse_from_str(info.limited_user_reset_date.as_deref()?, "%Y-%m-%d")
            .ok()?;
    let datetime = date.and_hms_opt(0, 0, 0)?;
    Local
        .from_local_datetime(&datetime)
        .earliest()
        .map(|value| value.timestamp())
}

pub fn format_copilot_reset_summary(info: &CopilotQuotaInfo) -> Option<String> {
    Some(format!(
        "monthly {}",
        info.limited_user_reset_date.as_deref()?.trim()
    ))
}
