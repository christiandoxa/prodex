use crate::MojoError;

const ABI_VERSION: i64 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum ResponsesLoopPhase {
    BudgetExhausted = 0,
    CandidateExhausted = 1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResponsesLoopInput {
    pub phase: ResponsesLoopPhase,
    pub budget_exhausted: bool,
    pub hard_affinity: bool,
    pub compact_followup: bool,
    pub transient_recovery_pending: bool,
    pub inflight_relief_pending: bool,
    pub cold_start_pending: bool,
    pub direct_fallback_allowed: bool,
    pub stream_committed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResponsesLoopAction {
    Attempt,
    WaitTransientRecovery,
    WaitInflightRelief,
    ReturnCompactFailure,
    WaitColdStart,
    DirectFallback,
    ReturnFinalFailure,
    ReturnWithoutRotation,
}

unsafe extern "C" {
    fn prodex_runtime_responses_loop_action_v1(
        abi_version: i64,
        phase: i64,
        budget_exhausted: i64,
        hard_affinity: i64,
        compact_followup: i64,
        transient_recovery_pending: i64,
        inflight_relief_pending: i64,
        cold_start_pending: i64,
        direct_fallback_allowed: i64,
        stream_committed: i64,
    ) -> i64;
}

/// Plans parent-loop precedence after Rust gathers live runtime facts.
pub fn responses_loop_action(input: ResponsesLoopInput) -> Result<ResponsesLoopAction, MojoError> {
    let result = unsafe {
        prodex_runtime_responses_loop_action_v1(
            ABI_VERSION,
            input.phase as i64,
            i64::from(input.budget_exhausted),
            i64::from(input.hard_affinity),
            i64::from(input.compact_followup),
            i64::from(input.transient_recovery_pending),
            i64::from(input.inflight_relief_pending),
            i64::from(input.cold_start_pending),
            i64::from(input.direct_fallback_allowed),
            i64::from(input.stream_committed),
        )
    };
    match result {
        0 => Ok(ResponsesLoopAction::Attempt),
        1 => Ok(ResponsesLoopAction::WaitTransientRecovery),
        2 => Ok(ResponsesLoopAction::WaitInflightRelief),
        3 => Ok(ResponsesLoopAction::ReturnCompactFailure),
        4 => Ok(ResponsesLoopAction::WaitColdStart),
        5 => Ok(ResponsesLoopAction::DirectFallback),
        6 => Ok(ResponsesLoopAction::ReturnFinalFailure),
        7 => Ok(ResponsesLoopAction::ReturnWithoutRotation),
        -4 => Err(MojoError::AbiMismatch),
        -1 => Err(MojoError::InvalidInput),
        _ => Err(MojoError::InvalidOutput),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn loop_input(phase: ResponsesLoopPhase) -> ResponsesLoopInput {
        ResponsesLoopInput {
            phase,
            budget_exhausted: false,
            hard_affinity: false,
            compact_followup: false,
            transient_recovery_pending: false,
            inflight_relief_pending: false,
            cold_start_pending: false,
            direct_fallback_allowed: false,
            stream_committed: false,
        }
    }

    #[test]
    fn loop_golden_cases_keep_precedence_and_commit_boundary() {
        let mut input = loop_input(ResponsesLoopPhase::BudgetExhausted);
        assert_eq!(
            responses_loop_action(input),
            Ok(ResponsesLoopAction::Attempt)
        );

        input.budget_exhausted = true;
        input.compact_followup = true;
        input.transient_recovery_pending = true;
        input.direct_fallback_allowed = true;
        assert_eq!(
            responses_loop_action(input),
            Ok(ResponsesLoopAction::ReturnCompactFailure)
        );

        input.compact_followup = false;
        assert_eq!(
            responses_loop_action(input),
            Ok(ResponsesLoopAction::WaitTransientRecovery)
        );
        input.hard_affinity = true;
        assert_eq!(
            responses_loop_action(input),
            Ok(ResponsesLoopAction::ReturnFinalFailure)
        );

        input = loop_input(ResponsesLoopPhase::CandidateExhausted);
        input.inflight_relief_pending = true;
        input.compact_followup = true;
        input.cold_start_pending = true;
        input.direct_fallback_allowed = true;
        assert_eq!(
            responses_loop_action(input),
            Ok(ResponsesLoopAction::WaitInflightRelief)
        );
        input.inflight_relief_pending = false;
        assert_eq!(
            responses_loop_action(input),
            Ok(ResponsesLoopAction::ReturnCompactFailure)
        );
        input.compact_followup = false;
        assert_eq!(
            responses_loop_action(input),
            Ok(ResponsesLoopAction::WaitColdStart)
        );
        input.cold_start_pending = false;
        assert_eq!(
            responses_loop_action(input),
            Ok(ResponsesLoopAction::DirectFallback)
        );
        input.stream_committed = true;
        assert_eq!(
            responses_loop_action(input),
            Ok(ResponsesLoopAction::ReturnWithoutRotation)
        );
    }

    #[test]
    fn abi_rejects_malformed_tags_and_versions() {
        assert_eq!(
            unsafe { prodex_runtime_responses_loop_action_v1(0, 0, 0, 0, 0, 0, 0, 0, 0, 0,) },
            -4
        );
        assert_eq!(
            unsafe {
                prodex_runtime_responses_loop_action_v1(ABI_VERSION, 2, 0, 0, 0, 0, 0, 0, 0, 0)
            },
            -1
        );
        assert_eq!(
            unsafe {
                prodex_runtime_responses_loop_action_v1(ABI_VERSION, 0, 0, 2, 0, 0, 0, 0, 0, 0)
            },
            -1
        );
    }
}
