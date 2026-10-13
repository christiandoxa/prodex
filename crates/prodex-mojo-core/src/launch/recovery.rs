//! Single semantic owner for live native recovery arbitration.
use crate::MojoError;

#[repr(i64)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecoveryObservationKind {
    Other = 0,
    Activity = 1,
    Terminal = 2,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecoveryObservationAction {
    Ignore,
    Clear,
    Arm,
}

#[repr(i64)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeGoalStatus {
    Absent = 0,
    Active = 1,
    Blocked = 2,
    UsageLimited = 3,
    Paused = 4,
    BudgetLimited = 5,
    Complete = 6,
    Unknown = 7,
}

#[repr(i64)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeThreadStatus {
    Unknown = 0,
    Idle = 1,
    SystemError = 2,
    Active = 3,
    NotLoaded = 4,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InPlaceRecoveryAction {
    Defer,
    Stop,
    ResumeGoal,
    ContinueTurn,
}

unsafe extern "C" {
    fn prodex_launch_recovery_observation_v1(
        abi: i64,
        kind: i64,
        session_matches: i64,
        recoverable: i64,
    ) -> i64;
    fn prodex_launch_in_place_recovery_v2(
        abi: i64,
        verified: i64,
        thread_status: i64,
        expected_goal: i64,
        goal_status: i64,
        budget_available: i64,
    ) -> i64;
    fn prodex_launch_workflow_recovery_class_v1(
        abi_version: i64,
        name_address: u64,
        name_length: i64,
        status: i64,
        output_address: u64,
    ) -> i64;
    fn prodex_launch_workflow_recovery_class_v2(
        abi_version: i64,
        name_address: u64,
        name_length: i64,
        status: i64,
        message_address: u64,
        message_length: i64,
        output_address: u64,
    ) -> i64;
    fn prodex_launch_workflow_evidence_v1(
        abi_version: i64,
        observation: i64,
        acceptance_state: i64,
        stream_state: i64,
        side_effect_state: i64,
        output_address: u64,
    ) -> i64;
}

#[repr(i64)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkflowObservationKind {
    Other = 0,
    TurnStarted = 1,
    UserAccepted = 2,
    AssistantCommitted = 3,
    SideEffectObserved = 4,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorkflowEvidencePlan {
    pub acceptance_state: i64,
    pub stream_state: i64,
    pub side_effect_state: i64,
}

#[repr(i64)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkflowRecoveryClass {
    UsageLimit = 1,
    RateLimit = 2,
    Overload = 3,
    Auth = 4,
    Transport = 5,
}

/// Classify a bounded Codex error tag through the required Mojo owner.
pub fn workflow_recovery_class(
    name: &str,
    status: Option<u16>,
) -> Result<Option<WorkflowRecoveryClass>, MojoError> {
    if name.len() > 256 {
        return Err(MojoError::InvalidInput);
    }
    let mut output = 0_i64;
    let status = status.map(i64::from).unwrap_or(-1);
    let result = unsafe {
        prodex_launch_workflow_recovery_class_v1(
            1,
            name.as_ptr() as usize as u64,
            name.len() as i64,
            status,
            (&mut output as *mut i64) as usize as u64,
        )
    };
    decode_workflow_recovery_class(result, output)
}

/// Classify a native terminal error including its exact compatibility message.
/// Envelope/session validation remains the caller's responsibility.
pub fn workflow_recovery_class_with_message(
    name: &str,
    status: Option<u16>,
    message: Option<&str>,
) -> Result<Option<WorkflowRecoveryClass>, MojoError> {
    if name.len() > 256 {
        return Err(MojoError::InvalidInput);
    }
    let message = message.unwrap_or_default();
    let message_length = i64::try_from(message.len()).map_err(|_| MojoError::InvalidInput)?;
    let mut output = 0_i64;
    let result = unsafe {
        prodex_launch_workflow_recovery_class_v2(
            2,
            name.as_ptr() as usize as u64,
            name.len() as i64,
            status.map(i64::from).unwrap_or(-1),
            message.as_ptr() as usize as u64,
            message_length,
            (&mut output as *mut i64) as usize as u64,
        )
    };
    decode_workflow_recovery_class(result, output)
}

fn decode_workflow_recovery_class(
    result: i64,
    output: i64,
) -> Result<Option<WorkflowRecoveryClass>, MojoError> {
    match result {
        0 => match output {
            0 => Ok(None),
            1 => Ok(Some(WorkflowRecoveryClass::UsageLimit)),
            2 => Ok(Some(WorkflowRecoveryClass::RateLimit)),
            3 => Ok(Some(WorkflowRecoveryClass::Overload)),
            4 => Ok(Some(WorkflowRecoveryClass::Auth)),
            5 => Ok(Some(WorkflowRecoveryClass::Transport)),
            _ => Err(MojoError::InvalidOutput),
        },
        1 => Err(MojoError::InvalidInput),
        4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Apply the workflow acceptance transition selected by Mojo.
pub fn workflow_evidence_plan(
    observation: WorkflowObservationKind,
    current: WorkflowEvidencePlan,
) -> Result<WorkflowEvidencePlan, MojoError> {
    if !(0..=3).contains(&current.acceptance_state)
        || !(-1..=1).contains(&current.stream_state)
        || !(0..=2).contains(&current.side_effect_state)
    {
        return Err(MojoError::InvalidInput);
    }
    let mut output = [
        current.acceptance_state,
        current.stream_state,
        current.side_effect_state,
    ];
    let result = unsafe {
        prodex_launch_workflow_evidence_v1(
            1,
            observation as i64,
            current.acceptance_state,
            current.stream_state,
            current.side_effect_state,
            output.as_mut_ptr() as usize as u64,
        )
    };
    match result {
        0 => {
            if !(0..=3).contains(&output[0])
                || !(-1..=1).contains(&output[1])
                || !(0..=2).contains(&output[2])
            {
                return Err(MojoError::InvalidOutput);
            }
            Ok(WorkflowEvidencePlan {
                acceptance_state: output[0],
                stream_state: output[1],
                side_effect_state: output[2],
            })
        }
        1 => Err(MojoError::InvalidInput),
        4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn recovery_observation_action(
    kind: RecoveryObservationKind,
    session_matches: bool,
    recoverable: bool,
) -> Result<RecoveryObservationAction, MojoError> {
    match unsafe {
        prodex_launch_recovery_observation_v1(
            1,
            kind as i64,
            i64::from(session_matches),
            i64::from(recoverable),
        )
    } {
        0 => Ok(RecoveryObservationAction::Ignore),
        1 => Ok(RecoveryObservationAction::Clear),
        2 => Ok(RecoveryObservationAction::Arm),
        -3 => Err(MojoError::AbiMismatch),
        -2 => Err(MojoError::InvalidInput),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn in_place_recovery_action(
    verified: bool,
    thread_status: NativeThreadStatus,
    expected_goal: bool,
    goal_status: NativeGoalStatus,
    budget_available: bool,
) -> Result<InPlaceRecoveryAction, MojoError> {
    match unsafe {
        prodex_launch_in_place_recovery_v2(
            2,
            i64::from(verified),
            thread_status as i64,
            i64::from(expected_goal),
            goal_status as i64,
            i64::from(budget_available),
        )
    } {
        0 => Ok(InPlaceRecoveryAction::Defer),
        1 => Ok(InPlaceRecoveryAction::Stop),
        2 => Ok(InPlaceRecoveryAction::ResumeGoal),
        3 => Ok(InPlaceRecoveryAction::ContinueTurn),
        -3 => Err(MojoError::AbiMismatch),
        -2 => Err(MojoError::InvalidInput),
        _ => Err(MojoError::InvalidOutput),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recovery_observation_defers_intermediate_errors_and_ignores_other_sessions() {
        use RecoveryObservationAction::*;
        use RecoveryObservationKind::*;
        assert_eq!(recovery_observation_action(Other, true, true), Ok(Ignore));
        assert_eq!(
            recovery_observation_action(Terminal, false, true),
            Ok(Ignore)
        );
        assert_eq!(recovery_observation_action(Terminal, true, true), Ok(Arm));
        assert_eq!(
            recovery_observation_action(Terminal, true, false),
            Ok(Clear)
        );
        assert_eq!(
            recovery_observation_action(Activity, true, false),
            Ok(Clear)
        );
    }

    #[test]
    fn in_place_recovery_never_overrides_pauses_budgets_completion_or_active_turns() {
        use InPlaceRecoveryAction::*;
        use NativeGoalStatus::*;
        for status in [Active, Blocked, UsageLimited] {
            assert_eq!(
                in_place_recovery_action(true, NativeThreadStatus::Idle, true, status, true),
                Ok(ResumeGoal)
            );
            assert_eq!(
                in_place_recovery_action(true, NativeThreadStatus::Active, true, status, true),
                Ok(Defer)
            );
            assert_eq!(
                in_place_recovery_action(false, NativeThreadStatus::Idle, true, status, true),
                Ok(Defer)
            );
            assert_eq!(
                in_place_recovery_action(true, NativeThreadStatus::Idle, true, status, false),
                Ok(Stop)
            );
        }
        for status in [Paused, BudgetLimited, Complete, Unknown] {
            assert_eq!(
                in_place_recovery_action(true, NativeThreadStatus::Idle, true, status, true),
                Ok(Stop)
            );
        }
        assert_eq!(
            in_place_recovery_action(true, NativeThreadStatus::Idle, true, Absent, true),
            Ok(Stop)
        );
        assert_eq!(
            in_place_recovery_action(true, NativeThreadStatus::Idle, false, Absent, true),
            Ok(ContinueTurn)
        );
        assert_eq!(
            unsafe { prodex_launch_in_place_recovery_v2(1, 1, 1, 1, 1, 1) },
            -3
        );
        assert_eq!(
            unsafe { prodex_launch_in_place_recovery_v2(2, 2, 1, 1, 1, 1) },
            -2
        );
        assert_eq!(
            unsafe { prodex_launch_recovery_observation_v1(1, 3, 1, 1) },
            -2
        );
    }

    #[test]
    fn native_thread_recovery_recognizes_system_errors_without_interrupting_active_work() {
        for thread_status in [NativeThreadStatus::Idle, NativeThreadStatus::SystemError] {
            assert_eq!(
                in_place_recovery_action(
                    true,
                    thread_status,
                    true,
                    NativeGoalStatus::Blocked,
                    true
                ),
                Ok(InPlaceRecoveryAction::ResumeGoal)
            );
            assert_eq!(
                in_place_recovery_action(
                    true,
                    thread_status,
                    false,
                    NativeGoalStatus::Absent,
                    true
                ),
                Ok(InPlaceRecoveryAction::ContinueTurn)
            );
            for stopped_goal in [
                NativeGoalStatus::Paused,
                NativeGoalStatus::BudgetLimited,
                NativeGoalStatus::Complete,
            ] {
                assert_eq!(
                    in_place_recovery_action(true, thread_status, true, stopped_goal, true),
                    Ok(InPlaceRecoveryAction::Stop)
                );
            }
        }
        for thread_status in [
            NativeThreadStatus::Active,
            NativeThreadStatus::NotLoaded,
            NativeThreadStatus::Unknown,
        ] {
            assert_eq!(
                in_place_recovery_action(
                    true,
                    thread_status,
                    true,
                    NativeGoalStatus::Blocked,
                    true
                ),
                Ok(InPlaceRecoveryAction::Defer)
            );
        }
        assert_eq!(
            unsafe { prodex_launch_in_place_recovery_v2(2, 1, 5, 1, 2, 1) },
            -2
        );
    }

    #[test]
    fn native_timeout_message_uses_versioned_mojo_policy_without_overriding_tags() {
        for status in [None, Some(408), Some(504)] {
            assert_eq!(
                workflow_recovery_class_with_message("other", status, Some("request timed out")),
                Ok(Some(WorkflowRecoveryClass::Transport))
            );
        }
        assert_eq!(workflow_recovery_class("other", None), Ok(None));
        assert_eq!(
            workflow_recovery_class_with_message("other", None, None),
            Ok(None)
        );
        assert_eq!(
            workflow_recovery_class_with_message("other", Some(400), Some("request timed out")),
            Ok(None)
        );
        assert_eq!(
            workflow_recovery_class_with_message("other", None, Some("quoted request timed out")),
            Ok(None)
        );
        assert_eq!(
            workflow_recovery_class_with_message("server_overloaded", None, Some("unrelated")),
            Ok(Some(WorkflowRecoveryClass::Overload))
        );
        let mut output = 77_i64;
        assert_eq!(
            unsafe {
                prodex_launch_workflow_recovery_class_v2(
                    1,
                    0,
                    0,
                    -1,
                    0,
                    0,
                    (&mut output as *mut i64) as u64,
                )
            },
            4
        );
        assert_eq!(output, 77, "bad ABI must not touch output");
        assert_eq!(
            unsafe {
                prodex_launch_workflow_recovery_class_v2(
                    2,
                    0,
                    0,
                    -1,
                    0,
                    1,
                    (&mut output as *mut i64) as u64,
                )
            },
            1
        );
    }

    #[test]
    fn workflow_recovery_class_and_evidence_are_mojo_owned() {
        assert_eq!(
            workflow_recovery_class("usageLimitExceeded", None),
            Ok(Some(WorkflowRecoveryClass::UsageLimit))
        );
        assert_eq!(
            workflow_recovery_class("responseStreamDisconnected", Some(503)),
            Ok(Some(WorkflowRecoveryClass::Transport))
        );
        assert_eq!(
            workflow_recovery_class("responseStreamDisconnected", Some(400)),
            Ok(None)
        );
        assert_eq!(workflow_recovery_class("unrelated", None), Ok(None));

        let accepted = workflow_evidence_plan(
            WorkflowObservationKind::UserAccepted,
            WorkflowEvidencePlan {
                acceptance_state: 0,
                stream_state: -1,
                side_effect_state: 0,
            },
        )
        .unwrap();
        assert_eq!(
            accepted,
            WorkflowEvidencePlan {
                acceptance_state: 1,
                stream_state: 0,
                side_effect_state: 1,
            }
        );
        assert_eq!(
            workflow_evidence_plan(
                WorkflowObservationKind::TurnStarted,
                WorkflowEvidencePlan {
                    acceptance_state: 3,
                    stream_state: 1,
                    side_effect_state: 2,
                },
            ),
            Ok(WorkflowEvidencePlan {
                acceptance_state: 0,
                stream_state: -1,
                side_effect_state: 0,
            })
        );
        assert_eq!(
            workflow_recovery_class("usageLimitExceeded", None),
            workflow_recovery_class("usage_limit_exceeded", Some(401))
        );
    }

    #[test]
    fn workflow_recovery_abi_rejects_invalid_inputs() {
        assert_eq!(
            unsafe { prodex_launch_workflow_recovery_class_v1(1, 0, 0, -1, 0) },
            1
        );
        assert_eq!(
            unsafe { prodex_launch_workflow_recovery_class_v1(99, 0, 0, -1, 1) },
            4
        );
        assert_eq!(
            unsafe { prodex_launch_workflow_evidence_v1(1, 9, 0, -1, 0, 1) },
            1
        );
        assert_eq!(
            unsafe { prodex_launch_workflow_evidence_v1(1, 0, 0, -1, 0, 0) },
            1
        );
    }
}
