#![cfg(feature = "mojo-runtime")]

use prodex_mojo_core::runtime_responses_attempt::{
    RuntimeResponsesAttemptRecoveryInput, RuntimeResponsesAttemptRecoveryPlan,
    RuntimeResponsesAttemptRetrySignalInput, RuntimeResponsesAttemptRetrySignalPlan,
    runtime_responses_attempt_recovery_plan, runtime_responses_attempt_retry_signal_plan,
};

fn valid_input() -> RuntimeResponsesAttemptRecoveryInput {
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
fn real_mojo_recovery_plan_preserves_precommit_owner_precedence() {
    const { assert!(prodex_mojo_core::MOJO_ACTIVE) }

    assert_eq!(
        runtime_responses_attempt_recovery_plan(valid_input()).unwrap(),
        RuntimeResponsesAttemptRecoveryPlan::RetryFullHistory
    );

    let mut foreign_owner = valid_input();
    foreign_owner.owner_matches_profile = false;
    assert_eq!(
        runtime_responses_attempt_recovery_plan(foreign_owner).unwrap(),
        RuntimeResponsesAttemptRecoveryPlan::NoRetry
    );
}

#[test]
fn real_mojo_recovery_plan_keeps_committed_streams_and_one_shot_retries_closed() {
    let mut committed = valid_input();
    committed.stream_committed = true;
    assert_eq!(
        runtime_responses_attempt_recovery_plan(committed).unwrap(),
        RuntimeResponsesAttemptRecoveryPlan::NoRetry
    );

    let mut already_recovered = valid_input();
    already_recovered.full_history_fallback_used = true;
    assert_eq!(
        runtime_responses_attempt_recovery_plan(already_recovered).unwrap(),
        RuntimeResponsesAttemptRecoveryPlan::NoRetry
    );
}

#[test]
fn real_mojo_retry_signal_plan_changes_for_each_recovery_fact() {
    const { assert!(prodex_mojo_core::MOJO_ACTIVE) }

    let valid = RuntimeResponsesAttemptRetrySignalInput {
        previous_response_present: true,
        owner_matches_profile: true,
        fallback_available: true,
    };
    assert_eq!(
        runtime_responses_attempt_retry_signal_plan(valid).unwrap(),
        RuntimeResponsesAttemptRetrySignalPlan::RetryFullHistory
    );
    for mutate in [
        |input: &mut RuntimeResponsesAttemptRetrySignalInput| {
            input.previous_response_present = false
        },
        |input: &mut RuntimeResponsesAttemptRetrySignalInput| input.owner_matches_profile = false,
        |input: &mut RuntimeResponsesAttemptRetrySignalInput| input.fallback_available = false,
    ] {
        let mut candidate = valid;
        mutate(&mut candidate);
        assert_eq!(
            runtime_responses_attempt_retry_signal_plan(candidate).unwrap(),
            RuntimeResponsesAttemptRetrySignalPlan::NoRetry,
            "Mojo retry signal must be sensitive to {candidate:?}"
        );
    }
}
