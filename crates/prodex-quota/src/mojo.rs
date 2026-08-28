pub(super) fn round_f64(value: f64) -> i64 {
    prodex_mojo_core::quota::round_f64(value)
}

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

pub(crate) fn gemini_bucket_numeric_batch(
    inputs: &[prodex_mojo_core::quota::GeminiBucketNumericInput],
) -> Result<Vec<prodex_mojo_core::quota::GeminiBucketNumericOutput>, prodex_mojo_core::MojoError> {
    prodex_mojo_core::quota::gemini_bucket_numeric_batch(inputs)
}

pub(crate) fn main_quota_aggregate(
    inputs: &[(Option<i64>, Option<i64>)],
) -> Result<(usize, i64, Option<i64>), prodex_mojo_core::MojoError> {
    let inputs = inputs
        .iter()
        .copied()
        .map(
            |(remaining_percent, reset_at)| prodex_mojo_core::quota::MainQuotaAggregationInput {
                remaining_percent,
                reset_at,
            },
        )
        .collect::<Vec<_>>();
    let aggregate = prodex_mojo_core::quota::main_quota_aggregate_batch(&inputs)?;
    Ok((
        aggregate.profiles_with_data,
        aggregate.pool_remaining,
        aggregate.earliest_reset_at,
    ))
}

pub(crate) fn quota_capacity_batch(
    inputs: &[prodex_mojo_core::quota::QuotaCapacityInput],
    route_kind: i64,
) -> Result<Vec<prodex_mojo_core::quota::QuotaCapacityOutput>, prodex_mojo_core::MojoError> {
    prodex_mojo_core::quota::quota_capacity_batch(inputs, route_kind)
}
