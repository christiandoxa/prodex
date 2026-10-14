//! Route-health grouping ABI; Mojo remains the semantic owner.
use super::{MojoError, ensure_rich_abi, mojo_mut_pointer_address, mojo_pointer_address, view};

const RUNTIME_DOCTOR_ROUTE_HEALTH_GROUPS_ABI_VERSION: i64 = 1;
pub const RUNTIME_DOCTOR_ROUTE_HEALTH_GROUPS_MAX_ROWS: usize = 65_536;

unsafe extern "C" {
    fn prodex_mojo_runtime_doctor_route_health_groups_v1(
        abi_version: i64,
        profiles: u64,
        routes: u64,
        event_flags: u64,
        count: i64,
        representative_indices: u64,
        event_counts: u64,
        latest_event_indices: u64,
        profile_row_indices: u64,
        output_count: u64,
    ) -> i64;
    fn prodex_mojo_runtime_doctor_route_latest_reason_v1(
        abi_version: i64,
        event_profiles: u64,
        event_routes: u64,
        event_details: u64,
        event_count: i64,
        latest_index: i64,
        requested_profile: u64,
        requested_route: u64,
        output: u64,
    ) -> i64;
}

/// One grouped route-health row returned by the Mojo reducer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeDoctorRouteHealthGroup {
    pub representative_index: usize,
    pub event_count: usize,
    pub latest_event_index: Option<usize>,
    pub profile_row_index: Option<usize>,
}

/// Group and order route-health event/profile rows through the Mojo authority.
pub fn runtime_doctor_route_health_groups<'a>(
    rows: impl IntoIterator<Item = (&'a str, &'a str, bool)>,
) -> Result<Vec<RuntimeDoctorRouteHealthGroup>, MojoError> {
    ensure_rich_abi()?;
    let mut profiles = Vec::new();
    let mut routes = Vec::new();
    let mut event_flags = Vec::new();
    for (profile, route, is_event) in rows {
        if profiles.len() >= RUNTIME_DOCTOR_ROUTE_HEALTH_GROUPS_MAX_ROWS {
            return Err(MojoError::Capacity);
        }
        profiles.push(view(profile));
        routes.push(view(route));
        event_flags.push(i64::from(is_event));
    }
    if profiles.is_empty() {
        return Ok(Vec::new());
    }
    let count = i64::try_from(profiles.len()).map_err(|_| MojoError::InvalidInput)?;
    let mut representative_indices = vec![0_i64; profiles.len()];
    let mut event_counts = vec![0_i64; profiles.len()];
    let mut latest_event_indices = vec![-1_i64; profiles.len()];
    let mut profile_row_indices = vec![-1_i64; profiles.len()];
    let mut output_count = 0_i64;
    let status = unsafe {
        prodex_mojo_runtime_doctor_route_health_groups_v1(
            RUNTIME_DOCTOR_ROUTE_HEALTH_GROUPS_ABI_VERSION,
            mojo_pointer_address(profiles.as_ptr()),
            mojo_pointer_address(routes.as_ptr()),
            mojo_pointer_address(event_flags.as_ptr()),
            count,
            mojo_mut_pointer_address(representative_indices.as_mut_ptr()),
            mojo_mut_pointer_address(event_counts.as_mut_ptr()),
            mojo_mut_pointer_address(latest_event_indices.as_mut_ptr()),
            mojo_mut_pointer_address(profile_row_indices.as_mut_ptr()),
            mojo_mut_pointer_address(&mut output_count),
        )
    };
    if status != 0 {
        return Err(match status {
            1 => MojoError::InvalidInput,
            2 => MojoError::InvalidOutput,
            3 => MojoError::Capacity,
            4 => MojoError::AbiMismatch,
            5 => MojoError::InvalidOutput,
            _ => MojoError::InvalidOutput,
        });
    }
    let output_count = usize::try_from(output_count).map_err(|_| MojoError::InvalidOutput)?;
    if output_count > profiles.len() {
        return Err(MojoError::InvalidOutput);
    }
    let mut groups = Vec::with_capacity(output_count);
    for index in 0..output_count {
        let representative_index =
            usize::try_from(representative_indices[index]).map_err(|_| MojoError::InvalidOutput)?;
        if representative_index >= profiles.len() {
            return Err(MojoError::InvalidOutput);
        }
        let event_count =
            usize::try_from(event_counts[index]).map_err(|_| MojoError::InvalidOutput)?;
        let latest_event_index =
            route_health_source_index(latest_event_indices[index], &event_flags, 1)?;
        let profile_row_index =
            route_health_source_index(profile_row_indices[index], &event_flags, 0)?;
        groups.push(RuntimeDoctorRouteHealthGroup {
            representative_index,
            event_count,
            latest_event_index,
            profile_row_index,
        });
    }
    Ok(groups)
}

#[derive(Debug, Clone, Copy)]
pub struct RuntimeDoctorRouteReasonEvent<'a> {
    pub profile: &'a str,
    pub route: &'a str,
    pub detail: &'a str,
}

/// Mojo chooses the newest matching route event with a reason/outcome token,
/// then returns a borrowed UTF-8 range. No Rust token parser is retained.
pub fn runtime_doctor_route_latest_reason<'a>(
    events: &[RuntimeDoctorRouteReasonEvent<'a>],
    latest_index: usize,
    profile: &str,
    route: &str,
) -> Result<Option<&'a str>, MojoError> {
    ensure_rich_abi()?;
    if events.len() > RUNTIME_DOCTOR_ROUTE_HEALTH_GROUPS_MAX_ROWS {
        return Err(MojoError::Capacity);
    }
    if latest_index >= events.len() {
        return Err(MojoError::InvalidInput);
    }
    let profiles = events
        .iter()
        .map(|event| view(event.profile))
        .collect::<Vec<_>>();
    let routes = events
        .iter()
        .map(|event| view(event.route))
        .collect::<Vec<_>>();
    let details = events
        .iter()
        .map(|event| view(event.detail))
        .collect::<Vec<_>>();
    let requested_profile = view(profile);
    let requested_route = view(route);
    let mut output = [-1_i64; 3];
    let status = unsafe {
        prodex_mojo_runtime_doctor_route_latest_reason_v1(
            1,
            mojo_pointer_address(profiles.as_ptr()),
            mojo_pointer_address(routes.as_ptr()),
            mojo_pointer_address(details.as_ptr()),
            i64::try_from(events.len()).map_err(|_| MojoError::InvalidInput)?,
            i64::try_from(latest_index).map_err(|_| MojoError::InvalidInput)?,
            mojo_pointer_address(&requested_profile),
            mojo_pointer_address(&requested_route),
            mojo_mut_pointer_address(output.as_mut_ptr()),
        )
    };
    if status != 0 {
        return Err(match status {
            1 => MojoError::InvalidInput,
            2 => MojoError::InvalidOutput,
            3 => MojoError::Capacity,
            4 => MojoError::AbiMismatch,
            _ => MojoError::InvalidOutput,
        });
    }
    if output == [-1; 3] {
        return Ok(None);
    }
    let index = usize::try_from(output[0]).map_err(|_| MojoError::InvalidOutput)?;
    let start = usize::try_from(output[1]).map_err(|_| MojoError::InvalidOutput)?;
    let end = usize::try_from(output[2]).map_err(|_| MojoError::InvalidOutput)?;
    if index > latest_index {
        return Err(MojoError::InvalidOutput);
    }
    let detail = events.get(index).ok_or(MojoError::InvalidOutput)?.detail;
    detail
        .get(start..end)
        .map(Some)
        .ok_or(MojoError::InvalidOutput)
}

// Both optional indices carry the same ABI sentinel/range contract, with a
// different source row kind. Keep validation shared, not the Mojo reducer.
fn route_health_source_index(
    value: i64,
    event_flags: &[i64],
    expected_kind: i64,
) -> Result<Option<usize>, MojoError> {
    if value == -1 {
        return Ok(None);
    }
    let index = usize::try_from(value).map_err(|_| MojoError::InvalidOutput)?;
    if event_flags.get(index) != Some(&expected_kind) {
        return Err(MojoError::InvalidOutput);
    }
    Ok(Some(index))
}

#[cfg(test)]
#[path = "runtime_doctor_route_reason_tests.rs"]
mod route_reason_tests;
