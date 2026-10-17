//! Typed ABI for the compiled previous-response recovery transition.

use crate::MojoError;

const ABI_VERSION: i64 = 1;
const RESULT_FIELDS: i64 = 7;

unsafe extern "C" {
    fn prodex_runtime_previous_response_recovery_transition_v1(
        abi_version: i64,
        retry_delay_present: i64,
        stale_continuation: i64,
        fresh_fallback_blocked: i64,
        policy_fail_stale: i64,
        policy_log_fresh_fallback_blocked: i64,
        policy_clear_trusted_affinity: i64,
        retry_index: u64,
        output_address: u64,
        output_capacity: i64,
    ) -> i64;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviousResponseRecoveryAction {
    RetryOwner,
    StaleContinuation,
    Rotate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreviousResponseRecoveryInput {
    pub retry_delay_present: bool,
    pub stale_continuation: bool,
    pub fresh_fallback_blocked: bool,
    pub fail_stale_continuation: bool,
    pub log_fresh_fallback_blocked: bool,
    pub clear_trusted_affinity_on_rotate: bool,
    pub retry_index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreviousResponseRecoveryTransition {
    pub action: PreviousResponseRecoveryAction,
    pub next_retry_index: usize,
    pub reset_retry_state: bool,
    pub log_fresh_fallback_blocked: bool,
    pub release_affinity: bool,
    pub clear_response_profile_affinity: bool,
    pub clear_trusted_affinity: bool,
}

fn boolean(value: u64) -> Result<bool, MojoError> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Single compiled owner of owner-retry, stale-rejection and affinity-release
/// precedence. Rust retains neither an alternate decision tree nor fallback.
pub fn previous_response_recovery_transition(
    input: PreviousResponseRecoveryInput,
) -> Result<PreviousResponseRecoveryTransition, MojoError> {
    let mut output = [u64::MAX; RESULT_FIELDS as usize];
    let status = unsafe {
        prodex_runtime_previous_response_recovery_transition_v1(
            ABI_VERSION,
            i64::from(input.retry_delay_present),
            i64::from(input.stale_continuation),
            i64::from(input.fresh_fallback_blocked),
            i64::from(input.fail_stale_continuation),
            i64::from(input.log_fresh_fallback_blocked),
            i64::from(input.clear_trusted_affinity_on_rotate),
            u64::try_from(input.retry_index).map_err(|_| MojoError::InvalidInput)?,
            output.as_mut_ptr() as usize as u64,
            RESULT_FIELDS,
        )
    };
    match status {
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    let action = match output[0] {
        0 => PreviousResponseRecoveryAction::RetryOwner,
        1 => PreviousResponseRecoveryAction::StaleContinuation,
        2 => PreviousResponseRecoveryAction::Rotate,
        _ => return Err(MojoError::InvalidOutput),
    };
    let plan = PreviousResponseRecoveryTransition {
        action,
        next_retry_index: usize::try_from(output[1]).map_err(|_| MojoError::InvalidOutput)?,
        reset_retry_state: boolean(output[2])?,
        log_fresh_fallback_blocked: boolean(output[3])?,
        release_affinity: boolean(output[4])?,
        clear_response_profile_affinity: boolean(output[5])?,
        clear_trusted_affinity: boolean(output[6])?,
    };
    // Check the full plan shape, not only the seven individual scalar types.
    if match plan.action {
        PreviousResponseRecoveryAction::RetryOwner => {
            plan.reset_retry_state
                || plan.log_fresh_fallback_blocked
                || plan.release_affinity
                || plan.clear_response_profile_affinity
                || plan.clear_trusted_affinity
        }
        PreviousResponseRecoveryAction::StaleContinuation => {
            !plan.reset_retry_state
                || plan.next_retry_index != 0
                || plan.log_fresh_fallback_blocked
                || plan.release_affinity
                || plan.clear_response_profile_affinity
                || plan.clear_trusted_affinity
        }
        PreviousResponseRecoveryAction::Rotate => {
            !plan.reset_retry_state
                || plan.next_retry_index != 0
                || !plan.release_affinity
                || !plan.clear_response_profile_affinity
        }
    } {
        return Err(MojoError::InvalidOutput);
    }
    Ok(plan)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> PreviousResponseRecoveryInput {
        PreviousResponseRecoveryInput {
            retry_delay_present: false,
            stale_continuation: false,
            fresh_fallback_blocked: false,
            fail_stale_continuation: false,
            log_fresh_fallback_blocked: true,
            clear_trusted_affinity_on_rotate: false,
            retry_index: 3,
        }
    }

    #[test]
    fn mojo_precedence_retries_owner_without_releasing_affinity() {
        let mut input = base();
        input.retry_delay_present = true;
        input.stale_continuation = true;
        input.fail_stale_continuation = true;
        input.clear_trusted_affinity_on_rotate = true;
        input.fresh_fallback_blocked = true;
        let result = previous_response_recovery_transition(input).unwrap();
        assert_eq!(
            result,
            PreviousResponseRecoveryTransition {
                action: PreviousResponseRecoveryAction::RetryOwner,
                next_retry_index: 4,
                reset_retry_state: false,
                log_fresh_fallback_blocked: false,
                release_affinity: false,
                clear_response_profile_affinity: false,
                clear_trusted_affinity: false,
            }
        );
    }

    #[test]
    fn mojo_stale_continuation_rejects_without_rebinding() {
        let mut input = base();
        input.stale_continuation = true;
        input.fail_stale_continuation = true;
        input.clear_trusted_affinity_on_rotate = true;
        input.fresh_fallback_blocked = true;
        let result = previous_response_recovery_transition(input).unwrap();
        assert_eq!(
            result,
            PreviousResponseRecoveryTransition {
                action: PreviousResponseRecoveryAction::StaleContinuation,
                next_retry_index: 0,
                reset_retry_state: true,
                log_fresh_fallback_blocked: false,
                release_affinity: false,
                clear_response_profile_affinity: false,
                clear_trusted_affinity: false,
            }
        );
    }

    #[test]
    fn mojo_rotation_only_releases_affinity_after_stale_policy_gate() {
        for (stale, fail_stale, fresh_blocked, log_blocked, clear_trusted, expected_log) in [
            (false, false, false, false, false, false),
            (false, false, true, false, true, false),
            (true, false, true, true, false, true),
            (false, true, true, true, true, true),
            (true, false, false, true, true, false),
        ] {
            let mut input = base();
            input.stale_continuation = stale;
            input.fail_stale_continuation = fail_stale;
            input.fresh_fallback_blocked = fresh_blocked;
            input.log_fresh_fallback_blocked = log_blocked;
            input.clear_trusted_affinity_on_rotate = clear_trusted;
            let result = previous_response_recovery_transition(input).unwrap();
            assert_eq!(result.action, PreviousResponseRecoveryAction::Rotate);
            assert_eq!(result.next_retry_index, 0);
            assert!(result.reset_retry_state);
            assert_eq!(result.log_fresh_fallback_blocked, expected_log);
            assert!(result.release_affinity);
            assert!(result.clear_response_profile_affinity);
            assert_eq!(result.clear_trusted_affinity, clear_trusted);
        }
    }

    #[test]
    fn recovery_transition_abi_rejects_bad_version_flags_and_overflow() {
        let mut output = [23_u64; 7];
        let out = output.as_mut_ptr() as usize as u64;
        let call = |version, retry, stale, blocked, fail_stale, log, clear, index, dst, cap| unsafe {
            prodex_runtime_previous_response_recovery_transition_v1(
                version, retry, stale, blocked, fail_stale, log, clear, index, dst, cap,
            )
        };
        assert_eq!(call(2, 0, 0, 0, 0, 1, 0, 0, out, 7), 4);
        assert_eq!(call(1, 2, 0, 0, 0, 1, 0, 0, out, 7), 1);
        assert_eq!(call(1, 0, 2, 0, 0, 1, 0, 0, out, 7), 1);
        assert_eq!(call(1, 0, 0, -1, 0, 1, 0, 0, out, 7), 1);
        assert_eq!(call(1, 0, 0, 0, 2, 1, 0, 0, out, 7), 1);
        assert_eq!(call(1, 0, 0, 0, 0, 2, 0, 0, out, 7), 1);
        assert_eq!(call(1, 0, 0, 0, 0, 1, 2, 0, out, 7), 1);
        assert_eq!(call(1, 1, 0, 0, 0, 1, 0, u64::MAX, out, 7), 1);
        assert_eq!(call(1, 0, 0, 0, 0, 1, 0, 0, 0, 7), 1);
        assert_eq!(call(1, 0, 0, 0, 0, 1, 0, 0, out, 6), 1);
        assert_eq!(output, [23; 7]);
    }
}
