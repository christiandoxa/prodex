use anyhow::Result;
use serde::Deserialize;
use serde_json::{Value, json};

use super::DashboardServer;
use crate::{
    ProviderQuotaSnapshot, format_copilot_main_quota, format_copilot_quota_status,
    format_copilot_reset_summary, format_gemini_main_quota, format_gemini_quota_status,
    format_gemini_reset_summary, format_main_windows, runtime_proxy_latest_log_path_from_pointer,
    runtime_proxy_log_dir,
};

const DASHBOARD_RUNTIME_LOG_TAIL_BYTES: usize = 128 * 1024;
const DASHBOARD_RUNTIME_LOG_MAX_LINES: usize = 200;
const DASHBOARD_RUNTIME_LOG_MAX_LINE_CHARS: usize = 2_000;

#[derive(Debug, Deserialize)]
pub(super) struct ActiveProfileRequest {
    pub(super) profile: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct AddProfileRequest {
    pub(super) name: String,
    #[serde(default)]
    pub(super) activate: bool,
}

impl DashboardServer {
    pub(super) fn runtime_status_json(&self) -> Result<Value> {
        let log_dir = runtime_proxy_log_dir();
        let latest_pointer = log_dir.join(crate::RUNTIME_PROXY_LATEST_LOG_POINTER);
        let latest_log = runtime_proxy_latest_log_path_from_pointer();
        let latest_log_exists = latest_log.as_ref().is_some_and(|path| path.exists());

        Ok(json!({
            "runtime": {
                "status": if latest_log_exists { "log-available" } else { "not-running-or-no-log" },
                "logDir": log_dir.display().to_string(),
                "latestLogPointer": latest_pointer.display().to_string(),
                "latestLog": latest_log.map(|path| path.display().to_string()),
                "latestLogExists": latest_log_exists,
                "doctorCommand": "prodex doctor --runtime",
            },
            "gateway": {
                "status": "available-on-demand",
                "startCommand": "prodex gateway --provider <provider>",
                "providersCommand": "prodex gateway providers --json",
                "modelsCommand": "prodex gateway models --provider <provider> --json",
            }
        }))
    }

    pub(super) fn logs_json(&self) -> Result<Value> {
        let Some(path) = runtime_proxy_latest_log_path_from_pointer() else {
            return Ok(json!({ "path": null, "lines": [] }));
        };
        if !path.exists() {
            return Ok(json!({ "path": path.display().to_string(), "lines": [] }));
        }

        let tail =
            prodex_runtime_doctor::read_runtime_log_tail(&path, DASHBOARD_RUNTIME_LOG_TAIL_BYTES)?;
        let text = String::from_utf8_lossy(&tail);
        let mut lines = text
            .lines()
            .filter(|line| !line.trim().is_empty())
            .rev()
            .take(DASHBOARD_RUNTIME_LOG_MAX_LINES)
            .map(|line| {
                redaction::redaction_text_snippet(
                    &redaction::redaction_redact_secret_like_text(line),
                    DASHBOARD_RUNTIME_LOG_MAX_LINE_CHARS,
                )
            })
            .collect::<Vec<_>>();
        lines.reverse();
        Ok(json!({
            "path": path.display().to_string(),
            "lines": lines,
        }))
    }
}

pub(super) fn quota_summary(snapshot: &ProviderQuotaSnapshot) -> Value {
    match snapshot {
        ProviderQuotaSnapshot::OpenAi(usage) => {
            let blocked = crate::collect_blocked_limits(usage, false);
            json!({
                "account": usage.email,
                "plan": usage.plan_type,
                "status": if prodex_quota::openai_quota_has_ready_limit(usage) {
                    "Ready".to_string()
                } else {
                    format!("Blocked ({})", crate::format_blocked_limits(&blocked))
                },
                "main": format_main_windows(usage),
                "reset": prodex_quota::format_main_reset_summary(usage),
                "allowed": usage.rate_limit.as_ref().and_then(|rate| rate.allowed),
                "limitReached": usage.rate_limit.as_ref().and_then(|rate| rate.limit_reached),
                "rateLimitReachedType": (usage.rate_limit.as_ref().and_then(|rate| {
                    ["rateLimitReachedType", "rate_limit_reached_type"]
                        .into_iter()
                        .find_map(|key| rate.extra.get(key))
                })),
                "spendControlReached": (usage.rate_limit.as_ref().and_then(|rate| {
                    ["spendControlReached", "spend_control_reached"]
                        .into_iter()
                        .find_map(|key| rate.extra.get(key))
                })),
                "windows": {
                    "fiveHour": usage.rate_limit.as_ref().and_then(|rate| rate.primary_window.as_ref()).map(window_json),
                    "weekly": usage.rate_limit.as_ref().and_then(|rate| rate.secondary_window.as_ref()).map(window_json),
                },
                "additionalRateLimits": usage
                    .additional_rate_limits
                    .iter()
                    .map(additional_rate_limit_json)
                    .collect::<Vec<_>>(),
            })
        }
        ProviderQuotaSnapshot::Copilot(info) => json!({
            "account": info.login,
            "plan": info.copilot_plan.as_ref().or(info.access_type_sku.as_ref()),
            "status": format_copilot_quota_status(info),
            "main": format_copilot_main_quota(info),
            "reset": format_copilot_reset_summary(info),
        }),
        ProviderQuotaSnapshot::Gemini(info) => json!({
            "account": info.email,
            "plan": info.plan,
            "project": info.project_id,
            "status": format_gemini_quota_status(info),
            "main": format_gemini_main_quota(info),
            "reset": format_gemini_reset_summary(info),
        }),
        ProviderQuotaSnapshot::External(info) => json!({
            "account": info.account,
            "plan": info.plan,
            "status": info.status,
            "main": info.main,
            "reset": info.reset,
            "details": info.details,
        }),
    }
}

fn window_json(window: &prodex_quota::UsageWindow) -> Value {
    json!({
        "usedPercent": window.used_percent,
        "remainingPercent": window
            .used_percent
            .map(|used_percent| prodex_quota::remaining_percent(Some(used_percent))),
        "resetAt": window.reset_at,
        "windowSeconds": window.limit_window_seconds,
    })
}

fn additional_rate_limit_json(additional: &prodex_quota::AdditionalRateLimit) -> Value {
    let pair = &additional.rate_limit;
    let normal_model = ["normalModelSlug", "normal_model_slug"]
        .into_iter()
        .find_map(|key| {
            additional
                .extra
                .get(key)
                .or_else(|| pair.extra.get(key))
                .and_then(Value::as_str)
        });
    json!({
        "limitId": additional.limit_id,
        "limitName": additional.limit_name,
        "meteredFeature": additional.metered_feature,
        "normalModelSlug": normal_model,
        "allowed": additional.allowed.or(pair.allowed),
        "limitReached": additional.limit_reached.or(pair.limit_reached),
        "rateLimitReachedType": (["rateLimitReachedType", "rate_limit_reached_type"]
            .into_iter()
            .find_map(|key| additional.extra.get(key).or_else(|| pair.extra.get(key)))),
        "spendControlReached": (["spendControlReached", "spend_control_reached"]
            .into_iter()
            .find_map(|key| additional.extra.get(key).or_else(|| pair.extra.get(key)))),
        "windows": {
            "fiveHour": pair.primary_window.as_ref().map(window_json),
            "weekly": pair.secondary_window.as_ref().map(window_json),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use prodex_quota::{UsageResponse, UsageWindow, WindowPair};
    use std::collections::BTreeMap;

    #[test]
    fn quota_json_keeps_unknown_and_additional_bucket_metadata() {
        let usage = UsageResponse {
            email: None,
            plan_type: None,
            rate_limit: Some(WindowPair {
                allowed: None,
                limit_reached: None,
                primary_window: Some(UsageWindow {
                    used_percent: None,
                    reset_at: Some(1_700_000_000),
                    limit_window_seconds: Some(18_000),
                }),
                secondary_window: None,
                extra: BTreeMap::new(),
            }),
            code_review_rate_limit: None,
            rate_limit_reset_credits: None,
            additional_rate_limits: vec![prodex_quota::AdditionalRateLimit {
                limit_id: Some("base_model_inference".to_string()),
                limit_name: Some("gpt-luna-reserve".to_string()),
                metered_feature: None,
                rate_limit: WindowPair {
                    allowed: None,
                    limit_reached: None,
                    primary_window: None,
                    secondary_window: Some(UsageWindow {
                        used_percent: Some(10),
                        reset_at: Some(1_700_000_100),
                        limit_window_seconds: Some(604_800),
                    }),
                    extra: BTreeMap::from([(
                        "normalModelSlug".to_string(),
                        serde_json::json!("gpt-5.6-luna"),
                    )]),
                },
                allowed: None,
                limit_reached: None,
                extra: BTreeMap::new(),
            }],
        };

        let payload = quota_summary(&ProviderQuotaSnapshot::OpenAi(usage));
        assert!(payload["windows"]["fiveHour"]["remainingPercent"].is_null());
        assert_eq!(
            payload["additionalRateLimits"][0]["limitId"],
            "base_model_inference"
        );
        assert_eq!(
            payload["additionalRateLimits"][0]["normalModelSlug"],
            "gpt-5.6-luna"
        );
        assert_eq!(
            payload["additionalRateLimits"][0]["windows"]["weekly"]["remainingPercent"],
            90
        );
    }
}
