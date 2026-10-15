use crate::MojoError;

const ABI_VERSION: i64 = 1;
const SUPER_DRY_RUN: i64 = 1;
const UPDATE_NOTICE: i64 = 2;
const PROFILE_RECOVERY: i64 = 4;
const AUTO_HOUSEKEEPING: i64 = 8;
const MINIMAL_STARTUP: i64 = 16;
const ALL_POLICY_BITS: i64 =
    SUPER_DRY_RUN | UPDATE_NOTICE | PROFILE_RECOVERY | AUTO_HOUSEKEEPING | MINIMAL_STARTUP;

/// Parsed command family used by the app startup policy kernel.
#[repr(i64)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum AppCommandClass {
    #[default]
    Other = 0,
    RuntimeBroker = 1,
    Info = 2,
    Log = 3,
    Update = 4,
    Ping = 5,
    SuperExpose = 6,
    McpBridge = 7,
    SubAgentExec = 8,
    ProfileRemove = 9,
    Doctor = 10,
    Super = 11,
}

/// Host-observed command facts passed to the startup policy kernel.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AppCommandStartupInput {
    pub command_class: AppCommandClass,
    pub super_direct_dry_run: bool,
    pub super_codex_dry_run: bool,
    pub super_expose_direct_dry_run: bool,
    pub super_expose_codex_dry_run: bool,
    pub native_agy: bool,
}

/// Startup effects selected for one parsed command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppCommandStartupPlan {
    pub super_dry_run: bool,
    pub show_update_notice: bool,
    pub run_profile_lifecycle_recovery: bool,
    pub run_auto_housekeeping: bool,
    pub minimal_startup: bool,
}

unsafe extern "C" {
    fn prodex_app_command_startup_policy_v1(
        abi_version: i64,
        command_class: i64,
        super_direct_dry_run: i64,
        super_codex_dry_run: i64,
        super_expose_direct_dry_run: i64,
        super_expose_codex_dry_run: i64,
        native_agy: i64,
        output_address: u64,
    ) -> i64;
}

/// Resolve app startup effects from parsed command facts.
pub fn startup_plan(input: AppCommandStartupInput) -> Result<AppCommandStartupPlan, MojoError> {
    let mut output = -1_i64;
    let status = unsafe {
        prodex_app_command_startup_policy_v1(
            ABI_VERSION,
            input.command_class as i64,
            i64::from(input.super_direct_dry_run),
            i64::from(input.super_codex_dry_run),
            i64::from(input.super_expose_direct_dry_run),
            i64::from(input.super_expose_codex_dry_run),
            i64::from(input.native_agy),
            (&mut output as *mut i64) as usize as u64,
        )
    };
    match status {
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    if output < 0 || output & !ALL_POLICY_BITS != 0 {
        return Err(MojoError::InvalidOutput);
    }
    Ok(AppCommandStartupPlan {
        super_dry_run: output & SUPER_DRY_RUN != 0,
        show_update_notice: output & UPDATE_NOTICE != 0,
        run_profile_lifecycle_recovery: output & PROFILE_RECOVERY != 0,
        run_auto_housekeeping: output & AUTO_HOUSEKEEPING != 0,
        minimal_startup: output & MINIMAL_STARTUP != 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startup_policy_preserves_native_and_dry_run_boundaries() {
        let dry_run = startup_plan(AppCommandStartupInput {
            command_class: AppCommandClass::Super,
            super_codex_dry_run: true,
            ..AppCommandStartupInput::default()
        })
        .expect("compiled Mojo startup policy should accept Super facts");
        assert_eq!(
            dry_run,
            AppCommandStartupPlan {
                super_dry_run: true,
                show_update_notice: false,
                run_profile_lifecycle_recovery: false,
                run_auto_housekeeping: false,
                minimal_startup: false,
            }
        );

        let native = startup_plan(AppCommandStartupInput {
            command_class: AppCommandClass::Super,
            native_agy: true,
            ..AppCommandStartupInput::default()
        })
        .expect("compiled Mojo startup policy should accept native facts");
        assert!(!native.show_update_notice);
        assert!(!native.run_profile_lifecycle_recovery);
        assert!(!native.run_auto_housekeeping);
        assert!(native.minimal_startup);
    }

    #[test]
    fn startup_policy_keeps_command_exclusion_matrix() {
        let profile_remove = startup_plan(AppCommandStartupInput {
            command_class: AppCommandClass::ProfileRemove,
            ..AppCommandStartupInput::default()
        })
        .unwrap();
        assert!(profile_remove.show_update_notice);
        assert!(!profile_remove.run_profile_lifecycle_recovery);
        assert!(profile_remove.run_auto_housekeeping);

        let info = startup_plan(AppCommandStartupInput {
            command_class: AppCommandClass::Info,
            ..AppCommandStartupInput::default()
        })
        .unwrap();
        assert!(!info.show_update_notice);
        assert!(!info.run_profile_lifecycle_recovery);
        assert!(!info.run_auto_housekeeping);

        let ping = startup_plan(AppCommandStartupInput {
            command_class: AppCommandClass::Ping,
            ..AppCommandStartupInput::default()
        })
        .unwrap();
        assert!(ping.minimal_startup);
        assert!(!ping.run_auto_housekeeping);
    }
}
