use super::{deserialize_null_default, quota_admission_value};
use prodex_mojo_core::quota::{
    QuotaIndexedRateLimitInput, QuotaUsageAdmissionAliases, QuotaUsageMainRateLimitSource,
    QuotaUsageMetadataSource, QuotaUsagePresenceAliases, QuotaUsageResponseInput,
    QuotaUsageStringAliases, QuotaUsageTextSelection, QuotaUsageTextSource,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize)]
pub struct UsageResponse {
    pub email: Option<String>,
    pub plan_type: Option<String>,
    pub rate_limit: Option<WindowPair>,
    pub code_review_rate_limit: Option<WindowPair>,
    #[serde(default, alias = "rateLimitResetCredits")]
    pub rate_limit_reset_credits: Option<RateLimitResetCreditsSummary>,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub additional_rate_limits: Vec<AdditionalRateLimit>,
}

impl<'de> Deserialize<'de> for UsageResponse {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let mut raw = RawUsageResponse::deserialize(deserializer)?;
        let indexed_rate_limits = raw.rate_limits_by_limit_id.unwrap_or_default();
        let indexed_inputs = indexed_rate_limits
            .iter()
            .map(|(map_key, pair)| QuotaIndexedRateLimitInput {
                map_key,
                limit_id: usage_text_aliases(&pair.extra, "limit_id", "limitId"),
                plan_type: usage_text_aliases(&pair.extra, "plan_type", "planType"),
                limit_name: usage_text_aliases(&pair.extra, "limit_name", "limitName"),
                metered_feature: usage_text_aliases(
                    &pair.extra,
                    "metered_feature",
                    "meteredFeature",
                ),
            })
            .collect::<Vec<_>>();
        let existing_additional_limit_ids = raw
            .additional_rate_limits
            .iter()
            .map(|additional| additional.limit_id.as_deref())
            .collect::<Vec<_>>();
        let plan = prodex_mojo_core::quota::quota_usage_response_plan(QuotaUsageResponseInput {
            indexed_rate_limits: &indexed_inputs,
            existing_additional_limit_ids: &existing_additional_limit_ids,
            rate_limit: raw
                .rate_limit
                .as_ref()
                .map(|pair| usage_text_aliases(&pair.extra, "plan_type", "planType")),
            rate_limits: raw
                .rate_limits
                .as_ref()
                .map(|pair| usage_text_aliases(&pair.extra, "plan_type", "planType")),
            plan_type: raw.plan_type.as_deref(),
            ordinary_usage_allowed: QuotaUsageAdmissionAliases {
                snake_case: quota_admission_value(raw.extra.get("ordinary_usage_allowed")),
                camel_case: quota_admission_value(raw.extra.get("ordinaryUsageAllowed")),
            },
            rate_limit_reached_type: quota_admission_value(raw.rate_limit_reached_type.as_ref()),
            rate_limit_upsell: QuotaUsagePresenceAliases {
                snake_case: raw.extra.contains_key("rate_limit_upsell"),
                camel_case: raw.extra.contains_key("rateLimitUpsell"),
            },
            account_id: QuotaUsagePresenceAliases {
                snake_case: raw.extra.contains_key("account_id"),
                camel_case: raw.extra.contains_key("accountId"),
            },
        })
        .map_err(|error| {
            <D::Error as serde::de::Error>::custom(format!("quota normalization failed: {error:?}"))
        })?;
        let mut rate_limit = match plan.main_rate_limit {
            QuotaUsageMainRateLimitSource::None => None,
            QuotaUsageMainRateLimitSource::Indexed(index) => indexed_rate_limits
                .iter()
                .nth(index)
                .map(|(_, pair)| pair.clone()),
            QuotaUsageMainRateLimitSource::RateLimit => raw.rate_limit.take(),
            QuotaUsageMainRateLimitSource::RateLimits => raw.rate_limits.take(),
        };
        if let Some(pair) = rate_limit.as_mut() {
            if plan.force_main_rate_limit_denied {
                pair.allowed = Some(false);
            }
            if plan.preserve_ordinary_usage_allowed
                && let Some(value) = usage_metadata_value(
                    &raw.extra,
                    plan.ordinary_usage_allowed_source,
                    "ordinary_usage_allowed",
                    "ordinaryUsageAllowed",
                )
            {
                pair.extra
                    .insert("ordinaryUsageAllowed".to_string(), value.clone());
            }
            if let Some(value) = usage_metadata_value(
                &raw.extra,
                plan.rate_limit_upsell_source,
                "rate_limit_upsell",
                "rateLimitUpsell",
            ) {
                pair.extra
                    .insert("rateLimitUpsell".to_string(), value.clone());
            }
            if let Some(value) = usage_metadata_value(
                &raw.extra,
                plan.account_id_source,
                "account_id",
                "accountId",
            ) {
                pair.extra.insert("accountId".to_string(), value.clone());
            }
        }
        let plan_type = if plan.plan_type_from_input {
            raw.plan_type.take()
        } else if let Some(pair) = rate_limit.as_ref() {
            selected_usage_extra_text(
                &pair.extra,
                plan.plan_type_source,
                plan.plan_type_start,
                plan.plan_type_end,
                "plan_type",
                "planType",
            )
            .map_err(<D::Error as serde::de::Error>::custom)?
        } else {
            None
        };
        let mut additional_rate_limits = std::mem::take(&mut raw.additional_rate_limits);
        for ((map_key, pair), item_plan) in indexed_rate_limits
            .into_iter()
            .zip(plan.indexed_rate_limits)
        {
            if !item_plan.include_as_additional {
                continue;
            }
            additional_rate_limits.push(
                additional_rate_limit_from_indexed_pair(
                    map_key,
                    pair,
                    item_plan.limit_id,
                    item_plan.limit_name,
                    item_plan.metered_feature,
                )
                .map_err(<D::Error as serde::de::Error>::custom)?,
            );
        }

        Ok(Self {
            email: raw.email,
            plan_type,
            rate_limit,
            code_review_rate_limit: raw.code_review_rate_limit,
            rate_limit_reset_credits: raw.rate_limit_reset_credits,
            additional_rate_limits,
        })
    }
}

#[derive(Debug, Deserialize)]
struct RawUsageResponse {
    #[serde(default)]
    email: Option<String>,
    #[serde(default, alias = "planType")]
    plan_type: Option<String>,
    #[serde(default)]
    rate_limit: Option<WindowPair>,
    #[serde(default, rename = "rateLimits", alias = "rate_limits")]
    rate_limits: Option<WindowPair>,
    #[serde(default, alias = "codeReviewRateLimit")]
    code_review_rate_limit: Option<WindowPair>,
    #[serde(default, alias = "rateLimitResetCredits")]
    rate_limit_reset_credits: Option<RateLimitResetCreditsSummary>,
    #[serde(
        default,
        alias = "additionalRateLimits",
        deserialize_with = "deserialize_null_default"
    )]
    additional_rate_limits: Vec<AdditionalRateLimit>,
    #[serde(default, alias = "rateLimitsByLimitId")]
    rate_limits_by_limit_id: Option<BTreeMap<String, WindowPair>>,
    #[serde(default, alias = "rateLimitReachedType")]
    rate_limit_reached_type: Option<serde_json::Value>,
    #[serde(flatten)]
    extra: BTreeMap<String, serde_json::Value>,
}

fn additional_rate_limit_from_indexed_pair(
    map_key: String,
    pair: WindowPair,
    limit_id: QuotaUsageTextSelection,
    limit_name: QuotaUsageTextSelection,
    metered_feature: QuotaUsageTextSelection,
) -> std::result::Result<AdditionalRateLimit, &'static str> {
    let limit_id = selected_usage_text(
        limit_id,
        &map_key,
        usage_text_aliases(&pair.extra, "limit_id", "limitId"),
    )?;
    let limit_name = selected_usage_text(
        limit_name,
        &map_key,
        usage_text_aliases(&pair.extra, "limit_name", "limitName"),
    )?;
    let metered_feature = selected_usage_text(
        metered_feature,
        &map_key,
        usage_text_aliases(&pair.extra, "metered_feature", "meteredFeature"),
    )?;
    let extra = pair.extra.clone();
    Ok(AdditionalRateLimit {
        limit_id,
        limit_name,
        metered_feature,
        allowed: pair.allowed,
        limit_reached: pair.limit_reached,
        rate_limit: pair,
        extra,
    })
}

fn usage_text_aliases<'a>(
    extra: &'a BTreeMap<String, serde_json::Value>,
    snake_case: &str,
    camel_case: &str,
) -> QuotaUsageStringAliases<'a> {
    QuotaUsageStringAliases {
        snake_case: extra.get(snake_case).and_then(serde_json::Value::as_str),
        camel_case: extra.get(camel_case).and_then(serde_json::Value::as_str),
    }
}

fn usage_metadata_value<'a>(
    extra: &'a BTreeMap<String, serde_json::Value>,
    source: QuotaUsageMetadataSource,
    snake_case: &str,
    camel_case: &str,
) -> Option<&'a serde_json::Value> {
    match source {
        QuotaUsageMetadataSource::None => None,
        QuotaUsageMetadataSource::SnakeCase => extra.get(snake_case),
        QuotaUsageMetadataSource::CamelCase => extra.get(camel_case),
    }
}

fn selected_usage_text(
    selection: QuotaUsageTextSelection,
    map_key: &str,
    aliases: QuotaUsageStringAliases<'_>,
) -> std::result::Result<Option<String>, &'static str> {
    let value = match selection.source {
        QuotaUsageTextSource::None => return Ok(None),
        QuotaUsageTextSource::MapKey => map_key,
        QuotaUsageTextSource::SnakeCase => aliases
            .snake_case
            .ok_or("selected snake_case quota field is not text")?,
        QuotaUsageTextSource::CamelCase => aliases
            .camel_case
            .ok_or("selected camelCase quota field is not text")?,
    };
    value
        .get(selection.start..selection.end)
        .map(|value| Some(value.to_owned()))
        .ok_or("Mojo quota normalization returned an invalid UTF-8 range")
}

fn selected_usage_extra_text(
    extra: &BTreeMap<String, serde_json::Value>,
    source: QuotaUsageMetadataSource,
    start: usize,
    end: usize,
    snake_case: &str,
    camel_case: &str,
) -> std::result::Result<Option<String>, &'static str> {
    let Some(value) = usage_metadata_value(extra, source, snake_case, camel_case) else {
        return Ok(None);
    };
    let value = value
        .as_str()
        .ok_or("selected quota plan field is not text")?;
    value
        .get(start..end)
        .map(|value| Some(value.to_owned()))
        .ok_or("Mojo quota normalization returned an invalid UTF-8 range")
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RateLimitResetCreditsSummary {
    #[serde(rename = "availableCount", alias = "available_count")]
    pub available_count: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WindowPair {
    /// Explicit backend admission state. `None` means unavailable, not denied.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed: Option<bool>,
    #[serde(
        default,
        alias = "limitReached",
        skip_serializing_if = "Option::is_none"
    )]
    pub limit_reached: Option<bool>,
    #[serde(default, alias = "primary", alias = "primaryWindow")]
    pub primary_window: Option<UsageWindow>,
    #[serde(default, alias = "secondary", alias = "secondaryWindow")]
    pub secondary_window: Option<UsageWindow>,
    /// Preserve future fields from the provider's rate-limit object.
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdditionalRateLimit {
    /// Optional backend bucket identifier. Unknown identifiers remain generic and non-routable.
    #[serde(default, alias = "limitId", skip_serializing_if = "Option::is_none")]
    pub limit_id: Option<String>,
    #[serde(alias = "limitName")]
    pub limit_name: Option<String>,
    #[serde(alias = "meteredFeature")]
    pub metered_feature: Option<String>,
    #[serde(
        default,
        alias = "rateLimit",
        deserialize_with = "deserialize_null_default"
    )]
    pub rate_limit: WindowPair,
    /// Exact backend admission state when the usage endpoint provides it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(alias = "limitReached")]
    pub limit_reached: Option<bool>,
    /// Retain fields introduced by a newer usage endpoint; unknown fields are not routing authority.
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize)]
pub struct UsageWindow {
    pub used_percent: Option<i64>,
    pub reset_at: Option<i64>,
    pub limit_window_seconds: Option<i64>,
}

impl<'de> Deserialize<'de> for UsageWindow {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = RawUsageWindow::deserialize(deserializer)?;
        Ok(Self {
            used_percent: raw.used_percent,
            reset_at: raw.reset_at,
            limit_window_seconds: raw.limit_window_seconds.or_else(|| {
                raw.window_duration_mins
                    .and_then(|minutes| minutes.checked_mul(60))
            }),
        })
    }
}

#[derive(Debug, Deserialize)]
struct RawUsageWindow {
    #[serde(default, alias = "usedPercent")]
    used_percent: Option<i64>,
    #[serde(default, alias = "resetAt", alias = "resetsAt")]
    reset_at: Option<i64>,
    #[serde(default, alias = "limitWindowSeconds")]
    limit_window_seconds: Option<i64>,
    #[serde(default, alias = "windowDurationMins")]
    window_duration_mins: Option<i64>,
}
