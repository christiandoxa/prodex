use super::*;

fn copilot_feature_values(info: &CopilotQuotaInfo, index: usize) -> (Option<i64>, Option<i64>) {
    let feature = prodex_mojo_core::quota::quota_copilot_feature_key(index)
        .expect("Mojo Copilot quota feature key returned invalid output");
    let remaining = info
        .limited_user_quotas
        .get(feature)
        .copied()
        .or_else(|| info.monthly_quotas.get(feature).copied());
    let total = info.monthly_quotas.get(feature).copied();
    (remaining, total)
}

fn copilot_display(info: &CopilotQuotaInfo) -> prodex_mojo_core::quota::CopilotQuotaDisplay {
    let (chat_remaining, chat_total) = copilot_feature_values(info, 0);
    let (completions_remaining, completions_total) = copilot_feature_values(info, 1);
    prodex_mojo_core::quota::quota_copilot_display(
        chat_remaining,
        chat_total,
        completions_remaining,
        completions_total,
    )
    .expect("Mojo Copilot quota display policy returned invalid output")
}

pub(super) fn copilot_main_remaining_percent(info: &CopilotQuotaInfo) -> Option<i64> {
    let (chat_remaining, chat_total) = copilot_feature_values(info, 0);
    let (completions_remaining, completions_total) = copilot_feature_values(info, 1);
    prodex_mojo_core::quota::quota_copilot_main_remaining_percent(
        chat_remaining,
        chat_total,
        completions_remaining,
        completions_total,
    )
    .expect("Mojo Copilot main quota percent policy returned invalid output")
}

pub fn copilot_quota_is_ready(info: &CopilotQuotaInfo) -> bool {
    copilot_display(info).ready
}

pub fn format_copilot_quota_status(info: &CopilotQuotaInfo) -> String {
    copilot_display(info).status
}

pub fn format_copilot_main_quota(info: &CopilotQuotaInfo) -> String {
    copilot_display(info).main
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
