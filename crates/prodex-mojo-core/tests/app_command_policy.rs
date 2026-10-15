#![cfg(feature = "mojo-runtime")]

use prodex_mojo_core::app_command_policy::{
    AppCommandClass, AppCommandStartupInput, AppCommandStartupPlan, startup_plan,
};

#[test]
fn compiled_mojo_app_command_policy_preserves_startup_matrix() {
    const { assert!(prodex_mojo_core::MOJO_REQUIRED) };

    let expected = [
        (
            AppCommandClass::Super,
            AppCommandStartupInput {
                command_class: AppCommandClass::Super,
                ..AppCommandStartupInput::default()
            },
            AppCommandStartupPlan {
                super_dry_run: false,
                show_update_notice: true,
                run_profile_lifecycle_recovery: true,
                run_auto_housekeeping: true,
                minimal_startup: false,
            },
        ),
        (
            AppCommandClass::Super,
            AppCommandStartupInput {
                command_class: AppCommandClass::Super,
                super_direct_dry_run: true,
                ..AppCommandStartupInput::default()
            },
            AppCommandStartupPlan {
                super_dry_run: true,
                show_update_notice: false,
                run_profile_lifecycle_recovery: false,
                run_auto_housekeeping: false,
                minimal_startup: false,
            },
        ),
        (
            AppCommandClass::SuperExpose,
            AppCommandStartupInput {
                command_class: AppCommandClass::SuperExpose,
                ..AppCommandStartupInput::default()
            },
            AppCommandStartupPlan {
                super_dry_run: false,
                show_update_notice: false,
                run_profile_lifecycle_recovery: true,
                run_auto_housekeeping: true,
                minimal_startup: false,
            },
        ),
        (
            AppCommandClass::Info,
            AppCommandStartupInput {
                command_class: AppCommandClass::Info,
                ..AppCommandStartupInput::default()
            },
            AppCommandStartupPlan {
                super_dry_run: false,
                show_update_notice: false,
                run_profile_lifecycle_recovery: false,
                run_auto_housekeeping: false,
                minimal_startup: false,
            },
        ),
        (
            AppCommandClass::ProfileRemove,
            AppCommandStartupInput {
                command_class: AppCommandClass::ProfileRemove,
                ..AppCommandStartupInput::default()
            },
            AppCommandStartupPlan {
                super_dry_run: false,
                show_update_notice: true,
                run_profile_lifecycle_recovery: false,
                run_auto_housekeeping: true,
                minimal_startup: false,
            },
        ),
        (
            AppCommandClass::Ping,
            AppCommandStartupInput {
                command_class: AppCommandClass::Ping,
                ..AppCommandStartupInput::default()
            },
            AppCommandStartupPlan {
                super_dry_run: false,
                show_update_notice: false,
                run_profile_lifecycle_recovery: false,
                run_auto_housekeeping: false,
                minimal_startup: true,
            },
        ),
        (
            AppCommandClass::Super,
            AppCommandStartupInput {
                command_class: AppCommandClass::Super,
                native_agy: true,
                ..AppCommandStartupInput::default()
            },
            AppCommandStartupPlan {
                super_dry_run: false,
                show_update_notice: false,
                run_profile_lifecycle_recovery: false,
                run_auto_housekeeping: false,
                minimal_startup: true,
            },
        ),
    ];

    for (_, input, expected_plan) in expected {
        assert_eq!(startup_plan(input).unwrap(), expected_plan);
    }
}
