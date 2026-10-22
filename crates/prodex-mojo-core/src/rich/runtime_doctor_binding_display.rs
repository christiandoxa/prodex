//! Runtime-doctor binding text from compiled policy over numeric host facts.
use super::{RichStringView, ensure_rich_abi, status_error, view};
use crate::MojoError;

/// One source whose profile order already comes from compiled binding aggregation.
#[derive(Clone, Copy, Default)]
pub struct RuntimeDoctorBindingDisplaySource<'a> {
    pub response_bindings: u64,
    pub session_bindings: u64,
    pub turn_state_bindings: u64,
    pub session_id_bindings: u64,
    pub total_bindings: u64,
    pub profile_count: u64,
    pub missing_profile_bindings: u64,
    pub top_profile: Option<(&'a str, u64)>,
}

/// Ordered state, runtime, journal and merged binding-source facts.
pub struct RuntimeDoctorBindingDisplayInput<'a> {
    pub active_profile: Option<&'a str>,
    pub profile_count: u64,
    pub selected_profiles: u64,
    pub sources: [RuntimeDoctorBindingDisplaySource<'a>; 4],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct BindingSource {
    response: u64,
    session: u64,
    turn_state: u64,
    session_id: u64,
    total: u64,
    profiles: u64,
    missing: u64,
    top_present: u64,
    top_name: RichStringView,
    top_count: u64,
}

#[repr(C)]
struct BindingInput {
    active_present: u64,
    active: RichStringView,
    profiles: u64,
    selected: u64,
    sources: [BindingSource; 4],
}

const _: () = assert!(std::mem::size_of::<BindingSource>() == 88);
const _: () = assert!(std::mem::size_of::<BindingInput>() == 392);

unsafe extern "C" {
    fn prodex_mojo_runtime_doctor_binding_display_v1(
        version: i64,
        input: u64,
        output: u64,
        capacity: i64,
        written: u64,
    ) -> i64;
}

/// Format binding counts, source labels and absent-name placeholders in Mojo.
pub fn runtime_doctor_binding_display(
    input: RuntimeDoctorBindingDisplayInput<'_>,
) -> Result<String, MojoError> {
    ensure_rich_abi()?;
    let capacity = input
        .sources
        .iter()
        .filter_map(|source| source.top_profile.map(|(name, _)| name))
        .chain(input.active_profile)
        .try_fold(2048_usize, |total, value| total.checked_add(value.len()))
        .ok_or(MojoError::InvalidInput)?;
    let capacity = i64::try_from(capacity).map_err(|_| MojoError::InvalidInput)?;
    let ffi = BindingInput {
        active_present: u64::from(input.active_profile.is_some()),
        active: view(input.active_profile.unwrap_or_default()),
        profiles: input.profile_count,
        selected: input.selected_profiles,
        sources: input.sources.map(|source| BindingSource {
            response: source.response_bindings,
            session: source.session_bindings,
            turn_state: source.turn_state_bindings,
            session_id: source.session_id_bindings,
            total: source.total_bindings,
            profiles: source.profile_count,
            missing: source.missing_profile_bindings,
            top_present: u64::from(source.top_profile.is_some()),
            top_name: view(source.top_profile.map_or("", |(name, _)| name)),
            top_count: source.top_profile.map_or(0, |(_, count)| count),
        }),
    };
    let mut output = vec![0_u8; capacity as usize];
    let mut written = -1_i64;
    let status = unsafe {
        prodex_mojo_runtime_doctor_binding_display_v1(
            1,
            &ffi as *const BindingInput as u64,
            output.as_mut_ptr() as u64,
            capacity,
            &mut written as *mut i64 as u64,
        )
    };
    if status != 0 {
        return Err(status_error(status, 12, 28, 0, 0));
    }
    let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    if written > output.len() {
        return Err(MojoError::InvalidOutput);
    }
    output.truncate(written);
    String::from_utf8(output).map_err(|_| MojoError::InvalidOutput)
}

#[cfg(test)]
#[path = "runtime_doctor_binding_display_tests.rs"]
mod tests;
