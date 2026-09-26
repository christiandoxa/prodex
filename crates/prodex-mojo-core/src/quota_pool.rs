//! OpenAI quota-pool aggregation over normalized window snapshots.

use crate::MojoError;

const INPUT_FIELD_COUNT: usize = 7;
const OUTPUT_FIELD_COUNT: usize = 14;
const STATUS_QUOTA_SUMMARY_ABI_VERSION: i64 = 1;
const STATUS_QUOTA_SUMMARY_INPUT_FIELD_COUNT: usize = 15;
const STATUS_QUOTA_SUMMARY_OUTPUT_FIELD_COUNT: usize = 10;

/// One already-classified quota window supplied to pool aggregation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuotaPoolWindowInput {
    pub remaining_percent: i64,
    /// `i64::MAX` means the window has no known reset.
    pub reset_at: i64,
}

/// One OpenAI profile's normalized main quota windows and readiness.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpenAiQuotaPoolInput {
    pub five_hour: Option<QuotaPoolWindowInput>,
    pub weekly: Option<QuotaPoolWindowInput>,
    pub ready: bool,
}

/// Aggregated OpenAI pool counts, remaining percentages, and reset times.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct OpenAiQuotaPoolAggregation {
    pub profiles_with_data: usize,
    pub ready_profiles_with_data: usize,
    pub five_hour_profiles_with_data: usize,
    pub weekly_profiles_with_data: usize,
    pub ready_five_hour_profiles_with_data: usize,
    pub ready_weekly_profiles_with_data: usize,
    pub five_hour_pool_remaining: i64,
    pub weekly_pool_remaining: i64,
    pub ready_five_hour_pool_remaining: i64,
    pub ready_weekly_pool_remaining: i64,
    pub earliest_five_hour_reset_at: Option<i64>,
    pub earliest_weekly_reset_at: Option<i64>,
}

/// Per-profile status inputs after host data has been normalized into quota windows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatusQuotaProfileInput {
    pub quota_compatible: bool,
    pub report_succeeded: bool,
    pub cached_snapshot_usable: bool,
    pub report_five_hour: Option<QuotaPoolWindowInput>,
    pub report_weekly: Option<QuotaPoolWindowInput>,
    pub cached_five_hour: Option<QuotaPoolWindowInput>,
    pub cached_weekly: Option<QuotaPoolWindowInput>,
}

/// Aggregated data for one status quota window.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StatusQuotaWindowAggregation {
    pub profiles: usize,
    pub total_remaining: i64,
    pub earliest_reset_at: Option<i64>,
}

/// Aggregated quota data shown by `prodex status`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StatusQuotaSummary {
    pub compatible_profiles: usize,
    pub unavailable_profiles: usize,
    pub five_hour: StatusQuotaWindowAggregation,
    pub weekly: StatusQuotaWindowAggregation,
}

unsafe extern "C" {
    fn prodex_quota_openai_pool_aggregate_v1(
        input_fields: *const i64,
        output_fields: *mut i64,
        count: i64,
    ) -> i64;
    fn prodex_quota_status_summary_v1(
        abi_version: i64,
        input_fields: *const i64,
        output_fields: *mut i64,
        count: i64,
    ) -> i64;
}

/// Aggregate normalized windows without imposing a profile-count cap.
pub fn openai_quota_pool_aggregate(
    inputs: &[OpenAiQuotaPoolInput],
) -> Result<OpenAiQuotaPoolAggregation, MojoError> {
    let count = i64::try_from(inputs.len()).map_err(|_| MojoError::InvalidInput)?;
    let field_count = inputs
        .len()
        .checked_mul(INPUT_FIELD_COUNT)
        .ok_or(MojoError::Capacity)?;
    let mut fields = Vec::new();
    fields
        .try_reserve_exact(field_count)
        .map_err(|_| MojoError::Capacity)?;
    for input in inputs {
        for window in [input.five_hour, input.weekly] {
            let (remaining, present, reset_at) = window.map_or((0, 0, i64::MAX), |window| {
                (window.remaining_percent, 1, window.reset_at)
            });
            if present == 1 && !(0..=100).contains(&remaining) {
                return Err(MojoError::InvalidInput);
            }
            fields.extend([remaining, present, reset_at]);
        }
        fields.push(i64::from(input.ready));
    }

    let mut output = [0_i64; OUTPUT_FIELD_COUNT];
    let status = unsafe {
        prodex_quota_openai_pool_aggregate_v1(fields.as_ptr(), output.as_mut_ptr(), count)
    };
    let count_limit = count;
    let max_remaining = count_limit.saturating_mul(100);
    if status != 0
        || output[..6]
            .iter()
            .any(|value| *value < 0 || *value > count_limit)
        || output[1] > output[0]
        || output[2] > output[0]
        || output[3] > output[0]
        || output[4] > output[2]
        || output[5] > output[3]
        || output[6..10]
            .iter()
            .any(|value| *value < 0 || *value > max_remaining)
        || output[8] > output[6]
        || output[9] > output[7]
        || !matches!(output[11], 0 | 1)
        || !matches!(output[13], 0 | 1)
        || (output[11] == 0 && output[10] != 0)
        || (output[11] == 1 && output[10] == i64::MAX)
        || (output[13] == 0 && output[12] != 0)
        || (output[13] == 1 && output[12] == i64::MAX)
    {
        return Err(MojoError::InvalidOutput);
    }
    Ok(OpenAiQuotaPoolAggregation {
        profiles_with_data: usize::try_from(output[0]).map_err(|_| MojoError::InvalidOutput)?,
        ready_profiles_with_data: usize::try_from(output[1])
            .map_err(|_| MojoError::InvalidOutput)?,
        five_hour_profiles_with_data: usize::try_from(output[2])
            .map_err(|_| MojoError::InvalidOutput)?,
        weekly_profiles_with_data: usize::try_from(output[3])
            .map_err(|_| MojoError::InvalidOutput)?,
        ready_five_hour_profiles_with_data: usize::try_from(output[4])
            .map_err(|_| MojoError::InvalidOutput)?,
        ready_weekly_profiles_with_data: usize::try_from(output[5])
            .map_err(|_| MojoError::InvalidOutput)?,
        five_hour_pool_remaining: output[6],
        weekly_pool_remaining: output[7],
        ready_five_hour_pool_remaining: output[8],
        ready_weekly_pool_remaining: output[9],
        earliest_five_hour_reset_at: (output[11] == 1).then_some(output[10]),
        earliest_weekly_reset_at: (output[13] == 1).then_some(output[12]),
    })
}

/// Aggregate status profile availability and normalized quota windows in Mojo.
pub fn status_quota_summary_batch(
    inputs: &[StatusQuotaProfileInput],
) -> Result<StatusQuotaSummary, MojoError> {
    let count = i64::try_from(inputs.len()).map_err(|_| MojoError::InvalidInput)?;
    let field_count = inputs
        .len()
        .checked_mul(STATUS_QUOTA_SUMMARY_INPUT_FIELD_COUNT)
        .ok_or(MojoError::Capacity)?;
    let mut fields = Vec::new();
    fields
        .try_reserve_exact(field_count)
        .map_err(|_| MojoError::Capacity)?;
    for input in inputs {
        fields.extend([
            i64::from(input.quota_compatible),
            i64::from(input.report_succeeded),
            i64::from(input.cached_snapshot_usable),
        ]);
        for window in [
            input.report_five_hour,
            input.report_weekly,
            input.cached_five_hour,
            input.cached_weekly,
        ] {
            if window.is_some_and(|window| window.remaining_percent < 0) {
                return Err(MojoError::InvalidInput);
            }
            let (remaining, present, reset_at) = window.map_or((0, 0, i64::MAX), |window| {
                (window.remaining_percent, 1, window.reset_at)
            });
            fields.extend([remaining, present, reset_at]);
        }
    }

    let mut output = [0_i64; STATUS_QUOTA_SUMMARY_OUTPUT_FIELD_COUNT];
    let status = unsafe {
        prodex_quota_status_summary_v1(
            STATUS_QUOTA_SUMMARY_ABI_VERSION,
            fields.as_ptr(),
            output.as_mut_ptr(),
            count,
        )
    };
    let count_limit = count;
    if status != 0
        || [output[0], output[1], output[2], output[6]]
            .into_iter()
            .any(|value| value < 0 || value > count_limit)
        || output[1] > output[0]
        || output[2] > output[0]
        || output[6] > output[0]
        || !matches!(output[5], 0 | 1)
        || !matches!(output[9], 0 | 1)
        || (output[5] == 0 && output[4] != 0)
        || (output[5] == 1 && output[4] == i64::MAX)
        || (output[9] == 0 && output[8] != 0)
        || (output[9] == 1 && output[8] == i64::MAX)
    {
        return Err(MojoError::InvalidOutput);
    }
    Ok(StatusQuotaSummary {
        compatible_profiles: usize::try_from(output[0]).map_err(|_| MojoError::InvalidOutput)?,
        unavailable_profiles: usize::try_from(output[1]).map_err(|_| MojoError::InvalidOutput)?,
        five_hour: StatusQuotaWindowAggregation {
            profiles: usize::try_from(output[2]).map_err(|_| MojoError::InvalidOutput)?,
            total_remaining: output[3],
            earliest_reset_at: (output[5] == 1).then_some(output[4]),
        },
        weekly: StatusQuotaWindowAggregation {
            profiles: usize::try_from(output[6]).map_err(|_| MojoError::InvalidOutput)?,
            total_remaining: output[7],
            earliest_reset_at: (output[9] == 1).then_some(output[8]),
        },
    })
}
