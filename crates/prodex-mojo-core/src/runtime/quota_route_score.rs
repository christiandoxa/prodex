use super::{
    QuotaRouteScoreInput, QuotaScore, RUNTIME_QUOTA_ROUTE_SCORE_FIELD_COUNT,
    RUNTIME_QUOTA_SCORE_MAX_COUNT,
};

mod probe_order;
pub use probe_order::{QuotaProbeOrderInput, QuotaProbeOrderPlan, quota_probe_order};

const RUNTIME_QUOTA_SUMMARY_SORT_KEY_ABI_VERSION: i64 = 1;
const RUNTIME_QUOTA_SUMMARY_SORT_KEY_FIELD_COUNT: usize = 9;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuotaSummarySortKeyInput {
    pub route_band: i64,
    pub weekly_status: i64,
    pub five_hour_status: i64,
    pub weekly_remaining: i64,
    pub five_hour_remaining: i64,
    pub weekly_reset_at: i64,
    pub five_hour_reset_at: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuotaSummarySortKey {
    pub route_band: i64,
    pub route_band_rank: i64,
    pub weekly_status_rank: i64,
    pub five_hour_status_rank: i64,
    pub reserve_floor: i64,
    pub weekly_remaining: i64,
    pub five_hour_remaining: i64,
    pub weekly_reset_at: i64,
    pub five_hour_reset_at: i64,
}

unsafe extern "C" {
    fn prodex_runtime_quota_summary_sort_key_v1(
        abi_version: i64,
        route_band: i64,
        weekly_status: i64,
        five_hour_status: i64,
        weekly_remaining: i64,
        five_hour_remaining: i64,
        weekly_reset_at: i64,
        five_hour_reset_at: i64,
        output_address: *mut i64,
    ) -> i64;
}

pub fn quota_summary_sort_key(
    input: QuotaSummarySortKeyInput,
) -> Result<QuotaSummarySortKey, crate::MojoError> {
    if !(0..=4).contains(&input.route_band)
        || !(0..=4).contains(&input.weekly_status)
        || !(0..=4).contains(&input.five_hour_status)
        || !(0..=100).contains(&input.weekly_remaining)
        || !(0..=100).contains(&input.five_hour_remaining)
    {
        return Err(crate::MojoError::InvalidInput);
    }
    let mut output = [0_i64; RUNTIME_QUOTA_SUMMARY_SORT_KEY_FIELD_COUNT];
    let status = unsafe {
        prodex_runtime_quota_summary_sort_key_v1(
            RUNTIME_QUOTA_SUMMARY_SORT_KEY_ABI_VERSION,
            input.route_band,
            input.weekly_status,
            input.five_hour_status,
            input.weekly_remaining,
            input.five_hour_remaining,
            input.weekly_reset_at,
            input.five_hour_reset_at,
            output.as_mut_ptr(),
        )
    };
    if status == -4 {
        return Err(crate::MojoError::AbiMismatch);
    }
    if status == -1 {
        return Err(crate::MojoError::InvalidInput);
    }
    if status != 0 {
        return Err(crate::MojoError::InvalidOutput);
    }
    if output[0] != input.route_band
        || !(0..=4).contains(&output[1])
        || !(0..=4).contains(&output[2])
        || !(0..=4).contains(&output[3])
        || !(0..=100).contains(&output[4])
        || output[5] != input.weekly_remaining
        || output[6] != input.five_hour_remaining
        || output[7] != input.weekly_reset_at
        || output[8] != input.five_hour_reset_at
    {
        return Err(crate::MojoError::InvalidOutput);
    }
    Ok(QuotaSummarySortKey {
        route_band: output[0],
        route_band_rank: output[1],
        weekly_status_rank: output[2],
        five_hour_status_rank: output[3],
        reserve_floor: output[4],
        weekly_remaining: output[5],
        five_hour_remaining: output[6],
        weekly_reset_at: output[7],
        five_hour_reset_at: output[8],
    })
}

/// Resolve route-aware profile scores from raw window observations in one
/// bounded Mojo call. Host code owns usage acquisition and plan selection;
/// Mojo owns scaling, window completeness, pressure-band, and score rules.
pub fn quota_route_score_batch(
    inputs: &[QuotaRouteScoreInput],
    route_kind: i64,
) -> Result<Vec<QuotaScore>, crate::MojoError> {
    if inputs.len() > RUNTIME_QUOTA_SCORE_MAX_COUNT
        || !(0..=3).contains(&route_kind)
        || inputs.iter().any(|input| {
            input.weekly_pressure < 0
                || input.five_hour_pressure < 0
                || input.scale_bps < 0
                || input.weekly_remaining < 0
                || input.weekly_remaining > 100
                || input.five_hour_remaining < 0
                || input.five_hour_remaining > 100
        })
    {
        return Err(crate::MojoError::InvalidInput);
    }

    let mut fields = Vec::with_capacity(inputs.len() * RUNTIME_QUOTA_ROUTE_SCORE_FIELD_COUNT);
    for input in inputs {
        fields.extend([
            input.weekly_pressure,
            input.five_hour_pressure,
            input.scale_bps,
            input.weekly_remaining,
            input.five_hour_remaining,
            i64::from(input.weekly_has_value),
            i64::from(input.five_hour_has_value),
            input.weekly_reset_at,
            input.five_hour_reset_at,
        ]);
    }

    let mut pressure_band = vec![0_i64; inputs.len()];
    let mut total_pressure = vec![0_i64; inputs.len()];
    let mut weekly_pressure = vec![0_i64; inputs.len()];
    let mut five_hour_pressure = vec![0_i64; inputs.len()];
    let mut reserve_floor = vec![0_i64; inputs.len()];
    let mut weekly_remaining = vec![0_i64; inputs.len()];
    let mut five_hour_remaining = vec![0_i64; inputs.len()];
    let mut weekly_reset_at = vec![0_i64; inputs.len()];
    let mut five_hour_reset_at = vec![0_i64; inputs.len()];
    let status = unsafe {
        super::prodex_runtime_quota_route_score_resolution_batch(
            fields.as_ptr() as u64,
            pressure_band.as_mut_ptr() as u64,
            total_pressure.as_mut_ptr() as u64,
            weekly_pressure.as_mut_ptr() as u64,
            five_hour_pressure.as_mut_ptr() as u64,
            reserve_floor.as_mut_ptr() as u64,
            weekly_remaining.as_mut_ptr() as u64,
            five_hour_remaining.as_mut_ptr() as u64,
            weekly_reset_at.as_mut_ptr() as u64,
            five_hour_reset_at.as_mut_ptr() as u64,
            i64::try_from(inputs.len()).map_err(|_| crate::MojoError::InvalidInput)?,
            route_kind,
        )
    };
    if status != 0
        || pressure_band.iter().any(|value| !(0..=4).contains(value))
        || total_pressure.iter().any(|value| *value < 0)
        || weekly_pressure.iter().any(|value| *value < 0)
        || five_hour_pressure.iter().any(|value| *value < 0)
        || reserve_floor.iter().any(|value| *value < 0)
        || weekly_remaining
            .iter()
            .any(|value| !(0..=100).contains(value))
        || five_hour_remaining
            .iter()
            .any(|value| !(0..=100).contains(value))
    {
        return Err(crate::MojoError::InvalidOutput);
    }

    Ok((0..inputs.len())
        .map(|index| QuotaScore {
            pressure_band: pressure_band[index],
            total_pressure: total_pressure[index],
            weekly_pressure: weekly_pressure[index],
            five_hour_pressure: five_hour_pressure[index],
            reserve_floor: reserve_floor[index],
            weekly_remaining: weekly_remaining[index],
            five_hour_remaining: five_hour_remaining[index],
            weekly_reset_at: weekly_reset_at[index],
            five_hour_reset_at: five_hour_reset_at[index],
        })
        .collect())
}

#[cfg(test)]
mod summary_sort_tests {
    use super::*;

    #[test]
    fn summary_sort_key_preserves_rank_and_remaining_tie_breaks() {
        let key = quota_summary_sort_key(QuotaSummarySortKeyInput {
            route_band: 2,
            weekly_status: 3,
            five_hour_status: 1,
            weekly_remaining: 24,
            five_hour_remaining: 9,
            weekly_reset_at: 400,
            five_hour_reset_at: 300,
        })
        .expect("compiled Mojo summary sort key should be valid");
        assert_eq!(
            key,
            QuotaSummarySortKey {
                route_band: 2,
                route_band_rank: 2,
                weekly_status_rank: 3,
                five_hour_status_rank: 1,
                reserve_floor: 9,
                weekly_remaining: 24,
                five_hour_remaining: 9,
                weekly_reset_at: 400,
                five_hour_reset_at: 300,
            }
        );
    }

    #[test]
    fn summary_sort_key_rejects_invalid_tags_and_percentages() {
        assert_eq!(
            quota_summary_sort_key(QuotaSummarySortKeyInput {
                route_band: 5,
                weekly_status: 0,
                five_hour_status: 0,
                weekly_remaining: 0,
                five_hour_remaining: 0,
                weekly_reset_at: 0,
                five_hour_reset_at: 0,
            }),
            Err(crate::MojoError::InvalidInput)
        );
        assert_eq!(
            quota_summary_sort_key(QuotaSummarySortKeyInput {
                route_band: 0,
                weekly_status: 0,
                five_hour_status: 0,
                weekly_remaining: 101,
                five_hour_remaining: 0,
                weekly_reset_at: 0,
                five_hour_reset_at: 0,
            }),
            Err(crate::MojoError::InvalidInput)
        );
    }

    #[test]
    fn summary_sort_key_abi_rejects_version_and_pointer_errors() {
        assert_eq!(
            unsafe {
                prodex_runtime_quota_summary_sort_key_v1(
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                    std::ptr::null_mut(),
                )
            },
            -4
        );
        assert_eq!(
            unsafe {
                prodex_runtime_quota_summary_sort_key_v1(
                    1,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                    std::ptr::null_mut(),
                )
            },
            -1
        );
        let mut output = [0_i64; RUNTIME_QUOTA_SUMMARY_SORT_KEY_FIELD_COUNT];
        assert_eq!(
            unsafe {
                prodex_runtime_quota_summary_sort_key_v1(
                    1,
                    0,
                    0,
                    0,
                    101,
                    0,
                    0,
                    0,
                    output.as_mut_ptr(),
                )
            },
            -1
        );
    }
}
