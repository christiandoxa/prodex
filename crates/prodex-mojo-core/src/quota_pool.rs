//! OpenAI quota-pool aggregation over normalized window snapshots.

use crate::MojoError;

const INPUT_FIELD_COUNT: usize = 7;
const OUTPUT_FIELD_COUNT: usize = 14;
const STATUS_QUOTA_SUMMARY_ABI_VERSION: i64 = 2;
const STATUS_QUOTA_SUMMARY_INPUT_FIELD_COUNT: usize = 15;
const STATUS_QUOTA_SUMMARY_OUTPUT_FIELD_COUNT: usize = 12;

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

/// Aggregated quota windows and source counts for Info and status reports.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StatusQuotaSummary {
    pub compatible_profiles: usize,
    pub unavailable_profiles: usize,
    pub live_profiles: usize,
    pub snapshot_profiles: usize,
    pub five_hour: StatusQuotaWindowAggregation,
    pub weekly: StatusQuotaWindowAggregation,
}

unsafe extern "C" {
    fn prodex_quota_pool_summary_sections_v1(
        abi_version: i64,
        openai_profiles: i64,
        main_profiles: i64,
        output_address: u64,
        output_capacity: i64,
    ) -> i64;
    fn prodex_quota_openai_pool_aggregate_v1(
        input_fields: *const i64,
        output_fields: *mut i64,
        count: i64,
    ) -> i64;
    fn prodex_quota_status_summary_v2(
        abi_version: i64,
        input_address: u64,
        output_address: u64,
        count: i64,
        output_capacity: i64,
    ) -> i64;
}

/// Which summary section the compiled quota owner requires the CLI to
/// render, after consuming aggregated OpenAI and Gemini/Copilot pool counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuotaPoolSummarySection {
    UsableNow,
    FiveHour,
    Weekly,
    Main,
    NoDataFiveHour,
    NoDataWeekly,
}

pub fn quota_pool_summary_sections(
    openai_profiles: usize,
    main_profiles: usize,
) -> Result<Vec<QuotaPoolSummarySection>, MojoError> {
    let mut output = [-1_i64; 4];
    let status = unsafe {
        prodex_quota_pool_summary_sections_v1(
            1,
            i64::try_from(openai_profiles).map_err(|_| MojoError::InvalidInput)?,
            i64::try_from(main_profiles).map_err(|_| MojoError::InvalidInput)?,
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
        )
    };
    match status {
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    // All section orders are fixed by the Mojo ABI; reject partial/unknown
    // responses rather than silently choosing a Rust-side fallback layout.
    match output {
        [3, 1, 2, 3] => Ok(vec![
            QuotaPoolSummarySection::UsableNow,
            QuotaPoolSummarySection::FiveHour,
            QuotaPoolSummarySection::Weekly,
        ]),
        [1, 4, 0, 0] => Ok(vec![QuotaPoolSummarySection::Main]),
        [2, 5, 6, 0] => Ok(vec![
            QuotaPoolSummarySection::NoDataFiveHour,
            QuotaPoolSummarySection::NoDataWeekly,
        ]),
        _ => Err(MojoError::InvalidOutput),
    }
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
        prodex_quota_status_summary_v2(
            STATUS_QUOTA_SUMMARY_ABI_VERSION,
            fields.as_ptr() as usize as u64,
            output.as_mut_ptr() as usize as u64,
            count,
            STATUS_QUOTA_SUMMARY_OUTPUT_FIELD_COUNT as i64,
        )
    };
    match status {
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        3 => return Err(MojoError::Capacity),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    let count_limit = count;
    if [
        output[0], output[1], output[2], output[6], output[10], output[11],
    ]
    .into_iter()
    .any(|value| value < 0 || value > count_limit)
        || output[10]
            .checked_add(output[11])
            .and_then(|n| n.checked_add(output[1]))
            != Some(output[0])
        || output[1] > output[0]
        || output[2] > output[0]
        || output[6] > output[0]
        || output[3] < 0
        || output[7] < 0
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
        live_profiles: usize::try_from(output[10]).map_err(|_| MojoError::InvalidOutput)?,
        snapshot_profiles: usize::try_from(output[11]).map_err(|_| MojoError::InvalidOutput)?,
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

#[cfg(test)]
mod summary_section_tests {
    use super::*;

    #[test]
    fn quota_source_summary_abi_rejects_bad_shapes_and_late_records_before_writes() {
        let mut output = [71_i64; 12];
        let destination = output.as_mut_ptr();
        let call = |version, input: *const i64, count, out: *mut i64, capacity| unsafe {
            prodex_quota_status_summary_v2(
                version,
                input as usize as u64,
                out as usize as u64,
                count,
                capacity,
            )
        };
        let mut fields = [0_i64; 30];
        fields[0] = 1;
        let source = fields.as_ptr();
        assert_eq!(call(1, source, 2, destination, 12), 4);
        assert_eq!(call(2, source, -1, destination, 12), 1);
        assert_eq!(call(2, source, i64::MAX, destination, 12), 1);
        assert_eq!(call(2, std::ptr::null(), 1, destination, 12), 1);
        assert_eq!(call(2, source, 2, std::ptr::null_mut(), 12), 1);
        assert_eq!(call(2, source, 2, destination, 11), 3);
        for index in [0, 1, 2, 4, 7, 10, 13] {
            fields[15 + index] = 2;
            assert_eq!(call(2, source, 2, destination, 12), 1);
            fields[15 + index] = 0;
        }
        for index in [3, 6, 9, 12] {
            fields[15 + index] = -1;
            fields[16 + index] = 1;
            assert_eq!(call(2, source, 2, destination, 12), 1);
            fields[15 + index] = 0;
            fields[16 + index] = 0;
        }
        assert_eq!(output, [71; 12]);
        assert_eq!(call(2, std::ptr::null(), 0, destination, 12), 0);
        assert_eq!(output, [0; 12]);
    }

    #[test]
    fn mojo_quota_pool_summary_selects_openai_main_or_missing_data_layout() {
        use QuotaPoolSummarySection::*;
        for (openai, main, expected) in [
            (0, 0, vec![NoDataFiveHour, NoDataWeekly]),
            (0, 5, vec![Main]),
            (1, 0, vec![UsableNow, FiveHour, Weekly]),
            (1, 5, vec![UsableNow, FiveHour, Weekly]),
            (1025, 0, vec![UsableNow, FiveHour, Weekly]),
            (0, 1025, vec![Main]),
        ] {
            assert_eq!(
                quota_pool_summary_sections(openai, main).unwrap(),
                expected,
                "openai={openai} main={main}",
            );
        }
    }

    #[test]
    fn mojo_quota_pool_summary_section_abi_rejects_invalid_inputs_without_writes() {
        let mut output = [71_i64; 4];
        let dest = output.as_mut_ptr() as usize as u64;
        let call = |version, openai, main, out, capacity| unsafe {
            prodex_quota_pool_summary_sections_v1(version, openai, main, out, capacity)
        };
        assert_eq!(call(2, 1, 0, dest, 4), 4);
        assert_eq!(call(1, -1, 0, dest, 4), 1);
        assert_eq!(call(1, 1, -1, dest, 4), 1);
        assert_eq!(call(1, 1, 0, 0, 4), 1);
        assert_eq!(call(1, 1, 0, dest, 3), 1);
        assert_eq!(output, [71; 4]);
    }
}
