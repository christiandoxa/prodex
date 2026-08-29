pub const QUOTA_MAIN_AGGREGATION_MAX_COUNT: usize = 1_024;
pub const QUOTA_GEMINI_BUCKET_BATCH_MAX_COUNT: usize = 1_024;
pub const QUOTA_CAPACITY_BATCH_MAX_COUNT: usize = 256;
pub const QUOTA_CAPACITY_FIELD_COUNT: usize = 11;

pub const QUOTA_CAPACITY_LANE_MAIN: i64 = 0;
pub const QUOTA_CAPACITY_LANE_SPARK: i64 = 1;
pub const QUOTA_CAPACITY_LANE_UNKNOWN_ADDITIONAL: i64 = 2;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuotaCapacityInput {
    pub lane: i64,
    pub allowed: i64,
    pub limit_reached: i64,
    pub five_hour_used_percent: i64,
    pub five_hour_has_value: bool,
    pub five_hour_seconds_until_reset: i64,
    pub weekly_used_percent: i64,
    pub weekly_has_value: bool,
    pub weekly_seconds_until_reset: i64,
    pub scale_bps: i64,
    pub weekly_weight: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuotaCapacityOutput {
    pub lane: i64,
    pub five_hour_remaining: i64,
    pub weekly_remaining: i64,
    pub five_hour_status: i64,
    pub weekly_status: i64,
    pub pressure_band: i64,
    pub admission_allowed: bool,
    pub pair_ready: bool,
    pub usable: bool,
    pub routing_eligible: bool,
    pub reserve_floor: i64,
    pub five_hour_pressure: i64,
    pub weekly_pressure: i64,
    pub total_pressure: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeminiRemainingAmount {
    Absent,
    Parsed(i64),
    Invalid,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GeminiBucketNumericInput {
    pub remaining_amount: GeminiRemainingAmount,
    pub remaining_fraction: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GeminiBucketNumericOutput {
    pub remaining: Option<i64>,
    pub total: Option<i64>,
    pub remaining_percent: Option<i64>,
    pub exhausted: bool,
}

pub fn self_test() -> bool {
    let gemini = gemini_bucket_numeric_batch(&[GeminiBucketNumericInput {
        remaining_amount: GeminiRemainingAmount::Parsed(50),
        remaining_fraction: Some(0.5),
    }])
    .is_ok_and(|outputs| {
        outputs
            == [GeminiBucketNumericOutput {
                remaining: Some(50),
                total: Some(100),
                remaining_percent: Some(50),
                exhausted: false,
            }]
    });
    let capacity = quota_capacity_batch(
        &[QuotaCapacityInput {
            lane: QUOTA_CAPACITY_LANE_MAIN,
            allowed: 0,
            limit_reached: 0,
            five_hour_used_percent: 10,
            five_hour_has_value: true,
            five_hour_seconds_until_reset: 0,
            weekly_used_percent: 20,
            weekly_has_value: true,
            weekly_seconds_until_reset: 0,
            scale_bps: 10_000,
            weekly_weight: 10,
        }],
        3,
    )
    .is_ok_and(|outputs| {
        outputs.len() == 1
            && outputs[0].five_hour_remaining == 90
            && outputs[0].weekly_remaining == 80
            && outputs[0].usable
            && outputs[0].routing_eligible
    });
    let window_pressure = quota_window_pressure(58, 1_700_003_600, 1_700_000_000)
        .is_ok_and(|pressure| pressure == 62_068);
    remaining_percent(Some(42)) == 58
        && window_status(5, true) == 2
        && pressure_band(1, 2) == 2
        && window_pair_has_ready_limit(Some(20), Some(30))
        && round_f64(1.5) == 2
        && round_f64(-0.5) == -1
        && capacity
        && window_pressure
        && gemini
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
    .is_ok_and(|result| {
        result.profiles_with_data == 2
            && result.pool_remaining == 110
            && result.earliest_reset_at == Some(10)
    })
}

unsafe extern "C" {
    fn prodex_quota_round_f64(value: f64) -> i64;
    fn prodex_quota_remaining_percent(used_percent: i64, has_value: i64) -> i64;
    fn prodex_quota_window_status(remaining_percent: i64, has_window: i64) -> i64;
    fn prodex_quota_pressure_band(five_hour_status: i64, weekly_status: i64) -> i64;
    fn prodex_quota_window_pair_has_ready_limit(
        first_used_percent: i64,
        first_has_value: i64,
        second_used_percent: i64,
        second_has_value: i64,
    ) -> i64;
    fn prodex_quota_gemini_bucket_batch(
        remaining_amount: *const i64,
        remaining_amount_state: *const i64,
        remaining_fraction: *const f64,
        remaining_fraction_present: *const i64,
        remaining: *mut i64,
        remaining_present: *mut i64,
        total: *mut i64,
        total_present: *mut i64,
        remaining_percent: *mut i64,
        remaining_percent_present: *mut i64,
        exhausted: *mut i64,
        count: i64,
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
    fn prodex_quota_capacity_batch(
        fields_address: u64,
        lane_address: u64,
        five_hour_remaining_address: u64,
        weekly_remaining_address: u64,
        five_hour_status_address: u64,
        weekly_status_address: u64,
        pressure_band_address: u64,
        admission_allowed_address: u64,
        pair_ready_address: u64,
        usable_address: u64,
        routing_eligible_address: u64,
        reserve_floor_address: u64,
        five_hour_pressure_address: u64,
        weekly_pressure_address: u64,
        total_pressure_address: u64,
        route_kind: i64,
        count: i64,
    ) -> i64;
    fn prodex_quota_window_pressure(remaining_percent: i64, reset_at: i64, now: i64) -> i64;
}

pub fn round_f64(value: f64) -> i64 {
    unsafe { prodex_quota_round_f64(value) }
}

pub fn gemini_bucket_numeric_batch(
    inputs: &[GeminiBucketNumericInput],
) -> Result<Vec<GeminiBucketNumericOutput>, crate::MojoError> {
    if inputs.len() > QUOTA_GEMINI_BUCKET_BATCH_MAX_COUNT {
        return Err(crate::MojoError::InvalidInput);
    }

    let remaining_amount = inputs
        .iter()
        .map(|input| match input.remaining_amount {
            GeminiRemainingAmount::Parsed(value) => value,
            GeminiRemainingAmount::Absent | GeminiRemainingAmount::Invalid => 0,
        })
        .collect::<Vec<_>>();
    let remaining_amount_state = inputs
        .iter()
        .map(|input| match input.remaining_amount {
            GeminiRemainingAmount::Absent => 0,
            GeminiRemainingAmount::Parsed(_) => 1,
            GeminiRemainingAmount::Invalid => 2,
        })
        .collect::<Vec<_>>();
    let remaining_fraction = inputs
        .iter()
        .map(|input| input.remaining_fraction.unwrap_or_default())
        .collect::<Vec<_>>();
    let remaining_fraction_present = inputs
        .iter()
        .map(|input| i64::from(input.remaining_fraction.is_some()))
        .collect::<Vec<_>>();
    let mut remaining = vec![0_i64; inputs.len()];
    let mut remaining_present = vec![0_i64; inputs.len()];
    let mut total = vec![0_i64; inputs.len()];
    let mut total_present = vec![0_i64; inputs.len()];
    let mut remaining_percent = vec![0_i64; inputs.len()];
    let mut remaining_percent_present = vec![0_i64; inputs.len()];
    let mut exhausted = vec![0_i64; inputs.len()];
    let status = unsafe {
        prodex_quota_gemini_bucket_batch(
            remaining_amount.as_ptr(),
            remaining_amount_state.as_ptr(),
            remaining_fraction.as_ptr(),
            remaining_fraction_present.as_ptr(),
            remaining.as_mut_ptr(),
            remaining_present.as_mut_ptr(),
            total.as_mut_ptr(),
            total_present.as_mut_ptr(),
            remaining_percent.as_mut_ptr(),
            remaining_percent_present.as_mut_ptr(),
            exhausted.as_mut_ptr(),
            i64::try_from(inputs.len()).map_err(|_| crate::MojoError::InvalidInput)?,
        )
    };
    if status != 0 {
        return Err(if status == 1 {
            crate::MojoError::InvalidInput
        } else {
            crate::MojoError::InvalidOutput
        });
    }

    let mut outputs = Vec::with_capacity(inputs.len());
    for index in 0..inputs.len() {
        if !matches!(
            (
                remaining_present[index],
                total_present[index],
                remaining_percent_present[index],
                exhausted[index],
            ),
            (0 | 1, 0 | 1, 0 | 1, 0 | 1)
        ) {
            return Err(crate::MojoError::InvalidOutput);
        }
        outputs.push(GeminiBucketNumericOutput {
            remaining: (remaining_present[index] == 1).then_some(remaining[index]),
            total: (total_present[index] == 1).then_some(total[index]),
            remaining_percent: (remaining_percent_present[index] == 1)
                .then_some(remaining_percent[index]),
            exhausted: exhausted[index] == 1,
        });
    }
    Ok(outputs)
}

pub fn quota_capacity_batch(
    inputs: &[QuotaCapacityInput],
    route_kind: i64,
) -> Result<Vec<QuotaCapacityOutput>, crate::MojoError> {
    if inputs.len() > QUOTA_CAPACITY_BATCH_MAX_COUNT
        || !(0..=3).contains(&route_kind)
        || inputs.iter().any(|input| {
            !(QUOTA_CAPACITY_LANE_MAIN..=QUOTA_CAPACITY_LANE_UNKNOWN_ADDITIONAL)
                .contains(&input.lane)
                || !(0..=2).contains(&input.allowed)
                || !(0..=2).contains(&input.limit_reached)
                || input.five_hour_seconds_until_reset < 0
                || input.weekly_seconds_until_reset < 0
                || input.scale_bps < 0
                || input.weekly_weight < 0
        })
    {
        return Err(crate::MojoError::InvalidInput);
    }

    let mut fields = Vec::with_capacity(inputs.len() * QUOTA_CAPACITY_FIELD_COUNT);
    for input in inputs {
        fields.extend([
            input.lane,
            input.allowed,
            input.limit_reached,
            input.five_hour_used_percent,
            i64::from(input.five_hour_has_value),
            input.five_hour_seconds_until_reset,
            input.weekly_used_percent,
            i64::from(input.weekly_has_value),
            input.weekly_seconds_until_reset,
            input.scale_bps,
            input.weekly_weight,
        ]);
    }

    let mut lane = vec![0_i64; inputs.len()];
    let mut five_hour_remaining = vec![0_i64; inputs.len()];
    let mut weekly_remaining = vec![0_i64; inputs.len()];
    let mut five_hour_status = vec![0_i64; inputs.len()];
    let mut weekly_status = vec![0_i64; inputs.len()];
    let mut pressure_band = vec![0_i64; inputs.len()];
    let mut admission_allowed = vec![0_i64; inputs.len()];
    let mut pair_ready = vec![0_i64; inputs.len()];
    let mut usable = vec![0_i64; inputs.len()];
    let mut routing_eligible = vec![0_i64; inputs.len()];
    let mut reserve_floor = vec![0_i64; inputs.len()];
    let mut five_hour_pressure = vec![0_i64; inputs.len()];
    let mut weekly_pressure = vec![0_i64; inputs.len()];
    let mut total_pressure = vec![0_i64; inputs.len()];
    let status = unsafe {
        prodex_quota_capacity_batch(
            fields.as_ptr() as u64,
            lane.as_mut_ptr() as u64,
            five_hour_remaining.as_mut_ptr() as u64,
            weekly_remaining.as_mut_ptr() as u64,
            five_hour_status.as_mut_ptr() as u64,
            weekly_status.as_mut_ptr() as u64,
            pressure_band.as_mut_ptr() as u64,
            admission_allowed.as_mut_ptr() as u64,
            pair_ready.as_mut_ptr() as u64,
            usable.as_mut_ptr() as u64,
            routing_eligible.as_mut_ptr() as u64,
            reserve_floor.as_mut_ptr() as u64,
            five_hour_pressure.as_mut_ptr() as u64,
            weekly_pressure.as_mut_ptr() as u64,
            total_pressure.as_mut_ptr() as u64,
            route_kind,
            i64::try_from(inputs.len()).map_err(|_| crate::MojoError::InvalidInput)?,
        )
    };
    if status != 0 {
        return Err(if status == 1 {
            crate::MojoError::InvalidInput
        } else {
            crate::MojoError::InvalidOutput
        });
    }

    let mut outputs = Vec::with_capacity(inputs.len());
    for index in 0..inputs.len() {
        if lane[index] != inputs[index].lane
            || !(0..=100).contains(&five_hour_remaining[index])
            || !(0..=100).contains(&weekly_remaining[index])
            || !(0..=4).contains(&five_hour_status[index])
            || !(0..=4).contains(&weekly_status[index])
            || !(0..=4).contains(&pressure_band[index])
            || !matches!(
                (
                    admission_allowed[index],
                    pair_ready[index],
                    usable[index],
                    routing_eligible[index]
                ),
                (0 | 1, 0 | 1, 0 | 1, 0 | 1)
            )
            || reserve_floor[index] < 0
            || reserve_floor[index] > 100
            || five_hour_pressure[index] < 0
            || weekly_pressure[index] < 0
            || total_pressure[index] < 0
            || usable[index] != admission_allowed[index] * pair_ready[index]
            || routing_eligible[index]
                != i64::from(
                    usable[index] == 1
                        && matches!(
                            inputs[index].lane,
                            QUOTA_CAPACITY_LANE_MAIN | QUOTA_CAPACITY_LANE_SPARK
                        ),
                )
        {
            return Err(crate::MojoError::InvalidOutput);
        }
        outputs.push(QuotaCapacityOutput {
            lane: lane[index],
            five_hour_remaining: five_hour_remaining[index],
            weekly_remaining: weekly_remaining[index],
            five_hour_status: five_hour_status[index],
            weekly_status: weekly_status[index],
            pressure_band: pressure_band[index],
            admission_allowed: admission_allowed[index] == 1,
            pair_ready: pair_ready[index] == 1,
            usable: usable[index] == 1,
            routing_eligible: routing_eligible[index] == 1,
            reserve_floor: reserve_floor[index],
            five_hour_pressure: five_hour_pressure[index],
            weekly_pressure: weekly_pressure[index],
            total_pressure: total_pressure[index],
        });
    }
    Ok(outputs)
}

pub fn quota_window_pressure(
    remaining_percent: i64,
    reset_at: i64,
    now: i64,
) -> Result<i64, crate::MojoError> {
    if !(0..=100).contains(&remaining_percent) {
        return Err(crate::MojoError::InvalidInput);
    }
    let pressure_score = unsafe { prodex_quota_window_pressure(remaining_percent, reset_at, now) };
    if pressure_score < 0 {
        return Err(crate::MojoError::InvalidOutput);
    }
    Ok(pressure_score)
}

pub fn main_quota_aggregate_batch(
    inputs: &[MainQuotaAggregationInput],
) -> Result<MainQuotaAggregation, crate::MojoError> {
    if inputs.len() > QUOTA_MAIN_AGGREGATION_MAX_COUNT {
        return Err(crate::MojoError::InvalidInput);
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
            i64::try_from(inputs.len()).map_err(|_| crate::MojoError::InvalidInput)?,
        )
    };
    if status != 0
        || profiles_with_data < 0
        || usize::try_from(profiles_with_data)
            .ok()
            .is_none_or(|count| count > inputs.len())
        || !matches!(earliest_present, 0 | 1)
    {
        return Err(crate::MojoError::InvalidOutput);
    }
    Ok(MainQuotaAggregation {
        profiles_with_data: usize::try_from(profiles_with_data)
            .map_err(|_| crate::MojoError::InvalidOutput)?,
        pool_remaining,
        earliest_reset_at: (earliest_present == 1).then_some(earliest_reset_at),
    })
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

#[cfg(all(test, feature = "mojo-quota"))]
#[test]
fn round_f64_matches_rust_float_to_int_semantics() {
    for value in [
        0.0,
        -0.0,
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
        -2.5,
        -2.499_999_999,
        -0.500_000_001,
        -0.5,
        -0.499_999_999,
        0.000_000_001,
        0.499_999_999,
        0.5,
        0.500_000_001,
        1.5,
        2.5,
        (i64::MAX as f64) * 0.5,
        i64::MAX as f64,
        i64::MIN as f64,
    ] {
        assert_eq!(round_f64(value), value.round() as i64, "value={value:?}");
    }
}

#[cfg(all(test, feature = "mojo-quota"))]
#[test]
fn gemini_bucket_batch_preserves_normalized_presence_states() {
    let outputs = gemini_bucket_numeric_batch(&[
        GeminiBucketNumericInput {
            remaining_amount: GeminiRemainingAmount::Parsed(50),
            remaining_fraction: Some(0.5),
        },
        GeminiBucketNumericInput {
            remaining_amount: GeminiRemainingAmount::Absent,
            remaining_fraction: Some(0.5),
        },
        GeminiBucketNumericInput {
            remaining_amount: GeminiRemainingAmount::Parsed(50),
            remaining_fraction: None,
        },
        GeminiBucketNumericInput {
            remaining_amount: GeminiRemainingAmount::Invalid,
            remaining_fraction: Some(0.5),
        },
        GeminiBucketNumericInput {
            remaining_amount: GeminiRemainingAmount::Parsed(0),
            remaining_fraction: Some(0.0),
        },
        GeminiBucketNumericInput {
            remaining_amount: GeminiRemainingAmount::Absent,
            remaining_fraction: Some(f64::NAN),
        },
        GeminiBucketNumericInput {
            remaining_amount: GeminiRemainingAmount::Parsed(50),
            remaining_fraction: Some(2.0),
        },
    ])
    .expect("valid normalized Gemini input");
    assert_eq!(
        outputs,
        [
            GeminiBucketNumericOutput {
                remaining: Some(50),
                total: Some(100),
                remaining_percent: Some(50),
                exhausted: false,
            },
            GeminiBucketNumericOutput {
                remaining: Some(50),
                total: Some(100),
                remaining_percent: Some(50),
                exhausted: false,
            },
            GeminiBucketNumericOutput {
                remaining: Some(50),
                total: None,
                remaining_percent: None,
                exhausted: false,
            },
            GeminiBucketNumericOutput {
                remaining: None,
                total: None,
                remaining_percent: Some(50),
                exhausted: false,
            },
            GeminiBucketNumericOutput {
                remaining: Some(0),
                total: None,
                remaining_percent: Some(0),
                exhausted: true,
            },
            GeminiBucketNumericOutput {
                remaining: Some(0),
                total: Some(100),
                remaining_percent: Some(0),
                exhausted: true,
            },
            GeminiBucketNumericOutput {
                remaining: Some(50),
                total: None,
                remaining_percent: Some(200),
                exhausted: false,
            },
        ]
    );
}
