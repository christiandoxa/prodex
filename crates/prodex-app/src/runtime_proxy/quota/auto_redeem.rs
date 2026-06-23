use super::{
    RUNTIME_PROFILE_SYNC_PROBE_FALLBACK_LIMIT, RateLimitResetCreditConsumeFlow,
    RateLimitResetCreditConsumeOutcome, RuntimePrecommitQuotaBlockReason,
    RuntimeRotationProxyShared, RuntimeRouteKind, active_profile_selection_order,
    fetch_usage_with_proxy_policy, prune_runtime_profile_selection_backoff,
    run_runtime_probe_jobs_inline, runtime_profile_auth_failure_active_with_auth_cache,
    runtime_profile_health_score, runtime_profile_inflight_soft_limit,
    runtime_profile_inflight_sort_key, runtime_profile_name_in_selection_backoff,
    runtime_profile_route_circuit_open_until, runtime_profile_transport_backoff_until_from_map,
    runtime_proxy_log, runtime_proxy_pressure_mode_active_for_route,
    runtime_quota_summary_for_route, runtime_quota_summary_log_fields, runtime_route_kind_label,
    update_runtime_profile_probe_cache_with_usage,
};
use crate::ProfileProviderExt;
use anyhow::Result;
use chrono::Local;
use prodex_quota::{RuntimeQuotaSummary, RuntimeQuotaWindowStatus};
use std::collections::BTreeSet;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static RUNTIME_AUTO_REDEEM_SEQUENCE: AtomicU64 = AtomicU64::new(1);
const RUNTIME_AUTO_REDEEM_NATURAL_RESET_GRACE_SECONDS: i64 = 120;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RuntimeAutoRedeemResetCreditOutcome {
    Redeemed,
    NothingToRedeem,
    Failed,
}

fn runtime_auto_redeem_idempotency_key() -> String {
    let sequence = RUNTIME_AUTO_REDEEM_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let now_nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    format!(
        "prodex-auto-redeem-{}-{now_nanos}-{sequence}",
        std::process::id()
    )
}

fn runtime_auto_redeem_weekly_exhausted_reset_at(summary: RuntimeQuotaSummary) -> Option<i64> {
    matches!(summary.weekly.status, RuntimeQuotaWindowStatus::Exhausted)
        .then_some(summary.weekly.reset_at)
        .filter(|reset_at| *reset_at != i64::MAX)
}

fn runtime_auto_redeem_quota_summary_warrants_credit(
    summary: RuntimeQuotaSummary,
    now: i64,
) -> bool {
    let Some(reset_at) = runtime_auto_redeem_weekly_exhausted_reset_at(summary) else {
        return false;
    };
    reset_at.saturating_sub(now) > RUNTIME_AUTO_REDEEM_NATURAL_RESET_GRACE_SECONDS
}

fn runtime_auto_redeem_quota_summary_allows_retry(summary: RuntimeQuotaSummary) -> bool {
    !matches!(
        summary.five_hour.status,
        RuntimeQuotaWindowStatus::Exhausted
    ) && !matches!(summary.weekly.status, RuntimeQuotaWindowStatus::Exhausted)
}

pub(crate) fn runtime_auto_redeem_precommit_reason_warrants_credit(
    reason: RuntimePrecommitQuotaBlockReason,
) -> bool {
    matches!(
        reason,
        RuntimePrecommitQuotaBlockReason::ExhaustedBeforeSend
    )
}

fn runtime_auto_redeem_plan_priority(plan_type: Option<&str>) -> usize {
    let normalized = plan_type
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase()
        .chars()
        .filter(|ch| !matches!(ch, ' ' | '-' | '_'))
        .collect::<String>();

    match normalized.as_str() {
        "plus" => 0,
        "free" | "basic" => 1,
        "" => 2,
        "prolite" | "pro" | "pro5x" | "5x" | "pro20x" | "pro20" | "20x" | "ultra" | "max"
        | "team" | "business" | "enterprise" => 3,
        _ => 2,
    }
}

fn runtime_auto_redeem_window_is_usable(status: RuntimeQuotaWindowStatus) -> bool {
    matches!(
        status,
        RuntimeQuotaWindowStatus::Ready
            | RuntimeQuotaWindowStatus::Thin
            | RuntimeQuotaWindowStatus::Critical
    )
}

fn runtime_auto_redeem_quota_summary_has_weekly_remaining(summary: RuntimeQuotaSummary) -> bool {
    runtime_auto_redeem_window_is_usable(summary.weekly.status)
}

fn refresh_runtime_auto_redeem_pool_missing_quota(
    shared: &RuntimeRotationProxyShared,
    route_kind: RuntimeRouteKind,
    excluded_profiles: &BTreeSet<String>,
    context: &str,
) -> Result<()> {
    let now = Local::now().timestamp();
    let pressure_mode = runtime_proxy_pressure_mode_active_for_route(shared, route_kind);
    let inflight_soft_limit = runtime_profile_inflight_soft_limit(route_kind, pressure_mode);
    let jobs = {
        let mut runtime = shared
            .runtime
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
        prune_runtime_profile_selection_backoff(&mut runtime, now);
        active_profile_selection_order(&runtime.state, &runtime.current_profile)
            .into_iter()
            .filter(|name| !excluded_profiles.contains(name))
            .filter(|name| !runtime.profile_probe_cache.contains_key(name))
            .filter_map(|name| {
                let profile = runtime.state.profiles.get(&name)?;
                if !matches!(profile.provider, crate::ProfileProvider::Openai) {
                    return None;
                }
                if runtime_profile_auth_failure_active_with_auth_cache(
                    &runtime.profile_health,
                    &runtime.profile_usage_auth,
                    &name,
                    now,
                ) {
                    return None;
                }
                if runtime_profile_name_in_selection_backoff(
                    &name,
                    &runtime.profile_retry_backoff_until,
                    &runtime.profile_transport_backoff_until,
                    &runtime.profile_route_circuit_open_until,
                    route_kind,
                    now,
                ) {
                    return None;
                }
                if runtime_profile_inflight_sort_key(&name, &runtime.profile_inflight)
                    >= inflight_soft_limit
                {
                    return None;
                }
                profile
                    .provider
                    .auth_summary(&profile.codex_home)
                    .quota_compatible
                    .then(|| (name, profile.codex_home.clone()))
            })
            .take(RUNTIME_PROFILE_SYNC_PROBE_FALLBACK_LIMIT)
            .collect::<Vec<_>>()
    };

    if !jobs.is_empty() {
        runtime_proxy_log(
            shared,
            format!(
                "{context}_auto_redeem_pool_probe_start route={} jobs={}",
                runtime_route_kind_label(route_kind),
                jobs.len(),
            ),
        );
        run_runtime_probe_jobs_inline(shared, jobs, &format!("{context}_auto_redeem_pool_probe"));
    }
    Ok(())
}

fn runtime_auto_redeem_pool_has_weekly_remaining_profile(
    shared: &RuntimeRotationProxyShared,
    route_kind: RuntimeRouteKind,
    excluded_profiles: &BTreeSet<String>,
) -> Result<Option<String>> {
    if !shared.auto_redeem_enabled {
        return Ok(None);
    }
    let now = Local::now().timestamp();
    let pressure_mode = runtime_proxy_pressure_mode_active_for_route(shared, route_kind);
    let inflight_soft_limit = runtime_profile_inflight_soft_limit(route_kind, pressure_mode);
    let runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;

    Ok(
        active_profile_selection_order(&runtime.state, &runtime.current_profile)
            .into_iter()
            .filter(|name| !excluded_profiles.contains(name))
            .find(|name| {
                let Some(profile) = runtime.state.profiles.get(name) else {
                    return false;
                };
                if !matches!(profile.provider, crate::ProfileProvider::Openai) {
                    return false;
                }
                if runtime_profile_auth_failure_active_with_auth_cache(
                    &runtime.profile_health,
                    &runtime.profile_usage_auth,
                    name,
                    now,
                ) {
                    return false;
                }
                if runtime_profile_transport_backoff_until_from_map(
                    &runtime.profile_transport_backoff_until,
                    name,
                    route_kind,
                    now,
                )
                .is_some()
                    || runtime_profile_route_circuit_open_until(&runtime, name, route_kind, now)
                        .is_some()
                {
                    return false;
                }
                if runtime_profile_inflight_sort_key(name, &runtime.profile_inflight)
                    >= inflight_soft_limit
                {
                    return false;
                }
                let Some(probe) = runtime.profile_probe_cache.get(name) else {
                    return false;
                };
                if !probe.auth.quota_compatible {
                    return false;
                }
                let Ok(usage) = probe.result.as_ref() else {
                    return false;
                };
                runtime_auto_redeem_quota_summary_has_weekly_remaining(
                    runtime_quota_summary_for_route(usage, route_kind),
                )
            }),
    )
}

pub(crate) fn runtime_best_auto_redeem_profile_name(
    shared: &RuntimeRotationProxyShared,
    route_kind: RuntimeRouteKind,
    excluded_profiles: &BTreeSet<String>,
) -> Result<Option<String>> {
    let now = Local::now().timestamp();
    let pressure_mode = runtime_proxy_pressure_mode_active_for_route(shared, route_kind);
    let inflight_soft_limit = runtime_profile_inflight_soft_limit(route_kind, pressure_mode);
    let runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let order = active_profile_selection_order(&runtime.state, &runtime.current_profile);

    Ok(order
        .into_iter()
        .enumerate()
        .filter(|(_, name)| !excluded_profiles.contains(name))
        .filter_map(|(order_index, name)| {
            let profile = runtime.state.profiles.get(&name)?;
            if !matches!(profile.provider, crate::ProfileProvider::Openai) {
                return None;
            }
            if runtime_profile_auth_failure_active_with_auth_cache(
                &runtime.profile_health,
                &runtime.profile_usage_auth,
                &name,
                now,
            ) {
                return None;
            }
            if runtime_profile_transport_backoff_until_from_map(
                &runtime.profile_transport_backoff_until,
                &name,
                route_kind,
                now,
            )
            .is_some()
                || runtime_profile_route_circuit_open_until(&runtime, &name, route_kind, now)
                    .is_some()
            {
                return None;
            }
            let inflight_count =
                runtime_profile_inflight_sort_key(&name, &runtime.profile_inflight);
            if inflight_count >= inflight_soft_limit {
                return None;
            }
            let probe = runtime.profile_probe_cache.get(&name)?;
            if !probe.auth.quota_compatible {
                return None;
            }
            let usage = probe.result.as_ref().ok()?;
            let available_count = usage
                .rate_limit_reset_credits
                .as_ref()
                .map(|credits| credits.available_count)
                .unwrap_or_default();
            if available_count <= 0 {
                return None;
            }
            let quota_summary = runtime_quota_summary_for_route(usage, route_kind);
            if !runtime_auto_redeem_quota_summary_warrants_credit(quota_summary, now) {
                return None;
            }
            let reset_sort_key = runtime_auto_redeem_weekly_exhausted_reset_at(quota_summary)
                .unwrap_or_default()
                .saturating_neg();
            let health_sort_key = runtime_profile_health_score(&runtime, &name, now, route_kind);
            Some((
                (
                    runtime_auto_redeem_plan_priority(usage.plan_type.as_deref()),
                    reset_sort_key,
                    inflight_count,
                    health_sort_key,
                    order_index,
                    name.clone(),
                ),
                name,
            ))
        })
        .min_by_key(|(sort_key, _)| sort_key.clone())
        .map(|(_, name)| name))
}

pub(crate) fn runtime_auto_redeem_usage_limit_reset_credit(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
    context: &str,
    prefer_best_pool_profile: bool,
) -> Result<RuntimeAutoRedeemResetCreditOutcome> {
    if !shared.auto_redeem_enabled {
        return Ok(RuntimeAutoRedeemResetCreditOutcome::NothingToRedeem);
    }
    if prefer_best_pool_profile {
        let excluded_profiles = BTreeSet::new();
        refresh_runtime_auto_redeem_pool_missing_quota(
            shared,
            route_kind,
            &excluded_profiles,
            context,
        )?;
        if let Some(weekly_profile_name) = runtime_auto_redeem_pool_has_weekly_remaining_profile(
            shared,
            route_kind,
            &excluded_profiles,
        )? {
            runtime_proxy_log(
                shared,
                format!(
                    "{context}_auto_redeem_deferred profile={profile_name} route={} reason=weekly_pool_profile weekly_profile={weekly_profile_name}",
                    runtime_route_kind_label(route_kind),
                ),
            );
            return Ok(RuntimeAutoRedeemResetCreditOutcome::NothingToRedeem);
        }
        if let Some(best_profile_name) =
            runtime_best_auto_redeem_profile_name(shared, route_kind, &excluded_profiles)?
            && best_profile_name != profile_name
        {
            runtime_proxy_log(
                shared,
                format!(
                    "{context}_auto_redeem_deferred profile={profile_name} route={} reason=better_pool_profile best_profile={best_profile_name}",
                    runtime_route_kind_label(route_kind),
                ),
            );
            return Ok(RuntimeAutoRedeemResetCreditOutcome::NothingToRedeem);
        }
    }

    let Some((codex_home, base_url)) = ({
        let runtime = shared
            .runtime
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
        runtime
            .state
            .profiles
            .get(profile_name)
            .and_then(|profile| {
                matches!(profile.provider, crate::ProfileProvider::Openai).then(|| {
                    (
                        profile.codex_home.clone(),
                        runtime.upstream_base_url.clone(),
                    )
                })
            })
    }) else {
        return Ok(RuntimeAutoRedeemResetCreditOutcome::NothingToRedeem);
    };

    let upstream_no_proxy = shared.upstream_no_proxy;
    let before_usage = fetch_usage_with_proxy_policy(&codex_home, Some(&base_url), upstream_no_proxy)
        .inspect_err(|err| {
            runtime_proxy_log(
                shared,
                format!(
                    "{context}_auto_redeem_availability_failed profile={profile_name} route={} error={}",
                    runtime_route_kind_label(route_kind),
                    err.to_string().replace('\n', " "),
                ),
            );
        });
    let Ok(before_usage) = before_usage else {
        return Ok(RuntimeAutoRedeemResetCreditOutcome::Failed);
    };
    update_runtime_profile_probe_cache_with_usage(shared, profile_name, before_usage.clone())?;
    let quota_summary = runtime_quota_summary_for_route(&before_usage, route_kind);
    if !runtime_auto_redeem_quota_summary_warrants_credit(quota_summary, Local::now().timestamp()) {
        runtime_proxy_log(
            shared,
            format!(
                "{context}_auto_redeem_deferred profile={profile_name} route={} reason=not_exhausted_or_natural_reset_near {}",
                runtime_route_kind_label(route_kind),
                runtime_quota_summary_log_fields(quota_summary),
            ),
        );
        return Ok(RuntimeAutoRedeemResetCreditOutcome::NothingToRedeem);
    }
    let available_count = before_usage
        .rate_limit_reset_credits
        .as_ref()
        .map(|credits| credits.available_count)
        .unwrap_or_default();
    if available_count <= 0 {
        runtime_proxy_log(
            shared,
            format!(
                "{context}_auto_redeem_unavailable profile={profile_name} route={} available_count={available_count}",
                runtime_route_kind_label(route_kind),
            ),
        );
        return Ok(RuntimeAutoRedeemResetCreditOutcome::NothingToRedeem);
    }

    let redeem_request_id = runtime_auto_redeem_idempotency_key();
    runtime_proxy_log(
        shared,
        format!(
            "{context}_auto_redeem_start profile={profile_name} route={} available_count={available_count}",
            runtime_route_kind_label(route_kind),
        ),
    );
    let consume = RateLimitResetCreditConsumeFlow::new_with_proxy_policy(
        &codex_home,
        Some(&base_url),
        upstream_no_proxy,
    )
    .and_then(|flow| flow.execute(&redeem_request_id))
    .inspect_err(|err| {
        runtime_proxy_log(
            shared,
            format!(
                "{context}_auto_redeem_failed profile={profile_name} route={} error={}",
                runtime_route_kind_label(route_kind),
                err.to_string().replace('\n', " "),
            ),
        );
    });
    let Ok(consume) = consume else {
        return Ok(RuntimeAutoRedeemResetCreditOutcome::Failed);
    };
    runtime_proxy_log(
        shared,
        format!(
            "{context}_auto_redeem_result profile={profile_name} route={} outcome={:?}",
            runtime_route_kind_label(route_kind),
            consume.outcome,
        ),
    );

    match consume.outcome {
        RateLimitResetCreditConsumeOutcome::Reset
        | RateLimitResetCreditConsumeOutcome::AlreadyRedeemed => {
            let after_usage = match fetch_usage_with_proxy_policy(
                &codex_home,
                Some(&base_url),
                upstream_no_proxy,
            ) {
                Ok(after_usage) => after_usage,
                Err(err) => {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "{context}_auto_redeem_refresh_failed profile={profile_name} route={} error={}",
                            runtime_route_kind_label(route_kind),
                            err.to_string().replace('\n', " "),
                        ),
                    );
                    return Ok(RuntimeAutoRedeemResetCreditOutcome::Failed);
                }
            };
            update_runtime_profile_probe_cache_with_usage(
                shared,
                profile_name,
                after_usage.clone(),
            )?;
            let after_summary = runtime_quota_summary_for_route(&after_usage, route_kind);
            if !runtime_auto_redeem_quota_summary_allows_retry(after_summary) {
                runtime_proxy_log(
                    shared,
                    format!(
                        "{context}_auto_redeem_still_blocked profile={profile_name} route={} {}",
                        runtime_route_kind_label(route_kind),
                        runtime_quota_summary_log_fields(after_summary),
                    ),
                );
                return Ok(RuntimeAutoRedeemResetCreditOutcome::Failed);
            }
            Ok(RuntimeAutoRedeemResetCreditOutcome::Redeemed)
        }
        RateLimitResetCreditConsumeOutcome::NothingToReset
        | RateLimitResetCreditConsumeOutcome::NoCredit => {
            Ok(RuntimeAutoRedeemResetCreditOutcome::NothingToRedeem)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_quota_summary(
        five_hour_status: RuntimeQuotaWindowStatus,
        five_hour_reset_at: i64,
        weekly_status: RuntimeQuotaWindowStatus,
        weekly_reset_at: i64,
    ) -> RuntimeQuotaSummary {
        RuntimeQuotaSummary {
            five_hour: prodex_quota::RuntimeQuotaWindowSummary {
                status: five_hour_status,
                remaining_percent: if matches!(
                    five_hour_status,
                    RuntimeQuotaWindowStatus::Exhausted
                ) {
                    0
                } else {
                    1
                },
                reset_at: five_hour_reset_at,
            },
            weekly: prodex_quota::RuntimeQuotaWindowSummary {
                status: weekly_status,
                remaining_percent: if matches!(weekly_status, RuntimeQuotaWindowStatus::Exhausted) {
                    0
                } else {
                    50
                },
                reset_at: weekly_reset_at,
            },
            route_band: if matches!(
                (five_hour_status, weekly_status),
                (RuntimeQuotaWindowStatus::Exhausted, _) | (_, RuntimeQuotaWindowStatus::Exhausted)
            ) {
                prodex_quota::RuntimeQuotaPressureBand::Exhausted
            } else {
                prodex_quota::RuntimeQuotaPressureBand::Critical
            },
        }
    }

    #[test]
    fn auto_redeem_requires_exhausted_window_not_critical_floor() {
        let now = 1_000;
        assert!(!runtime_auto_redeem_quota_summary_warrants_credit(
            test_quota_summary(
                RuntimeQuotaWindowStatus::Critical,
                now + 3_600,
                RuntimeQuotaWindowStatus::Ready,
                now + 86_400,
            ),
            now,
        ));
        assert!(!runtime_auto_redeem_precommit_reason_warrants_credit(
            RuntimePrecommitQuotaBlockReason::CriticalFloorBeforeSend,
        ));
    }

    #[test]
    fn auto_redeem_requires_weekly_exhausted_and_uses_weekly_reset_time() {
        let now = 1_000;
        assert!(!runtime_auto_redeem_quota_summary_warrants_credit(
            test_quota_summary(
                RuntimeQuotaWindowStatus::Exhausted,
                now + 3_600,
                RuntimeQuotaWindowStatus::Ready,
                now + 86_400,
            ),
            now,
        ));
        assert!(runtime_auto_redeem_quota_summary_warrants_credit(
            test_quota_summary(
                RuntimeQuotaWindowStatus::Ready,
                now + 3_600,
                RuntimeQuotaWindowStatus::Exhausted,
                now + 86_400,
            ),
            now,
        ));
        assert!(!runtime_auto_redeem_quota_summary_warrants_credit(
            test_quota_summary(
                RuntimeQuotaWindowStatus::Exhausted,
                now + 30,
                RuntimeQuotaWindowStatus::Exhausted,
                now + 30,
            ),
            now,
        ));
        assert!(runtime_auto_redeem_precommit_reason_warrants_credit(
            RuntimePrecommitQuotaBlockReason::ExhaustedBeforeSend,
        ));
    }

    #[test]
    fn auto_redeem_retry_requires_refreshed_usage_to_be_unblocked() {
        let now = 1_000;
        assert!(runtime_auto_redeem_quota_summary_allows_retry(
            test_quota_summary(
                RuntimeQuotaWindowStatus::Ready,
                now + 3_600,
                RuntimeQuotaWindowStatus::Ready,
                now + 86_400,
            )
        ));
        assert!(!runtime_auto_redeem_quota_summary_allows_retry(
            test_quota_summary(
                RuntimeQuotaWindowStatus::Ready,
                now + 3_600,
                RuntimeQuotaWindowStatus::Exhausted,
                now + 86_400,
            )
        ));
    }
}
