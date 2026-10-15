#![cfg(feature = "mojo-runtime")]
#![allow(unsafe_code)]

use prodex_mojo_core::launch::runtime_exit_status_cancelled;

unsafe extern "C" {
    fn prodex_launch_exit_status_cancelled_v1(
        abi_version: i64,
        exit_code: i64,
        signal: i64,
        interrupt_signal: i64,
        control_c_exit_code: i64,
        output_address: u64,
    ) -> i64;
}

#[test]
fn compiled_mojo_exit_status_policy_preserves_recovery_cancellation_matrix() {
    const { assert!(prodex_mojo_core::MOJO_REQUIRED) };

    for (facts, expected) in [
        ((130, -1, -1, -1), true),
        ((1, 2, 2, -1), true),
        ((1, 2, 15, -1), false),
        ((99, -1, -1, 99), true),
        ((99, -1, -1, 100), false),
    ] {
        assert_eq!(
            runtime_exit_status_cancelled(facts.0, facts.1, facts.2, facts.3),
            Ok(expected)
        );
    }
}

#[test]
fn compiled_mojo_exit_status_policy_rejects_bad_abi_without_touching_output() {
    const { assert!(prodex_mojo_core::MOJO_ACTIVE) };
    let mut output = 77_i64;
    assert_eq!(
        unsafe {
            prodex_launch_exit_status_cancelled_v1(
                2,
                130,
                -1,
                -1,
                -1,
                (&mut output as *mut i64) as u64,
            )
        },
        -3
    );
    assert_eq!(output, 77);
    assert_eq!(
        unsafe {
            prodex_launch_exit_status_cancelled_v1(
                1,
                -2,
                -1,
                -1,
                -1,
                (&mut output as *mut i64) as u64,
            )
        },
        -2
    );
    assert_eq!(output, 77);
}
