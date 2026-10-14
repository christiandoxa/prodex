use crate::MojoError;

use super::{ABI_VERSION, length, ptr};

const MAX_INPUTS: usize = 65_536;
const OUTPUT_COUNT: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BrokerPreviousResponseContinuityPlan {
    pub entries: [usize; 4],
    pub failures: [usize; 4],
}

unsafe extern "C" {
    fn prodex_runtime_broker_previous_response_continuity_v1(
        abi_version: i64,
        keys_address: u64,
        lengths_address: u64,
        scores_address: u64,
        updated_address: u64,
        count: i64,
        now: i64,
        decay_seconds: i64,
        output_address: u64,
    ) -> i64;
}

fn status_error(status: i64) -> MojoError {
    match status {
        -1 => MojoError::InvalidInput,
        1 => MojoError::InvalidInput,
        2 => MojoError::InvalidOutput,
        3 => MojoError::Capacity,
        4 => MojoError::AbiMismatch,
        _ => MojoError::InvalidOutput,
    }
}

pub fn previous_response_continuity_metrics<'a, I>(
    profile_health: I,
    now: i64,
    decay_seconds: i64,
) -> Result<BrokerPreviousResponseContinuityPlan, MojoError>
where
    I: IntoIterator<Item = (&'a str, u32, i64)>,
{
    let rows = profile_health.into_iter().collect::<Vec<_>>();
    if rows.len() > MAX_INPUTS {
        return Err(MojoError::Capacity);
    }
    let keys = rows.iter().map(|(key, _, _)| ptr(key)).collect::<Vec<_>>();
    let lengths = rows
        .iter()
        .map(|(key, _, _)| length(key))
        .collect::<Result<Vec<_>, _>>()?;
    let scores = rows
        .iter()
        .map(|(_, score, _)| i64::from(*score))
        .collect::<Vec<_>>();
    let updated = rows
        .iter()
        .map(|(_, _, updated_at)| *updated_at)
        .collect::<Vec<_>>();
    let mut output = [0_i64; OUTPUT_COUNT];
    let count = i64::try_from(rows.len()).map_err(|_| MojoError::InvalidInput)?;
    let status = unsafe {
        prodex_runtime_broker_previous_response_continuity_v1(
            ABI_VERSION,
            keys.as_ptr() as usize as u64,
            lengths.as_ptr() as usize as u64,
            scores.as_ptr() as usize as u64,
            updated.as_ptr() as usize as u64,
            count,
            now,
            decay_seconds,
            output.as_mut_ptr() as usize as u64,
        )
    };
    if status != 0 {
        return Err(status_error(status));
    }
    let mut entries = [0_usize; 4];
    let mut failures = [0_usize; 4];
    for index in 0..4 {
        entries[index] = usize::try_from(output[index]).map_err(|_| MojoError::InvalidOutput)?;
        failures[index] =
            usize::try_from(output[index + 4]).map_err(|_| MojoError::InvalidOutput)?;
    }
    Ok(BrokerPreviousResponseContinuityPlan { entries, failures })
}
