pub(super) fn remaining_percent(used_percent: Option<i64>) -> i64 {
    prodex_mojo_core::quota::remaining_percent(used_percent)
}

pub(super) fn window_status(remaining_percent: i64, has_window: bool) -> i64 {
    prodex_mojo_core::quota::window_status(remaining_percent, has_window)
}

pub(super) fn pressure_band(five_hour_status: i64, weekly_status: i64) -> i64 {
    prodex_mojo_core::quota::pressure_band(five_hour_status, weekly_status)
}

pub(super) fn window_pair_has_ready_limit(
    first_used_percent: Option<i64>,
    second_used_percent: Option<i64>,
) -> bool {
    prodex_mojo_core::quota::window_pair_has_ready_limit(first_used_percent, second_used_percent)
}
