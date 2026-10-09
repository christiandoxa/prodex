//! Bounded, typed host adapter for the Mojo recovery batch planner.

use super::RUNTIME_PROFILE_SCHEDULE_MAX_COUNT;

unsafe extern "C" {
    fn prodex_runtime_profile_recovery_plan_batch_v1(
        abi_version: i64,
        fields: u64,
        output: u64,
        count: i64,
        now: i64,
    ) -> i64;
}

/// Host-resolved candidate and backoff timestamps for route recovery planning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProfileRecoveryCandidate {
    pub eligible: bool,
    pub retry_until: Option<i64>,
    pub transport_until: Option<i64>,
    pub circuit_until: Option<i64>,
}

/// Recovered candidates and the earliest per-profile recovery time in a batch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileRecoveryBatchPlan {
    pub can_clear: Vec<bool>,
    pub earliest_recovery_at: Option<i64>,
}

/// Plans recovered profiles and the earliest retry time through the bounded Mojo ABI.
pub fn profile_recovery_plan_batch(
    inputs: &[ProfileRecoveryCandidate],
    now: i64,
) -> Result<ProfileRecoveryBatchPlan, crate::MojoError> {
    let mut plan = ProfileRecoveryBatchPlan {
        can_clear: Vec::with_capacity(inputs.len()),
        earliest_recovery_at: None,
    };
    for chunk in inputs.chunks(RUNTIME_PROFILE_SCHEDULE_MAX_COUNT) {
        let chunk_plan = recovery_chunk_plan(chunk, now)?;
        plan.can_clear.extend(chunk_plan.can_clear);
        if let Some(next) = chunk_plan.earliest_recovery_at {
            plan.earliest_recovery_at = Some(
                plan.earliest_recovery_at
                    .map_or(next, |current| current.min(next)),
            );
        }
    }
    Ok(plan)
}

/// Marshal and validate one bounded ABI frame independently of cross-frame assembly.
fn recovery_chunk_plan(
    chunk: &[ProfileRecoveryCandidate],
    now: i64,
) -> Result<ProfileRecoveryBatchPlan, crate::MojoError> {
    let mut plan = ProfileRecoveryBatchPlan {
        can_clear: Vec::with_capacity(chunk.len()),
        earliest_recovery_at: None,
    };
    let mut fields = Vec::with_capacity(chunk.len() * 7);
    for input in chunk {
        fields.extend([
            i64::from(input.eligible),
            i64::from(input.retry_until.is_some()),
            input.retry_until.unwrap_or_default(),
            i64::from(input.transport_until.is_some()),
            input.transport_until.unwrap_or_default(),
            i64::from(input.circuit_until.is_some()),
            input.circuit_until.unwrap_or_default(),
        ]);
    }
    let mut output = vec![0_i64; chunk.len() * 2 + 2];
    let status = unsafe {
        prodex_runtime_profile_recovery_plan_batch_v1(
            1,
            fields.as_ptr() as u64,
            output.as_mut_ptr() as u64,
            i64::try_from(chunk.len()).map_err(|_| crate::MojoError::InvalidInput)?,
            now,
        )
    };
    if status != 0 {
        return Err(match status {
            1 | 2 => crate::MojoError::InvalidInput,
            4 => crate::MojoError::AbiMismatch,
            _ => crate::MojoError::InvalidOutput,
        });
    }

    for (index, input) in chunk.iter().enumerate() {
        let can_clear = match output[index * 2] {
            0 => false,
            1 => true,
            _ => return Err(crate::MojoError::InvalidOutput),
        };
        if can_clear && !input.eligible {
            return Err(crate::MojoError::InvalidOutput);
        }
        plan.can_clear.push(can_clear);
    }

    let summary = chunk.len() * 2;
    plan.earliest_recovery_at = match output[summary] {
        0 => None,
        1 => Some(output[summary + 1]),
        _ => return Err(crate::MojoError::InvalidOutput),
    };
    Ok(plan)
}
