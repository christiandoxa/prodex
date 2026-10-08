use crate::MojoError;

const PROFILE_IDENTITY_ABI_VERSION: i64 = 1;
const PROFILE_IDENTITY_RECORD_EMAIL_PRESENT: i64 = 1;
const PROFILE_IDENTITY_RECORD_ACCOUNT_PRESENT: i64 = 2;
const PROFILE_PRIMARY_PRESENT: i64 = 1;
const PROFILE_SECONDARY_PRESENT: i64 = 2;
const PROFILE_MANAGEMENT_STATUS_ABI_VERSION: i64 = 1;

#[repr(i64)]
#[derive(Clone, Copy)]
enum ProfileIdentityOperation {
    NormalizeEmail = 0,
    NormalizeAccount = 1,
    ProfileName = 2,
    CanonicalKey = 3,
    ValidateProfileName = 4,
    AddProfileSource = 5,
    ShouldActivate = 6,
    OtherEmailName = 7,
    FindMatch = 8,
    RemoveTargets = 9,
    DeleteHome = 10,
    SanitizeSlug = 11,
    TrimmedEqual = 12,
    TrimmedCasefoldEqual = 13,
    OptionalCasefoldEqual = 14,
    OptionalCasefoldWildcard = 15,
    OptionalNonemptyCasefoldEqual = 16,
    FirstPresentSource = 17,
    RemovedActiveChoice = 18,
    NameCandidate = 19,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct RawProfileIdentityRecord {
    email_address: u64,
    email_length: u64,
    account_address: u64,
    account_length: u64,
    flags: i64,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct RawProfileRemovalRecord {
    name_address: u64,
    name_length: u64,
    managed: i64,
}

#[derive(Clone, Copy, Debug)]
pub struct ProfileIdentityRecord<'a> {
    pub email: Option<&'a str>,
    pub account_id: Option<&'a str>,
}

#[derive(Clone, Copy, Debug)]
pub struct ProfileRemovalRecord<'a> {
    pub name: &'a str,
    pub managed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RemoveProfileTargetsPlan {
    All,
    One(usize),
    MissingRequested,
    NotFound,
    ExternalBulk(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProfileHomeDeletePlan {
    Keep,
    Delete,
    RejectExternal,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProfileNameValidation {
    Valid,
    Empty,
    PathSeparator,
    DotPath,
    InvalidCharacter,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AddProfileSourcePlan {
    ExternalHome,
    CopyFrom,
    CopyCurrent,
    EmptyManaged,
    ExternalHomeConflict,
    CopyConflict,
}

/// Inputs projected from the state adapter for the profile-management screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProfileManagementStatusInput {
    pub active: bool,
    pub managed: bool,
    pub identity_present: bool,
}

/// The screen-level state selected by the Mojo profile-management policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProfileManagementScreenStatus {
    NoActive,
    OnlyProfile,
    Active,
}

/// The current/storage state selected for one profile row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProfileManagementRowStatus {
    ActiveManagedWithIdentity,
    ActiveManagedWithoutIdentity,
    InactiveManagedWithIdentity,
    InactiveManagedWithoutIdentity,
    ActiveExternalWithIdentity,
    ActiveExternalWithoutIdentity,
    InactiveExternalWithIdentity,
    InactiveExternalWithoutIdentity,
}

impl ProfileManagementRowStatus {
    /// Render the stable current label for this row status.
    pub fn current_label(self) -> &'static str {
        match self {
            Self::ActiveManagedWithIdentity
            | Self::ActiveManagedWithoutIdentity
            | Self::ActiveExternalWithIdentity
            | Self::ActiveExternalWithoutIdentity => "Yes",
            Self::InactiveManagedWithIdentity
            | Self::InactiveManagedWithoutIdentity
            | Self::InactiveExternalWithIdentity
            | Self::InactiveExternalWithoutIdentity => "No",
        }
    }

    /// Render the stable storage-kind label for this row status.
    pub fn kind_label(self) -> &'static str {
        match self {
            Self::ActiveManagedWithIdentity
            | Self::ActiveManagedWithoutIdentity
            | Self::InactiveManagedWithIdentity
            | Self::InactiveManagedWithoutIdentity => "managed",
            Self::ActiveExternalWithIdentity
            | Self::ActiveExternalWithoutIdentity
            | Self::InactiveExternalWithIdentity
            | Self::InactiveExternalWithoutIdentity => "external",
        }
    }

    /// Render the stable managed/external boolean label for this row status.
    pub fn managed_label(self) -> &'static str {
        match self {
            Self::ActiveManagedWithIdentity
            | Self::ActiveManagedWithoutIdentity
            | Self::InactiveManagedWithIdentity
            | Self::InactiveManagedWithoutIdentity => "Yes",
            Self::ActiveExternalWithIdentity
            | Self::ActiveExternalWithoutIdentity
            | Self::InactiveExternalWithIdentity
            | Self::InactiveExternalWithoutIdentity => "No",
        }
    }

    /// Whether this row contains a stored profile identity.
    pub fn identity_present(self) -> bool {
        matches!(
            self,
            Self::ActiveManagedWithIdentity
                | Self::InactiveManagedWithIdentity
                | Self::ActiveExternalWithIdentity
                | Self::InactiveExternalWithIdentity
        )
    }
}

/// Complete deterministic decision for a profile-management screen snapshot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfileManagementStatusPlan {
    pub screen: ProfileManagementScreenStatus,
    pub rows: Vec<ProfileManagementRowStatus>,
}

unsafe extern "C" {
    fn prodex_mojo_profile_identity_v1(
        abi_version: i64,
        operation: i64,
        primary_address: u64,
        primary_length: i64,
        secondary_address: u64,
        secondary_length: i64,
        flags: i64,
        records_address: u64,
        record_count: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
        result_address: u64,
    ) -> i64;
    fn prodex_profile_management_status_v1(
        abi_version: i64,
        active_profile_present: i64,
        profile_count: i64,
        flags_address: u64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
    ) -> i64;
}

fn signed(value: usize) -> Result<i64, MojoError> {
    i64::try_from(value).map_err(|_| MojoError::InvalidInput)
}

fn ptr(value: &str) -> u64 {
    value.as_ptr() as usize as u64
}

fn optional_flag(value: Option<&str>, bit: i64) -> i64 {
    if value.is_some() { bit } else { 0 }
}

fn raw_record(record: ProfileIdentityRecord<'_>) -> Result<RawProfileIdentityRecord, MojoError> {
    let email = record.email.unwrap_or_default();
    let account = record.account_id.unwrap_or_default();
    Ok(RawProfileIdentityRecord {
        email_address: ptr(email),
        email_length: u64::try_from(email.len()).map_err(|_| MojoError::InvalidInput)?,
        account_address: ptr(account),
        account_length: u64::try_from(account.len()).map_err(|_| MojoError::InvalidInput)?,
        flags: optional_flag(record.email, PROFILE_IDENTITY_RECORD_EMAIL_PRESENT)
            | optional_flag(record.account_id, PROFILE_IDENTITY_RECORD_ACCOUNT_PRESENT),
    })
}

fn raw_removal_record(
    record: ProfileRemovalRecord<'_>,
) -> Result<RawProfileRemovalRecord, MojoError> {
    Ok(RawProfileRemovalRecord {
        name_address: ptr(record.name),
        name_length: u64::try_from(record.name.len()).map_err(|_| MojoError::InvalidInput)?,
        managed: i64::from(record.managed),
    })
}

struct KernelResult {
    output: Vec<u8>,
    written: usize,
    result: i64,
}

fn call_kernel(
    operation: ProfileIdentityOperation,
    primary: &str,
    secondary: &str,
    flags: i64,
    records_address: u64,
    record_count: usize,
    output_capacity: usize,
) -> Result<KernelResult, MojoError> {
    let mut output = vec![0_u8; output_capacity.max(1)];
    let mut written = 0_i64;
    let mut result = 0_i64;
    let status = unsafe {
        prodex_mojo_profile_identity_v1(
            PROFILE_IDENTITY_ABI_VERSION,
            operation as i64,
            ptr(primary),
            signed(primary.len())?,
            ptr(secondary),
            signed(secondary.len())?,
            flags,
            records_address,
            signed(record_count)?,
            output.as_mut_ptr() as usize as u64,
            signed(output.len())?,
            (&mut written as *mut i64) as usize as u64,
            (&mut result as *mut i64) as usize as u64,
        )
    };
    if status != 0 {
        return Err(match status {
            1 => MojoError::InvalidInput,
            2 => MojoError::Capacity,
            4 => MojoError::AbiMismatch,
            _ => MojoError::InvalidOutput,
        });
    }
    let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    if written > output.len() {
        return Err(MojoError::InvalidOutput);
    }
    Ok(KernelResult {
        output,
        written,
        result,
    })
}

fn string_result(
    operation: ProfileIdentityOperation,
    input: &str,
    minimum_capacity: usize,
) -> Result<String, MojoError> {
    let capacity = input.len().max(minimum_capacity);
    let result = call_kernel(operation, input, "", 0, 0, 0, capacity)?;
    String::from_utf8(result.output[..result.written].to_vec())
        .map_err(|_| MojoError::InvalidOutput)
}

pub fn normalize_email(value: &str) -> Result<String, MojoError> {
    string_result(ProfileIdentityOperation::NormalizeEmail, value, 1)
}

pub fn normalize_account_id(value: &str) -> Result<String, MojoError> {
    string_result(ProfileIdentityOperation::NormalizeAccount, value, 1)
}

pub fn profile_name_from_email(value: &str) -> Result<String, MojoError> {
    string_result(
        ProfileIdentityOperation::ProfileName,
        value,
        "profile".len(),
    )
}

pub fn profile_name_candidate(
    base_name: &str,
    fallback_name: &str,
    attempt: u64,
) -> Result<String, MojoError> {
    let capacity = base_name
        .len()
        .max(fallback_name.len())
        .checked_add(20)
        .ok_or(MojoError::InvalidInput)?;
    let result = call_kernel(
        ProfileIdentityOperation::NameCandidate,
        base_name,
        fallback_name,
        i64::try_from(attempt).map_err(|_| MojoError::InvalidInput)?,
        0,
        0,
        capacity,
    )?;
    String::from_utf8(result.output[..result.written].to_vec())
        .map_err(|_| MojoError::InvalidOutput)
}

pub fn sanitize_profile_slug(value: &str) -> Result<String, MojoError> {
    string_result(
        ProfileIdentityOperation::SanitizeSlug,
        value,
        "api_key".len(),
    )
}

pub fn canonical_profile_identity_key(
    account_id: Option<&str>,
    email: Option<&str>,
) -> Result<Option<String>, MojoError> {
    let account = account_id.unwrap_or_default();
    let email_value = email.unwrap_or_default();
    let capacity = account
        .len()
        .checked_add(email_value.len())
        .and_then(|value| value.checked_add("account:|email:".len()))
        .ok_or(MojoError::InvalidInput)?;
    let flags = optional_flag(account_id, PROFILE_PRIMARY_PRESENT)
        | optional_flag(email, PROFILE_SECONDARY_PRESENT);
    let result = call_kernel(
        ProfileIdentityOperation::CanonicalKey,
        account,
        email_value,
        flags,
        0,
        0,
        capacity,
    )?;
    match result.result {
        0 if result.written == 0 => Ok(None),
        1 => String::from_utf8(result.output[..result.written].to_vec())
            .map(Some)
            .map_err(|_| MojoError::InvalidOutput),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn validate_profile_name(value: &str) -> Result<ProfileNameValidation, MojoError> {
    let result = call_kernel(
        ProfileIdentityOperation::ValidateProfileName,
        value,
        "",
        0,
        0,
        0,
        1,
    )?;
    match result.result {
        0 => Ok(ProfileNameValidation::Valid),
        1 => Ok(ProfileNameValidation::Empty),
        2 => Ok(ProfileNameValidation::PathSeparator),
        3 => Ok(ProfileNameValidation::DotPath),
        4 => Ok(ProfileNameValidation::InvalidCharacter),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn add_profile_source_plan(
    codex_home_provided: bool,
    copy_from_provided: bool,
    copy_current: bool,
) -> Result<AddProfileSourcePlan, MojoError> {
    let flags = i64::from(codex_home_provided)
        | (i64::from(copy_from_provided) << 1)
        | (i64::from(copy_current) << 2);
    let result = call_kernel(
        ProfileIdentityOperation::AddProfileSource,
        "",
        "",
        flags,
        0,
        0,
        1,
    )?;
    match result.result {
        0 => Ok(AddProfileSourcePlan::ExternalHome),
        1 => Ok(AddProfileSourcePlan::CopyFrom),
        2 => Ok(AddProfileSourcePlan::CopyCurrent),
        3 => Ok(AddProfileSourcePlan::EmptyManaged),
        10 => Ok(AddProfileSourcePlan::ExternalHomeConflict),
        11 => Ok(AddProfileSourcePlan::CopyConflict),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn should_activate_profile(
    active_profile_exists: bool,
    activate_requested: bool,
) -> Result<bool, MojoError> {
    let flags = i64::from(active_profile_exists) | (i64::from(activate_requested) << 1);
    let result = call_kernel(
        ProfileIdentityOperation::ShouldActivate,
        "",
        "",
        flags,
        0,
        0,
        1,
    )?;
    match result.result {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn profile_name_looks_email_derived_for_other_email(
    profile_name: &str,
    email: &str,
) -> Result<bool, MojoError> {
    let result = call_kernel(
        ProfileIdentityOperation::OtherEmailName,
        profile_name,
        email,
        0,
        0,
        0,
        email.len().max("profile".len()),
    )?;
    match result.result {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn find_matching_profile_identity(
    records: &[ProfileIdentityRecord<'_>],
    target_account_id: Option<&str>,
    target_email: Option<&str>,
) -> Result<Option<usize>, MojoError> {
    let raw = records
        .iter()
        .copied()
        .map(raw_record)
        .collect::<Result<Vec<_>, _>>()?;
    let account = target_account_id.unwrap_or_default();
    let email = target_email.unwrap_or_default();
    let flags = optional_flag(target_account_id, PROFILE_PRIMARY_PRESENT)
        | optional_flag(target_email, PROFILE_SECONDARY_PRESENT);
    let result = call_kernel(
        ProfileIdentityOperation::FindMatch,
        account,
        email,
        flags,
        raw.as_ptr() as usize as u64,
        raw.len(),
        1,
    )?;
    if result.result == -1 {
        return Ok(None);
    }
    let index = usize::try_from(result.result).map_err(|_| MojoError::InvalidOutput)?;
    (index < records.len())
        .then_some(Some(index))
        .ok_or(MojoError::InvalidOutput)
}

fn profile_identity_relation(
    operation: ProfileIdentityOperation,
    left: Option<&str>,
    right: Option<&str>,
) -> Result<bool, MojoError> {
    let left_value = left.unwrap_or_default();
    let right_value = right.unwrap_or_default();
    let flags = optional_flag(left, PROFILE_PRIMARY_PRESENT)
        | optional_flag(right, PROFILE_SECONDARY_PRESENT);
    let result = call_kernel(operation, left_value, right_value, flags, 0, 0, 1)?;
    match result.result {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn trimmed_equal(left: &str, right: &str) -> Result<bool, MojoError> {
    profile_identity_relation(
        ProfileIdentityOperation::TrimmedEqual,
        Some(left),
        Some(right),
    )
}

pub fn trimmed_casefold_equal(left: &str, right: &str) -> Result<bool, MojoError> {
    profile_identity_relation(
        ProfileIdentityOperation::TrimmedCasefoldEqual,
        Some(left),
        Some(right),
    )
}

pub fn optional_trimmed_casefold_equal(
    left: Option<&str>,
    right: Option<&str>,
) -> Result<bool, MojoError> {
    profile_identity_relation(ProfileIdentityOperation::OptionalCasefoldEqual, left, right)
}

pub fn optional_trimmed_casefold_wildcard(
    left: Option<&str>,
    right: Option<&str>,
) -> Result<bool, MojoError> {
    profile_identity_relation(
        ProfileIdentityOperation::OptionalCasefoldWildcard,
        left,
        right,
    )
}

pub fn optional_nonempty_trimmed_casefold_equal(
    left: Option<&str>,
    right: Option<&str>,
) -> Result<bool, MojoError> {
    profile_identity_relation(
        ProfileIdentityOperation::OptionalNonemptyCasefoldEqual,
        left,
        right,
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RemovedActiveProfileChoice {
    None,
    Current,
    FirstRemaining,
}

pub fn first_present_identity_source(present: &[bool]) -> Result<Option<usize>, MojoError> {
    if present.len() > 8 {
        return Err(MojoError::InvalidInput);
    }
    let flags = present
        .iter()
        .enumerate()
        .fold(0_i64, |flags, (index, present)| {
            flags | (i64::from(*present) << index)
        });
    let result = call_kernel(
        ProfileIdentityOperation::FirstPresentSource,
        "",
        "",
        flags,
        0,
        0,
        1,
    )?;
    if result.result == -1 {
        return Ok(None);
    }
    let index = usize::try_from(result.result).map_err(|_| MojoError::InvalidOutput)?;
    (index < present.len() && present[index])
        .then_some(Some(index))
        .ok_or(MojoError::InvalidOutput)
}

pub fn removed_active_profile_choice(
    current_present: bool,
    current_removed: bool,
    remaining_present: bool,
) -> Result<RemovedActiveProfileChoice, MojoError> {
    let flags = i64::from(current_present)
        | (i64::from(current_removed) << 1)
        | (i64::from(remaining_present) << 2);
    let result = call_kernel(
        ProfileIdentityOperation::RemovedActiveChoice,
        "",
        "",
        flags,
        0,
        0,
        1,
    )?;
    match result.result {
        0 => Ok(RemovedActiveProfileChoice::None),
        1 => Ok(RemovedActiveProfileChoice::Current),
        2 => Ok(RemovedActiveProfileChoice::FirstRemaining),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn remove_profile_targets_plan(
    records: &[ProfileRemovalRecord<'_>],
    remove_all: bool,
    requested_name: Option<&str>,
    delete_home: bool,
) -> Result<RemoveProfileTargetsPlan, MojoError> {
    let raw = records
        .iter()
        .copied()
        .map(raw_removal_record)
        .collect::<Result<Vec<_>, _>>()?;
    let requested = requested_name.unwrap_or_default();
    let flags = i64::from(remove_all)
        | (i64::from(requested_name.is_some()) << 1)
        | (i64::from(delete_home) << 2);
    let output_capacity = records
        .iter()
        .try_fold(0_usize, |total, record| {
            total
                .checked_add(record.name.len())
                .and_then(|value| value.checked_add(2))
                .ok_or(MojoError::InvalidInput)
        })?
        .max(1);
    let result = call_kernel(
        ProfileIdentityOperation::RemoveTargets,
        requested,
        "",
        flags,
        raw.as_ptr() as usize as u64,
        raw.len(),
        output_capacity,
    )?;
    match result.result {
        -2 => Ok(RemoveProfileTargetsPlan::All),
        -10 => Ok(RemoveProfileTargetsPlan::MissingRequested),
        -11 => Ok(RemoveProfileTargetsPlan::NotFound),
        -12 => String::from_utf8(result.output[..result.written].to_vec())
            .map(RemoveProfileTargetsPlan::ExternalBulk)
            .map_err(|_| MojoError::InvalidOutput),
        index if index >= 0 => {
            let index = usize::try_from(index).map_err(|_| MojoError::InvalidOutput)?;
            (index < records.len())
                .then_some(RemoveProfileTargetsPlan::One(index))
                .ok_or(MojoError::InvalidOutput)
        }
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn profile_home_delete_plan(
    managed_profile: bool,
    delete_home: bool,
) -> Result<ProfileHomeDeletePlan, MojoError> {
    let flags = i64::from(managed_profile) | (i64::from(delete_home) << 1);
    let result = call_kernel(ProfileIdentityOperation::DeleteHome, "", "", flags, 0, 0, 1)?;
    match result.result {
        0 => Ok(ProfileHomeDeletePlan::Keep),
        1 => Ok(ProfileHomeDeletePlan::Delete),
        2 => Ok(ProfileHomeDeletePlan::RejectExternal),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Ask Mojo to choose the profile-management screen and row status matrix.
pub fn profile_management_status(
    active_profile_present: bool,
    inputs: &[ProfileManagementStatusInput],
) -> Result<ProfileManagementStatusPlan, MojoError> {
    let profile_count = i64::try_from(inputs.len()).map_err(|_| MojoError::InvalidInput)?;
    let mut flags = Vec::with_capacity(inputs.len().saturating_mul(3));
    for input in inputs {
        flags.extend([
            i64::from(input.active),
            i64::from(input.managed),
            i64::from(input.identity_present),
        ]);
    }
    let output_capacity = 1_usize
        .checked_add(inputs.len())
        .ok_or(MojoError::InvalidInput)?;
    let mut output = vec![-1_i64; output_capacity];
    let mut written = 0_i64;
    let status = unsafe {
        prodex_profile_management_status_v1(
            PROFILE_MANAGEMENT_STATUS_ABI_VERSION,
            i64::from(active_profile_present),
            profile_count,
            flags.as_ptr() as usize as u64,
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut written as *mut i64) as usize as u64,
        )
    };
    match status {
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        2 => return Err(MojoError::Capacity),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    if written != i64::try_from(output_capacity).map_err(|_| MojoError::InvalidOutput)? {
        return Err(MojoError::InvalidOutput);
    }
    let screen = match output[0] {
        0 => ProfileManagementScreenStatus::NoActive,
        1 => ProfileManagementScreenStatus::OnlyProfile,
        2 => ProfileManagementScreenStatus::Active,
        _ => return Err(MojoError::InvalidOutput),
    };
    let mut rows = Vec::with_capacity(inputs.len());
    for index in 0..inputs.len() {
        let row_status = match output[1 + index] {
            0 => ProfileManagementRowStatus::ActiveManagedWithIdentity,
            1 => ProfileManagementRowStatus::ActiveManagedWithoutIdentity,
            2 => ProfileManagementRowStatus::InactiveManagedWithIdentity,
            3 => ProfileManagementRowStatus::InactiveManagedWithoutIdentity,
            4 => ProfileManagementRowStatus::ActiveExternalWithIdentity,
            5 => ProfileManagementRowStatus::ActiveExternalWithoutIdentity,
            6 => ProfileManagementRowStatus::InactiveExternalWithIdentity,
            7 => ProfileManagementRowStatus::InactiveExternalWithoutIdentity,
            _ => return Err(MojoError::InvalidOutput),
        };
        rows.push(row_status);
    }
    Ok(ProfileManagementStatusPlan { screen, rows })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_identity_kernel_smoke() {
        assert_eq!(
            normalize_email(" User@Example.COM ").unwrap(),
            "user@example.com"
        );
        assert_eq!(normalize_account_id(" acct ").unwrap(), "acct");
        assert_eq!(
            profile_name_from_email(" User+Work@example.com ").unwrap(),
            "user-work_example.com"
        );
        assert_eq!(
            profile_name_candidate("", "fallback", 0).unwrap(),
            "fallback"
        );
        assert_eq!(
            profile_name_candidate("  base  ", "fallback", 1).unwrap(),
            "  base  -2"
        );
        assert_eq!(
            profile_name_candidate("base", "fallback", 10).unwrap(),
            "base-11"
        );
        assert_eq!(
            sanitize_profile_slug("  API_KEY_User@EXAMPLE.com  ").unwrap(),
            "api_key_user_example.com"
        );
        assert_eq!(
            sanitize_profile_slug("雪@EXAMPLE.com").unwrap(),
            "example.com"
        );
        assert_eq!(sanitize_profile_slug("...").unwrap(), "api_key");
        assert_eq!(sanitize_profile_slug(" A/B ").unwrap(), "a-b");
        assert_eq!(
            canonical_profile_identity_key(Some(" acct "), Some(" User@Example.COM ")).unwrap(),
            Some("account:acct|email:user@example.com".to_string())
        );
        assert_eq!(
            validate_profile_name("bad/name").unwrap(),
            ProfileNameValidation::PathSeparator
        );
        assert_eq!(
            add_profile_source_plan(false, false, true).unwrap(),
            AddProfileSourcePlan::CopyCurrent
        );
        assert!(should_activate_profile(false, false).unwrap());
        assert!(trimmed_equal(" GitHub.COM ", "GitHub.COM").unwrap());
        assert!(!trimmed_equal("GitHub.COM", "github.com").unwrap());
        assert!(trimmed_casefold_equal(" User@Example.COM ", "user@example.com").unwrap());
        assert!(optional_trimmed_casefold_equal(Some(" ACCT "), Some("acct")).unwrap());
        assert!(!optional_trimmed_casefold_equal(Some(""), None).unwrap());
        assert!(optional_trimmed_casefold_wildcard(None, Some("oauth")).unwrap());
        assert!(optional_trimmed_casefold_wildcard(Some(" OAuth "), Some("oauth")).unwrap());
        assert!(optional_nonempty_trimmed_casefold_equal(Some("  "), None).unwrap());
        assert!(!optional_nonempty_trimmed_casefold_equal(Some("arn:a"), None).unwrap());
        assert_eq!(
            first_present_identity_source(&[false, true, true]).unwrap(),
            Some(1)
        );
        assert_eq!(
            first_present_identity_source(&[false, false]).unwrap(),
            None
        );
        assert_eq!(
            removed_active_profile_choice(true, true, true).unwrap(),
            RemovedActiveProfileChoice::FirstRemaining
        );
        assert_eq!(
            removed_active_profile_choice(true, false, true).unwrap(),
            RemovedActiveProfileChoice::Current
        );
    }
}
