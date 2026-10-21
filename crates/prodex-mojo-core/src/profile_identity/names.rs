use super::{MojoError, ProfileIdentityOperation, call_kernel, string_result};

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
