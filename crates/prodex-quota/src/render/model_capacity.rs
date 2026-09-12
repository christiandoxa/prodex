use super::windows::required_window_snapshot_at;
use super::{
    MainWindowSnapshot, UsageResponse, WindowPair, find_main_window, openai_quota_has_ready_limit,
    openai_quota_runtime_window_pair, window_pair_has_ready_limit,
};

pub fn required_window_snapshot_for_pair_at(
    pair: &WindowPair,
    label: &str,
    now: i64,
) -> Option<MainWindowSnapshot> {
    required_window_snapshot_at(pair, label, now)
}

pub const OPENAI_LUNA_MODEL: &str = "gpt-5.6-luna";

pub fn openai_model_is_luna(model: Option<&str>) -> bool {
    model.is_some_and(|model| matches!(normalized_identifier(model).as_str(), "luna" | "gpt56luna"))
}

pub fn openai_model_is_retired_spark(model: Option<&str>) -> bool {
    model.is_some_and(|model| {
        matches!(
            normalized_identifier(model).as_str(),
            "spark" | "gpt53codexspark" | "gpt53spark"
        )
    })
}

pub fn openai_quota_runtime_window_pair_for_model<'a>(
    usage: &'a UsageResponse,
    model: Option<&str>,
) -> Option<&'a WindowPair> {
    if openai_model_is_retired_spark(model) {
        return None;
    }
    if openai_model_is_luna(model)
        && usage
            .rate_limit
            .as_ref()
            .is_some_and(window_pair_has_ready_limit)
    {
        return usage.rate_limit.as_ref();
    }
    if openai_model_is_luna(model)
        && !openai_quota_has_ready_regular_limit(usage)
        && let Some(reserve) = usage
            .additional_rate_limits
            .iter()
            .filter(|additional| additional_rate_limit_is_luna_reserve(additional))
            .find(|additional| super::additional_rate_limit_is_usable(additional))
    {
        return Some(&reserve.rate_limit);
    }
    if model.is_some() {
        return usage.rate_limit.as_ref();
    }
    openai_quota_runtime_window_pair(usage)
}

pub fn openai_quota_has_ready_limit_for_model(usage: &UsageResponse, model: Option<&str>) -> bool {
    if openai_model_is_retired_spark(model) {
        return false;
    }
    if openai_model_is_luna(model)
        && (openai_quota_has_ready_regular_limit(usage)
            || openai_quota_has_ready_luna_reserve(usage))
    {
        return true;
    }
    if model.is_some() {
        return openai_quota_has_ready_regular_limit(usage);
    }
    openai_quota_has_ready_limit(usage)
}

pub fn openai_quota_has_ready_luna_reserve(usage: &UsageResponse) -> bool {
    usage.additional_rate_limits.iter().any(|additional| {
        additional_rate_limit_is_luna_reserve(additional)
            && super::additional_rate_limit_is_usable(additional)
            && window_pair_has_ready_limit(&additional.rate_limit)
    })
}

pub fn openai_quota_has_ready_regular_limit(usage: &UsageResponse) -> bool {
    usage
        .rate_limit
        .as_ref()
        .is_some_and(window_pair_has_ready_limit)
}

pub fn additional_rate_limit_is_luna_reserve(additional: &super::AdditionalRateLimit) -> bool {
    if let Some(model) = additional_rate_limit_model_slug(additional)
        && !openai_model_is_luna(Some(model))
    {
        return false;
    }
    [
        additional.limit_id.as_deref(),
        additional.limit_name.as_deref(),
        additional.metered_feature.as_deref(),
    ]
    .into_iter()
    .flatten()
    .any(is_luna_reserve_identifier)
}

pub(crate) fn additional_rate_limit_model_slug(
    additional: &super::AdditionalRateLimit,
) -> Option<&str> {
    ["normal_model_slug", "normalModelSlug"]
        .into_iter()
        .find_map(|key| {
            additional
                .extra
                .get(key)
                .or_else(|| additional.rate_limit.extra.get(key))
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
        })
}

pub fn openai_usage_has_unknown_luna_capacity(usage: &UsageResponse) -> bool {
    let Some(pair) = usage.rate_limit.as_ref() else {
        return false;
    };
    if pair.allowed == Some(false)
        || pair.limit_reached == Some(true)
        || !["rate_limit_reached_type", "rateLimitReachedType"]
            .into_iter()
            .all(|key| pair.extra.get(key).is_none_or(serde_json::Value::is_null))
        || ["spend_control_reached", "spendControlReached"]
            .into_iter()
            .any(|key| pair.extra.get(key).and_then(serde_json::Value::as_bool) == Some(true))
        || window_pair_has_ready_limit(pair)
    {
        return false;
    }
    let windows = [pair.primary_window.as_ref(), pair.secondary_window.as_ref()];
    windows
        .into_iter()
        .flatten()
        .any(|window| window.used_percent.is_none())
        && !windows
            .into_iter()
            .flatten()
            .any(|window| window.used_percent.is_some_and(|used| used >= 100))
}

pub fn openai_usage_supports_model(
    usage: &UsageResponse,
    include_code_review: bool,
    model: Option<&str>,
) -> bool {
    (if model.is_none() {
        openai_quota_has_ready_limit(usage)
    } else {
        openai_quota_has_ready_limit_for_model(usage, model)
            || (openai_model_is_luna(model) && openai_usage_has_unknown_luna_capacity(usage))
    }) && (!include_code_review
        || usage.code_review_rate_limit.as_ref().is_none_or(|pair| {
            [
                find_main_window(pair, "5h"),
                find_main_window(pair, "weekly"),
            ]
            .into_iter()
            .flatten()
            .all(|window| window.used_percent.is_none_or(|used| used < 100))
        }))
}

fn is_luna_reserve_identifier(value: &str) -> bool {
    let normalized = normalized_identifier(value);
    normalized.contains("luna") && normalized.contains("reserve")
}

pub(crate) fn normalized_identifier(value: &str) -> String {
    value
        .trim()
        .to_ascii_lowercase()
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .collect()
}
