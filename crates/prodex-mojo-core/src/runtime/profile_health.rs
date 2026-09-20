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
    fn prodex_runtime_health_policy_v1(
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

const RUNTIME_HEALTH_POLICY_BAD_PAIRING_NEXT: i64 = 8;
const RUNTIME_HEALTH_POLICY_BUMP_DECISION: i64 = 9;
const RUNTIME_HEALTH_POLICY_RECOVERY_DECISION: i64 = 10;
const RUNTIME_HEALTH_POLICY_INFLIGHT_WEIGHT: i64 = 11;
const RUNTIME_HEALTH_POLICY_INFLIGHT_HARD_LIMIT: i64 = 12;
const RUNTIME_HEALTH_POLICY_INFLIGHT_SOFT_LIMIT: i64 = 13;
const RUNTIME_HEALTH_POLICY_LATENCY_PENALTY: i64 = 14;
const RUNTIME_HEALTH_POLICY_LATENCY_NEXT_SCORE: i64 = 15;
const RUNTIME_HEALTH_POLICY_LATENCY_FAILURE_SCORE: i64 = 16;

fn runtime_health_policy<const N: usize, const M: usize>(
    operation: i64,
    fields: [i64; N],
) -> Result<[i64; M], crate::MojoError> {
    let mut output = [0_i64; M];
    let status = unsafe {
        prodex_runtime_health_policy_v1(
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

pub fn profile_bad_pairing_next_score(
    current_score: u32,
    delta: u32,
    max_score: u32,
) -> Result<u32, crate::MojoError> {
    let [score] = runtime_health_policy::<3, 1>(
        RUNTIME_HEALTH_POLICY_BAD_PAIRING_NEXT,
        [
            i64::from(current_score),
            i64::from(delta),
            i64::from(max_score),
        ],
    )?;
    u32::try_from(score).map_err(|_| crate::MojoError::InvalidOutput)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProfileHealthBumpInput {
    pub current_score: u32,
    pub delta: u32,
    pub max_score: u32,
    pub circuit_open_threshold: u32,
    pub circuit_already_open: bool,
    pub current_reopen_stage: u32,
    pub max_reopen_stage: u32,
    pub circuit_open_seconds: i64,
    pub circuit_open_max_seconds: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProfileHealthBumpPlan {
    pub next_score: u32,
    pub circuit_reopen_stage: Option<u32>,
    pub circuit_open_seconds: Option<i64>,
}

pub fn profile_health_bump_plan(
    input: ProfileHealthBumpInput,
) -> Result<ProfileHealthBumpPlan, crate::MojoError> {
    let output = runtime_health_policy::<9, 5>(
        RUNTIME_HEALTH_POLICY_BUMP_DECISION,
        [
            i64::from(input.current_score),
            i64::from(input.delta),
            i64::from(input.max_score),
            i64::from(input.circuit_open_threshold),
            i64::from(input.circuit_already_open),
            i64::from(input.current_reopen_stage),
            i64::from(input.max_reopen_stage),
            input.circuit_open_seconds,
            input.circuit_open_max_seconds,
        ],
    )?;
    if !matches!(output[1], 0 | 1) || !matches!(output[3], 0 | 1) {
        return Err(crate::MojoError::InvalidOutput);
    }
    Ok(ProfileHealthBumpPlan {
        next_score: u32::try_from(output[0]).map_err(|_| crate::MojoError::InvalidOutput)?,
        circuit_reopen_stage: (output[1] == 1)
            .then(|| u32::try_from(output[2]).map_err(|_| crate::MojoError::InvalidOutput))
            .transpose()?,
        circuit_open_seconds: (output[3] == 1).then_some(output[4]),
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProfileHealthRecoveryPlan {
    pub next_score: Option<u32>,
    pub next_success_streak: Option<u32>,
}

pub fn profile_health_recovery_plan(
    current_score: Option<u32>,
    current_success_streak: u32,
    max_success_streak: u32,
    recovery_score: u32,
) -> Result<ProfileHealthRecoveryPlan, crate::MojoError> {
    let output = runtime_health_policy::<5, 4>(
        RUNTIME_HEALTH_POLICY_RECOVERY_DECISION,
        [
            i64::from(current_score.is_some()),
            i64::from(current_score.unwrap_or(0)),
            i64::from(current_success_streak),
            i64::from(max_success_streak),
            i64::from(recovery_score),
        ],
    )?;
    if !matches!(output[0], 0 | 1) || !matches!(output[2], 0 | 1) {
        return Err(crate::MojoError::InvalidOutput);
    }
    Ok(ProfileHealthRecoveryPlan {
        next_score: (output[0] == 1)
            .then(|| u32::try_from(output[1]).map_err(|_| crate::MojoError::InvalidOutput))
            .transpose()?,
        next_success_streak: (output[2] == 1)
            .then(|| u32::try_from(output[3]).map_err(|_| crate::MojoError::InvalidOutput))
            .transpose()?,
    })
}

pub fn profile_inflight_weight(heavy_context: bool) -> Result<usize, crate::MojoError> {
    let [value] = runtime_health_policy::<1, 1>(
        RUNTIME_HEALTH_POLICY_INFLIGHT_WEIGHT,
        [i64::from(heavy_context)],
    )?;
    usize::try_from(value).map_err(|_| crate::MojoError::InvalidOutput)
}

pub fn profile_inflight_effective_hard_limit(
    configured_limit: usize,
    weight: usize,
) -> Result<usize, crate::MojoError> {
    let [value] = runtime_health_policy::<2, 1>(
        RUNTIME_HEALTH_POLICY_INFLIGHT_HARD_LIMIT,
        [
            i64::try_from(configured_limit).map_err(|_| crate::MojoError::InvalidInput)?,
            i64::try_from(weight).map_err(|_| crate::MojoError::InvalidInput)?,
        ],
    )?;
    usize::try_from(value).map_err(|_| crate::MojoError::InvalidOutput)
}

pub fn profile_inflight_soft_limit(
    route_kind: i64,
    pressure_mode: bool,
    base_limit: usize,
) -> Result<usize, crate::MojoError> {
    let [value] = runtime_health_policy::<3, 1>(
        RUNTIME_HEALTH_POLICY_INFLIGHT_SOFT_LIMIT,
        [
            route_kind,
            i64::from(pressure_mode),
            i64::try_from(base_limit).map_err(|_| crate::MojoError::InvalidInput)?,
        ],
    )?;
    usize::try_from(value).map_err(|_| crate::MojoError::InvalidOutput)
}

pub fn profile_latency_penalty(
    elapsed_ms: u64,
    route_kind: i64,
    stage_kind: i64,
    max_penalty: u32,
) -> Result<u32, crate::MojoError> {
    let [value] = runtime_health_policy::<4, 1>(
        RUNTIME_HEALTH_POLICY_LATENCY_PENALTY,
        [
            i64::try_from(elapsed_ms).map_err(|_| crate::MojoError::InvalidInput)?,
            route_kind,
            stage_kind,
            i64::from(max_penalty),
        ],
    )?;
    u32::try_from(value).map_err(|_| crate::MojoError::InvalidOutput)
}

pub fn profile_latency_next_score(
    current_score: u32,
    observed_penalty: u32,
) -> Result<u32, crate::MojoError> {
    let [value] = runtime_health_policy::<2, 1>(
        RUNTIME_HEALTH_POLICY_LATENCY_NEXT_SCORE,
        [i64::from(current_score), i64::from(observed_penalty)],
    )?;
    u32::try_from(value).map_err(|_| crate::MojoError::InvalidOutput)
}

pub fn profile_latency_failure_score(
    current_score: u32,
    failure_penalty: u32,
    max_penalty: u32,
) -> Result<u32, crate::MojoError> {
    let [value] = runtime_health_policy::<3, 1>(
        RUNTIME_HEALTH_POLICY_LATENCY_FAILURE_SCORE,
        [
            i64::from(current_score),
            i64::from(failure_penalty),
            i64::from(max_penalty),
        ],
    )?;
    u32::try_from(value).map_err(|_| crate::MojoError::InvalidOutput)
}
