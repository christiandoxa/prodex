use super::*;

fn observation() -> NativeRecoveryWireInput<'static> {
    NativeRecoveryWireInput {
        thread_status: Some("idle"),
        expected_goal: true,
        goal_present: true,
        goal_status: Some("blocked"),
        token_budget_present: true,
        token_budget_is_null: false,
        token_budget: Some(100),
        tokens_used: Some(10),
    }
}

#[test]
fn compiled_native_wire_recovery_preserves_statuses_and_signed_budget_boundaries() {
    use InPlaceRecoveryAction::{ContinueTurn, Defer, ResumeGoal, Stop};
    for (thread, quiescent) in [
        (Some("idle"), true),
        (Some("systemError"), true),
        (Some("active"), false),
        (Some("notLoaded"), false),
        (Some("futureStatus"), false),
        (Some("Idle"), false),
        (Some("systemError "), false),
        (Some("未知"), false),
        (Some(""), false),
        (None, false),
    ] {
        for (goal, expected) in [
            (Some("active"), ResumeGoal),
            (Some("blocked"), ResumeGoal),
            (Some("usageLimited"), ResumeGoal),
            (Some("paused"), Stop),
            (Some("budgetLimited"), Stop),
            (Some("complete"), Stop),
            (Some("Active"), Stop),
            (Some("blocked\0"), Stop),
            (Some("未知"), Stop),
            (Some(""), Stop),
            (None, Stop),
        ] {
            let input = NativeRecoveryWireInput {
                thread_status: thread,
                goal_status: goal,
                ..observation()
            };
            assert_eq!(
                in_place_recovery_action_from_wire(input),
                Ok(if quiescent { expected } else { Defer }),
                "{thread:?} {goal:?}"
            );
        }
    }
    for (limit, used, expected) in [
        (Some(100), Some(0), ResumeGoal),
        (Some(100), Some(99), ResumeGoal),
        (Some(i64::MAX), Some(i64::MAX - 1), ResumeGoal),
        (Some(100), Some(100), Stop),
        (Some(100), Some(101), Stop),
        (Some(0), Some(0), Stop),
        (Some(-1), Some(0), Stop),
        (Some(i64::MIN), Some(i64::MIN), Stop),
        (Some(100), Some(-1), Stop),
        (Some(100), None, Stop),
        (None, Some(1), Stop),
        (None, None, Stop),
    ] {
        assert_eq!(
            in_place_recovery_action_from_wire(NativeRecoveryWireInput {
                token_budget: limit,
                tokens_used: used,
                ..observation()
            }),
            Ok(expected),
            "{limit:?} {used:?}"
        );
    }
    for (present, is_null) in [(false, false), (true, true)] {
        assert_eq!(
            in_place_recovery_action_from_wire(NativeRecoveryWireInput {
                token_budget_present: present,
                token_budget_is_null: is_null,
                token_budget: None,
                tokens_used: Some(-10),
                ..observation()
            }),
            Ok(ResumeGoal)
        );
    }
    for (expected_goal, expected) in [(true, Stop), (false, ContinueTurn)] {
        assert_eq!(
            in_place_recovery_action_from_wire(NativeRecoveryWireInput {
                expected_goal,
                goal_present: false,
                goal_status: None,
                token_budget_present: false,
                token_budget: None,
                tokens_used: None,
                ..observation()
            }),
            Ok(expected)
        );
    }
}

#[test]
fn compiled_native_wire_abi_rejects_invalid_versions_counts_flags_and_views() {
    let mut statuses = views(&[Some("idle"), Some("blocked")]).unwrap();
    let address = statuses.as_ptr() as u64;
    for (version, input, count, flags, expected) in [
        (2, 0, 0, 0, -3),
        (1, 0, 2, 0, -2),
        (1, address + 1, 2, 0, -2),
        (1, address, 1, 0, -2),
        (1, address, 3, 0, -2),
        (1, address, -1, 0, -2),
        (1, address, 2, -1, -2),
        (1, address, 2, 64, -2),
    ] {
        assert_eq!(
            unsafe { prodex_launch_native_recovery_wire_v1(version, input, count, flags, 0, 0) },
            expected
        );
    }
    let original = statuses[0];
    let invalid_utf8 = [255_u8];
    for (input, length, valid_utf8) in [
        (original.address, original.length, 2),
        (original.address, original.length, 0),
        (0, 1, 1),
        (original.address, u64::MAX, 1),
        (invalid_utf8.as_ptr() as u64, 1, 1),
    ] {
        statuses[0].address = input;
        statuses[0].length = length;
        statuses[0].valid_utf8 = valid_utf8;
        assert_eq!(
            unsafe { prodex_launch_native_recovery_wire_v1(1, address, 2, 0, 0, 0) },
            -2
        );
    }
}
