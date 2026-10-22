//! Ordered runtime-doctor fields from borrowed host metadata.
use super::{RichStringView, ensure_rich_abi, status_error, view};
use crate::MojoError;

/// Report inputs; paths and binding state have already crossed their host adapters.
pub struct RuntimeDoctorReportInput<'a> {
    pub pointer_path: &'a str,
    pub pointer_exists: bool,
    pub log_path: Option<&'a str>,
    pub log_exists: bool,
    pub line_count: u64,
    pub diagnosis: &'a str,
    pub selection_pressure: &'a str,
    pub transport_pressure: &'a str,
    pub persistence_pressure: &'a str,
    pub quota_freshness_pressure: &'a str,
    pub profile_count: u64,
    pub binding_state: &'a str,
    pub degraded_routes: &'a [String],
    pub suspect_continuations: &'a [String],
    pub runtime_brokers: &'a [String],
    pub legacy_previous: u64,
    pub compact_final: u64,
    pub sync_probe: u64,
    pub probe_pressure: u64,
    pub compact_exit: Option<&'a str>,
    pub compact_reason: Option<&'a str>,
    pub compact_last_failure: Option<&'a str>,
    pub sync_jobs: Option<&'a str>,
    pub sync_profiles: Option<&'a str>,
    pub probe_profile: Option<&'a str>,
    pub hot_reason: Option<&'a str>,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct ReportList {
    values: u64,
    count: i64,
}

#[repr(C)]
struct ReportInput {
    values: u64,
    presence: u64,
    pointer_exists: u64,
    log_exists: u64,
    line_count: u64,
    profile_count: u64,
    legacy_previous: u64,
    compact_final: u64,
    sync_probe: u64,
    probe_pressure: u64,
    lists: [ReportList; 3],
}

const _: () = assert!(std::mem::size_of::<ReportInput>() == 128);

unsafe extern "C" {
    fn prodex_mojo_runtime_doctor_report_render_v1(
        version: i64,
        input: u64,
        output: u64,
        capacity: i64,
        written: u64,
    ) -> i64;
}

/// Render ordered label/value pairs as JSON, without reconstructing the report policy.
pub fn runtime_doctor_report_fields_json(
    input: RuntimeDoctorReportInput<'_>,
) -> Result<String, MojoError> {
    ensure_rich_abi()?;
    let text = [
        Some(input.pointer_path),
        input.log_path,
        Some(input.diagnosis),
        Some(input.selection_pressure),
        Some(input.transport_pressure),
        Some(input.persistence_pressure),
        Some(input.quota_freshness_pressure),
        Some(input.binding_state),
        input.compact_exit,
        input.compact_reason,
        input.compact_last_failure,
        input.sync_jobs,
        input.sync_profiles,
        input.probe_profile,
        input.hot_reason,
    ];
    let lists = [
        input.degraded_routes,
        input.suspect_continuations,
        input.runtime_brokers,
    ];
    let capacity = text
        .iter()
        .flatten()
        .copied()
        .chain(
            lists
                .iter()
                .flat_map(|list| list.iter().map(String::as_str)),
        )
        .try_fold(2048_usize, |total, value| {
            total.checked_add(value.len().checked_mul(6)?)
        })
        .ok_or(MojoError::InvalidInput)?;
    let capacity = i64::try_from(capacity).map_err(|_| MojoError::InvalidInput)?;
    let values = text.map(|value| view(value.unwrap_or_default()));
    let presence = text.iter().enumerate().fold(0_u64, |bits, (index, value)| {
        bits | (u64::from(value.is_some()) << index)
    });
    let list_values = lists.map(|list| {
        list.iter()
            .map(|value| view(value))
            .collect::<Vec<RichStringView>>()
    });
    let mut ffi_lists = [ReportList {
        values: 0,
        count: 0,
    }; 3];
    for (ffi, values) in ffi_lists.iter_mut().zip(&list_values) {
        ffi.values = values.as_ptr() as u64;
        ffi.count = i64::try_from(values.len()).map_err(|_| MojoError::InvalidInput)?;
    }
    let ffi_input = ReportInput {
        values: values.as_ptr() as u64,
        presence,
        pointer_exists: u64::from(input.pointer_exists),
        log_exists: u64::from(input.log_exists),
        line_count: input.line_count,
        profile_count: input.profile_count,
        legacy_previous: input.legacy_previous,
        compact_final: input.compact_final,
        sync_probe: input.sync_probe,
        probe_pressure: input.probe_pressure,
        lists: ffi_lists,
    };
    let mut output = vec![0_u8; capacity as usize];
    let mut written = -1_i64;
    let status = unsafe {
        prodex_mojo_runtime_doctor_report_render_v1(
            1,
            &ffi_input as *const ReportInput as u64,
            output.as_mut_ptr() as u64,
            capacity,
            &mut written as *mut i64 as u64,
        )
    };
    if status != 0 {
        return Err(status_error(status, 12, 27, 0, 0));
    }
    let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    if written > output.len() {
        return Err(MojoError::InvalidOutput);
    }
    output.truncate(written);
    String::from_utf8(output).map_err(|_| MojoError::InvalidOutput)
}

#[cfg(test)]
#[path = "runtime_doctor_report_render_tests.rs"]
mod tests;
