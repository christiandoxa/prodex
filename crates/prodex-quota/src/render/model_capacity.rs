use super::windows::required_window_snapshot_at;
use super::{
    MainWindowSnapshot, UsageResponse, WindowPair, collect_blocked_limits, find_main_window,
    openai_quota_has_ready_limit, openai_quota_runtime_window_pair, window_pair_has_ready_limit,
};

pub fn required_window_snapshot_for_pair_at(
    pair: &WindowPair,
    label: &str,
    now: i64,
) -> Option<MainWindowSnapshot> {
    required_window_snapshot_at(pair, label, now)
}

pub const OPENAI_LUNA_MODEL: &str = "gpt-5.6-luna";
pub const OPENAI_SPARK_MODEL: &str = "gpt-5.3-codex-spark";

pub fn openai_luna_spark_fallback_model(
    requested_model: Option<&str>,
    effective_model: Option<&str>,
) -> Option<&'static str> {
    (openai_model_is_luna(requested_model) && !openai_model_is_spark(effective_model))
        .then_some(OPENAI_SPARK_MODEL)
}

pub fn openai_model_is_luna(model: Option<&str>) -> bool {
    matches!(
        model.map(str::trim).map(str::to_ascii_lowercase).as_deref(),
        Some("luna" | OPENAI_LUNA_MODEL)
    )
}

pub fn openai_model_is_spark(model: Option<&str>) -> bool {
    matches!(
        model.map(str::trim).map(str::to_ascii_lowercase).as_deref(),
        Some("spark" | OPENAI_SPARK_MODEL)
    )
}

pub fn openai_quota_runtime_window_pair_for_model<'a>(
    usage: &'a UsageResponse,
    model: Option<&str>,
) -> Option<&'a WindowPair> {
    if openai_model_is_spark(model) {
        return usage
            .additional_rate_limits
            .iter()
            .find(|additional| crate::capacity::additional_rate_limit_is_spark(additional))
            .map(|additional| &additional.rate_limit);
    }
    if model.is_some() {
        return usage.rate_limit.as_ref();
    }
    openai_quota_runtime_window_pair(usage)
}

pub fn openai_quota_has_ready_limit_for_model(usage: &UsageResponse, model: Option<&str>) -> bool {
    if openai_model_is_spark(model) {
        return usage
            .additional_rate_limits
            .iter()
            .find(|additional| crate::capacity::additional_rate_limit_is_spark(additional))
            .is_some_and(|additional| {
                super::additional_rate_limit_is_usable(additional)
                    && window_pair_has_ready_limit(&additional.rate_limit)
            });
    }
    if model.is_some() {
        return usage
            .rate_limit
            .as_ref()
            .is_some_and(window_pair_has_ready_limit);
    }
    openai_quota_has_ready_limit(usage)
}

pub fn openai_usage_has_unknown_luna_capacity(usage: &UsageResponse) -> bool {
    usage.rate_limit.as_ref().is_some_and(|pair| {
        pair.allowed != Some(false)
            && pair.limit_reached != Some(true)
            && !window_pair_has_ready_limit(pair)
    })
}

pub fn openai_usage_supports_model(
    usage: &UsageResponse,
    include_code_review: bool,
    model: Option<&str>,
) -> bool {
    if model.is_none() {
        return collect_blocked_limits(usage, include_code_review).is_empty();
    }
    (openai_quota_has_ready_limit_for_model(usage, model)
        || (openai_model_is_luna(model) && openai_usage_has_unknown_luna_capacity(usage)))
        && (!include_code_review
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
