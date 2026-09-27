use crate::MojoError;

const PROFILE_IDENTITY_ABI_VERSION: i64 = 1;
const PROFILE_IDENTITY_RECORD_EMAIL_PRESENT: i64 = 1;
const PROFILE_IDENTITY_RECORD_ACCOUNT_PRESENT: i64 = 2;
const PROFILE_PRIMARY_PRESENT: i64 = 1;
const PROFILE_SECONDARY_PRESENT: i64 = 2;

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
    }
}
