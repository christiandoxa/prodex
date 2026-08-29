//! Auto-redeem quota summary predicates.

use super::RuntimePrecommitQuotaBlockReason;
use prodex_quota::{RuntimeQuotaSummary, RuntimeQuotaWindowStatus};

#[cfg(not(feature = "mojo-quota"))]
const RUNTIME_AUTO_REDEEM_NATURAL_RESET_GRACE_SECONDS: i64 = 5 * 60;

#[cfg(not(feature = "mojo-quota"))]
pub(super) fn runtime_auto_redeem_weekly_exhausted_reset_at(
    summary: RuntimeQuotaSummary,
) -> Option<i64> {
    matches!(summary.weekly.status, RuntimeQuotaWindowStatus::Exhausted)
        .then_some(summary.weekly.reset_at)
        .filter(|reset_at| *reset_at != i64::MAX)
}

#[cfg(feature = "mojo-quota")]
pub(super) fn runtime_auto_redeem_candidate_is_eligible(
    summary: RuntimeQuotaSummary,
    available_count: i64,
    now: i64,
) -> anyhow::Result<bool> {
    let input = prodex_mojo_core::runtime::AutoRedeemCandidateInput {
        plan_type: None,
        available_count,
        weekly_status: match summary.weekly.status {
            prodex_quota::RuntimeQuotaWindowStatus::Ready => 0,
            prodex_quota::RuntimeQuotaWindowStatus::Thin => 1,
            prodex_quota::RuntimeQuotaWindowStatus::Critical => 2,
            prodex_quota::RuntimeQuotaWindowStatus::Exhausted => 3,
            prodex_quota::RuntimeQuotaWindowStatus::Unknown => 4,
        },
        weekly_reset_at: summary.weekly.reset_at,
        inflight_count: 0,
        health_sort_key: 0,
        order_index: 0,
    };
    let selected = prodex_mojo_core::runtime::auto_redeem_plan_batch(&[input], now)
        .map_err(|error| anyhow::anyhow!("Mojo auto-redeem planner failed: {error:?}"))?;
    Ok(selected.is_some())
}

#[cfg(not(feature = "mojo-quota"))]
pub(super) fn runtime_auto_redeem_quota_summary_warrants_credit(
    summary: RuntimeQuotaSummary,
    now: i64,
) -> bool {
    let Some(reset_at) = runtime_auto_redeem_weekly_exhausted_reset_at(summary) else {
        return false;
    };
    reset_at.saturating_sub(now) > RUNTIME_AUTO_REDEEM_NATURAL_RESET_GRACE_SECONDS
}

pub(super) fn runtime_auto_redeem_quota_summary_allows_retry(summary: RuntimeQuotaSummary) -> bool {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "mojo-quota")]
    fn quota_summary_warrants_credit(summary: RuntimeQuotaSummary, now: i64) -> bool {
        runtime_auto_redeem_candidate_is_eligible(summary, 1, now)
            .expect("Mojo auto-redeem planner should return a valid result")
    }

    #[cfg(not(feature = "mojo-quota"))]
    fn quota_summary_warrants_credit(summary: RuntimeQuotaSummary, now: i64) -> bool {
        runtime_auto_redeem_quota_summary_warrants_credit(summary, now)
    }

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
        assert!(!quota_summary_warrants_credit(
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
        assert!(!quota_summary_warrants_credit(
            test_quota_summary(
                RuntimeQuotaWindowStatus::Exhausted,
                now + 3_600,
                RuntimeQuotaWindowStatus::Ready,
                now + 86_400,
            ),
            now,
        ));
        assert!(quota_summary_warrants_credit(
            test_quota_summary(
                RuntimeQuotaWindowStatus::Ready,
                now + 3_600,
                RuntimeQuotaWindowStatus::Exhausted,
                now + 86_400,
            ),
            now,
        ));
        assert!(!quota_summary_warrants_credit(
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
