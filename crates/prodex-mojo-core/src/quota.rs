pub const QUOTA_MAIN_AGGREGATION_MAX_COUNT: usize = 1_024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MainQuotaAggregationInput {
    pub remaining_percent: Option<i64>,
    pub reset_at: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MainQuotaAggregation {
    pub profiles_with_data: usize,
    pub pool_remaining: i64,
    pub earliest_reset_at: Option<i64>,
}

pub fn self_test() -> bool {
    remaining_percent(Some(42)) == 58
        && window_status(5, true) == 2
        && pressure_band(1, 2) == 2
        && window_pair_has_ready_limit(Some(20), Some(30))
}

pub fn main_quota_aggregation_self_test() -> bool {
    main_quota_aggregate_batch(&[
        MainQuotaAggregationInput {
            remaining_percent: Some(80),
            reset_at: Some(20),
        },
        MainQuotaAggregationInput {
            remaining_percent: Some(30),
            reset_at: Some(10),
        },
    ])
    .is_some_and(|result| {
        result.profiles_with_data == 2
            && result.pool_remaining == 110
            && result.earliest_reset_at == Some(10)
    })
}

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
    fn prodex_quota_main_aggregate_batch(
        remaining_percent: *const i64,
        remaining_present: *const i64,
        reset_at: *const i64,
        reset_present: *const i64,
        profiles_with_data: *mut i64,
        pool_remaining: *mut i64,
        earliest_reset_at: *mut i64,
        earliest_present: *mut i64,
        count: i64,
    ) -> i64;
}

pub fn main_quota_aggregate_batch(
    inputs: &[MainQuotaAggregationInput],
) -> Option<MainQuotaAggregation> {
    if inputs.len() > QUOTA_MAIN_AGGREGATION_MAX_COUNT {
        return None;
    }

    let remaining_percent = inputs
        .iter()
        .map(|input| input.remaining_percent.unwrap_or_default())
        .collect::<Vec<_>>();
    let remaining_present = inputs
        .iter()
        .map(|input| i64::from(input.remaining_percent.is_some()))
        .collect::<Vec<_>>();
    let reset_at = inputs
        .iter()
        .map(|input| input.reset_at.unwrap_or_default())
        .collect::<Vec<_>>();
    let reset_present = inputs
        .iter()
        .map(|input| i64::from(input.reset_at.is_some()))
        .collect::<Vec<_>>();
    let mut profiles_with_data = 0_i64;
    let mut pool_remaining = 0_i64;
    let mut earliest_reset_at = 0_i64;
    let mut earliest_present = 0_i64;
    let status = unsafe {
        prodex_quota_main_aggregate_batch(
            remaining_percent.as_ptr(),
            remaining_present.as_ptr(),
            reset_at.as_ptr(),
            reset_present.as_ptr(),
            &mut profiles_with_data,
            &mut pool_remaining,
            &mut earliest_reset_at,
            &mut earliest_present,
            i64::try_from(inputs.len()).ok()?,
        )
    };
    if status == 0 && profiles_with_data >= 0 && matches!(earliest_present, 0 | 1) {
        return Some(MainQuotaAggregation {
            profiles_with_data: usize::try_from(profiles_with_data).ok()?,
            pool_remaining,
            earliest_reset_at: (earliest_present == 1).then_some(earliest_reset_at),
        });
    }
    None
}

pub fn remaining_percent(used_percent: Option<i64>) -> i64 {
    unsafe {
        prodex_quota_remaining_percent(used_percent.unwrap_or(0), i64::from(used_percent.is_some()))
    }
}

pub fn window_status(remaining_percent: i64, has_window: bool) -> i64 {
    unsafe { prodex_quota_window_status(remaining_percent, i64::from(has_window)) }
}

pub fn pressure_band(five_hour_status: i64, weekly_status: i64) -> i64 {
    unsafe { prodex_quota_pressure_band(five_hour_status, weekly_status) }
}

pub fn window_pair_has_ready_limit(first: Option<i64>, second: Option<i64>) -> bool {
    unsafe {
        prodex_quota_window_pair_has_ready_limit(
            first.unwrap_or(0),
            i64::from(first.is_some()),
            second.unwrap_or(0),
            i64::from(second.is_some()),
        ) != 0
    }
}
