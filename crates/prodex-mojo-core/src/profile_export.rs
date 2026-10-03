use crate::{MojoError, rich::ensure_rich_abi};

const ABI_VERSION: i64 = 1;
const PROFILE_IMPORT_PLAN_ABI_VERSION: i64 = 1;
const PROFILE_IMPORT_PLAN_OUTPUT_STRIDE: usize = 5;
const PROFILE_IMPORT_PLAN_SCRATCH_STRIDE: usize = 3;

#[repr(i64)]
#[derive(Clone, Copy)]
enum ProfileExportPolicyMode {
    Collection = 0,
    ProfileSecretFiles = 1,
    NestedSecretBytes = 2,
    PasswordBytes = 3,
    Pbkdf2Iterations = 4,
    Argon2 = 5,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileExportPolicyViolation {
    ProfileCount,
    SecretFileCount,
    ProfileSecretFileCount,
    NestedSecretSize,
    PasswordSize,
    Pbkdf2Iterations,
    Argon2Version,
    Argon2Memory,
    Argon2Iterations,
    Argon2Parallelism,
}

unsafe extern "C" {
    fn prodex_profile_export_password_plan_v1(
        abi_version: i64,
        operation: i64,
        input0: i64,
        input1: i64,
        input2: i64,
    ) -> i64;
    fn prodex_profile_import_auth_update_plan_v1(
        abi_version: i64,
        existing_update_present: i64,
        incoming_email_present: i64,
    ) -> i64;
    fn prodex_profile_export_policy_v1(
        abi_version: i64,
        mode: i64,
        input0: u64,
        input1: u64,
        input2: u64,
        input3: u64,
    ) -> i64;
    fn prodex_profile_import_plan_v1(
        abi_version: i64,
        profile_count: i64,
        names_address: u64,
        identity_keys_address: u64,
        lookup_names_address: u64,
        flags_address: u64,
        scratch_address: u64,
        scratch_capacity: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
    ) -> i64;
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct ProfileImportStringView {
    ptr: u64,
    len: u64,
}

impl ProfileImportStringView {
    fn from(value: Option<&str>) -> Result<Self, MojoError> {
        let Some(value) = value else {
            return Ok(Self::default());
        };
        Ok(Self {
            ptr: u64::try_from(value.as_ptr() as usize).map_err(|_| MojoError::InvalidInput)?,
            len: u64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)?,
        })
    }
}

/// Lookup state supplied by the filesystem or profile-state adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileImportIdentityLookup<'a> {
    NotRequested,
    Pending,
    Missing,
    Found(&'a str),
}

/// One profile record consumed by the Mojo-owned import planner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProfileImportPlanInput<'a> {
    pub profile_name: &'a str,
    pub identity_key: Option<&'a str>,
    pub supports_codex_runtime: bool,
    pub existing_profile_supports_codex_runtime: Option<bool>,
    pub identity_lookup: ProfileImportIdentityLookup<'a>,
}

/// Target reference returned by the profile import planner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileImportPlanTarget {
    SourceProfile(usize),
    ExistingProfileLookup(usize),
}

/// Action selected by the Mojo-owned profile import planner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileImportPlanAction {
    UpdateExisting {
        source_index: usize,
        target: ProfileImportPlanTarget,
    },
    StageNew {
        source_index: usize,
        staged_index: usize,
    },
    RewriteStagedAuth {
        source_index: usize,
        staged_index: usize,
        target_source_index: usize,
    },
}

/// Next deterministic result from the profile import planner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProfileImportPlanStep {
    Empty,
    DuplicateName(usize),
    ProviderMismatch(usize),
    LookupIdentity(usize),
    Complete(Vec<ProfileImportPlanAction>),
}

fn profile_import_lookup_parts(lookup: ProfileImportIdentityLookup<'_>) -> (i64, Option<&str>) {
    match lookup {
        ProfileImportIdentityLookup::NotRequested => (0, None),
        ProfileImportIdentityLookup::Pending => (1, None),
        ProfileImportIdentityLookup::Missing => (2, None),
        ProfileImportIdentityLookup::Found(name) => (3, Some(name)),
    }
}

/// Ask Mojo to advance the deterministic import plan using caller-owned lookup results.
pub fn profile_import_plan_step(
    inputs: &[ProfileImportPlanInput<'_>],
) -> Result<ProfileImportPlanStep, MojoError> {
    ensure_rich_abi()?;

    let names = inputs
        .iter()
        .map(|input| ProfileImportStringView::from(Some(input.profile_name)))
        .collect::<Result<Vec<_>, _>>()?;
    let identity_keys = inputs
        .iter()
        .map(|input| ProfileImportStringView::from(input.identity_key))
        .collect::<Result<Vec<_>, _>>()?;
    let lookup_names = inputs
        .iter()
        .map(|input| {
            ProfileImportStringView::from(profile_import_lookup_parts(input.identity_lookup).1)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut flags = Vec::with_capacity(inputs.len() * 3);
    for input in inputs {
        let (lookup_status, _) = profile_import_lookup_parts(input.identity_lookup);
        flags.extend([
            i64::from(input.supports_codex_runtime),
            input
                .existing_profile_supports_codex_runtime
                .map(i64::from)
                .unwrap_or(-1),
            lookup_status,
        ]);
    }

    let scratch_capacity = inputs
        .len()
        .checked_mul(PROFILE_IMPORT_PLAN_SCRATCH_STRIDE)
        .ok_or(MojoError::InvalidInput)?;
    let output_capacity = inputs
        .len()
        .checked_mul(PROFILE_IMPORT_PLAN_OUTPUT_STRIDE)
        .ok_or(MojoError::InvalidInput)?;
    let mut scratch = vec![0_i64; scratch_capacity.max(1)];
    let mut output = vec![-1_i64; output_capacity.max(1)];
    let mut written = 0_i64;
    let status = unsafe {
        prodex_profile_import_plan_v1(
            PROFILE_IMPORT_PLAN_ABI_VERSION,
            i64::try_from(inputs.len()).map_err(|_| MojoError::InvalidInput)?,
            names.as_ptr() as usize as u64,
            identity_keys.as_ptr() as usize as u64,
            lookup_names.as_ptr() as usize as u64,
            flags.as_ptr() as usize as u64,
            scratch.as_mut_ptr() as usize as u64,
            i64::try_from(scratch_capacity).map_err(|_| MojoError::InvalidInput)?,
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output_capacity).map_err(|_| MojoError::InvalidInput)?,
            (&mut written as *mut i64) as usize as u64,
        )
    };
    match status {
        5 if inputs.is_empty() && written == 0 => return Ok(ProfileImportPlanStep::Empty),
        6 => {
            return Ok(ProfileImportPlanStep::DuplicateName(
                profile_import_status_index(&output, written, inputs.len())?,
            ));
        }
        7 => {
            return Ok(ProfileImportPlanStep::ProviderMismatch(
                profile_import_status_index(&output, written, inputs.len())?,
            ));
        }
        8 => {
            let index = profile_import_status_index(&output, written, inputs.len())?;
            if !matches!(
                inputs[index].identity_lookup,
                ProfileImportIdentityLookup::Pending
            ) {
                return Err(MojoError::InvalidOutput);
            }
            return Ok(ProfileImportPlanStep::LookupIdentity(index));
        }
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        2 => return Err(MojoError::Capacity),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }

    let expected_written = output_capacity;
    if written != i64::try_from(expected_written).map_err(|_| MojoError::InvalidOutput)? {
        return Err(MojoError::InvalidOutput);
    }
    let mut actions = Vec::with_capacity(inputs.len());
    let mut staged_sources = Vec::with_capacity(inputs.len());
    for source_index in 0..inputs.len() {
        let offset = source_index
            .checked_mul(PROFILE_IMPORT_PLAN_OUTPUT_STRIDE)
            .ok_or(MojoError::InvalidOutput)?;
        let source = usize::try_from(*output.get(offset).ok_or(MojoError::InvalidOutput)?)
            .map_err(|_| MojoError::InvalidOutput)?;
        let action = *output.get(offset + 1).ok_or(MojoError::InvalidOutput)?;
        let staged = *output.get(offset + 2).ok_or(MojoError::InvalidOutput)?;
        let target_kind = *output.get(offset + 3).ok_or(MojoError::InvalidOutput)?;
        let target_index =
            usize::try_from(*output.get(offset + 4).ok_or(MojoError::InvalidOutput)?)
                .map_err(|_| MojoError::InvalidOutput)?;
        if source != source_index || target_index >= inputs.len() {
            return Err(MojoError::InvalidOutput);
        }
        actions.push(match action {
            0 if staged == -1 && target_kind == 0 => ProfileImportPlanAction::UpdateExisting {
                source_index,
                target: ProfileImportPlanTarget::SourceProfile(target_index),
            },
            0 if staged == -1 && target_kind == 1 => {
                if !matches!(
                    inputs[target_index].identity_lookup,
                    ProfileImportIdentityLookup::Found(_)
                ) {
                    return Err(MojoError::InvalidOutput);
                }
                ProfileImportPlanAction::UpdateExisting {
                    source_index,
                    target: ProfileImportPlanTarget::ExistingProfileLookup(target_index),
                }
            }
            1 if target_kind == 0 && target_index == source_index => {
                let staged_index = usize::try_from(staged).map_err(|_| MojoError::InvalidOutput)?;
                if staged_index != staged_sources.len() {
                    return Err(MojoError::InvalidOutput);
                }
                staged_sources.push(source_index);
                ProfileImportPlanAction::StageNew {
                    source_index,
                    staged_index,
                }
            }
            2 if target_kind == 0 => {
                let staged_index = usize::try_from(staged).map_err(|_| MojoError::InvalidOutput)?;
                let target_source_index = *staged_sources
                    .get(staged_index)
                    .ok_or(MojoError::InvalidOutput)?;
                if target_index != target_source_index {
                    return Err(MojoError::InvalidOutput);
                }
                ProfileImportPlanAction::RewriteStagedAuth {
                    source_index,
                    staged_index,
                    target_source_index,
                }
            }
            _ => return Err(MojoError::InvalidOutput),
        });
    }
    Ok(ProfileImportPlanStep::Complete(actions))
}

fn profile_import_status_index(
    output: &[i64],
    written: i64,
    input_count: usize,
) -> Result<usize, MojoError> {
    if written != 1 {
        return Err(MojoError::InvalidOutput);
    }
    let index = usize::try_from(*output.first().ok_or(MojoError::InvalidOutput)?)
        .map_err(|_| MojoError::InvalidOutput)?;
    (index < input_count)
        .then_some(index)
        .ok_or(MojoError::InvalidOutput)
}

/// Return the first duplicate profile-name index, if one exists.
pub fn profile_import_duplicate_name_index(names: &[&str]) -> Result<Option<usize>, MojoError> {
    let inputs = names
        .iter()
        .map(|profile_name| ProfileImportPlanInput {
            profile_name,
            identity_key: None,
            supports_codex_runtime: false,
            existing_profile_supports_codex_runtime: None,
            identity_lookup: ProfileImportIdentityLookup::NotRequested,
        })
        .collect::<Vec<_>>();
    match profile_import_plan_step(&inputs)? {
        ProfileImportPlanStep::Empty | ProfileImportPlanStep::Complete(_) => Ok(None),
        ProfileImportPlanStep::DuplicateName(index) => Ok(Some(index)),
        _ => Err(MojoError::InvalidOutput),
    }
}

fn call(
    mode: ProfileExportPolicyMode,
    input0: u64,
    input1: u64,
    input2: u64,
    input3: u64,
) -> Result<(), ProfileExportPolicyViolation> {
    let code = unsafe {
        prodex_profile_export_policy_v1(ABI_VERSION, mode as i64, input0, input1, input2, input3)
    };
    match code {
        0 => Ok(()),
        1 => Err(ProfileExportPolicyViolation::ProfileCount),
        2 => Err(ProfileExportPolicyViolation::SecretFileCount),
        3 => Err(ProfileExportPolicyViolation::ProfileSecretFileCount),
        4 => Err(ProfileExportPolicyViolation::NestedSecretSize),
        5 => Err(ProfileExportPolicyViolation::PasswordSize),
        6 => Err(ProfileExportPolicyViolation::Pbkdf2Iterations),
        7 => Err(ProfileExportPolicyViolation::Argon2Version),
        8 => Err(ProfileExportPolicyViolation::Argon2Memory),
        9 => Err(ProfileExportPolicyViolation::Argon2Iterations),
        10 => Err(ProfileExportPolicyViolation::Argon2Parallelism),
        _ => panic!("Mojo profile-export policy returned invalid code {code}"),
    }
}

fn usize_u64(value: usize) -> Result<u64, MojoError> {
    u64::try_from(value).map_err(|_| MojoError::InvalidInput)
}

pub fn validate_collection(
    profile_count: usize,
    secret_file_count: usize,
) -> Result<(), ProfileExportPolicyViolation> {
    call(
        ProfileExportPolicyMode::Collection,
        usize_u64(profile_count).expect("usize fits profile-export ABI"),
        usize_u64(secret_file_count).expect("usize fits profile-export ABI"),
        0,
        0,
    )
}

pub fn validate_profile_secret_files(count: usize) -> Result<(), ProfileExportPolicyViolation> {
    call(
        ProfileExportPolicyMode::ProfileSecretFiles,
        usize_u64(count).expect("usize fits profile-export ABI"),
        0,
        0,
        0,
    )
}

pub fn validate_nested_secret_bytes(bytes: usize) -> Result<(), ProfileExportPolicyViolation> {
    call(
        ProfileExportPolicyMode::NestedSecretBytes,
        usize_u64(bytes).expect("usize fits profile-export ABI"),
        0,
        0,
        0,
    )
}

pub fn validate_password_bytes(bytes: usize) -> Result<(), ProfileExportPolicyViolation> {
    call(
        ProfileExportPolicyMode::PasswordBytes,
        usize_u64(bytes).expect("usize fits profile-export ABI"),
        0,
        0,
        0,
    )
}

pub fn validate_pbkdf2_iterations(iterations: u32) -> Result<(), ProfileExportPolicyViolation> {
    call(
        ProfileExportPolicyMode::Pbkdf2Iterations,
        u64::from(iterations),
        0,
        0,
        0,
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileExportPasswordAction {
    Protect,
    Unprotected,
    Prompt,
    Environment,
    NonInteractiveError,
    Valid,
    Empty,
    Mismatch,
}

fn profile_password_plan(
    operation: i64,
    input0: bool,
    input1: bool,
    input2: bool,
) -> Result<ProfileExportPasswordAction, MojoError> {
    let result = unsafe {
        prodex_profile_export_password_plan_v1(
            ABI_VERSION,
            operation,
            i64::from(input0),
            i64::from(input1),
            i64::from(input2),
        )
    };
    match result {
        0 => Ok(ProfileExportPasswordAction::Protect),
        1 => Ok(ProfileExportPasswordAction::Unprotected),
        2 => Ok(ProfileExportPasswordAction::Prompt),
        3 => Ok(ProfileExportPasswordAction::Environment),
        4 => Ok(ProfileExportPasswordAction::NonInteractiveError),
        5 => Ok(ProfileExportPasswordAction::Valid),
        6 => Ok(ProfileExportPasswordAction::Empty),
        7 => Ok(ProfileExportPasswordAction::Mismatch),
        -1 => Err(MojoError::InvalidInput),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn profile_export_password_mode_action(
    password_protect: bool,
    no_password: bool,
    interactive: bool,
) -> Result<ProfileExportPasswordAction, MojoError> {
    profile_password_plan(1, password_protect, no_password, interactive)
}

pub fn profile_password_source_action(
    env_nonempty: bool,
    interactive: bool,
) -> Result<ProfileExportPasswordAction, MojoError> {
    profile_password_plan(2, env_nonempty, interactive, false)
}

pub fn profile_export_password_validation(
    empty: bool,
    matches: bool,
) -> Result<ProfileExportPasswordAction, MojoError> {
    profile_password_plan(3, empty, matches, false)
}

pub fn profile_import_password_validation(
    empty: bool,
) -> Result<ProfileExportPasswordAction, MojoError> {
    profile_password_plan(4, empty, false, false)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileImportAuthUpdateAction {
    Append,
    ReplaceAuth,
    ReplaceAuthAndEmail,
}

pub fn profile_import_auth_update_action(
    existing_update_present: bool,
    incoming_email_present: bool,
) -> Result<ProfileImportAuthUpdateAction, MojoError> {
    let result = unsafe {
        prodex_profile_import_auth_update_plan_v1(
            ABI_VERSION,
            i64::from(existing_update_present),
            i64::from(incoming_email_present),
        )
    };
    match result {
        0 => Ok(ProfileImportAuthUpdateAction::Append),
        1 => Ok(ProfileImportAuthUpdateAction::ReplaceAuth),
        2 => Ok(ProfileImportAuthUpdateAction::ReplaceAuthAndEmail),
        -1 => Err(MojoError::InvalidInput),
        -4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn validate_argon2(
    version: u32,
    memory_kib: u32,
    iterations: u32,
    parallelism: u32,
) -> Result<(), ProfileExportPolicyViolation> {
    call(
        ProfileExportPolicyMode::Argon2,
        u64::from(version),
        u64::from(memory_kib),
        u64::from(iterations),
        u64::from(parallelism),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_export_policy_kernel_smoke() {
        assert!(validate_collection(256, 4096).is_ok());
        assert_eq!(
            validate_collection(257, 0),
            Err(ProfileExportPolicyViolation::ProfileCount)
        );
        assert_eq!(
            validate_profile_secret_files(17),
            Err(ProfileExportPolicyViolation::ProfileSecretFileCount)
        );
        assert_eq!(
            validate_password_bytes(0),
            Err(ProfileExportPolicyViolation::PasswordSize)
        );
        assert!(validate_pbkdf2_iterations(50_000).is_ok());
        assert_eq!(validate_argon2(0x13, 8 * 1024, 1, 1), Ok(()));
        assert_eq!(
            profile_export_password_mode_action(false, false, false).unwrap(),
            ProfileExportPasswordAction::NonInteractiveError
        );
        assert_eq!(
            profile_export_password_mode_action(true, true, false).unwrap(),
            ProfileExportPasswordAction::Protect
        );
        assert_eq!(
            profile_password_source_action(true, false).unwrap(),
            ProfileExportPasswordAction::Environment
        );
        assert_eq!(
            profile_export_password_validation(false, false).unwrap(),
            ProfileExportPasswordAction::Mismatch
        );
        assert_eq!(
            profile_import_password_validation(true).unwrap(),
            ProfileExportPasswordAction::Empty
        );
        assert_eq!(
            profile_import_auth_update_action(false, false).unwrap(),
            ProfileImportAuthUpdateAction::Append
        );
        assert_eq!(
            profile_import_auth_update_action(true, false).unwrap(),
            ProfileImportAuthUpdateAction::ReplaceAuth
        );
        assert_eq!(
            profile_import_auth_update_action(true, true).unwrap(),
            ProfileImportAuthUpdateAction::ReplaceAuthAndEmail
        );
    }
}
