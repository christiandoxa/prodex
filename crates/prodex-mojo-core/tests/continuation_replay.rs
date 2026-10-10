#![cfg(feature = "mojo-runtime")]

use prodex_mojo_core::runtime::{
    RuntimeContinuationBindingSource, RuntimeContinuationBindingSourceInput,
    RuntimeContinuationOwnerKind, RuntimePreviousResponseCandidateAction,
    RuntimePreviousResponseCandidateInput, RuntimePreviousResponseOwnerAction,
    RuntimePreviousResponseOwnerInput, profile_selection_order_batch,
    runtime_continuation_binding_source_plan, runtime_previous_response_candidate_plan,
    runtime_previous_response_owner_plan,
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
fn required_mojo_previous_response_boundaries_fail_closed_with_nonzero_cases() {
    const { assert!(prodex_mojo_core::MOJO_ACTIVE) }

    let base = owner_input(RuntimeContinuationOwnerKind::Owned);
    let owner_cases = [
        (
            false,
            true,
            RuntimeContinuationOwnerKind::Owned,
            RuntimePreviousResponseOwnerAction::Unbound,
        ),
        (
            true,
            false,
            RuntimeContinuationOwnerKind::Owned,
            RuntimePreviousResponseOwnerAction::Unusable,
        ),
        (
            true,
            true,
            RuntimeContinuationOwnerKind::Unavailable,
            RuntimePreviousResponseOwnerAction::Unusable,
        ),
        (
            true,
            true,
            RuntimeContinuationOwnerKind::Conflict,
            RuntimePreviousResponseOwnerAction::Conflict,
        ),
    ];
    let mut exercised = 0;
    for (id_present, id_valid, owner_kind, expected) in owner_cases {
        let mut input = base;
        input.id_present = id_present;
        input.id_valid = id_valid;
        input.owner_kind = owner_kind;
        assert_eq!(
            runtime_previous_response_owner_plan(input).unwrap(),
            expected
        );
        exercised += 1;
    }
    assert!(exercised > 0);

    let mut candidate = RuntimePreviousResponseCandidateInput {
        negative_cache: false,
        auth_failure: false,
        quota_exhausted: false,
        quota_guard: false,
        cached_auth_present: false,
        cached_auth_compatible: false,
        allow_disk_fallback: true,
    };
    assert_eq!(
        runtime_previous_response_candidate_plan(candidate).unwrap(),
        RuntimePreviousResponseCandidateAction::DiskFallback,
        "an expired negative/auth/quota observation may use the disk fallback"
    );
    candidate.allow_disk_fallback = false;
    assert_eq!(
        runtime_previous_response_candidate_plan(candidate).unwrap(),
        RuntimePreviousResponseCandidateAction::Skip
    );
    candidate.allow_disk_fallback = true;
    candidate.cached_auth_present = true;
    assert_eq!(
        runtime_previous_response_candidate_plan(candidate).unwrap(),
        RuntimePreviousResponseCandidateAction::Skip,
        "an incompatible cached row must not silently fall back to disk"
    );
    candidate.cached_auth_compatible = true;
    assert_eq!(
        runtime_previous_response_candidate_plan(candidate).unwrap(),
        RuntimePreviousResponseCandidateAction::SelectCached
    );
    candidate.quota_guard = true;
    assert_eq!(
        runtime_previous_response_candidate_plan(candidate).unwrap(),
        RuntimePreviousResponseCandidateAction::RejectQuota
    );
    candidate.auth_failure = true;
    assert_eq!(
        runtime_previous_response_candidate_plan(candidate).unwrap(),
        RuntimePreviousResponseCandidateAction::RejectAuth
    );
    candidate.negative_cache = true;
    assert_eq!(
        runtime_previous_response_candidate_plan(candidate).unwrap(),
        RuntimePreviousResponseCandidateAction::RejectNegativeCache
    );
}

#[test]
fn required_mojo_previous_response_ordering_handles_empty_duplicate_and_malformed_rows() {
    const { assert!(prodex_mojo_core::MOJO_ACTIVE) }

    assert_eq!(
        profile_selection_order_batch(&[], None, false).unwrap(),
        Vec::<usize>::new()
    );
    assert_eq!(
        profile_selection_order_batch(&[0, 0, 0], Some(1), true).unwrap(),
        [1, 2, 0],
        "equal provider priorities retain current-relative order"
    );
    assert_eq!(
        profile_selection_order_batch(&[0, 0, 0], Some(1), false).unwrap(),
        [2, 0],
        "current profile exclusion stays bounded and deterministic"
    );
    assert_eq!(
        profile_selection_order_batch(&[0], Some(1), true),
        Err(prodex_mojo_core::MojoError::InvalidInput)
    );
    let oversized = vec![0; 257];
    assert_eq!(
        profile_selection_order_batch(&oversized, None, false),
        Err(prodex_mojo_core::MojoError::InvalidInput)
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
