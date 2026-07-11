//! Auto-redeem quota summary predicates.

use super::RuntimePrecommitQuotaBlockReason;
use prodex_quota::{RuntimeQuotaSummary, RuntimeQuotaWindowStatus};

const RUNTIME_AUTO_REDEEM_NATURAL_RESET_GRACE_SECONDS: i64 = 5 * 60;

pub(super) fn runtime_auto_redeem_weekly_exhausted_reset_at(
    summary: RuntimeQuotaSummary,
) -> Option<i64> {
    matches!(summary.weekly.status, RuntimeQuotaWindowStatus::Exhausted)
        .then_some(summary.weekly.reset_at)
        .filter(|reset_at| *reset_at != i64::MAX)
}

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

fn runtime_auto_redeem_window_is_usable(status: RuntimeQuotaWindowStatus) -> bool {
    matches!(
        status,
        RuntimeQuotaWindowStatus::Ready
            | RuntimeQuotaWindowStatus::Thin
            | RuntimeQuotaWindowStatus::Critical
    )
}

pub(super) fn runtime_auto_redeem_quota_summary_has_weekly_remaining(
    summary: RuntimeQuotaSummary,
) -> bool {
    runtime_auto_redeem_window_is_usable(summary.weekly.status)
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
