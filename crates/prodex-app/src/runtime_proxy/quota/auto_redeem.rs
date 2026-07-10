use super::{
    RateLimitResetCreditConsumeFlow, RateLimitResetCreditConsumeOutcome,
    RuntimePrecommitQuotaBlockReason, RuntimeRotationProxyShared, RuntimeRouteKind,
    fetch_usage_with_proxy_policy, runtime_proxy_log, runtime_quota_summary_for_route,
    runtime_quota_summary_log_fields, runtime_route_kind_label,
    update_runtime_profile_probe_cache_with_usage,
};
use anyhow::Result;
use chrono::Local;
use prodex_domain::RequestId;
use redaction::redaction_redact_secret_like_text;
use std::collections::BTreeSet;

#[path = "auto_redeem/pool.rs"]
mod pool;
#[path = "auto_redeem/summary.rs"]
mod summary;
pub(crate) use pool::runtime_best_auto_redeem_profile_name;
use pool::{
    refresh_runtime_auto_redeem_pool_missing_quota,
    runtime_auto_redeem_pool_has_weekly_remaining_profile,
};
pub(crate) use summary::runtime_auto_redeem_precommit_reason_warrants_credit;
use summary::{
    runtime_auto_redeem_quota_summary_allows_retry,
    runtime_auto_redeem_quota_summary_warrants_credit,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RuntimeAutoRedeemResetCreditOutcome {
    Redeemed,
    NothingToRedeem,
    Failed,
}

fn runtime_auto_redeem_idempotency_key() -> String {
    format!("prodex-auto-redeem-{}", RequestId::new())
}

fn runtime_auto_redeem_error_log_value(err: &anyhow::Error) -> String {
    redaction_redact_secret_like_text(&err.to_string()).replace('\n', " ")
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
                    runtime_auto_redeem_error_log_value(err),
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
                runtime_auto_redeem_error_log_value(err),
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
                            runtime_auto_redeem_error_log_value(&err),
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

    #[test]
    fn auto_redeem_idempotency_key_uses_request_id_uuidv7() {
        let key = runtime_auto_redeem_idempotency_key();
        let uuid = key
            .strip_prefix("prodex-auto-redeem-")
            .expect("auto redeem key should be prodex scoped");

        assert_eq!(
            uuid.parse::<RequestId>()
                .unwrap()
                .as_uuid()
                .get_version_num(),
            7
        );
    }

    #[test]
    fn auto_redeem_error_log_value_redacts_secret_like_material() {
        let err = anyhow::anyhow!(
            "quota request failed\nAuthorization: Bearer auto-redeem-token\napi_key=auto-redeem-key"
        );
        let message = runtime_auto_redeem_error_log_value(&err);

        assert!(!message.contains('\n'));
        assert!(message.contains("Authorization: Bearer <redacted>"));
        assert!(message.contains("api_key=<redacted>"));
        assert!(!message.contains("auto-redeem-token"));
        assert!(!message.contains("auto-redeem-key"));
    }
}
