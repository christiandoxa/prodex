#![cfg(feature = "mojo-runtime")]

use prodex_mojo_core::runtime::{
    QuotaSummarySortKey, QuotaSummarySortKeyInput, quota_summary_sort_key,
};

#[test]
fn required_mojo_summary_sort_key_is_authoritative() {
    const { assert!(prodex_mojo_core::MOJO_REQUIRED) };

    assert_eq!(
        quota_summary_sort_key(QuotaSummarySortKeyInput {
            route_band: 4,
            weekly_status: 4,
            five_hour_status: 0,
            weekly_remaining: 0,
            five_hour_remaining: 100,
            weekly_reset_at: i64::MAX,
            five_hour_reset_at: i64::MAX,
        }),
        Ok(QuotaSummarySortKey {
            route_band: 4,
            route_band_rank: 4,
            weekly_status_rank: 4,
            five_hour_status_rank: 0,
            reserve_floor: 0,
            weekly_remaining: 0,
            five_hour_remaining: 100,
            weekly_reset_at: i64::MAX,
            five_hour_reset_at: i64::MAX,
        })
    );
}
