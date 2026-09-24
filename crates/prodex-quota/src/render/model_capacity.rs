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
pub const OPENAI_LUNA_RESERVE_MODEL: &str = "gpt-reserve";

pub fn openai_model_is_luna(model: Option<&str>) -> bool {
    #[cfg(feature = "mojo")]
    {
        crate::mojo::openai_model_kind(model) == prodex_mojo_core::quota::QUOTA_MODEL_KIND_LUNA
    }

    #[cfg(not(feature = "mojo"))]
    {
        model.is_some_and(|model| {
            matches!(normalized_identifier(model).as_str(), "luna" | "gpt56luna")
        })
    }
}

pub fn openai_model_is_retired_spark(model: Option<&str>) -> bool {
    #[cfg(feature = "mojo")]
    {
        crate::mojo::openai_model_kind(model)
            == prodex_mojo_core::quota::QUOTA_MODEL_KIND_RETIRED_SPARK
    }

    #[cfg(not(feature = "mojo"))]
    {
        model.is_some_and(|model| {
            matches!(
                normalized_identifier(model).as_str(),
                "spark" | "gpt53codexspark" | "gpt53spark"
            )
        })
    }
}

pub fn openai_quota_runtime_window_pair_for_model<'a>(
    usage: &'a UsageResponse,
    model: Option<&str>,
) -> Option<&'a WindowPair> {
    #[cfg(feature = "mojo")]
    {
        if model.is_none() {
            return openai_quota_runtime_window_pair(usage);
        }
        let plan = openai_model_capacity_plan_for_usage(usage, false, model);
        match plan.selected_pair {
            prodex_mojo_core::quota::QUOTA_MODEL_PAIR_REGULAR => usage.rate_limit.as_ref(),
            prodex_mojo_core::quota::QUOTA_MODEL_PAIR_RESERVE => {
                ready_luna_reserve(usage).map(|reserve| &reserve.rate_limit)
            }
            prodex_mojo_core::quota::QUOTA_MODEL_PAIR_NONE => None,
            prodex_mojo_core::quota::QUOTA_MODEL_PAIR_DEFAULT => {
                openai_quota_runtime_window_pair(usage)
            }
            _ => unreachable!("validated Mojo model-capacity pair"),
        }
    }

    #[cfg(not(feature = "mojo"))]
    {
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
            && openai_usage_advertises_luna_reserve(usage)
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
}

pub fn openai_quota_has_ready_limit_for_model(usage: &UsageResponse, model: Option<&str>) -> bool {
    #[cfg(feature = "mojo")]
    {
        openai_model_capacity_plan_for_usage(usage, false, model).ready
    }

    #[cfg(not(feature = "mojo"))]
    {
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
}

pub fn openai_quota_has_ready_luna_reserve(usage: &UsageResponse) -> bool {
    ready_luna_reserve(usage).is_some()
}

pub fn openai_quota_has_ready_regular_limit(usage: &UsageResponse) -> bool {
    usage
        .rate_limit
        .as_ref()
        .is_some_and(window_pair_has_ready_limit)
}

pub fn additional_rate_limit_is_luna_reserve(additional: &super::AdditionalRateLimit) -> bool {
    #[cfg(feature = "mojo")]
    {
        crate::mojo::luna_reserve_identifier(
            additional_rate_limit_model_slug(additional),
            additional.limit_id.as_deref(),
            additional.limit_name.as_deref(),
            additional.metered_feature.as_deref(),
        )
    }

    #[cfg(not(feature = "mojo"))]
    {
        if additional_rate_limit_model_slug(additional) != Some(OPENAI_LUNA_MODEL) {
            return false;
        }
        [
            additional.limit_id.as_deref(),
            additional.limit_name.as_deref(),
            additional.metered_feature.as_deref(),
        ]
        .into_iter()
        .flatten()
        .any(|value| {
            value.eq_ignore_ascii_case(OPENAI_LUNA_RESERVE_MODEL)
                || is_luna_reserve_identifier(value)
        })
    }
}

pub fn openai_effective_model_for_usage(
    usage: &UsageResponse,
    requested_model: Option<&str>,
    authenticated_account_id: Option<&str>,
    fetched_for_authenticated_account: bool,
) -> Option<&'static str> {
    if requested_model != Some(OPENAI_LUNA_MODEL)
        || usage.rate_limit.as_ref()?.allowed != Some(false)
        || !openai_usage_account_matches(
            usage,
            authenticated_account_id,
            fetched_for_authenticated_account,
        )
        || !openai_quota_has_ready_luna_reserve(usage)
    {
        return None;
    }
    Some(OPENAI_LUNA_RESERVE_MODEL)
}

fn ready_luna_reserve(usage: &UsageResponse) -> Option<&super::AdditionalRateLimit> {
    if !openai_usage_advertises_luna_reserve(usage) {
        return None;
    }
    usage.additional_rate_limits.iter().find(|additional| {
        additional_rate_limit_is_luna_reserve(additional)
            && super::additional_rate_limit_is_usable(additional)
            && window_pair_has_ready_limit(&additional.rate_limit)
    })
}

fn openai_usage_advertises_luna_reserve(usage: &UsageResponse) -> bool {
    if usage
        .additional_rate_limits
        .iter()
        .any(additional_rate_limit_is_luna_reserve)
    {
        return true;
    }

    usage
        .rate_limit
        .as_ref()
        .and_then(|pair| {
            pair.extra
                .get("rateLimitUpsell")
                .or_else(|| pair.extra.get("rate_limit_upsell"))
        })
        .and_then(serde_json::Value::as_object)
        .and_then(|upsell| {
            upsell
                .get("banner_type")
                .or_else(|| upsell.get("bannerType"))
        })
        .and_then(serde_json::Value::as_str)
        .is_some_and(|banner| banner.eq_ignore_ascii_case("luna_reserve"))
}

fn openai_usage_account_matches(
    usage: &UsageResponse,
    authenticated_account_id: Option<&str>,
    fetched_for_authenticated_account: bool,
) -> bool {
    let authenticated_account_id = authenticated_account_id.filter(|id| !id.is_empty());
    let response_account_id = usage.rate_limit.as_ref().and_then(|pair| {
        ["accountId", "account_id"].into_iter().find_map(|key| {
            pair.extra
                .get(key)
                .and_then(serde_json::Value::as_str)
                .filter(|id| !id.is_empty())
        })
    });
    match response_account_id {
        Some(response_account_id) => authenticated_account_id == Some(response_account_id),
        None => fetched_for_authenticated_account && authenticated_account_id.is_some(),
    }
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
    #[cfg(feature = "mojo")]
    {
        openai_model_capacity_plan_for_usage(usage, false, Some(OPENAI_LUNA_MODEL))
            .unknown_luna_capacity
    }

    #[cfg(not(feature = "mojo"))]
    {
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
}

pub fn openai_usage_supports_model(
    usage: &UsageResponse,
    include_code_review: bool,
    model: Option<&str>,
) -> bool {
    #[cfg(feature = "mojo")]
    {
        openai_model_capacity_plan_for_usage(usage, include_code_review, model).supports
    }

    #[cfg(not(feature = "mojo"))]
    {
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
}

#[cfg(feature = "mojo")]
fn openai_model_capacity_plan_for_usage(
    usage: &UsageResponse,
    include_code_review: bool,
    model: Option<&str>,
) -> prodex_mojo_core::quota::OpenAiModelCapacityPlan {
    let regular = usage.rate_limit.as_ref();
    let regular_ready = openai_quota_has_ready_regular_limit(usage);
    let (any_unknown_window, any_exhausted_window) = regular.map_or((false, false), |pair| {
        let windows = [pair.primary_window.as_ref(), pair.secondary_window.as_ref()];
        (
            windows
                .into_iter()
                .flatten()
                .any(|window| window.used_percent.is_none()),
            windows
                .into_iter()
                .flatten()
                .any(|window| window.used_percent.is_some_and(|used| used >= 100)),
        )
    });
    let code_review_ready = usage.code_review_rate_limit.as_ref().is_none_or(|pair| {
        [
            find_main_window(pair, "5h"),
            find_main_window(pair, "weekly"),
        ]
        .into_iter()
        .flatten()
        .all(|window| window.used_percent.is_none_or(|used| used < 100))
    });

    crate::mojo::openai_model_capacity_plan(prodex_mojo_core::quota::OpenAiModelCapacityInput {
        model_kind: crate::mojo::openai_model_kind(model),
        regular_present: regular.is_some(),
        regular_ready,
        generic_ready: openai_quota_has_ready_limit(usage),
        reserve_ready: ready_luna_reserve(usage).is_some(),
        regular_blocked: regular.is_some_and(super::windows::window_pair_has_blocking_admission),
        any_unknown_window,
        any_exhausted_window,
        include_code_review,
        code_review_ready,
    })
}

#[cfg(not(feature = "mojo"))]
fn is_luna_reserve_identifier(value: &str) -> bool {
    let normalized = normalized_identifier(value);
    normalized.contains("luna") && normalized.contains("reserve")
}

#[cfg(not(feature = "mojo"))]
pub(crate) fn normalized_identifier(value: &str) -> String {
    value
        .trim()
        .to_ascii_lowercase()
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .collect()
}
