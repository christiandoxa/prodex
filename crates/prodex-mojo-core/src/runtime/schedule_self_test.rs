//! Built-in smoke check for the Mojo-owned profile scheduler.
use super::{ProfileScheduleInput, ProfileScoreInput, profile_schedule_batch};

pub fn profile_schedule_self_test() -> bool {
    profile_schedule_batch(&[
        ProfileScheduleInput {
            score: ProfileScoreInput {
                weekly_pressure: 100,
                five_hour_pressure: 100,
                scale_bps: 10_000,
                weekly_remaining: 90,
                five_hour_remaining: 90,
                windows_complete: true,
                weekly_weight: 10,
            },
            provider_priority: 0,
            in_selection_cooldown: false,
            last_selected_at: i64::MIN,
            weekly_reset_at: 10,
            five_hour_reset_at: 20,
            quota_source: 0,
            preferred: false,
            affinity_preferred: false,
            order_index: 0,
        },
        ProfileScheduleInput {
            score: ProfileScoreInput {
                weekly_pressure: 200,
                five_hour_pressure: 200,
                scale_bps: 10_000,
                weekly_remaining: 80,
                five_hour_remaining: 80,
                windows_complete: true,
                weekly_weight: 10,
            },
            provider_priority: 1,
            in_selection_cooldown: false,
            last_selected_at: i64::MIN,
            weekly_reset_at: 10,
            five_hour_reset_at: 20,
            quota_source: 0,
            preferred: false,
            affinity_preferred: false,
            order_index: 1,
        },
    ])
    .is_ok_and(|order| order == [0, 1])
}
