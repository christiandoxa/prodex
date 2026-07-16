use super::{
    RUNTIME_PROFILE_QUOTA_QUARANTINE_FALLBACK_SECONDS, RuntimeProfileUsageSnapshot,
    RuntimeQuotaWindowStatus, RuntimeRotationProxyShared, RuntimeRouteKind, UsageResponse,
    prune_runtime_profile_selection_backoff, runtime_profile_known_quota_reset_at,
    runtime_proxy_log, runtime_proxy_quota_reset_at_from_message, runtime_route_kind_label,
    schedule_runtime_state_save_from_runtime,
};
use crate::RuntimeStateMutation;
use anyhow::Result;
use chrono::Local;

pub(crate) fn mark_runtime_profile_quota_quarantine(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
    quota_message: Option<&str>,
) -> Result<()> {
    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let now = Local::now().timestamp();
    prune_runtime_profile_selection_backoff(&mut runtime, now);
    let resolved_reset_at = quota_message
        .and_then(runtime_proxy_quota_reset_at_from_message)
        .or_else(|| runtime_profile_known_quota_reset_at(&runtime, profile_name, route_kind))
        .filter(|reset_at| *reset_at > now);
    let until = resolved_reset_at
        .unwrap_or_else(|| now.saturating_add(RUNTIME_PROFILE_QUOTA_QUARANTINE_FALLBACK_SECONDS));
    runtime.profile_probe_cache.remove(profile_name);
    let snapshot = runtime
        .profile_usage_snapshots
        .entry(profile_name.to_string())
        .or_insert(RuntimeProfileUsageSnapshot {
            checked_at: now,
            five_hour_status: RuntimeQuotaWindowStatus::Unknown,
            five_hour_remaining_percent: 0,
            five_hour_reset_at: i64::MAX,
            weekly_status: RuntimeQuotaWindowStatus::Unknown,
            weekly_remaining_percent: 0,
            weekly_reset_at: i64::MAX,
        });
    snapshot.checked_at = now;
    snapshot.five_hour_status = RuntimeQuotaWindowStatus::Exhausted;
    snapshot.five_hour_remaining_percent = 0;
    snapshot.five_hour_reset_at = if snapshot.five_hour_reset_at == i64::MAX {
        until
    } else {
        snapshot.five_hour_reset_at.max(until)
    };
    snapshot.weekly_status = RuntimeQuotaWindowStatus::Exhausted;
    snapshot.weekly_remaining_percent = 0;
    snapshot.weekly_reset_at = if snapshot.weekly_reset_at == i64::MAX {
        until
    } else {
        snapshot.weekly_reset_at.max(until)
    };
    runtime
        .profile_retry_backoff_until
        .entry(profile_name.to_string())
        .and_modify(|current| *current = (*current).max(until))
        .or_insert(until);
    schedule_runtime_state_save_from_runtime(
        shared,
        &runtime,
        RuntimeStateMutation::ProfileRetryBackoff(profile_name.to_string()),
    );
    drop(runtime);
    runtime_proxy_log(
        shared,
        format!(
            "profile_quota_quarantine profile={profile_name} route={} until={} reset_at={} message={}",
            runtime_route_kind_label(route_kind),
            until,
            resolved_reset_at.unwrap_or(i64::MAX),
            quota_message.unwrap_or("-"),
        ),
    );
    runtime_proxy_log(
        shared,
        format!("profile_retry_backoff profile={profile_name} until={until}"),
    );
    Ok(())
}

pub(crate) fn mark_runtime_profile_quota_quarantine_for_request_model(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
    quota_message: Option<&str>,
    request_model_name: Option<&str>,
) -> Result<()> {
    if runtime_spark_scoped_quota_block_has_main_remaining(
        shared,
        profile_name,
        request_model_name,
        quota_message,
    )? {
        runtime_proxy_log(
            shared,
            format!(
                "profile_spark_quota_quarantine_skipped profile={profile_name} route={} model={} message={}",
                runtime_route_kind_label(route_kind),
                request_model_name.unwrap_or("-"),
                quota_message.unwrap_or("-"),
            ),
        );
        return Ok(());
    }

    mark_runtime_profile_quota_quarantine(shared, profile_name, route_kind, quota_message)
}

fn runtime_spark_scoped_quota_block_has_main_remaining(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    request_model_name: Option<&str>,
    quota_message: Option<&str>,
) -> Result<bool> {
    if !runtime_quota_block_is_spark_scoped(request_model_name, quota_message) {
        return Ok(false);
    }

    let runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    Ok(runtime
        .profile_probe_cache
        .get(profile_name)
        .and_then(|entry| entry.result.as_ref().ok())
        .is_some_and(runtime_usage_main_quota_has_remaining))
}

fn runtime_quota_block_is_spark_scoped(
    request_model_name: Option<&str>,
    quota_message: Option<&str>,
) -> bool {
    runtime_text_mentions_spark_quota_target(request_model_name)
        || runtime_text_mentions_spark_quota_target(quota_message)
}

fn runtime_text_mentions_spark_quota_target(value: Option<&str>) -> bool {
    let Some(value) = value else {
        return false;
    };
    let normalized = value.to_ascii_lowercase();
    normalized.contains("bengalfox")
        || normalized.contains("gpt-5.3-codex-spark")
        || normalized.contains("codex-spark")
        || (normalized.contains("codex") && normalized.contains("spark"))
}

fn runtime_usage_main_quota_has_remaining(usage: &UsageResponse) -> bool {
    usage
        .rate_limit
        .as_ref()
        .is_some_and(prodex_quota::window_pair_has_ready_limit)
}

#[cfg(test)]
mod tests {
    use super::*;
    use prodex_quota::{AdditionalRateLimit, UsageWindow, WindowPair};

    fn usage_response(
        five_hour_used_percent: i64,
        weekly_used_percent: i64,
        now: i64,
    ) -> UsageResponse {
        UsageResponse {
            email: None,
            plan_type: None,
            rate_limit: Some(WindowPair {
                primary_window: Some(UsageWindow {
                    used_percent: Some(five_hour_used_percent),
                    reset_at: Some(now + 3_600),
                    limit_window_seconds: Some(18_000),
                }),
                secondary_window: Some(UsageWindow {
                    used_percent: Some(weekly_used_percent),
                    reset_at: Some(now + 86_400),
                    limit_window_seconds: Some(604_800),
                }),
            }),
            code_review_rate_limit: None,
            rate_limit_reset_credits: None,
            additional_rate_limits: Vec::new(),
        }
    }

    fn spark_limit(
        five_hour_remaining: i64,
        weekly_remaining: i64,
        now: i64,
    ) -> AdditionalRateLimit {
        AdditionalRateLimit {
            limit_name: Some("GPT-5.3-Codex-Spark".to_string()),
            metered_feature: Some("codex_bengalfox".to_string()),
            rate_limit: WindowPair {
                primary_window: Some(UsageWindow {
                    used_percent: Some(100 - five_hour_remaining),
                    reset_at: Some(now + 7_200),
                    limit_window_seconds: Some(18_000),
                }),
                secondary_window: Some(UsageWindow {
                    used_percent: Some(100 - weekly_remaining),
                    reset_at: Some(now + 172_800),
                    limit_window_seconds: Some(604_800),
                }),
            },
        }
    }

    #[test]
    fn spark_quota_scope_detection_matches_model_and_message() {
        assert!(runtime_quota_block_is_spark_scoped(
            Some("gpt-5.3-codex-spark"),
            None,
        ));
        assert!(runtime_quota_block_is_spark_scoped(
            None,
            Some("codex_bengalfox usage limit reached"),
        ));
        assert!(!runtime_quota_block_is_spark_scoped(
            Some("gpt-5.3-codex"),
            Some("insufficient_quota"),
        ));
    }

    #[test]
    fn spark_exhaustion_does_not_remove_main_remaining_signal() {
        let now = Local::now().timestamp();
        let mut usage = usage_response(0, 65, now);
        usage.additional_rate_limits.push(spark_limit(0, 0, now));

        assert!(runtime_usage_main_quota_has_remaining(&usage));
    }

    #[test]
    fn exhausted_main_quota_is_not_treated_as_remaining() {
        let now = Local::now().timestamp();
        let mut usage = usage_response(100, 65, now);
        usage.additional_rate_limits.push(spark_limit(90, 90, now));

        assert!(!runtime_usage_main_quota_has_remaining(&usage));
    }

    #[test]
    fn weekly_only_main_quota_survives_spark_scoped_exhaustion() {
        let now = Local::now().timestamp();
        let mut usage = usage_response(0, 65, now);
        usage.rate_limit.as_mut().unwrap().primary_window = None;
        usage.additional_rate_limits.push(spark_limit(0, 0, now));

        assert!(runtime_usage_main_quota_has_remaining(&usage));
    }
}
