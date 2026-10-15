use crate::MojoError;

unsafe extern "C" {
    fn prodex_launch_usage_limit_recovery_candidate_v1(
        abi_version: i64,
        allow_failed_profile: i64,
        skip_quota_check: i64,
        failed_profile_eligible: i64,
        failed_profile_attempted: i64,
        failed_profile_ready: i64,
        candidate_eligible_address: u64,
        candidate_attempted_address: u64,
        candidate_count: i64,
        output_address: u64,
    ) -> i64;
}

const MAX_CANDIDATES: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsageLimitRecoverySelection {
    None,
    Candidate(usize),
    FailedProfile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UsageLimitRecoveryCandidateInput<'a> {
    pub allow_failed_profile: bool,
    pub skip_quota_check: bool,
    pub failed_profile_eligible: bool,
    pub failed_profile_attempted: bool,
    pub failed_profile_ready: bool,
    pub candidate_eligible: &'a [bool],
    pub candidate_attempted: &'a [bool],
}

/// Select the next bounded session-recovery profile from host-acquired facts.
/// Rust retains profile/auth/quota reads and applies the returned selection.
pub fn usage_limit_recovery_candidate(
    input: UsageLimitRecoveryCandidateInput<'_>,
) -> Result<UsageLimitRecoverySelection, MojoError> {
    if input.candidate_eligible.len() != input.candidate_attempted.len()
        || input.candidate_eligible.len() > MAX_CANDIDATES
    {
        return Err(MojoError::InvalidInput);
    }
    let eligible = input
        .candidate_eligible
        .iter()
        .map(|value| i64::from(*value))
        .collect::<Vec<_>>();
    let attempted = input
        .candidate_attempted
        .iter()
        .map(|value| i64::from(*value))
        .collect::<Vec<_>>();
    let mut selected = -1_i64;
    let status = unsafe {
        prodex_launch_usage_limit_recovery_candidate_v1(
            1,
            i64::from(input.allow_failed_profile),
            i64::from(input.skip_quota_check),
            i64::from(input.failed_profile_eligible),
            i64::from(input.failed_profile_attempted),
            i64::from(input.failed_profile_ready),
            eligible.as_ptr() as u64,
            attempted.as_ptr() as u64,
            i64::try_from(eligible.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut selected as *mut i64) as u64,
        )
    };
    match status {
        0 => {}
        -3 => return Err(MojoError::AbiMismatch),
        -2 => return Err(MojoError::InvalidInput),
        _ => return Err(MojoError::InvalidOutput),
    }
    match selected {
        -1 => Ok(UsageLimitRecoverySelection::None),
        value if value >= 0 && usize::try_from(value).is_ok_and(|index| index < eligible.len()) => {
            Ok(UsageLimitRecoverySelection::Candidate(
                usize::try_from(value).map_err(|_| MojoError::InvalidOutput)?,
            ))
        }
        value if value == i64::try_from(eligible.len()).unwrap_or(-2) => {
            Ok(UsageLimitRecoverySelection::FailedProfile)
        }
        _ => Err(MojoError::InvalidOutput),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidate_order_and_attempt_budget_are_mojo_owned() {
        assert_eq!(
            usage_limit_recovery_candidate(UsageLimitRecoveryCandidateInput {
                allow_failed_profile: true,
                skip_quota_check: false,
                failed_profile_eligible: true,
                failed_profile_attempted: false,
                failed_profile_ready: false,
                candidate_eligible: &[false, true, true],
                candidate_attempted: &[false, false, true],
            }),
            Ok(UsageLimitRecoverySelection::Candidate(1))
        );
    }

    #[test]
    fn failed_profile_is_only_the_last_bounded_fallback() {
        let input = UsageLimitRecoveryCandidateInput {
            allow_failed_profile: true,
            skip_quota_check: false,
            failed_profile_eligible: true,
            failed_profile_attempted: false,
            failed_profile_ready: true,
            candidate_eligible: &[false, false],
            candidate_attempted: &[false, false],
        };
        assert_eq!(
            usage_limit_recovery_candidate(input),
            Ok(UsageLimitRecoverySelection::FailedProfile)
        );
        assert_eq!(
            usage_limit_recovery_candidate(UsageLimitRecoveryCandidateInput {
                failed_profile_ready: false,
                ..input
            }),
            Ok(UsageLimitRecoverySelection::None)
        );
        assert_eq!(
            usage_limit_recovery_candidate(UsageLimitRecoveryCandidateInput {
                skip_quota_check: true,
                failed_profile_ready: false,
                ..input
            }),
            Ok(UsageLimitRecoverySelection::FailedProfile)
        );
    }

    #[test]
    fn candidate_abi_rejects_mismatched_or_oversized_inputs_without_writing() {
        assert_eq!(
            usage_limit_recovery_candidate(UsageLimitRecoveryCandidateInput {
                candidate_eligible: &[true],
                candidate_attempted: &[],
                ..UsageLimitRecoveryCandidateInput {
                    allow_failed_profile: false,
                    skip_quota_check: false,
                    failed_profile_eligible: false,
                    failed_profile_attempted: false,
                    failed_profile_ready: false,
                    candidate_eligible: &[],
                    candidate_attempted: &[],
                }
            }),
            Err(MojoError::InvalidInput)
        );
        let mut output = 77_i64;
        let values = [0_i64; MAX_CANDIDATES + 1];
        assert_eq!(
            unsafe {
                prodex_launch_usage_limit_recovery_candidate_v1(
                    2,
                    0,
                    0,
                    0,
                    0,
                    0,
                    values.as_ptr() as u64,
                    values.as_ptr() as u64,
                    (MAX_CANDIDATES + 1) as i64,
                    (&mut output as *mut i64) as u64,
                )
            },
            -3
        );
        assert_eq!(output, 77);
        assert_eq!(
            unsafe {
                prodex_launch_usage_limit_recovery_candidate_v1(
                    1,
                    0,
                    0,
                    0,
                    0,
                    0,
                    values.as_ptr() as u64,
                    values.as_ptr() as u64,
                    (MAX_CANDIDATES + 1) as i64,
                    (&mut output as *mut i64) as u64,
                )
            },
            -2
        );
        assert_eq!(output, 77);
    }
}
