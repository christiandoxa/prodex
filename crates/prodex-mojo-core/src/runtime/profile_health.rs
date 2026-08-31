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
