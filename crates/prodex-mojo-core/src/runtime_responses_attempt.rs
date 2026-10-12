//! Mojo-owned pre-commit Responses recovery planning.

use crate::MojoError;

const ABI_VERSION: i64 = 1;
const RETRY_SIGNAL_ABI_VERSION: i64 = 1;

unsafe extern "C" {
    fn prodex_runtime_responses_attempt_recovery_v1(
        abi_version: i64,
        exact_invalid_previous_response_id: i64,
        full_history_fallback_used: i64,
        previous_response_present: i64,
        owner_matches_profile: i64,
        session_present: i64,
        reconstructable_full_history: i64,
        stream_committed: i64,
        hard_affinity: i64,
    ) -> i64;
    fn prodex_runtime_responses_attempt_retry_signal_v1(
        abi_version: i64,
        previous_response_present: i64,
        owner_matches_profile: i64,
        fallback_available: i64,
    ) -> i64;
}

/// Facts observed by Rust before a Responses retry or recovery decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeResponsesAttemptRecoveryInput {
    pub exact_invalid_previous_response_id: bool,
    pub full_history_fallback_used: bool,
    pub previous_response_present: bool,
    pub owner_matches_profile: bool,
    pub session_present: bool,
    pub reconstructable_full_history: bool,
    pub stream_committed: bool,
    pub hard_affinity: bool,
}

/// Side-effect-free recovery action selected by Mojo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeResponsesAttemptRecoveryPlan {
    NoRetry,
    RetryFullHistory,
}

/// Facts observed before deciding whether a failed continuation may request a full-history retry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeResponsesAttemptRetrySignalInput {
    pub previous_response_present: bool,
    pub owner_matches_profile: bool,
    pub fallback_available: bool,
}

/// Side-effect-free full-history retry signal selected by Mojo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeResponsesAttemptRetrySignalPlan {
    NoRetry,
    RetryFullHistory,
}

/// Decide whether a stale incremental Responses request may retry once from full history.
pub fn runtime_responses_attempt_recovery_plan(
    input: RuntimeResponsesAttemptRecoveryInput,
) -> Result<RuntimeResponsesAttemptRecoveryPlan, MojoError> {
    let result = unsafe {
        prodex_runtime_responses_attempt_recovery_v1(
            ABI_VERSION,
            i64::from(input.exact_invalid_previous_response_id),
            i64::from(input.full_history_fallback_used),
            i64::from(input.previous_response_present),
            i64::from(input.owner_matches_profile),
            i64::from(input.session_present),
            i64::from(input.reconstructable_full_history),
            i64::from(input.stream_committed),
            i64::from(input.hard_affinity),
        )
    };
    match result {
        0 => Ok(RuntimeResponsesAttemptRecoveryPlan::NoRetry),
        1 => Ok(RuntimeResponsesAttemptRecoveryPlan::RetryFullHistory),
        -1 => Err(MojoError::InvalidInput),
        -4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Decide whether a failed Responses continuation may signal full-history replay.
pub fn runtime_responses_attempt_retry_signal_plan(
    input: RuntimeResponsesAttemptRetrySignalInput,
) -> Result<RuntimeResponsesAttemptRetrySignalPlan, MojoError> {
    let result = unsafe {
        prodex_runtime_responses_attempt_retry_signal_v1(
            RETRY_SIGNAL_ABI_VERSION,
            i64::from(input.previous_response_present),
            i64::from(input.owner_matches_profile),
            i64::from(input.fallback_available),
        )
    };
    match result {
        0 => Ok(RuntimeResponsesAttemptRetrySignalPlan::NoRetry),
        1 => Ok(RuntimeResponsesAttemptRetrySignalPlan::RetryFullHistory),
        -1 => Err(MojoError::InvalidInput),
        -4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input() -> RuntimeResponsesAttemptRecoveryInput {
        RuntimeResponsesAttemptRecoveryInput {
            exact_invalid_previous_response_id: true,
            full_history_fallback_used: false,
            previous_response_present: true,
            owner_matches_profile: true,
            session_present: true,
            reconstructable_full_history: true,
            stream_committed: false,
            hard_affinity: true,
        }
    }

    #[test]
    fn recovery_requires_owner_history_and_precommit_boundary() {
        assert_eq!(
            runtime_responses_attempt_recovery_plan(input()).unwrap(),
            RuntimeResponsesAttemptRecoveryPlan::RetryFullHistory
        );
        let mutators: &[fn(&mut RuntimeResponsesAttemptRecoveryInput)] = &[
            |input: &mut RuntimeResponsesAttemptRecoveryInput| {
                input.exact_invalid_previous_response_id = false
            },
            |input: &mut RuntimeResponsesAttemptRecoveryInput| {
                input.full_history_fallback_used = true
            },
            |input: &mut RuntimeResponsesAttemptRecoveryInput| {
                input.previous_response_present = false
            },
            |input: &mut RuntimeResponsesAttemptRecoveryInput| input.owner_matches_profile = false,
            |input: &mut RuntimeResponsesAttemptRecoveryInput| input.session_present = false,
            |input: &mut RuntimeResponsesAttemptRecoveryInput| {
                input.reconstructable_full_history = false
            },
            |input: &mut RuntimeResponsesAttemptRecoveryInput| input.stream_committed = true,
        ];
        for mutate in mutators {
            let mut candidate = input();
            mutate(&mut candidate);
            assert_eq!(
                runtime_responses_attempt_recovery_plan(candidate).unwrap(),
                RuntimeResponsesAttemptRecoveryPlan::NoRetry,
                "recovery must fail closed for {candidate:?}"
            );
        }
    }

    #[test]
    fn hard_affinity_never_recovers_on_a_foreign_owner() {
        let mut candidate = input();
        candidate.owner_matches_profile = false;
        assert_eq!(
            runtime_responses_attempt_recovery_plan(candidate).unwrap(),
            RuntimeResponsesAttemptRecoveryPlan::NoRetry
        );

        candidate.hard_affinity = false;
        assert_eq!(
            runtime_responses_attempt_recovery_plan(candidate).unwrap(),
            RuntimeResponsesAttemptRecoveryPlan::NoRetry
        );
    }

    #[test]
    fn malformed_and_unknown_abi_inputs_fail_closed() {
        assert_eq!(
            unsafe { prodex_runtime_responses_attempt_recovery_v1(0, 1, 0, 1, 1, 1, 1, 0, 1,) },
            -4
        );
        assert_eq!(
            unsafe {
                prodex_runtime_responses_attempt_recovery_v1(ABI_VERSION, 2, 0, 1, 1, 1, 1, 0, 1)
            },
            -1
        );
        assert_eq!(
            unsafe {
                prodex_runtime_responses_attempt_recovery_v1(ABI_VERSION, 1, 0, 1, 1, 1, 1, 0, 2)
            },
            -1
        );
    }

    #[test]
    fn retry_signal_requires_owned_previous_response_and_fallback() {
        let valid = RuntimeResponsesAttemptRetrySignalInput {
            previous_response_present: true,
            owner_matches_profile: true,
            fallback_available: true,
        };
        assert_eq!(
            runtime_responses_attempt_retry_signal_plan(valid).unwrap(),
            RuntimeResponsesAttemptRetrySignalPlan::RetryFullHistory
        );
        let mutators: &[fn(&mut RuntimeResponsesAttemptRetrySignalInput)] = &[
            |input: &mut RuntimeResponsesAttemptRetrySignalInput| {
                input.previous_response_present = false
            },
            |input: &mut RuntimeResponsesAttemptRetrySignalInput| {
                input.owner_matches_profile = false
            },
            |input: &mut RuntimeResponsesAttemptRetrySignalInput| input.fallback_available = false,
        ];
        for mutate in mutators {
            let mut candidate = valid;
            mutate(&mut candidate);
            assert_eq!(
                runtime_responses_attempt_retry_signal_plan(candidate).unwrap(),
                RuntimeResponsesAttemptRetrySignalPlan::NoRetry,
                "retry signal must fail closed for {candidate:?}"
            );
        }
    }
}
