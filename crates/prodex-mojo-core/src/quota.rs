pub const QUOTA_MAIN_AGGREGATION_MAX_COUNT: usize = 1_024;
pub const QUOTA_GEMINI_BUCKET_BATCH_MAX_COUNT: usize = 1_024;

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
    remaining_percent(Some(42)) == 58
        && window_status(5, true) == 2
        && pressure_band(1, 2) == 2
        && window_pair_has_ready_limit(Some(20), Some(30))
        && round_f64(1.5) == 2
        && round_f64(-0.5) == -1
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
    #[cfg(all(test, feature = "mojo-quota"))]
    fn prodex_quota_gemini_float_probe(first: f64, second: f64, operation: i64) -> f64;
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

#[cfg(all(test, feature = "mojo-quota"))]
fn gemini_float_probe(first: f64, second: f64, operation: i64) -> f64 {
    unsafe { prodex_quota_gemini_float_probe(first, second, operation) }
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
fn gemini_float_probe_matches_rust_f64_bits() {
    fn assert_same_bits(actual: f64, expected: f64, input: (f64, f64, i64)) {
        assert_eq!(actual.is_nan(), expected.is_nan(), "input={input:?}");
        if !expected.is_nan() {
            assert_eq!(actual.to_bits(), expected.to_bits(), "input={input:?}");
        }
    }

    let fractions = [
        0.0,
        -0.0,
        0.5,
        1.0,
        1.5,
        -1.0,
        2.0,
        f64::from_bits(1),
        -f64::from_bits(1),
        f64::MIN_POSITIVE,
        f64::MAX,
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
    ];
    for fraction in fractions {
        let input = (fraction, 0.0, 0);
        assert_same_bits(
            gemini_float_probe(input.0, input.1, input.2),
            fraction * 100.0,
            input,
        );
    }

    let amounts = [
        0.0,
        -0.0,
        1.0,
        50.0,
        -50.0,
        (i64::MAX as f64),
        (i64::MIN as f64),
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
    ];
    let divisors = [
        0.0,
        -0.0,
        0.5,
        1.0,
        0.005,
        0.015,
        f64::from_bits(1),
        -f64::from_bits(1),
        f64::MIN_POSITIVE,
        f64::MAX,
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
    ];
    for first in amounts {
        for second in divisors {
            for operation in [1, 2] {
                let input = (first, second, operation);
                let expected = match operation {
                    1 => first / second,
                    2 => first / second * 100.0,
                    _ => unreachable!(),
                };
                assert_same_bits(
                    gemini_float_probe(first, second, operation),
                    expected,
                    input,
                );
            }
        }
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
