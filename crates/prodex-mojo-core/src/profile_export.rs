use crate::MojoError;

const ABI_VERSION: i64 = 1;

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
