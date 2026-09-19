use super::RUNTIME_PROFILE_SCHEDULE_MAX_COUNT;

/// Normalized health observations for one profile and one route.
///
/// Rust resolves profile names, route keys, and persisted state before building
/// this fixed-width record. Mojo owns only decay and saturating score arithmetic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProfileHealthScoreInput {
    pub global_score: u32,
    pub global_updated_at: i64,
    pub route_health_score: u32,
    pub route_health_updated_at: i64,
    pub route_bad_pairing_score: u32,
    pub route_bad_pairing_updated_at: i64,
    pub coupled_health_score: u32,
    pub coupled_health_updated_at: i64,
    pub coupled_bad_pairing_score: u32,
    pub coupled_bad_pairing_updated_at: i64,
    pub route_performance_score: u32,
    pub route_performance_updated_at: i64,
    pub coupled_performance_score: u32,
    pub coupled_performance_updated_at: i64,
}

pub const RUNTIME_PROFILE_HEALTH_SCORE_FIELD_COUNT: usize = 14;
pub const RUNTIME_PROFILE_HEALTH_SCORE_MAX_COUNT: usize = RUNTIME_PROFILE_SCHEDULE_MAX_COUNT;

unsafe extern "C" {
    fn prodex_runtime_health_scalar_v1(
        abi_version: i64,
        operation: i64,
        fields: u64,
        field_count: i64,
        output: u64,
        output_count: i64,
    ) -> i64;
    fn prodex_runtime_profile_health_sort_key_batch_v1(
        abi_version: i64,
        fields: u64,
        output: u64,
        count: i64,
        now: i64,
        health_decay_seconds: i64,
        bad_pairing_decay_seconds: i64,
        performance_decay_seconds: i64,
    ) -> i64;
}

/// Computes route health sort keys for a bounded batch of normalized profiles.
pub fn profile_health_sort_key_batch(
    inputs: &[ProfileHealthScoreInput],
    now: i64,
    health_decay_seconds: i64,
    bad_pairing_decay_seconds: i64,
    performance_decay_seconds: i64,
) -> Result<Vec<u32>, crate::MojoError> {
    if inputs.len() > RUNTIME_PROFILE_HEALTH_SCORE_MAX_COUNT {
        return Err(crate::MojoError::InvalidInput);
    }
    if inputs.is_empty() {
        return Ok(Vec::new());
    }

    let mut fields = Vec::with_capacity(inputs.len() * RUNTIME_PROFILE_HEALTH_SCORE_FIELD_COUNT);
    for input in inputs {
        fields.extend([
            i64::from(input.global_score),
            input.global_updated_at,
            i64::from(input.route_health_score),
            input.route_health_updated_at,
            i64::from(input.route_bad_pairing_score),
            input.route_bad_pairing_updated_at,
            i64::from(input.coupled_health_score),
            input.coupled_health_updated_at,
            i64::from(input.coupled_bad_pairing_score),
            input.coupled_bad_pairing_updated_at,
            i64::from(input.route_performance_score),
            input.route_performance_updated_at,
            i64::from(input.coupled_performance_score),
            input.coupled_performance_updated_at,
        ]);
    }
    let mut output = vec![0_i64; inputs.len()];
    let status = unsafe {
        prodex_runtime_profile_health_sort_key_batch_v1(
            1,
            fields.as_ptr() as u64,
            output.as_mut_ptr() as u64,
            i64::try_from(inputs.len()).map_err(|_| crate::MojoError::InvalidInput)?,
            now,
            health_decay_seconds,
            bad_pairing_decay_seconds,
            performance_decay_seconds,
        )
    };
    if status != 0 {
        return Err(match status {
            1 | 2 => crate::MojoError::InvalidInput,
            4 => crate::MojoError::AbiMismatch,
            _ => crate::MojoError::InvalidOutput,
        });
    }
    output
        .into_iter()
        .map(|value| u32::try_from(value).map_err(|_| crate::MojoError::InvalidOutput))
        .collect()
}

const RUNTIME_HEALTH_SCALAR_EFFECTIVE_SCORE: i64 = 1;
const RUNTIME_HEALTH_SCALAR_COUPLING_SCORE: i64 = 2;
const RUNTIME_HEALTH_SCALAR_PERFORMANCE_SCORE: i64 = 3;
const RUNTIME_HEALTH_SCALAR_BACKOFF_SORT_KEY: i64 = 4;
const RUNTIME_HEALTH_SCALAR_HALF_OPEN_SECONDS: i64 = 5;
const RUNTIME_HEALTH_SCALAR_OPEN_SECONDS: i64 = 6;
const RUNTIME_HEALTH_SCALAR_SOFTEN_UNTIL: i64 = 7;

fn runtime_health_scalar<const N: usize, const M: usize>(
    operation: i64,
    fields: [i64; N],
) -> Result<[i64; M], crate::MojoError> {
    let mut output = [0_i64; M];
    let status = unsafe {
        prodex_runtime_health_scalar_v1(
            1,
            operation,
            fields.as_ptr() as u64,
            i64::try_from(N).map_err(|_| crate::MojoError::InvalidInput)?,
            output.as_mut_ptr() as u64,
            i64::try_from(M).map_err(|_| crate::MojoError::InvalidInput)?,
        )
    };
    if status != 0 {
        return Err(match status {
            1 | 2 => crate::MojoError::InvalidInput,
            4 => crate::MojoError::AbiMismatch,
            _ => crate::MojoError::InvalidOutput,
        });
    }
    Ok(output)
}

pub fn profile_health_effective_score(
    score: u32,
    updated_at: i64,
    now: i64,
    decay_seconds: i64,
) -> Result<u32, crate::MojoError> {
    let [score] = runtime_health_scalar::<4, 1>(
        RUNTIME_HEALTH_SCALAR_EFFECTIVE_SCORE,
        [i64::from(score), updated_at, now, decay_seconds],
    )?;
    u32::try_from(score).map_err(|_| crate::MojoError::InvalidOutput)
}

pub fn profile_health_coupling_score(
    route_score: u32,
    route_updated_at: i64,
    bad_pairing_score: u32,
    bad_pairing_updated_at: i64,
    now: i64,
    health_decay_seconds: i64,
    bad_pairing_decay_seconds: i64,
) -> Result<u32, crate::MojoError> {
    let [score] = runtime_health_scalar::<7, 1>(
        RUNTIME_HEALTH_SCALAR_COUPLING_SCORE,
        [
            i64::from(route_score),
            route_updated_at,
            i64::from(bad_pairing_score),
            bad_pairing_updated_at,
            now,
            health_decay_seconds,
            bad_pairing_decay_seconds,
        ],
    )?;
    u32::try_from(score).map_err(|_| crate::MojoError::InvalidOutput)
}

pub fn profile_health_performance_score(
    route_score: u32,
    route_updated_at: i64,
    coupled_score: u32,
    coupled_updated_at: i64,
    now: i64,
    decay_seconds: i64,
) -> Result<u32, crate::MojoError> {
    let [score] = runtime_health_scalar::<6, 1>(
        RUNTIME_HEALTH_SCALAR_PERFORMANCE_SCORE,
        [
            i64::from(route_score),
            route_updated_at,
            i64::from(coupled_score),
            coupled_updated_at,
            now,
            decay_seconds,
        ],
    )?;
    u32::try_from(score).map_err(|_| crate::MojoError::InvalidOutput)
}

pub fn profile_backoff_sort_key(
    circuit_until: Option<i64>,
    transport_until: Option<i64>,
    retry_until: Option<i64>,
    now: i64,
) -> Result<(usize, i64, i64, i64), crate::MojoError> {
    let output = runtime_health_scalar::<4, 4>(
        RUNTIME_HEALTH_SCALAR_BACKOFF_SORT_KEY,
        [
            circuit_until.unwrap_or(i64::MIN),
            transport_until.unwrap_or(i64::MIN),
            retry_until.unwrap_or(i64::MIN),
            now,
        ],
    )?;
    Ok((
        usize::try_from(output[0]).map_err(|_| crate::MojoError::InvalidOutput)?,
        output[1],
        output[2],
        output[3],
    ))
}

pub fn profile_circuit_half_open_seconds(
    score: u32,
    threshold: u32,
    base_seconds: i64,
    max_seconds: i64,
) -> Result<i64, crate::MojoError> {
    Ok(runtime_health_scalar::<4, 1>(
        RUNTIME_HEALTH_SCALAR_HALF_OPEN_SECONDS,
        [
            i64::from(score),
            i64::from(threshold),
            base_seconds,
            max_seconds,
        ],
    )?[0])
}

pub fn profile_circuit_open_seconds(
    score: u32,
    reopen_stage: u32,
    threshold: u32,
    max_reopen_stage: u32,
    base_seconds: i64,
    max_seconds: i64,
) -> Result<i64, crate::MojoError> {
    Ok(runtime_health_scalar::<6, 1>(
        RUNTIME_HEALTH_SCALAR_OPEN_SECONDS,
        [
            i64::from(score),
            i64::from(reopen_stage),
            i64::from(threshold),
            i64::from(max_reopen_stage),
            base_seconds,
            max_seconds,
        ],
    )?[0])
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProfileBackoffSoftening {
    pub keep: bool,
    pub until: i64,
    pub changed: bool,
}

pub fn profile_soften_backoff_until(
    until: i64,
    now: i64,
    max_future_seconds: i64,
) -> Result<ProfileBackoffSoftening, crate::MojoError> {
    let output = runtime_health_scalar::<3, 3>(
        RUNTIME_HEALTH_SCALAR_SOFTEN_UNTIL,
        [until, now, max_future_seconds],
    )?;
    if !matches!(output[0], 0 | 1) || !matches!(output[2], 0 | 1) {
        return Err(crate::MojoError::InvalidOutput);
    }
    Ok(ProfileBackoffSoftening {
        keep: output[0] == 1,
        until: output[1],
        changed: output[2] == 1,
    })
}
