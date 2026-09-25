mod capacity;
pub use capacity::quota_capacity_batch;

pub const QUOTA_MAIN_AGGREGATION_MAX_COUNT: usize = 1_024;
pub const QUOTA_GEMINI_BUCKET_BATCH_MAX_COUNT: usize = 1_024;
pub const QUOTA_CAPACITY_BATCH_MAX_COUNT: usize = 256;
pub const QUOTA_CAPACITY_FIELD_COUNT: usize = 22;

pub const QUOTA_CAPACITY_LANE_MAIN: i64 = 0;
pub const QUOTA_CAPACITY_LANE_MODEL_SPECIFIC: i64 = 1;
pub const QUOTA_CAPACITY_LANE_UNKNOWN_ADDITIONAL: i64 = 2;

pub const QUOTA_MODEL_KIND_NONE: i64 = 0;
pub const QUOTA_MODEL_KIND_LUNA: i64 = 1;
pub const QUOTA_MODEL_KIND_RETIRED_SPARK: i64 = 2;
pub const QUOTA_MODEL_KIND_OTHER: i64 = 3;

pub const QUOTA_MODEL_PAIR_NONE: i64 = 0;
pub const QUOTA_MODEL_PAIR_REGULAR: i64 = 1;
pub const QUOTA_MODEL_PAIR_RESERVE: i64 = 2;
pub const QUOTA_MODEL_PAIR_DEFAULT: i64 = 3;

pub const QUOTA_ERROR_KIND_UNKNOWN: i64 = 0;
pub const QUOTA_ERROR_KIND_UNAVAILABLE: i64 = 1;
pub const QUOTA_ERROR_KIND_CONFIG: i64 = 2;
pub const QUOTA_ERROR_KIND_SERVER: i64 = 3;
pub const QUOTA_ERROR_KIND_TIMEOUT: i64 = 4;
pub const QUOTA_ERROR_KIND_NETWORK: i64 = 5;
pub const QUOTA_ERROR_KIND_PROXY: i64 = 6;
pub const QUOTA_ERROR_KIND_CONNECTION: i64 = 7;
pub const QUOTA_ERROR_KIND_INVALID_AUTH: i64 = 8;
pub const QUOTA_ERROR_KIND_RATE_LIMIT: i64 = 9;
pub const QUOTA_ERROR_KIND_PARSE: i64 = 10;
pub const QUOTA_ERROR_KIND_EMPTY: i64 = 11;
pub const QUOTA_ERROR_KIND_CANCELLED: i64 = 12;
pub const QUOTA_ERROR_KIND_FORBIDDEN: i64 = 13;
pub const QUOTA_ERROR_KIND_NOT_FOUND: i64 = 14;
pub const QUOTA_ERROR_KIND_OTHER: i64 = 15;

pub const QUOTA_BLOCKED_KIND_NONE: i64 = 0;
pub const QUOTA_BLOCKED_KIND_EXHAUSTED: i64 = 1;
pub const QUOTA_BLOCKED_KIND_WEEKLY: i64 = 2;
pub const QUOTA_BLOCKED_KIND_FIVE_HOUR: i64 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpenAiModelCapacityInput {
    pub model_kind: i64,
    pub regular_present: bool,
    pub regular_ready: bool,
    pub generic_ready: bool,
    pub reserve_ready: bool,
    pub regular_blocked: bool,
    pub any_unknown_window: bool,
    pub any_exhausted_window: bool,
    pub include_code_review: bool,
    pub code_review_ready: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpenAiModelCapacityPlan {
    pub selected_pair: i64,
    pub ready: bool,
    pub supports: bool,
    pub unknown_luna_capacity: bool,
}

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

/// Shape of an admission field preserved from the upstream quota payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuotaAdmissionValue {
    Missing,
    Null,
    True,
    False,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuotaCapacityInput {
    pub lane: i64,
    pub pair_allowed: Option<bool>,
    pub outer_allowed: Option<bool>,
    pub pair_limit_reached: Option<bool>,
    pub outer_limit_reached: Option<bool>,
    pub rate_limit_reached_type: QuotaAdmissionValue,
    pub camel_rate_limit_reached_type: QuotaAdmissionValue,
    pub spend_control_reached: QuotaAdmissionValue,
    pub camel_spend_control_reached: QuotaAdmissionValue,
    pub ordinary_usage_allowed: QuotaAdmissionValue,
    pub five_hour_used_percent: i64,
    pub five_hour_has_value: bool,
    pub five_hour_reset_at: i64,
    pub weekly_used_percent: i64,
    pub weekly_has_value: bool,
    pub weekly_reset_at: i64,
    pub primary_used_percent: i64,
    pub primary_has_value: bool,
    pub secondary_used_percent: i64,
    pub secondary_has_value: bool,
    pub scale_bps: i64,
    pub now: i64,
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
    pub any_window_exhausted: bool,
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
            pair_allowed: None,
            outer_allowed: None,
            pair_limit_reached: None,
            outer_limit_reached: None,
            rate_limit_reached_type: QuotaAdmissionValue::Missing,
            camel_rate_limit_reached_type: QuotaAdmissionValue::Missing,
            spend_control_reached: QuotaAdmissionValue::Missing,
            camel_spend_control_reached: QuotaAdmissionValue::Missing,
            ordinary_usage_allowed: QuotaAdmissionValue::Missing,
            five_hour_used_percent: 10,
            five_hour_has_value: true,
            five_hour_reset_at: 0,
            weekly_used_percent: 20,
            weekly_has_value: true,
            weekly_reset_at: 0,
            primary_used_percent: 10,
            primary_has_value: true,
            secondary_used_percent: 20,
            secondary_has_value: true,
            scale_bps: 10_000,
            now: 0,
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
    fn prodex_quota_capacity_batch_v2(
        fields_address: u64,
        lane_address: u64,
        five_hour_remaining_address: u64,
        weekly_remaining_address: u64,
        five_hour_status_address: u64,
        weekly_status_address: u64,
        pressure_band_address: u64,
        admission_allowed_address: u64,
        pair_ready_address: u64,
        any_window_exhausted_address: u64,
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
    fn prodex_quota_openai_model_kind(address: u64, length: i64, present: i64) -> i64;
    fn prodex_quota_luna_reserve_identifier(
        model_slug_address: u64,
        model_slug_length: i64,
        limit_id_address: u64,
        limit_id_length: i64,
        limit_name_address: u64,
        limit_name_length: i64,
        metered_feature_address: u64,
        metered_feature_length: i64,
    ) -> i64;
    fn prodex_quota_openai_model_capacity_plan(fields_address: u64, output_address: u64) -> i64;
    fn prodex_quota_error_summary_kind(address: u64, length: i64) -> i64;
    fn prodex_quota_blocked_limit_kind(address: u64, length: i64) -> i64;
}

fn quota_text_address(value: Option<&str>) -> (u64, i64) {
    match value {
        Some(value) => (
            value.as_ptr() as usize as u64,
            i64::try_from(value.len()).unwrap_or(i64::MAX),
        ),
        None => (0, 0),
    }
}

pub fn openai_model_kind(model: Option<&str>) -> Result<i64, crate::MojoError> {
    let (address, length) = quota_text_address(model);
    if length == i64::MAX {
        return Err(crate::MojoError::InvalidInput);
    }
    let kind =
        unsafe { prodex_quota_openai_model_kind(address, length, i64::from(model.is_some())) };
    if matches!(
        kind,
        QUOTA_MODEL_KIND_NONE
            | QUOTA_MODEL_KIND_LUNA
            | QUOTA_MODEL_KIND_RETIRED_SPARK
            | QUOTA_MODEL_KIND_OTHER
    ) {
        Ok(kind)
    } else {
        Err(crate::MojoError::InvalidOutput)
    }
}

pub fn luna_reserve_identifier(
    model_slug: Option<&str>,
    limit_id: Option<&str>,
    limit_name: Option<&str>,
    metered_feature: Option<&str>,
) -> Result<bool, crate::MojoError> {
    let (model_slug_address, model_slug_length) = quota_text_address(model_slug);
    let (limit_id_address, limit_id_length) = quota_text_address(limit_id);
    let (limit_name_address, limit_name_length) = quota_text_address(limit_name);
    let (metered_feature_address, metered_feature_length) = quota_text_address(metered_feature);
    if [
        model_slug_length,
        limit_id_length,
        limit_name_length,
        metered_feature_length,
    ]
    .contains(&i64::MAX)
    {
        return Err(crate::MojoError::InvalidInput);
    }
    let result = unsafe {
        prodex_quota_luna_reserve_identifier(
            model_slug_address,
            model_slug_length,
            limit_id_address,
            limit_id_length,
            limit_name_address,
            limit_name_length,
            metered_feature_address,
            metered_feature_length,
        )
    };
    match result {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(crate::MojoError::InvalidOutput),
    }
}

pub fn openai_model_capacity_plan(
    input: OpenAiModelCapacityInput,
) -> Result<OpenAiModelCapacityPlan, crate::MojoError> {
    if !matches!(
        input.model_kind,
        QUOTA_MODEL_KIND_NONE
            | QUOTA_MODEL_KIND_LUNA
            | QUOTA_MODEL_KIND_RETIRED_SPARK
            | QUOTA_MODEL_KIND_OTHER
    ) {
        return Err(crate::MojoError::InvalidInput);
    }
    let fields = [
        input.model_kind,
        i64::from(input.regular_present),
        i64::from(input.regular_ready),
        i64::from(input.generic_ready),
        i64::from(input.reserve_ready),
        i64::from(input.regular_blocked),
        i64::from(input.any_unknown_window),
        i64::from(input.any_exhausted_window),
        i64::from(input.include_code_review),
        i64::from(input.code_review_ready),
    ];
    let mut output = [0_i64; 4];
    let status = unsafe {
        prodex_quota_openai_model_capacity_plan(
            fields.as_ptr() as usize as u64,
            output.as_mut_ptr() as usize as u64,
        )
    };
    if status != 0
        || !matches!(
            output[0],
            QUOTA_MODEL_PAIR_NONE
                | QUOTA_MODEL_PAIR_REGULAR
                | QUOTA_MODEL_PAIR_RESERVE
                | QUOTA_MODEL_PAIR_DEFAULT
        )
        || !matches!((output[1], output[2], output[3]), (0 | 1, 0 | 1, 0 | 1))
    {
        return Err(if status == 1 {
            crate::MojoError::InvalidInput
        } else {
            crate::MojoError::InvalidOutput
        });
    }
    Ok(OpenAiModelCapacityPlan {
        selected_pair: output[0],
        ready: output[1] == 1,
        supports: output[2] == 1,
        unknown_luna_capacity: output[3] == 1,
    })
}

pub fn quota_error_summary_kind(value: &str) -> Result<i64, crate::MojoError> {
    let (address, length) = quota_text_address(Some(value));
    if length == i64::MAX {
        return Err(crate::MojoError::InvalidInput);
    }
    let kind = unsafe { prodex_quota_error_summary_kind(address, length) };
    if (QUOTA_ERROR_KIND_UNKNOWN..=QUOTA_ERROR_KIND_OTHER).contains(&kind) {
        Ok(kind)
    } else {
        Err(crate::MojoError::InvalidOutput)
    }
}

pub fn blocked_limit_kind(value: &str) -> Result<i64, crate::MojoError> {
    let (address, length) = quota_text_address(Some(value));
    if length == i64::MAX {
        return Err(crate::MojoError::InvalidInput);
    }
    let kind = unsafe { prodex_quota_blocked_limit_kind(address, length) };
    if (QUOTA_BLOCKED_KIND_NONE..=QUOTA_BLOCKED_KIND_FIVE_HOUR).contains(&kind) {
        Ok(kind)
    } else {
        Err(crate::MojoError::InvalidOutput)
    }
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
