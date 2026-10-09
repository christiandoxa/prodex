use crate::MojoError;

const NONCOMPACT_LOOP_ABI_VERSION: i64 = 1;
const NONCOMPACT_BUDGET_FIELD_COUNT: usize = 8;
const NONCOMPACT_ACTION_FIELD_COUNT: usize = 12;

/// Inputs gathered by Rust for the standard noncompact precommit policy.
///
/// Mojo owns the route-specific precedence. Rust supplies observations and
/// keeps the elapsed-time measurement, profile lock, and recovery effects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoncompactPrecommitInput {
    pub normal_budget_exhausted: bool,
    pub continuation: bool,
    pub saw_transient_failure: bool,
    pub route_has_retryable_profile: bool,
    pub recovery_sweeps: usize,
    pub attempts: usize,
    pub profile_count: usize,
    pub attempt_limit: usize,
}

/// Result of the standard noncompact precommit policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoncompactPrecommitAction {
    Proceed,
    WaitTransient,
    Return,
}

/// Stages used by the standard noncompact candidate/recovery state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoncompactLoopStage {
    SelectPreferred,
    AfterFreshSelection,
    CandidateAdmission,
    AfterInflightWait,
    AfterTransientWait,
}

/// Inputs gathered by Rust for one noncompact loop state-machine step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoncompactLoopInput {
    pub stage: NoncompactLoopStage,
    pub excluded_profiles_empty: bool,
    pub preferred_is_session: bool,
    pub preferred_hard_limited: bool,
    pub fresh_candidate_present: bool,
    pub candidate_hard_affinity: bool,
    pub candidate_hard_limited: bool,
    pub inflight_relieved: bool,
    pub session_present: bool,
    pub cold_start_profiles_present: bool,
    pub cold_start_probe_waited: bool,
    pub transient_recovered: bool,
}

/// Host action selected by the Mojo noncompact loop policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoncompactLoopAction {
    AttemptPreferred,
    SelectFreshCandidate,
    AttemptCandidate,
    WaitInflight,
    Continue,
    WaitColdStart,
    WaitTransient,
    Return,
}

unsafe extern "C" {
    fn prodex_runtime_noncompact_precommit_decision_v1(
        abi_version: i64,
        fields_address: u64,
        field_count: i64,
        output_address: u64,
    ) -> i64;
    fn prodex_runtime_noncompact_next_action_v1(
        abi_version: i64,
        fields_address: u64,
        field_count: i64,
        output_address: u64,
    ) -> i64;
}

fn usize_to_u64(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

fn decode_status(status: i64, output: i64) -> Result<i64, MojoError> {
    match status {
        0 => Ok(output),
        1 => Err(MojoError::InvalidInput),
        4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Ask Mojo whether the standard precommit loop may proceed, should wait for
/// transient recovery, or must return its last failure.
pub fn noncompact_precommit_action(
    input: NoncompactPrecommitInput,
) -> Result<NoncompactPrecommitAction, MojoError> {
    let fields = [
        u64::from(input.normal_budget_exhausted),
        u64::from(input.continuation),
        u64::from(input.saw_transient_failure),
        u64::from(input.route_has_retryable_profile),
        usize_to_u64(input.recovery_sweeps),
        usize_to_u64(input.attempts),
        usize_to_u64(input.profile_count),
        usize_to_u64(input.attempt_limit),
    ];
    let mut output = -1_i64;
    let status = unsafe {
        prodex_runtime_noncompact_precommit_decision_v1(
            NONCOMPACT_LOOP_ABI_VERSION,
            fields.as_ptr() as usize as u64,
            i64::try_from(NONCOMPACT_BUDGET_FIELD_COUNT).map_err(|_| MojoError::InvalidInput)?,
            &mut output as *mut i64 as usize as u64,
        )
    };
    match decode_status(status, output)? {
        0 => Ok(NoncompactPrecommitAction::Proceed),
        1 => Ok(NoncompactPrecommitAction::WaitTransient),
        2 => Ok(NoncompactPrecommitAction::Return),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Ask Mojo for the next host-side action in the standard noncompact loop.
pub fn noncompact_loop_action(
    input: NoncompactLoopInput,
) -> Result<NoncompactLoopAction, MojoError> {
    let stage = match input.stage {
        NoncompactLoopStage::SelectPreferred => 0,
        NoncompactLoopStage::AfterFreshSelection => 1,
        NoncompactLoopStage::CandidateAdmission => 2,
        NoncompactLoopStage::AfterInflightWait => 3,
        NoncompactLoopStage::AfterTransientWait => 4,
    };
    let fields = [
        stage,
        u64::from(input.excluded_profiles_empty),
        u64::from(input.preferred_is_session),
        u64::from(input.preferred_hard_limited),
        u64::from(input.fresh_candidate_present),
        u64::from(input.candidate_hard_affinity),
        u64::from(input.candidate_hard_limited),
        u64::from(input.inflight_relieved),
        u64::from(input.session_present),
        u64::from(input.cold_start_profiles_present),
        u64::from(input.cold_start_probe_waited),
        u64::from(input.transient_recovered),
    ];
    let mut output = -1_i64;
    let status = unsafe {
        prodex_runtime_noncompact_next_action_v1(
            NONCOMPACT_LOOP_ABI_VERSION,
            fields.as_ptr() as usize as u64,
            i64::try_from(NONCOMPACT_ACTION_FIELD_COUNT).map_err(|_| MojoError::InvalidInput)?,
            &mut output as *mut i64 as usize as u64,
        )
    };
    match decode_status(status, output)? {
        0 => Ok(NoncompactLoopAction::AttemptPreferred),
        1 => Ok(NoncompactLoopAction::SelectFreshCandidate),
        2 => Ok(NoncompactLoopAction::AttemptCandidate),
        3 => Ok(NoncompactLoopAction::WaitInflight),
        4 => Ok(NoncompactLoopAction::Continue),
        5 => Ok(NoncompactLoopAction::WaitColdStart),
        6 => Ok(NoncompactLoopAction::WaitTransient),
        7 => Ok(NoncompactLoopAction::Return),
        _ => Err(MojoError::InvalidOutput),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test-only oracle for boundary parity; production invokes only Mojo.
    fn expected_precommit(input: NoncompactPrecommitInput) -> NoncompactPrecommitAction {
        if !input.continuation && input.saw_transient_failure && input.route_has_retryable_profile {
            return NoncompactPrecommitAction::Proceed;
        }
        let profile_count = input.profile_count.max(1);
        let exhausted = if input.recovery_sweeps == 0 {
            input.attempts >= profile_count && input.normal_budget_exhausted
        } else {
            input.attempts >= input.attempt_limit
        };
        if !exhausted {
            NoncompactPrecommitAction::Proceed
        } else if input.continuation {
            NoncompactPrecommitAction::Return
        } else {
            NoncompactPrecommitAction::WaitTransient
        }
    }

    #[test]
    fn precommit_policy_matches_independent_state_machine_at_boundaries() {
        for flags in 0_u8..16 {
            for sweeps in [0, 1, 2, usize::MAX] {
                for attempts in [0, 1, 2, usize::MAX] {
                    for profile_count in [0, 1, 2, usize::MAX] {
                        let input = NoncompactPrecommitInput {
                            normal_budget_exhausted: flags & 1 != 0,
                            continuation: flags & 2 != 0,
                            saw_transient_failure: flags & 4 != 0,
                            route_has_retryable_profile: flags & 8 != 0,
                            recovery_sweeps: sweeps,
                            attempts,
                            profile_count,
                            attempt_limit: profile_count,
                        };
                        assert_eq!(
                            noncompact_precommit_action(input),
                            Ok(expected_precommit(input)),
                            "input={input:?}",
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn loop_policy_preserves_affinity_and_saturation_precedence() {
        let preferred = NoncompactLoopInput {
            stage: NoncompactLoopStage::SelectPreferred,
            excluded_profiles_empty: true,
            preferred_is_session: true,
            preferred_hard_limited: true,
            ..NoncompactLoopInput {
                stage: NoncompactLoopStage::SelectPreferred,
                excluded_profiles_empty: false,
                preferred_is_session: false,
                preferred_hard_limited: false,
                fresh_candidate_present: false,
                candidate_hard_affinity: false,
                candidate_hard_limited: false,
                inflight_relieved: false,
                session_present: false,
                cold_start_profiles_present: false,
                cold_start_probe_waited: false,
                transient_recovered: false,
            }
        };
        assert_eq!(
            noncompact_loop_action(preferred),
            Ok(NoncompactLoopAction::AttemptPreferred)
        );

        let candidate = NoncompactLoopInput {
            stage: NoncompactLoopStage::CandidateAdmission,
            candidate_hard_affinity: true,
            candidate_hard_limited: true,
            ..preferred
        };
        assert_eq!(
            noncompact_loop_action(candidate),
            Ok(NoncompactLoopAction::AttemptCandidate)
        );

        let saturated = NoncompactLoopInput {
            candidate_hard_affinity: false,
            candidate_hard_limited: true,
            ..candidate
        };
        assert_eq!(
            noncompact_loop_action(saturated),
            Ok(NoncompactLoopAction::WaitInflight)
        );
    }

    #[test]
    fn loop_policy_orders_empty_pool_waits_before_terminal_return() {
        let input = NoncompactLoopInput {
            stage: NoncompactLoopStage::AfterInflightWait,
            session_present: false,
            cold_start_profiles_present: true,
            cold_start_probe_waited: false,
            ..NoncompactLoopInput {
                stage: NoncompactLoopStage::SelectPreferred,
                excluded_profiles_empty: false,
                preferred_is_session: false,
                preferred_hard_limited: false,
                fresh_candidate_present: false,
                candidate_hard_affinity: false,
                candidate_hard_limited: false,
                inflight_relieved: false,
                session_present: false,
                cold_start_profiles_present: false,
                cold_start_probe_waited: false,
                transient_recovered: false,
            }
        };
        assert_eq!(
            noncompact_loop_action(input),
            Ok(NoncompactLoopAction::WaitColdStart)
        );
        assert_eq!(
            noncompact_loop_action(NoncompactLoopInput {
                cold_start_probe_waited: true,
                ..input
            }),
            Ok(NoncompactLoopAction::WaitTransient)
        );
        assert_eq!(
            noncompact_loop_action(NoncompactLoopInput {
                stage: NoncompactLoopStage::AfterTransientWait,
                transient_recovered: false,
                ..input
            }),
            Ok(NoncompactLoopAction::Return)
        );
    }

    #[test]
    fn abi_rejects_bad_version_and_nonboolean_boundary_fields() {
        let fields = [0_u64; NONCOMPACT_ACTION_FIELD_COUNT];
        let mut output = -1_i64;
        assert_eq!(
            unsafe {
                prodex_runtime_noncompact_next_action_v1(
                    0,
                    fields.as_ptr() as usize as u64,
                    NONCOMPACT_ACTION_FIELD_COUNT as i64,
                    &mut output as *mut i64 as usize as u64,
                )
            },
            4
        );

        let mut invalid = fields;
        invalid[1] = 2;
        assert_eq!(
            unsafe {
                prodex_runtime_noncompact_next_action_v1(
                    NONCOMPACT_LOOP_ABI_VERSION,
                    invalid.as_ptr() as usize as u64,
                    NONCOMPACT_ACTION_FIELD_COUNT as i64,
                    &mut output as *mut i64 as usize as u64,
                )
            },
            1
        );
    }
}
