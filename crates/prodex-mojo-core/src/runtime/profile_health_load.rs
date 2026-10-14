//! Load and latency projections through the shared profile-health policy ABI.
use super::*;

pub fn profile_inflight_weight(heavy_context: bool) -> Result<usize, crate::MojoError> {
    let [value] = runtime_health_policy::<1, 1>(
        RUNTIME_HEALTH_POLICY_INFLIGHT_WEIGHT,
        [u64::from(heavy_context)],
    )?;
    usize::try_from(value).map_err(|_| crate::MojoError::InvalidOutput)
}

pub fn profile_inflight_effective_hard_limit(
    configured_limit: usize,
    weight: usize,
) -> Result<usize, crate::MojoError> {
    let [value] = runtime_health_policy::<2, 1>(
        RUNTIME_HEALTH_POLICY_INFLIGHT_HARD_LIMIT,
        [configured_limit as u64, weight as u64],
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
            u64::try_from(route_kind).map_err(|_| crate::MojoError::InvalidInput)?,
            u64::from(pressure_mode),
            base_limit as u64,
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
            elapsed_ms,
            u64::try_from(route_kind).map_err(|_| crate::MojoError::InvalidInput)?,
            u64::try_from(stage_kind).map_err(|_| crate::MojoError::InvalidInput)?,
            u64::from(max_penalty),
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
        [u64::from(current_score), u64::from(observed_penalty)],
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
            u64::from(current_score),
            u64::from(failure_penalty),
            u64::from(max_penalty),
        ],
    )?;
    u32::try_from(value).map_err(|_| crate::MojoError::InvalidOutput)
}
