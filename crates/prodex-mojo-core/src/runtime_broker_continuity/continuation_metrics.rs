use crate::MojoError;

use super::ABI_VERSION;

const MAX_INPUTS: usize = 65_536;
const RECORD_FIELDS: usize = 10;
const OUTPUT_COUNT: usize = 13;

/// One host-observed continuation status projected into the bounded Mojo reducer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BrokerContinuationMetricRow {
    /// Binding kind: response, turn-state, or session ID (`0..=2`).
    pub kind: i64,
    /// Lifecycle state: warm, verified, suspect, or dead (`0..=3`).
    pub state: i64,
    pub failure_count: u32,
    pub not_found_streak: u32,
    pub last_not_found_at: Option<i64>,
    pub last_verified_at: Option<i64>,
    pub last_touched_at: Option<i64>,
}

/// Aggregated continuation lifecycle counters returned by Mojo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BrokerContinuationMetricsPlan {
    pub states: [usize; 4],
    pub failures: [usize; 3],
    pub not_found_streaks: [usize; 3],
    pub stale_verified: [usize; 3],
}

unsafe extern "C" {
    fn prodex_runtime_broker_continuation_metrics_v1(
        abi_version: i64,
        records_address: u64,
        count: i64,
        now: i64,
        stale_verified_seconds: i64,
        output_address: u64,
    ) -> i64;
}

fn optional_parts(value: Option<i64>) -> (i64, i64) {
    value.map_or((0, 0), |value| (1, value))
}

/// Reduces bounded continuation lifecycle observations through the Mojo ABI.
pub fn continuation_metrics(
    rows: impl IntoIterator<Item = BrokerContinuationMetricRow>,
    now: i64,
    stale_verified_seconds: i64,
) -> Result<BrokerContinuationMetricsPlan, MojoError> {
    let rows = rows.into_iter().collect::<Vec<_>>();
    if rows.len() > MAX_INPUTS {
        return Err(MojoError::Capacity);
    }
    let mut records = Vec::with_capacity(
        rows.len()
            .checked_mul(RECORD_FIELDS)
            .ok_or(MojoError::InvalidInput)?,
    );
    for row in &rows {
        let (not_found_present, not_found_at) = optional_parts(row.last_not_found_at);
        let (verified_present, verified_at) = optional_parts(row.last_verified_at);
        let (touched_present, touched_at) = optional_parts(row.last_touched_at);
        records.extend([
            row.kind,
            row.state,
            i64::from(row.failure_count),
            i64::from(row.not_found_streak),
            not_found_present,
            not_found_at,
            verified_present,
            verified_at,
            touched_present,
            touched_at,
        ]);
    }
    let mut output = [0_i64; OUTPUT_COUNT];
    let status = unsafe {
        prodex_runtime_broker_continuation_metrics_v1(
            ABI_VERSION,
            records.as_ptr() as usize as u64,
            i64::try_from(rows.len()).map_err(|_| MojoError::InvalidInput)?,
            now,
            stale_verified_seconds,
            output.as_mut_ptr() as usize as u64,
        )
    };
    if status != 0 {
        return Err(MojoError::InvalidInput);
    }
    let mut plan = BrokerContinuationMetricsPlan {
        states: [0; 4],
        failures: [0; 3],
        not_found_streaks: [0; 3],
        stale_verified: [0; 3],
    };
    for (index, state) in plan.states.iter_mut().enumerate() {
        *state = usize::try_from(output[index]).map_err(|_| MojoError::InvalidOutput)?;
    }
    for (index, failure) in plan.failures.iter_mut().enumerate() {
        *failure = usize::try_from(output[4 + index]).map_err(|_| MojoError::InvalidOutput)?;
    }
    for (index, streak) in plan.not_found_streaks.iter_mut().enumerate() {
        *streak = usize::try_from(output[7 + index]).map_err(|_| MojoError::InvalidOutput)?;
    }
    for (index, stale) in plan.stale_verified.iter_mut().enumerate() {
        *stale = usize::try_from(output[10 + index]).map_err(|_| MojoError::InvalidOutput)?;
    }
    Ok(plan)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn continuation_metrics_rejects_unknown_rows_and_preserves_capacity_bound() {
        const { assert!(crate::MOJO_REQUIRED) };
        assert_eq!(
            continuation_metrics(
                [BrokerContinuationMetricRow {
                    kind: 3,
                    state: 0,
                    failure_count: 0,
                    not_found_streak: 0,
                    last_not_found_at: None,
                    last_verified_at: None,
                    last_touched_at: None,
                }],
                100,
                10,
            ),
            Err(MojoError::InvalidInput)
        );
        let rows = vec![
            BrokerContinuationMetricRow {
                kind: 0,
                state: 0,
                failure_count: 0,
                not_found_streak: 0,
                last_not_found_at: None,
                last_verified_at: None,
                last_touched_at: None,
            };
            MAX_INPUTS + 1
        ];
        assert_eq!(
            continuation_metrics(rows, 100, 10),
            Err(MojoError::Capacity)
        );
    }
}
