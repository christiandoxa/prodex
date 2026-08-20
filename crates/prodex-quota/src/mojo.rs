unsafe extern "C" {
    fn prodex_quota_remaining_percent(used_percent: i64, has_value: i64) -> i64;
    fn prodex_quota_window_status(remaining_percent: i64, has_window: i64) -> i64;
    fn prodex_quota_pressure_band(five_hour_status: i64, weekly_status: i64) -> i64;
    fn prodex_quota_window_pair_has_ready_limit(
        first_used_percent: i64,
        first_has_value: i64,
        second_used_percent: i64,
        second_has_value: i64,
    ) -> i64;
}

pub(super) fn remaining_percent(used_percent: Option<i64>) -> i64 {
    let (used_percent, has_value) = match used_percent {
        Some(value) => (value, 1),
        None => (0, 0),
    };

    // SAFETY: build.rs links the scalar-only Mojo C-ABI object when this feature is enabled.
    unsafe { prodex_quota_remaining_percent(used_percent, has_value) }
}

pub(super) fn window_status(remaining_percent: i64, has_window: bool) -> i64 {
    // SAFETY: build.rs links the scalar-only Mojo C-ABI object when this feature is enabled.
    unsafe { prodex_quota_window_status(remaining_percent, i64::from(has_window)) }
}

pub(super) fn pressure_band(five_hour_status: i64, weekly_status: i64) -> i64 {
    // SAFETY: build.rs links the scalar-only Mojo C-ABI object when this feature is enabled.
    unsafe { prodex_quota_pressure_band(five_hour_status, weekly_status) }
}

pub(super) fn window_pair_has_ready_limit(
    first_used_percent: Option<i64>,
    second_used_percent: Option<i64>,
) -> bool {
    let (first_used_percent, first_has_value) =
        first_used_percent.map_or((0, 0), |value| (value, 1));
    let (second_used_percent, second_has_value) =
        second_used_percent.map_or((0, 0), |value| (value, 1));
    // SAFETY: build.rs links the scalar-only Mojo C-ABI object when this feature is enabled.
    unsafe {
        prodex_quota_window_pair_has_ready_limit(
            first_used_percent,
            first_has_value,
            second_used_percent,
            second_has_value,
        ) != 0
    }
}
