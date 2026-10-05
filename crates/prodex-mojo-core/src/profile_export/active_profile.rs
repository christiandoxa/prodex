use super::{ABI_VERSION, ProfileImportStringView};
use crate::{MojoError, rich::ensure_rich_abi};

unsafe extern "C" {
    fn prodex_profile_export_active_profile_selected_v1(
        abi_version: i64,
        active_address: u64,
        selected_address: u64,
        selected_count: i64,
        is_selected_address: u64,
    ) -> i64;
    fn prodex_profile_import_active_profile_plan_v1(
        abi_version: i64,
        existing_address: u64,
        source_address: u64,
        mapping_sources_address: u64,
        mapping_targets_address: u64,
        mapping_count: i64,
        output_address: u64,
    ) -> i64;
}

/// Ask Mojo whether the active profile belongs to the selected export set.
pub fn profile_export_active_profile_selected(
    active_profile: Option<&str>,
    selected_profile_names: &[&str],
) -> Result<bool, MojoError> {
    ensure_rich_abi()?;
    let active = ProfileImportStringView::from(active_profile)?;
    let selected = selected_profile_names
        .iter()
        .map(|name| ProfileImportStringView::from(Some(name)))
        .collect::<Result<Vec<_>, _>>()?;
    let mut is_selected = 0_i64;
    let status = unsafe {
        prodex_profile_export_active_profile_selected_v1(
            ABI_VERSION,
            (&active as *const ProfileImportStringView) as usize as u64,
            selected.as_ptr() as usize as u64,
            i64::try_from(selected.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut is_selected as *mut i64) as usize as u64,
        )
    };
    match (status, is_selected) {
        (0, 0) => Ok(false),
        (0, 1) => Ok(true),
        (99, _) => Err(MojoError::InvalidInput),
        (100, _) => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileImportActiveProfilePlan {
    None,
    Existing,
    Resolved(usize),
}

/// Resolve imported active-profile precedence and source-name mapping in Mojo.
pub fn profile_import_active_profile_plan(
    existing_active_profile: Option<&str>,
    source_active_profile: Option<&str>,
    resolved_profile_names: &[(&str, &str)],
) -> Result<ProfileImportActiveProfilePlan, MojoError> {
    ensure_rich_abi()?;
    let existing = ProfileImportStringView::from(existing_active_profile)?;
    let source = ProfileImportStringView::from(source_active_profile)?;
    let mapping_sources = resolved_profile_names
        .iter()
        .map(|(name, _)| ProfileImportStringView::from(Some(name)))
        .collect::<Result<Vec<_>, _>>()?;
    let mapping_targets = resolved_profile_names
        .iter()
        .map(|(_, name)| ProfileImportStringView::from(Some(name)))
        .collect::<Result<Vec<_>, _>>()?;
    let mut output = [0_i64, -1_i64];
    let status = unsafe {
        prodex_profile_import_active_profile_plan_v1(
            ABI_VERSION,
            (&existing as *const ProfileImportStringView) as usize as u64,
            (&source as *const ProfileImportStringView) as usize as u64,
            mapping_sources.as_ptr() as usize as u64,
            mapping_targets.as_ptr() as usize as u64,
            i64::try_from(resolved_profile_names.len()).map_err(|_| MojoError::InvalidInput)?,
            output.as_mut_ptr() as usize as u64,
        )
    };
    match status {
        0 => match output {
            [0, -1] => Ok(ProfileImportActiveProfilePlan::None),
            [1, -1] if existing_active_profile.is_some() => {
                Ok(ProfileImportActiveProfilePlan::Existing)
            }
            [2, index] => {
                let index = usize::try_from(index).map_err(|_| MojoError::InvalidOutput)?;
                if index >= resolved_profile_names.len() {
                    return Err(MojoError::InvalidOutput);
                }
                Ok(ProfileImportActiveProfilePlan::Resolved(index))
            }
            _ => Err(MojoError::InvalidOutput),
        },
        99 => Err(MojoError::InvalidInput),
        100 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}
