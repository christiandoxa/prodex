#![cfg(feature = "mojo-runtime")]
use prodex_mojo_core::{
    MojoError,
    runtime::{QuotaSnapshotPlanInput, quota_snapshot_plan},
};

#[test]
fn snapshot_signed_observations_match_existing_window_and_hold_semantics() {
    const { assert!(prodex_mojo_core::MOJO_ACTIVE) };
    let observations = [i64::MIN, -300, -1, 0, 1, 90, 100, 101, 300, i64::MAX];
    for five_status in 0..=4 {
        for weekly_status in 0..=4 {
            for (index, &remaining) in observations.iter().enumerate() {
                let weekly_remaining = observations[observations.len() - index - 1];
                for (now, five_reset, weekly_reset) in [
                    (0, -1, 1),
                    (100, 100, 101),
                    (i64::MIN, i64::MAX, i64::MIN),
                    (i64::MAX, i64::MAX, 1),
                ] {
                    for route in 0..=3 {
                        let input = QuotaSnapshotPlanInput {
                            five_hour_status: five_status,
                            five_hour_remaining: remaining,
                            five_hour_reset_at: five_reset,
                            weekly_status,
                            weekly_remaining,
                            weekly_reset_at: weekly_reset,
                            route_kind: route,
                            checked_at: 0,
                            now,
                            stale_grace_seconds: 60,
                        };
                        let plan = quota_snapshot_plan(input).unwrap();
                        let window = |status, remaining, reset| {
                            if reset != i64::MAX && reset <= now {
                                (0, 100, reset)
                            } else {
                                (status, remaining, reset)
                            }
                        };
                        let mut five = window(five_status, remaining, five_reset);
                        let mut weekly = window(weekly_status, weekly_remaining, weekly_reset);
                        if five.0 == 4 && weekly.0 != 4 {
                            five = (0, 100, i64::MAX);
                        } else if weekly.0 == 4 && five.0 != 4 {
                            weekly = (0, 100, i64::MAX);
                        }
                        assert_eq!(
                            (
                                plan.five_hour_status,
                                plan.five_hour_remaining,
                                plan.five_hour_reset_at
                            ),
                            five
                        );
                        assert_eq!(
                            (
                                plan.weekly_status,
                                plan.weekly_remaining,
                                plan.weekly_reset_at
                            ),
                            weekly
                        );
                        assert_eq!(plan.route_band, five.0.max(weekly.0));
                        let holds = [(five_status, five_reset), (weekly_status, weekly_reset)];
                        let active = holds.iter().any(|&(status, reset)| {
                            status == 3 && reset != i64::MAX && reset > now
                        });
                        let expired = holds.iter().any(|&(status, reset)| {
                            status == 3 && reset != i64::MAX && reset <= now
                        });
                        assert_eq!(plan.hold_active, active);
                        assert_eq!(plan.hold_expired, expired);
                        assert_eq!(
                            plan.usable,
                            active || (!expired && now.saturating_sub(0) <= 60)
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn snapshot_still_rejects_invalid_status_route_and_grace_tags() {
    let input = QuotaSnapshotPlanInput {
        five_hour_status: 0,
        five_hour_remaining: 300,
        five_hour_reset_at: i64::MAX,
        weekly_status: 0,
        weekly_remaining: -1,
        weekly_reset_at: i64::MAX,
        route_kind: 0,
        checked_at: 0,
        now: 0,
        stale_grace_seconds: 0,
    };
    for invalid in [
        QuotaSnapshotPlanInput {
            five_hour_status: 5,
            ..input
        },
        QuotaSnapshotPlanInput {
            weekly_status: -1,
            ..input
        },
        QuotaSnapshotPlanInput {
            route_kind: 4,
            ..input
        },
        QuotaSnapshotPlanInput {
            stale_grace_seconds: -1,
            ..input
        },
    ] {
        assert_eq!(quota_snapshot_plan(invalid), Err(MojoError::InvalidInput));
    }
}
