//! Standard-route pre-commit quota guards.

use super::*;

pub(super) enum RuntimeStandardPrecommitGuard {
    Continue,
    RetryWithoutGuard,
    Blocked(RuntimeStandardAttempt),
}

pub(super) fn runtime_standard_precommit_quota_guard(
    request_id: u64,
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    fresh_request: bool,
) -> Result<RuntimeStandardPrecommitGuard> {
    let (quota_summary, quota_source) =
        runtime_profile_quota_summary_for_route(shared, profile_name, RuntimeRouteKind::Standard)?;
    if quota_summary.route_band != RuntimeQuotaPressureBand::Exhausted {
        return Ok(RuntimeStandardPrecommitGuard::Continue);
    }

    if runtime_auto_redeem_usage_limit_reset_credit(
        shared,
        profile_name,
        RuntimeRouteKind::Standard,
        "standard_precommit",
        fresh_request,
    )? == RuntimeAutoRedeemResetCreditOutcome::Redeemed
    {
        let (redeemed_summary, _) = runtime_profile_quota_summary_for_route(
            shared,
            profile_name,
            RuntimeRouteKind::Standard,
        )?;
        if redeemed_summary.route_band != RuntimeQuotaPressureBand::Exhausted {
            return Ok(RuntimeStandardPrecommitGuard::RetryWithoutGuard);
        }
    }

    runtime_proxy_log(
        shared,
        format!(
            "request={request_id} transport=http standard_pre_send_skip profile={profile_name} route=standard quota_source={} {}",
            quota_source
                .map(runtime_quota_source_label)
                .unwrap_or("unknown"),
            runtime_quota_summary_log_fields(quota_summary),
        ),
    );
    Ok(RuntimeStandardPrecommitGuard::Blocked(
        RuntimeStandardAttempt::LocalSelectionBlocked {
            profile_name: profile_name.to_string(),
        },
    ))
}

pub(super) fn runtime_compact_precommit_quota_guard(
    request_id: u64,
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    allow_quota_exhausted_send: bool,
    fresh_request: bool,
) -> Result<RuntimeStandardPrecommitGuard> {
    let (quota_summary, quota_source) =
        runtime_profile_quota_summary_for_route(shared, profile_name, RuntimeRouteKind::Compact)?;
    if quota_summary.route_band != RuntimeQuotaPressureBand::Exhausted {
        return Ok(RuntimeStandardPrecommitGuard::Continue);
    }

    if !allow_quota_exhausted_send {
        if runtime_auto_redeem_usage_limit_reset_credit(
            shared,
            profile_name,
            RuntimeRouteKind::Compact,
            "compact_precommit",
            fresh_request,
        )? == RuntimeAutoRedeemResetCreditOutcome::Redeemed
        {
            let (redeemed_summary, _) = runtime_profile_quota_summary_for_route(
                shared,
                profile_name,
                RuntimeRouteKind::Compact,
            )?;
            if redeemed_summary.route_band != RuntimeQuotaPressureBand::Exhausted {
                return Ok(RuntimeStandardPrecommitGuard::RetryWithoutGuard);
            }
        }
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=http standard_pre_send_skip profile={profile_name} route=compact quota_source={} {}",
                quota_source
                    .map(runtime_quota_source_label)
                    .unwrap_or("unknown"),
                runtime_quota_summary_log_fields(quota_summary),
            ),
        );
        return Ok(RuntimeStandardPrecommitGuard::Blocked(
            RuntimeStandardAttempt::LocalSelectionBlocked {
                profile_name: profile_name.to_string(),
            },
        ));
    }

    runtime_proxy_log(
        shared,
        format!(
            "request={request_id} transport=http compact_pre_send_allow_quota_exhausted profile={profile_name} quota_source={} {}",
            quota_source
                .map(runtime_quota_source_label)
                .unwrap_or("unknown"),
            runtime_quota_summary_log_fields(quota_summary),
        ),
    );
    Ok(RuntimeStandardPrecommitGuard::Continue)
}
