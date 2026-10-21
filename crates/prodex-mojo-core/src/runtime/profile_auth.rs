//! Runtime usage-auth cache revalidation and HTTP failure scoring in Mojo.
use crate::MojoError;

const ABI_VERSION: i64 = 1;

unsafe extern "C" {
    fn prodex_runtime_profile_auth_cache_policy_v1(
        abi_version: i64,
        operation: i64,
        fields_address: u64,
        field_count: i64,
        output_address: u64,
    ) -> i64;
}

fn policy(operation: i64, facts: [i64; 4]) -> Result<i64, MojoError> {
    let mut output = -1_i64;
    let status = unsafe {
        prodex_runtime_profile_auth_cache_policy_v1(
            ABI_VERSION,
            operation,
            facts.as_ptr() as usize as u64,
            4,
            (&mut output as *mut i64) as usize as u64,
        )
    };
    match status {
        0 => Ok(output),
        1 => Err(MojoError::InvalidInput),
        4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Choose whether cached usage auth needs a new background inspection.
/// The host still gets quota-auth proactive-refresh advice separately.
pub fn profile_usage_auth_revalidation_due(
    now: i64,
    checked_at: i64,
    fresh_seconds: i64,
    proactive_refresh: bool,
) -> Result<bool, MojoError> {
    match policy(
        0,
        [now, checked_at, fresh_seconds, i64::from(proactive_refresh)],
    )? {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Compute the score to persist following an HTTP auth failure.
/// A 401 receives the supplied unauthorized score; all other codes receive
/// the historical alternate score, without another Rust status match.
pub fn profile_auth_failure_next_score(
    status: u16,
    existing_score: u32,
    unauthorized_score: u32,
    alternate_score: u32,
) -> Result<u32, MojoError> {
    u32::try_from(policy(
        1,
        [
            i64::from(status),
            i64::from(existing_score),
            i64::from(unauthorized_score),
            i64::from(alternate_score),
        ],
    )?)
    .map_err(|_| MojoError::InvalidOutput)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiled_auth_cache_refresh_respects_clock_jump_ttl_and_proactive_flag() {
        for (now, checked_at, ttl, proactive, due) in [
            (100, 100, 30, false, false),
            (129, 100, 30, false, false),
            (130, 100, 30, false, true),
            (80, 100, 30, false, true),
            (100, 100, 30, true, true),
            (i64::MAX, i64::MIN, 30, false, true),
            (i64::MIN, i64::MAX, 30, false, true),
            (0, 0, -1, false, true),
        ] {
            assert_eq!(
                profile_usage_auth_revalidation_due(now, checked_at, ttl, proactive),
                Ok(due)
            );
        }
    }

    #[test]
    fn compiled_auth_failure_status_scoring_keeps_existing_higher_score() {
        assert_eq!(profile_auth_failure_next_score(401, 0, 12, 2), Ok(12));
        assert_eq!(profile_auth_failure_next_score(403, 0, 12, 2), Ok(2));
        assert_eq!(profile_auth_failure_next_score(429, 0, 12, 2), Ok(2));
        assert_eq!(profile_auth_failure_next_score(401, 20, 12, 2), Ok(20));
        assert_eq!(
            profile_auth_failure_next_score(401, u32::MAX, 12, 2),
            Ok(u32::MAX)
        );
    }
}
