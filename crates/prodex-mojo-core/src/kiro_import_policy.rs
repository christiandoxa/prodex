use crate::MojoError;

const ABI_VERSION: i64 = 1;

unsafe extern "C" {
    fn prodex_kiro_auth_key_choice_v1(abi_version: i64, priority_presence_mask: i64) -> i64;
    fn prodex_kiro_auth_kind_v1(
        abi_version: i64,
        auth_source: i64,
        start_url_present: i64,
        start_url_is_builder: i64,
    ) -> i64;
    fn prodex_kiro_profile_match_v1(
        abi_version: i64,
        records_address: u64,
        record_count: i64,
    ) -> i64;
    fn prodex_kiro_import_action_v1(
        abi_version: i64,
        matching_profile_present: i64,
        requested_name_present: i64,
        requested_name_valid: i64,
        requested_name_matches: i64,
    ) -> i64;
    fn prodex_kiro_auth_store_action_v1(
        abi_version: i64,
        incoming_expiry_present: i64,
        stored_auth_present: i64,
        stored_expiry_present: i64,
        incoming_is_newer: i64,
    ) -> i64;
}

/// The priority source selected from Kiro's credential database.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KiroAuthKeySource {
    Social,
    ExternalIdp,
    OtherPriority,
}

/// Kiro authentication mode derived from non-secret source facts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KiroAuthKind {
    Social,
    ExternalIdp,
    IdentityCenter,
    BuilderId,
}

/// Side effect selected after identity matching and requested-name validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KiroImportAction {
    Create,
    Update,
}

/// Typed validation failures for the Kiro import action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KiroImportActionError {
    Mojo(MojoError),
    InvalidRequestedName,
    ExistingNameMismatch,
}

/// Whether an incoming auth snapshot should replace the stored snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KiroAuthStoreAction {
    UseIncoming,
    UseStored,
}

pub fn kiro_auth_key_source(
    priority_present: [bool; 3],
) -> Result<Option<KiroAuthKeySource>, MojoError> {
    let mask = priority_present
        .into_iter()
        .enumerate()
        .fold(0_i64, |mask, (index, present)| {
            mask | (i64::from(present) << index)
        });
    let result = unsafe { prodex_kiro_auth_key_choice_v1(ABI_VERSION, mask) };
    match result {
        -2 => Ok(None),
        0 => Ok(Some(KiroAuthKeySource::Social)),
        1 => Ok(Some(KiroAuthKeySource::ExternalIdp)),
        2 => Ok(Some(KiroAuthKeySource::OtherPriority)),
        -1 => Err(MojoError::InvalidInput),
        -4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn kiro_auth_kind(
    source: Option<KiroAuthKeySource>,
    start_url_present: bool,
    start_url_is_builder: bool,
) -> Result<KiroAuthKind, MojoError> {
    let auth_source = match source {
        Some(KiroAuthKeySource::Social) => 0,
        Some(KiroAuthKeySource::ExternalIdp) => 1,
        Some(KiroAuthKeySource::OtherPriority) => 2,
        None => 2,
    };
    let result = unsafe {
        prodex_kiro_auth_kind_v1(
            ABI_VERSION,
            auth_source,
            i64::from(start_url_present),
            i64::from(start_url_is_builder),
        )
    };
    match result {
        0 => Ok(KiroAuthKind::Social),
        1 => Ok(KiroAuthKind::ExternalIdp),
        2 => Ok(KiroAuthKind::IdentityCenter),
        3 => Ok(KiroAuthKind::BuilderId),
        -1 => Err(MojoError::InvalidInput),
        -4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn kiro_profile_match(flags: &[i64]) -> Result<Option<usize>, MojoError> {
    if flags.len() > i64::MAX as usize {
        return Err(MojoError::InvalidInput);
    }
    let result = unsafe {
        prodex_kiro_profile_match_v1(
            ABI_VERSION,
            flags.as_ptr() as usize as u64,
            i64::try_from(flags.len()).map_err(|_| MojoError::InvalidInput)?,
        )
    };
    match result {
        -2 => Ok(None),
        -1 => Err(MojoError::InvalidInput),
        -4 => Err(MojoError::AbiMismatch),
        index if index >= 0 => usize::try_from(index)
            .map(Some)
            .map_err(|_| MojoError::InvalidOutput),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn kiro_import_action(
    matching_profile_present: bool,
    requested_name_present: bool,
    requested_name_valid: bool,
    requested_name_matches: bool,
) -> Result<KiroImportAction, KiroImportActionError> {
    let result = unsafe {
        prodex_kiro_import_action_v1(
            ABI_VERSION,
            i64::from(matching_profile_present),
            i64::from(requested_name_present),
            i64::from(requested_name_valid),
            i64::from(requested_name_matches),
        )
    };
    match result {
        0 => Ok(KiroImportAction::Create),
        1 => Ok(KiroImportAction::Update),
        -2 => Err(KiroImportActionError::InvalidRequestedName),
        -3 => Err(KiroImportActionError::ExistingNameMismatch),
        -1 => Err(KiroImportActionError::Mojo(MojoError::InvalidInput)),
        -4 => Err(KiroImportActionError::Mojo(MojoError::AbiMismatch)),
        _ => Err(KiroImportActionError::Mojo(MojoError::InvalidOutput)),
    }
}

pub fn kiro_auth_store_action(
    incoming_expiry_present: bool,
    stored_auth_present: bool,
    stored_expiry_present: bool,
    incoming_is_newer: bool,
) -> Result<KiroAuthStoreAction, MojoError> {
    let result = unsafe {
        prodex_kiro_auth_store_action_v1(
            ABI_VERSION,
            i64::from(incoming_expiry_present),
            i64::from(stored_auth_present),
            i64::from(stored_expiry_present),
            i64::from(incoming_is_newer),
        )
    };
    match result {
        0 => Ok(KiroAuthStoreAction::UseIncoming),
        1 => Ok(KiroAuthStoreAction::UseStored),
        -1 => Err(MojoError::InvalidInput),
        -4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kiro_import_policy_matrix_covers_precedence_matching_actions_and_freshness() {
        assert_eq!(
            kiro_auth_key_source([true, true, true]).unwrap(),
            Some(KiroAuthKeySource::Social)
        );
        assert_eq!(
            kiro_auth_key_source([false, true, false]).unwrap(),
            Some(KiroAuthKeySource::ExternalIdp)
        );
        assert_eq!(
            kiro_auth_key_source([false, false, true]).unwrap(),
            Some(KiroAuthKeySource::OtherPriority)
        );
        assert_eq!(kiro_auth_key_source([false, false, false]).unwrap(), None);

        assert_eq!(
            kiro_auth_kind(Some(KiroAuthKeySource::Social), true, false).unwrap(),
            KiroAuthKind::Social
        );
        assert_eq!(
            kiro_auth_kind(Some(KiroAuthKeySource::ExternalIdp), true, false).unwrap(),
            KiroAuthKind::ExternalIdp
        );
        assert_eq!(
            kiro_auth_kind(None, true, false).unwrap(),
            KiroAuthKind::IdentityCenter
        );
        assert_eq!(
            kiro_auth_kind(None, true, true).unwrap(),
            KiroAuthKind::BuilderId
        );

        assert_eq!(kiro_profile_match(&[1, 7, 7]).unwrap(), Some(1));
        assert_eq!(kiro_profile_match(&[0, 1, 3]).unwrap(), None);

        assert_eq!(
            kiro_import_action(false, false, false, true).unwrap(),
            KiroImportAction::Create
        );
        assert_eq!(
            kiro_import_action(true, false, false, true).unwrap(),
            KiroImportAction::Update
        );
        assert_eq!(
            kiro_import_action(true, true, true, true).unwrap(),
            KiroImportAction::Update
        );
        assert_eq!(
            kiro_import_action(false, true, true, false).unwrap(),
            KiroImportAction::Create
        );
        assert_eq!(
            kiro_import_action(false, true, false, false),
            Err(KiroImportActionError::InvalidRequestedName)
        );
        assert_eq!(
            kiro_import_action(true, true, true, false),
            Err(KiroImportActionError::ExistingNameMismatch)
        );

        assert_eq!(
            kiro_auth_store_action(false, false, false, false).unwrap(),
            KiroAuthStoreAction::UseIncoming
        );
        assert_eq!(
            kiro_auth_store_action(true, true, true, true).unwrap(),
            KiroAuthStoreAction::UseIncoming
        );
        assert_eq!(
            kiro_auth_store_action(true, true, false, true).unwrap(),
            KiroAuthStoreAction::UseIncoming
        );
        assert_eq!(
            kiro_auth_store_action(true, true, true, false).unwrap(),
            KiroAuthStoreAction::UseStored
        );
    }
}
