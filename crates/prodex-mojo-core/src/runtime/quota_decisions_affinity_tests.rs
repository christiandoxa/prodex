use super::*;

fn compact_retry_input(stage: CompactRetryStage) -> CompactRetryDecisionInput<'static> {
    CompactRetryDecisionInput {
        stage,
        overload: false,
        auto_redeemed: false,
        owner_retry_used: false,
        quota_fallback_available: None,
        committed: false,
        candidate_profile: "candidate",
        current_profile: "current",
        compact_followup_profile: None,
        previous_response_profile: None,
        session_profile: None,
    }
}

#[test]
fn compact_retry_hard_affinity_matches_only_bound_profiles() {
    let mut input = compact_retry_input(CompactRetryStage::AfterBackoff);
    input.overload = true;
    assert_eq!(
        compact_retry_decision(input).unwrap().action,
        CompactRetryAction::RotateOverload
    );

    input.current_profile = input.candidate_profile;
    assert_eq!(
        compact_retry_decision(input).unwrap().action,
        CompactRetryAction::RotateOverload,
        "the current profile is an overload retry owner, not hard affinity"
    );

    input.current_profile = "other-owner";
    input.compact_followup_profile = Some(input.candidate_profile);
    assert_eq!(
        compact_retry_decision(input).unwrap().action,
        CompactRetryAction::RecoverHardAffinity
    );

    input.compact_followup_profile = None;
    input.previous_response_profile = Some(input.candidate_profile);
    assert_eq!(
        compact_retry_decision(input).unwrap().action,
        CompactRetryAction::RecoverHardAffinity
    );

    input.previous_response_profile = None;
    input.session_profile = Some(input.candidate_profile);
    assert_eq!(
        compact_retry_decision(input).unwrap().action,
        CompactRetryAction::RecoverHardAffinity
    );

    input.committed = true;
    assert_eq!(
        compact_retry_decision(input).unwrap().action,
        CompactRetryAction::ReturnCommitted,
        "a committed compact response must never rotate mid-stream"
    );
}

#[test]
fn compact_retry_affinity_handles_unicode_and_long_profile_names() {
    let candidate = "候補-🌙/".repeat(1_024);
    let other = "候補-🌙/".repeat(1_023) + "x";
    let mut input = CompactRetryDecisionInput {
        stage: CompactRetryStage::AfterBackoff,
        overload: true,
        auto_redeemed: false,
        owner_retry_used: false,
        quota_fallback_available: None,
        committed: false,
        candidate_profile: &candidate,
        current_profile: "other",
        compact_followup_profile: Some(candidate.as_str()),
        previous_response_profile: None,
        session_profile: None,
    };
    assert_eq!(
        compact_retry_decision(input).unwrap().action,
        CompactRetryAction::RecoverHardAffinity
    );

    input.compact_followup_profile = Some(other.as_str());
    assert_eq!(
        compact_retry_decision(input).unwrap().action,
        CompactRetryAction::RotateOverload
    );
}

#[test]
fn compact_retry_raw_abi_rejects_invalid_affinity_inputs_without_output() {
    let profile_views = [super::super::RuntimeStringView { ptr: 0, len: 0 }; 5];
    let mut output = [i64::MIN; 2];
    let status = unsafe {
        prodex_runtime_compact_retry_decision_v3(
            COMPACT_RETRY_DECISION_ABI_VERSION,
            profile_views.as_ptr(),
            1,
            1,
            0,
            0,
            0,
            6,
            0,
            0,
            0,
            -1,
            0,
            output.as_mut_ptr(),
        )
    };
    assert_eq!(status, 1);
    assert_eq!(output, [i64::MIN; 2]);

    let status = unsafe {
        prodex_runtime_compact_retry_decision_v3(
            COMPACT_RETRY_DECISION_ABI_VERSION - 1,
            profile_views.as_ptr(),
            1,
            1,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            -1,
            0,
            output.as_mut_ptr(),
        )
    };
    assert_eq!(status, -4);

    let invalid_presence = unsafe {
        prodex_runtime_compact_retry_decision_v3(
            COMPACT_RETRY_DECISION_ABI_VERSION,
            profile_views.as_ptr(),
            2,
            1,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            -1,
            0,
            output.as_mut_ptr(),
        )
    };
    assert_eq!(invalid_presence, 1);

    let invalid_view = [super::super::RuntimeStringView { ptr: 0, len: 1 }; 5];
    let invalid_pointer = unsafe {
        prodex_runtime_compact_retry_decision_v3(
            COMPACT_RETRY_DECISION_ABI_VERSION,
            invalid_view.as_ptr(),
            1,
            1,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            -1,
            0,
            output.as_mut_ptr(),
        )
    };
    assert_eq!(invalid_pointer, 1);
}
