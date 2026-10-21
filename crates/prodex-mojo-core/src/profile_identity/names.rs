use super::{
    MojoError, PROFILE_PRIMARY_PRESENT, ProfileIdentityOperation, call_kernel, optional_flag,
    string_result,
};

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

pub fn default_api_key_profile_name(host: Option<&str>) -> Result<String, MojoError> {
    let host_value = host.unwrap_or_default();
    let capacity = host_value
        .len()
        .checked_add("api_key_".len())
        .ok_or(MojoError::InvalidInput)?
        .max("api_key".len());
    let result = call_kernel(
        ProfileIdentityOperation::DefaultApiKeyName,
        host_value,
        "",
        optional_flag(host, PROFILE_PRIMARY_PRESENT),
        0,
        0,
        capacity,
    )?;
    String::from_utf8(result.output[..result.written].to_vec())
        .map_err(|_| MojoError::InvalidOutput)
}
