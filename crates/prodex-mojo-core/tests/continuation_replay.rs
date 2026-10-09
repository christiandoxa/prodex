#![cfg(feature = "mojo-runtime")]

use prodex_mojo_core::runtime::{
    RuntimeContinuationBindingSource, RuntimeContinuationBindingSourceInput,
    RuntimeContinuationOwnerKind, RuntimePreviousResponseCandidateAction,
    RuntimePreviousResponseCandidateInput, RuntimePreviousResponseOwnerAction,
    RuntimePreviousResponseOwnerInput, runtime_continuation_binding_source_plan,
    runtime_previous_response_candidate_plan, runtime_previous_response_owner_plan,
};
use prodex_mojo_core::runtime_responses_attempt::{
    RuntimeResponsesAttemptRecoveryInput, RuntimeResponsesAttemptRecoveryPlan,
    runtime_responses_attempt_recovery_plan,
};

fn owner_input(owner_kind: RuntimeContinuationOwnerKind) -> RuntimePreviousResponseOwnerInput {
    RuntimePreviousResponseOwnerInput {
        id_present: true,
        id_valid: true,
        owner_kind,
        excluded: false,
        auth_failure: false,
        negative_cache: false,
        binding_present: true,
        identity_matches: true,
    }
}

#[test]
fn required_mojo_owner_plan_covers_missing_invalid_and_unusable_ids() {
    const { assert!(prodex_mojo_core::MOJO_ACTIVE) }

    let mut missing = owner_input(RuntimeContinuationOwnerKind::Unbound);
    missing.id_present = false;
    assert_eq!(
        runtime_previous_response_owner_plan(missing).unwrap(),
        RuntimePreviousResponseOwnerAction::Unbound
    );

    let mut invalid = owner_input(RuntimeContinuationOwnerKind::Owned);
    assert_eq!(
        runtime_previous_response_owner_plan(invalid).unwrap(),
        RuntimePreviousResponseOwnerAction::Usable
    );
    invalid.id_valid = false;
    assert_eq!(
        runtime_previous_response_owner_plan(invalid).unwrap(),
        RuntimePreviousResponseOwnerAction::Unusable
    );

    for owner_kind in [
        RuntimeContinuationOwnerKind::Unavailable,
        RuntimeContinuationOwnerKind::Conflict,
    ] {
        assert_ne!(
            runtime_previous_response_owner_plan(owner_input(owner_kind)).unwrap(),
            RuntimePreviousResponseOwnerAction::Usable
        );
    }
}

#[test]
fn required_mojo_candidate_plan_preserves_negative_auth_quota_precedence() {
    let mut input = RuntimePreviousResponseCandidateInput {
        negative_cache: false,
        auth_failure: false,
        quota_exhausted: false,
        quota_guard: false,
        cached_auth_present: false,
        cached_auth_compatible: false,
        allow_disk_fallback: true,
    };
    assert_eq!(
        runtime_previous_response_candidate_plan(input).unwrap(),
        RuntimePreviousResponseCandidateAction::DiskFallback
    );
    input.quota_guard = true;
    assert_eq!(
        runtime_previous_response_candidate_plan(input).unwrap(),
        RuntimePreviousResponseCandidateAction::RejectQuota
    );
    input.auth_failure = true;
    assert_eq!(
        runtime_previous_response_candidate_plan(input).unwrap(),
        RuntimePreviousResponseCandidateAction::RejectAuth
    );
    input.negative_cache = true;
    assert_eq!(
        runtime_previous_response_candidate_plan(input).unwrap(),
        RuntimePreviousResponseCandidateAction::RejectNegativeCache
    );
}

#[test]
fn required_mojo_binding_plan_keeps_turn_state_ahead_of_session_conflicts() {
    assert_eq!(
        runtime_continuation_binding_source_plan(RuntimeContinuationBindingSourceInput {
            turn_state_present: true,
            session_id_present: true,
            owner_kind: RuntimeContinuationOwnerKind::Owned,
        })
        .unwrap(),
        RuntimeContinuationBindingSource::TurnState
    );
    assert_eq!(
        runtime_continuation_binding_source_plan(RuntimeContinuationBindingSourceInput {
            turn_state_present: true,
            session_id_present: true,
            owner_kind: RuntimeContinuationOwnerKind::Conflict,
        })
        .unwrap(),
        RuntimeContinuationBindingSource::ConflictTurnState
    );
}

#[test]
fn required_mojo_replay_plan_closes_missing_history_and_committed_streams() {
    const { assert!(prodex_mojo_core::MOJO_ACTIVE) }

    let mut input = RuntimeResponsesAttemptRecoveryInput {
        exact_invalid_previous_response_id: true,
        full_history_fallback_used: false,
        previous_response_present: true,
        owner_matches_profile: true,
        session_present: true,
        reconstructable_full_history: true,
        stream_committed: false,
        hard_affinity: true,
    };
    assert_eq!(
        runtime_responses_attempt_recovery_plan(input).unwrap(),
        RuntimeResponsesAttemptRecoveryPlan::RetryFullHistory
    );
    input.reconstructable_full_history = false;
    assert_eq!(
        runtime_responses_attempt_recovery_plan(input).unwrap(),
        RuntimeResponsesAttemptRecoveryPlan::NoRetry
    );
    input.reconstructable_full_history = true;
    input.stream_committed = true;
    assert_eq!(
        runtime_responses_attempt_recovery_plan(input).unwrap(),
        RuntimeResponsesAttemptRecoveryPlan::NoRetry
    );
}
