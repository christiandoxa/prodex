#![cfg(feature = "mojo-runtime")]

use prodex_mojo_core::runtime::{
    ResponsesLoopAction, ResponsesLoopInput, ResponsesLoopPhase, responses_loop_action,
};

fn input(phase: ResponsesLoopPhase, bits: u16) -> ResponsesLoopInput {
    ResponsesLoopInput {
        phase,
        budget_exhausted: bits & (1 << 0) != 0,
        hard_affinity: bits & (1 << 1) != 0,
        compact_followup: bits & (1 << 2) != 0,
        transient_recovery_pending: bits & (1 << 3) != 0,
        inflight_relief_pending: bits & (1 << 4) != 0,
        cold_start_pending: bits & (1 << 5) != 0,
        direct_fallback_allowed: bits & (1 << 6) != 0,
        stream_committed: bits & (1 << 7) != 0,
    }
}

fn expected(input: ResponsesLoopInput) -> ResponsesLoopAction {
    if input.stream_committed {
        return ResponsesLoopAction::ReturnWithoutRotation;
    }
    if input.phase == ResponsesLoopPhase::BudgetExhausted {
        if !input.budget_exhausted {
            return ResponsesLoopAction::Attempt;
        }
        if input.compact_followup {
            return ResponsesLoopAction::ReturnCompactFailure;
        }
        if input.transient_recovery_pending && !input.hard_affinity {
            return ResponsesLoopAction::WaitTransientRecovery;
        }
        if input.direct_fallback_allowed && !input.hard_affinity {
            return ResponsesLoopAction::DirectFallback;
        }
        return ResponsesLoopAction::ReturnFinalFailure;
    }
    if input.transient_recovery_pending && !input.hard_affinity {
        return ResponsesLoopAction::WaitTransientRecovery;
    }
    if input.inflight_relief_pending {
        return ResponsesLoopAction::WaitInflightRelief;
    }
    if input.compact_followup {
        return ResponsesLoopAction::ReturnCompactFailure;
    }
    if input.cold_start_pending {
        return ResponsesLoopAction::WaitColdStart;
    }
    if input.direct_fallback_allowed && !input.hard_affinity {
        return ResponsesLoopAction::DirectFallback;
    }
    ResponsesLoopAction::ReturnFinalFailure
}

#[test]
fn responses_loop_golden_cases_cover_affinity_retry_and_commit() {
    let cases = [
        (
            input(ResponsesLoopPhase::BudgetExhausted, 0),
            ResponsesLoopAction::Attempt,
        ),
        (
            input(
                ResponsesLoopPhase::BudgetExhausted,
                1 << 0 | 1 << 2 | 1 << 3,
            ),
            ResponsesLoopAction::ReturnCompactFailure,
        ),
        (
            input(
                ResponsesLoopPhase::BudgetExhausted,
                1 << 0 | 1 << 3 | 1 << 6,
            ),
            ResponsesLoopAction::WaitTransientRecovery,
        ),
        (
            input(
                ResponsesLoopPhase::BudgetExhausted,
                1 << 0 | 1 << 1 | 1 << 6,
            ),
            ResponsesLoopAction::ReturnFinalFailure,
        ),
        (
            input(ResponsesLoopPhase::CandidateExhausted, 1 << 4 | 1 << 2),
            ResponsesLoopAction::WaitInflightRelief,
        ),
        (
            input(ResponsesLoopPhase::CandidateExhausted, 1 << 5 | 1 << 6),
            ResponsesLoopAction::WaitColdStart,
        ),
        (
            input(ResponsesLoopPhase::CandidateExhausted, 1 << 7 | 1 << 6),
            ResponsesLoopAction::ReturnWithoutRotation,
        ),
    ];
    for (input, expected_action) in cases {
        assert_eq!(
            responses_loop_action(input),
            Ok(expected_action),
            "input={input:?}"
        );
    }
}

#[test]
fn responses_loop_state_machine_matches_independent_oracle_with_nonzero_action_counts() {
    let mut counts = [0_usize; 8];
    for phase in [
        ResponsesLoopPhase::BudgetExhausted,
        ResponsesLoopPhase::CandidateExhausted,
    ] {
        for bits in 0_u16..=255 {
            let observed = responses_loop_action(input(phase, bits)).unwrap();
            assert_eq!(
                observed,
                expected(input(phase, bits)),
                "phase={phase:?} bits={bits:08b}"
            );
            let index = match observed {
                ResponsesLoopAction::Attempt => 0,
                ResponsesLoopAction::WaitTransientRecovery => 1,
                ResponsesLoopAction::WaitInflightRelief => 2,
                ResponsesLoopAction::ReturnCompactFailure => 3,
                ResponsesLoopAction::WaitColdStart => 4,
                ResponsesLoopAction::DirectFallback => 5,
                ResponsesLoopAction::ReturnFinalFailure => 6,
                ResponsesLoopAction::ReturnWithoutRotation => 7,
            };
            counts[index] += 1;
        }
    }
    assert_eq!(counts.iter().sum::<usize>(), 512);
    assert!(counts[1] > 0, "retry path must be exercised: {counts:?}");
    assert!(
        counts[7] > 0,
        "commit boundary must be exercised: {counts:?}"
    );
}
