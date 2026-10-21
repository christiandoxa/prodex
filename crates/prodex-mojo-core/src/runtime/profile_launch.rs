use crate::MojoError;

const ABI_VERSION: i64 = 1;
const MAX_OBSERVATIONS: usize = 1_048_576;

unsafe extern "C" {
    fn prodex_runtime_profile_next_selection_at_v1(
        abi_version: i64,
        timestamps_address: u64,
        count: i64,
        selected_at: i64,
        output_address: u64,
    ) -> i64;
    fn prodex_runtime_profile_exact_visibility_v1(
        abi_version: i64,
        names_address: u64,
        count: i64,
        selected_address: u64,
        selected_length: i64,
        output_address: u64,
        output_capacity: i64,
    ) -> i64;
    fn prodex_runtime_profile_name_resolution_v1(
        abi_version: i64,
        has_requested: i64,
        requested_exists: i64,
        has_active: i64,
        active_exists: i64,
        profile_count: i64,
        output_address: u64,
    ) -> i64;
}

fn check_status(status: i64) -> Result<(), MojoError> {
    match status {
        0 => Ok(()),
        1 => Err(MojoError::InvalidInput),
        4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Compiled monotonic last-selected timestamp, including saturated i64 values.
/// The caller handles state pruning and storage, without Rust recomputation.
pub fn profile_next_selection_at(timestamps: &[i64], selected_at: i64) -> Result<i64, MojoError> {
    if timestamps.len() > MAX_OBSERVATIONS {
        return Err(MojoError::InvalidInput);
    }
    let mut output = 0_i64;
    check_status(unsafe {
        prodex_runtime_profile_next_selection_at_v1(
            ABI_VERSION,
            timestamps.as_ptr() as usize as u64,
            i64::try_from(timestamps.len()).map_err(|_| MojoError::InvalidInput)?,
            selected_at,
            (&mut output as *mut i64) as usize as u64,
        )
    })?;
    Ok(output)
}

/// Resolution action chosen by compiled Mojo; Rust only owns profile names and
/// the messages used to report unresolved selections.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileNameResolution {
    Requested,
    Active,
    Single,
    MissingRequested,
    MissingActive,
    NoSelection,
}

/// Choose the named-profile resolution path from host map-lookup facts.
pub fn profile_name_resolution_plan(
    requested: Option<bool>,
    active: Option<bool>,
    profile_count: usize,
) -> Result<ProfileNameResolution, MojoError> {
    if profile_count > MAX_OBSERVATIONS {
        return Err(MojoError::InvalidInput);
    }
    let mut output = -1_i64;
    check_status(unsafe {
        prodex_runtime_profile_name_resolution_v1(
            ABI_VERSION,
            i64::from(requested.is_some()),
            i64::from(requested.unwrap_or(false)),
            i64::from(active.is_some()),
            i64::from(active.unwrap_or(false)),
            i64::try_from(profile_count).map_err(|_| MojoError::InvalidInput)?,
            (&mut output as *mut i64) as usize as u64,
        )
    })?;
    match output {
        0 => Ok(ProfileNameResolution::Requested),
        1 => Ok(ProfileNameResolution::Active),
        2 => Ok(ProfileNameResolution::Single),
        3 => Ok(ProfileNameResolution::MissingRequested),
        4 => Ok(ProfileNameResolution::MissingActive),
        5 => Ok(ProfileNameResolution::NoSelection),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Return the exact-match keep mask for borrowed profile identifiers. Mojo
/// owns the comparison; the host only applies the returned flags to its maps.
pub fn profile_exact_visibility_mask(
    names: &[&str],
    selected: &str,
) -> Result<Vec<bool>, MojoError> {
    if names.len() > MAX_OBSERVATIONS
        || selected.len() > 65_536
        || names.iter().any(|name| name.len() > 65_536)
    {
        return Err(MojoError::InvalidInput);
    }
    let mut views = Vec::<u64>::with_capacity(names.len().saturating_mul(2));
    for name in names {
        views.extend([
            name.as_ptr() as usize as u64,
            u64::try_from(name.len()).map_err(|_| MojoError::InvalidInput)?,
        ]);
    }
    let mut output = vec![-1_i64; names.len()];
    check_status(unsafe {
        prodex_runtime_profile_exact_visibility_v1(
            ABI_VERSION,
            views.as_ptr() as usize as u64,
            i64::try_from(names.len()).map_err(|_| MojoError::InvalidInput)?,
            selected.as_ptr() as usize as u64,
            i64::try_from(selected.len()).map_err(|_| MojoError::InvalidInput)?,
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
        )
    })?;
    output
        .into_iter()
        .map(|tag| match tag {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(MojoError::InvalidOutput),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiled_timestamp_planner_preserves_monotonic_saturation() {
        assert_eq!(profile_next_selection_at(&[], -10), Ok(1));
        assert_eq!(profile_next_selection_at(&[-5, -1], 0), Ok(1));
        assert_eq!(profile_next_selection_at(&[12, 20], 4), Ok(21));
        assert_eq!(profile_next_selection_at(&[12, 20], 30), Ok(30));
        assert_eq!(profile_next_selection_at(&[i64::MAX], 0), Ok(i64::MAX));
        assert_eq!(profile_next_selection_at(&[i64::MIN, 0], i64::MIN), Ok(1));
    }

    #[test]
    fn compiled_profile_visibility_is_exact_and_handles_unicode() {
        assert_eq!(
            profile_exact_visibility_mask(&["main", "main-extra", "MAIN", "雪", "雪", ""], "雪",),
            Ok(vec![false, false, false, true, true, false])
        );
        assert_eq!(profile_exact_visibility_mask(&[], "anything"), Ok(vec![]));
        assert_eq!(
            profile_exact_visibility_mask(&["", "main"], ""),
            Ok(vec![true, false])
        );
    }

    #[test]
    fn compiled_resolution_respects_requested_active_and_single_precedence() {
        use ProfileNameResolution as Action;
        assert_eq!(
            profile_name_resolution_plan(Some(true), Some(true), 2),
            Ok(Action::Requested)
        );
        assert_eq!(
            profile_name_resolution_plan(Some(false), Some(true), 2),
            Ok(Action::MissingRequested)
        );
        assert_eq!(
            profile_name_resolution_plan(None, Some(true), 2),
            Ok(Action::Active)
        );
        assert_eq!(
            profile_name_resolution_plan(None, Some(false), 1),
            Ok(Action::MissingActive)
        );
        assert_eq!(
            profile_name_resolution_plan(None, None, 1),
            Ok(Action::Single)
        );
        assert_eq!(
            profile_name_resolution_plan(None, None, 0),
            Ok(Action::NoSelection)
        );
        assert_eq!(
            profile_name_resolution_plan(None, None, 2),
            Ok(Action::NoSelection)
        );
    }
}
