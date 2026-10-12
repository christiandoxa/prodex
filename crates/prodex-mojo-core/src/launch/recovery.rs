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
    fn prodex_launch_in_place_recovery_v1(
        abi: i64,
        verified: i64,
        idle: i64,
        expected_goal: i64,
        goal_status: i64,
        budget_available: i64,
    ) -> i64;
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
    idle: bool,
    expected_goal: bool,
    goal_status: NativeGoalStatus,
    budget_available: bool,
) -> Result<InPlaceRecoveryAction, MojoError> {
    match unsafe {
        prodex_launch_in_place_recovery_v1(
            1,
            i64::from(verified),
            i64::from(idle),
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
                in_place_recovery_action(true, true, true, status, true),
                Ok(ResumeGoal)
            );
            assert_eq!(
                in_place_recovery_action(true, false, true, status, true),
                Ok(Defer)
            );
            assert_eq!(
                in_place_recovery_action(false, true, true, status, true),
                Ok(Defer)
            );
            assert_eq!(
                in_place_recovery_action(true, true, true, status, false),
                Ok(Stop)
            );
        }
        for status in [Paused, BudgetLimited, Complete, Unknown] {
            assert_eq!(
                in_place_recovery_action(true, true, true, status, true),
                Ok(Stop)
            );
        }
        assert_eq!(
            in_place_recovery_action(true, true, true, Absent, true),
            Ok(Stop)
        );
        assert_eq!(
            in_place_recovery_action(true, true, false, Absent, true),
            Ok(ContinueTurn)
        );
        assert_eq!(
            unsafe { prodex_launch_in_place_recovery_v1(2, 1, 1, 1, 1, 1) },
            -3
        );
        assert_eq!(
            unsafe { prodex_launch_in_place_recovery_v1(1, 2, 1, 1, 1, 1) },
            -2
        );
        assert_eq!(
            unsafe { prodex_launch_recovery_observation_v1(1, 3, 1, 1) },
            -2
        );
    }
}
