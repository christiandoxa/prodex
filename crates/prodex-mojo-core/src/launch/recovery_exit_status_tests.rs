//! Cross-platform regression for sign-extended Windows process statuses.
use super::{prodex_launch_exit_status_cancelled_v1, runtime_exit_status_cancelled};
use crate::MojoError;

const CONTROL_C_EXIT: i64 = (0xc000_013a_u32 as i32) as i64;

#[test]
fn windows_signed_exit_facts_preserve_recovery_and_cancellation() {
    for (code, cancelled) in [
        (0, false),
        (1, false),
        (130, true),
        (CONTROL_C_EXIT, true),
        ((0xc000_0005_u32 as i32) as i64, false),
        (-2, false),
        (-1, false),
        (i64::from(i32::MIN), false),
        (i64::from(i32::MAX), false),
    ] {
        assert_eq!(
            runtime_exit_status_cancelled(code, -1, -1, CONTROL_C_EXIT),
            Ok(cancelled),
            "Windows signed exit code {code}",
        );
    }
}

#[test]
fn absent_platform_status_does_not_turn_missing_exit_into_cancellation() {
    assert_eq!(runtime_exit_status_cancelled(-1, -1, -1, -1), Ok(false));
    assert_eq!(runtime_exit_status_cancelled(-1, 2, 2, -1), Ok(true));
    assert_eq!(runtime_exit_status_cancelled(-1, 15, 2, -1), Ok(false));
    assert_eq!(runtime_exit_status_cancelled(130, -1, 2, -1), Ok(true));
    assert_eq!(
        runtime_exit_status_cancelled(-2, -1, 2, -1),
        Err(MojoError::InvalidInput),
    );
}

#[test]
fn malformed_exit_abi_still_rejects_before_touching_output() {
    let mut output = 77_i64;
    for (version, code, signal, interrupt, ctrl, expected) in [
        (2, 0, -1, -1, CONTROL_C_EXIT, -3),
        (1, i64::from(i32::MIN) - 1, -1, -1, CONTROL_C_EXIT, -2),
        (1, i64::from(i32::MAX) + 1, -1, -1, CONTROL_C_EXIT, -2),
        (1, 0, -2, -1, CONTROL_C_EXIT, -2),
        (1, 0, -1, -2, CONTROL_C_EXIT, -2),
        (1, 0, -1, -1, i64::from(i32::MIN) - 1, -2),
        (1, 0, -1, -1, i64::from(i32::MAX) + 1, -2),
    ] {
        let status = unsafe {
            prodex_launch_exit_status_cancelled_v1(
                version,
                code,
                signal,
                interrupt,
                ctrl,
                (&mut output as *mut i64) as u64,
            )
        };
        assert_eq!(status, expected);
        assert_eq!(output, 77);
    }
}
