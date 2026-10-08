use crate::profile_export::ProfileImportStringView;
use crate::{MojoError, rich::ensure_rich_abi};

const ABI_IMPORT_LIFECYCLE_VERSION: i64 = 1;

unsafe extern "C" {
    fn prodex_profile_import_lifecycle_order_v1(
        abi_version: i64,
        operation_count: i64,
        names_address: u64,
        output_address: u64,
        output_capacity: i64,
        operation_count_address: u64,
        profile_count_address: u64,
    ) -> i64;
    fn prodex_profile_import_secret_path_valid_v1(
        abi_version: i64,
        path_address: u64,
        path_is_absolute: i64,
    ) -> i64;
    fn prodex_profile_import_auth_journal_commit_v1(
        abi_version: i64,
        profile_exists: i64,
        codex_home_matches: i64,
        state_after_known: i64,
        has_next_state: i64,
        email_matches: i64,
        provider_matches: i64,
        auth_matches: i64,
        secret_files_match: i64,
    ) -> i64;
}

/// Stable Mojo order for import mutations and their distinct lifecycle profile names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileImportLifecycleOrder {
    pub operation_indices: Vec<usize>,
    pub profile_indices: Vec<usize>,
}

/// Inputs used to decide whether an auth update journal describes committed state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProfileImportAuthJournalCommitInput {
    pub profile_exists: bool,
    pub codex_home_matches: bool,
    pub state_after_known: bool,
    pub has_next_state: bool,
    pub email_matches: bool,
    pub provider_matches: bool,
    pub auth_matches: bool,
    pub secret_files_match: bool,
}

/// Ask Mojo to sort mutations by profile name and select one lifecycle row per name.
pub fn profile_import_lifecycle_order(
    profile_names: &[&str],
) -> Result<ProfileImportLifecycleOrder, MojoError> {
    ensure_rich_abi()?;
    let names = profile_names
        .iter()
        .map(|name| ProfileImportStringView::from(Some(name)))
        .collect::<Result<Vec<_>, _>>()?;
    let output_capacity = profile_names
        .len()
        .checked_mul(2)
        .and_then(|capacity| capacity.checked_add(2))
        .ok_or(MojoError::InvalidInput)?;
    let mut output = vec![-1_i64; output_capacity.max(1)];
    let mut operation_count = 0_i64;
    let mut profile_count = 0_i64;
    let status = unsafe {
        prodex_profile_import_lifecycle_order_v1(
            ABI_IMPORT_LIFECYCLE_VERSION,
            i64::try_from(profile_names.len()).map_err(|_| MojoError::InvalidInput)?,
            names.as_ptr() as usize as u64,
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output_capacity).map_err(|_| MojoError::InvalidInput)?,
            (&mut operation_count as *mut i64) as usize as u64,
            (&mut profile_count as *mut i64) as usize as u64,
        )
    };
    match status {
        0 => {
            let operation_count =
                usize::try_from(output[0]).map_err(|_| MojoError::InvalidOutput)?;
            let profile_count = usize::try_from(output[1]).map_err(|_| MojoError::InvalidOutput)?;
            if operation_count != profile_names.len() || profile_count > operation_count {
                return Err(MojoError::InvalidOutput);
            }
            let mut seen_operations = vec![false; operation_count];
            let operation_indices = output[2..2 + operation_count]
                .iter()
                .map(|value| {
                    let index = usize::try_from(*value).map_err(|_| MojoError::InvalidOutput)?;
                    let Some(seen) = seen_operations.get_mut(index) else {
                        return Err(MojoError::InvalidOutput);
                    };
                    if *seen {
                        return Err(MojoError::InvalidOutput);
                    }
                    *seen = true;
                    Ok(index)
                })
                .collect::<Result<Vec<_>, _>>()?;
            if seen_operations.iter().any(|seen| !seen) {
                return Err(MojoError::InvalidOutput);
            }
            let mut seen_profiles = vec![false; operation_count];
            let profile_indices = output[2 + operation_count..2 + operation_count + profile_count]
                .iter()
                .map(|value| {
                    let index = usize::try_from(*value).map_err(|_| MojoError::InvalidOutput)?;
                    let Some(seen) = seen_profiles.get_mut(index) else {
                        return Err(MojoError::InvalidOutput);
                    };
                    if *seen {
                        return Err(MojoError::InvalidOutput);
                    }
                    *seen = true;
                    Ok(index)
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(ProfileImportLifecycleOrder {
                operation_indices,
                profile_indices,
            })
        }
        2 => Err(MojoError::Capacity),
        99 => Err(MojoError::InvalidInput),
        100 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Ask Mojo whether an exported secret path is a safe, nonempty leaf name.
pub fn profile_import_secret_path_is_safe(
    path: &str,
    is_absolute: bool,
) -> Result<bool, MojoError> {
    ensure_rich_abi()?;
    let view = ProfileImportStringView::from(Some(path))?;
    let status = unsafe {
        prodex_profile_import_secret_path_valid_v1(
            ABI_IMPORT_LIFECYCLE_VERSION,
            (&view as *const ProfileImportStringView) as usize as u64,
            i64::from(is_absolute),
        )
    };
    match status {
        0 => Ok(true),
        1 => Ok(false),
        99 => Err(MojoError::InvalidInput),
        100 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Ask Mojo whether all journaled post-update fields match the current host snapshot.
pub fn profile_import_auth_journal_is_committed(
    input: ProfileImportAuthJournalCommitInput,
) -> Result<bool, MojoError> {
    ensure_rich_abi()?;
    let status = unsafe {
        prodex_profile_import_auth_journal_commit_v1(
            ABI_IMPORT_LIFECYCLE_VERSION,
            i64::from(input.profile_exists),
            i64::from(input.codex_home_matches),
            i64::from(input.state_after_known),
            i64::from(input.has_next_state),
            i64::from(input.email_matches),
            i64::from(input.provider_matches),
            i64::from(input.auth_matches),
            i64::from(input.secret_files_match),
        )
    };
    match status {
        0 => Ok(true),
        1 => Ok(false),
        99 => Err(MojoError::InvalidInput),
        100 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_import_lifecycle_order_is_mojo_owned() {
        assert_eq!(
            profile_import_lifecycle_order(&[]).unwrap(),
            ProfileImportLifecycleOrder {
                operation_indices: Vec::new(),
                profile_indices: Vec::new(),
            }
        );
        let names = ["zeta", "alpha", "zeta", "beta", "alpha"];
        assert_eq!(
            profile_import_lifecycle_order(&names).unwrap(),
            ProfileImportLifecycleOrder {
                operation_indices: vec![1, 4, 3, 0, 2],
                profile_indices: vec![1, 3, 0],
            }
        );
    }

    #[test]
    fn profile_import_secret_path_rules_are_mojo_owned() {
        assert!(profile_import_secret_path_is_safe("auth.json", false).unwrap());
        for path in ["", " \u{3000} ", "../auth.json", "a/b", "a\\b", ".", ".."] {
            assert!(
                !profile_import_secret_path_is_safe(path, false).unwrap(),
                "{path:?}"
            );
        }
        assert!(!profile_import_secret_path_is_safe("auth.json", true).unwrap());
    }

    #[test]
    fn profile_import_auth_journal_commit_requires_every_match() {
        let committed = ProfileImportAuthJournalCommitInput {
            profile_exists: true,
            codex_home_matches: true,
            state_after_known: true,
            has_next_state: true,
            email_matches: true,
            provider_matches: true,
            auth_matches: true,
            secret_files_match: true,
        };
        assert!(profile_import_auth_journal_is_committed(committed).unwrap());
        assert!(
            !profile_import_auth_journal_is_committed(ProfileImportAuthJournalCommitInput {
                auth_matches: false,
                ..committed
            })
            .unwrap()
        );
        assert!(
            !profile_import_auth_journal_is_committed(ProfileImportAuthJournalCommitInput {
                has_next_state: false,
                ..committed
            })
            .unwrap()
        );
    }
}
