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
        let latest_event_index = match latest_event_indices[index] {
            -1 => None,
            value if value >= 0 => {
                Some(usize::try_from(value).map_err(|_| MojoError::InvalidOutput)?)
            }
            _ => return Err(MojoError::InvalidOutput),
        };
        if let Some(latest_event_index) = latest_event_index
            && (latest_event_index >= profiles.len() || event_flags[latest_event_index] != 1)
        {
            return Err(MojoError::InvalidOutput);
        }
        let profile_row_index = match profile_row_indices[index] {
            -1 => None,
            value if value >= 0 => {
                Some(usize::try_from(value).map_err(|_| MojoError::InvalidOutput)?)
            }
            _ => return Err(MojoError::InvalidOutput),
        };
        if let Some(profile_row_index) = profile_row_index
            && (profile_row_index >= profiles.len() || event_flags[profile_row_index] != 0)
        {
            return Err(MojoError::InvalidOutput);
        }
        groups.push(RuntimeDoctorRouteHealthGroup {
            representative_index,
            event_count,
            latest_event_index,
            profile_row_index,
        });
    }
    Ok(groups)
}
