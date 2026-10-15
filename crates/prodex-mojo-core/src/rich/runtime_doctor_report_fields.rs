//! Runtime-doctor human report field planning ABI.
//!
//! Mojo owns optional-row precedence and order. Rust keeps the summary maps,
//! value formatting, and the existing `(label, value)` DTOs.

use super::{ensure_rich_abi, mojo_mut_pointer_address};
use crate::MojoError;

/// Version of the runtime-doctor report-field planning ABI.
pub const RUNTIME_DOCTOR_REPORT_FIELDS_ABI_VERSION: i64 = 1;
/// Optional field for a legacy previous-response recovery count.
pub const RUNTIME_DOCTOR_REPORT_FIELD_LEGACY_PREV: i64 = 0;
/// Optional field for the compact final-failure count.
pub const RUNTIME_DOCTOR_REPORT_FIELD_COMPACT_FINAL: i64 = 1;
/// Optional field for the compact exit value.
pub const RUNTIME_DOCTOR_REPORT_FIELD_COMPACT_EXIT: i64 = 2;
/// Optional field for the compact reason value.
pub const RUNTIME_DOCTOR_REPORT_FIELD_COMPACT_REASON: i64 = 3;
/// Optional field for the compact last-failure value.
pub const RUNTIME_DOCTOR_REPORT_FIELD_COMPACT_LAST_FAILURE: i64 = 4;
/// Optional field for deferred sync-probe work.
pub const RUNTIME_DOCTOR_REPORT_FIELD_SYNC_DEFERRED: i64 = 5;
/// Optional field for the profile under probe-refresh pressure.
pub const RUNTIME_DOCTOR_REPORT_FIELD_PROBE_PRESSURE_PROFILE: i64 = 6;
/// Optional field for the most common marker reason.
pub const RUNTIME_DOCTOR_REPORT_FIELD_HOT_REASON: i64 = 7;
/// No sync-probe detail source is present.
pub const RUNTIME_DOCTOR_REPORT_SYNC_SOURCE_NONE: i64 = 0;
/// Sync-probe detail came from `cold_start_jobs`.
pub const RUNTIME_DOCTOR_REPORT_SYNC_SOURCE_JOBS: i64 = 1;
/// Sync-probe detail came from `cold_start_profiles`.
pub const RUNTIME_DOCTOR_REPORT_SYNC_SOURCE_PROFILES: i64 = 2;
pub const RUNTIME_DOCTOR_REPORT_FIELDS_MAX: usize = 8;

unsafe extern "C" {
    fn prodex_mojo_runtime_doctor_report_fields_v1(
        abi_version: i64,
        presence_bits: i64,
        fields_address: u64,
        field_count_address: u64,
        sync_source_address: u64,
    ) -> i64;
}

/// Mojo-selected optional rows for `runtime_doctor_fields_for_summary`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeDoctorReportFieldPlan {
    pub fields: [i64; RUNTIME_DOCTOR_REPORT_FIELDS_MAX],
    pub field_count: usize,
    pub sync_source: i64,
}

/// Plan optional human-facing runtime-doctor rows through compiled Mojo.
pub fn runtime_doctor_report_field_plan(
    legacy_prev_present: bool,
    compact_final_present: bool,
    sync_probe_present: bool,
    probe_pressure_present: bool,
    hot_reason_present: bool,
    sync_jobs_present: bool,
    sync_profiles_present: bool,
) -> Result<RuntimeDoctorReportFieldPlan, MojoError> {
    ensure_rich_abi()?;
    let presence_bits = i64::from(legacy_prev_present)
        | (i64::from(compact_final_present) << 1)
        | (i64::from(sync_probe_present) << 2)
        | (i64::from(probe_pressure_present) << 3)
        | (i64::from(hot_reason_present) << 4)
        | (i64::from(sync_jobs_present) << 5)
        | (i64::from(sync_profiles_present) << 6);
    let mut fields = [-1_i64; RUNTIME_DOCTOR_REPORT_FIELDS_MAX];
    let mut field_count = -1_i64;
    let mut sync_source = -1_i64;
    let status = unsafe {
        prodex_mojo_runtime_doctor_report_fields_v1(
            RUNTIME_DOCTOR_REPORT_FIELDS_ABI_VERSION,
            presence_bits,
            mojo_mut_pointer_address(fields.as_mut_ptr()),
            mojo_mut_pointer_address(&mut field_count),
            mojo_mut_pointer_address(&mut sync_source),
        )
    };
    if status != 0 {
        return Err(match status {
            1 => MojoError::InvalidInput,
            3 => MojoError::Capacity,
            4 => MojoError::AbiMismatch,
            _ => MojoError::InvalidOutput,
        });
    }
    let field_count = usize::try_from(field_count).map_err(|_| MojoError::InvalidOutput)?;
    if field_count > RUNTIME_DOCTOR_REPORT_FIELDS_MAX
        || !matches!(sync_source, 0..=2)
        || fields[..field_count].iter().any(|field| {
            !(*field >= RUNTIME_DOCTOR_REPORT_FIELD_LEGACY_PREV
                && *field <= RUNTIME_DOCTOR_REPORT_FIELD_HOT_REASON)
        })
    {
        return Err(MojoError::InvalidOutput);
    }
    Ok(RuntimeDoctorReportFieldPlan {
        fields,
        field_count,
        sync_source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_field_plan_preserves_optional_order_and_sync_precedence() {
        let plan =
            runtime_doctor_report_field_plan(true, true, true, true, true, true, true).unwrap();
        assert_eq!(
            &plan.fields[..plan.field_count],
            &[
                RUNTIME_DOCTOR_REPORT_FIELD_LEGACY_PREV,
                RUNTIME_DOCTOR_REPORT_FIELD_COMPACT_FINAL,
                RUNTIME_DOCTOR_REPORT_FIELD_COMPACT_EXIT,
                RUNTIME_DOCTOR_REPORT_FIELD_COMPACT_REASON,
                RUNTIME_DOCTOR_REPORT_FIELD_COMPACT_LAST_FAILURE,
                RUNTIME_DOCTOR_REPORT_FIELD_SYNC_DEFERRED,
                RUNTIME_DOCTOR_REPORT_FIELD_PROBE_PRESSURE_PROFILE,
                RUNTIME_DOCTOR_REPORT_FIELD_HOT_REASON,
            ],
        );
        assert_eq!(plan.sync_source, RUNTIME_DOCTOR_REPORT_SYNC_SOURCE_JOBS);
        let profiles =
            runtime_doctor_report_field_plan(false, false, true, false, false, false, true)
                .unwrap();
        assert_eq!(profiles.field_count, 1);
        assert_eq!(
            profiles.fields[0],
            RUNTIME_DOCTOR_REPORT_FIELD_SYNC_DEFERRED
        );
        assert_eq!(
            profiles.sync_source,
            RUNTIME_DOCTOR_REPORT_SYNC_SOURCE_PROFILES
        );
    }
}
