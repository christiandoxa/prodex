use crate::MojoError;

const SECRET_POLICY_ABI_VERSION: i64 = 1;

unsafe extern "C" {
    fn prodex_mojo_secret_reference_valid_v1(
        abi_version: i64,
        provider_address: u64,
        provider_length: i64,
        name_address: u64,
        name_length: i64,
        version_address: u64,
        version_length: i64,
        version_present: i64,
        result_address: u64,
    ) -> i64;
    fn prodex_mojo_secret_rotation_policy_validate_v1(
        abi_version: i64,
        max_age_seconds: u64,
        overlap_seconds: u64,
        decision_address: u64,
    ) -> i64;
}

/// Mojo's validation result for a secret rotation policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretRotationPolicyDecision {
    /// Maximum age is nonzero and overlap is shorter than maximum age.
    Valid,
    /// Maximum age is zero.
    ZeroMaxAge,
    /// Overlap is at least as long as maximum age.
    OverlapNotShorterThanMaxAge,
}

fn view(value: &str) -> Result<(u64, i64), MojoError> {
    Ok((
        u64::try_from(value.as_ptr() as usize).map_err(|_| MojoError::InvalidInput)?,
        i64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)?,
    ))
}

/// Validate all components of a secret reference in the Mojo policy kernel.
pub fn secret_reference_is_well_formed(
    provider: &str,
    name: &str,
    version: Option<&str>,
) -> Result<bool, MojoError> {
    let (provider_address, provider_length) = view(provider)?;
    let (name_address, name_length) = view(name)?;
    let (version_address, version_length) = version.map(view).transpose()?.unwrap_or_default();
    let mut result = -1_i64;
    let status = unsafe {
        prodex_mojo_secret_reference_valid_v1(
            SECRET_POLICY_ABI_VERSION,
            provider_address,
            provider_length,
            name_address,
            name_length,
            version_address,
            version_length,
            i64::from(version.is_some()),
            (&mut result as *mut i64) as usize as u64,
        )
    };
    match status {
        0 => match result {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(MojoError::InvalidOutput),
        },
        1 => Err(MojoError::InvalidInput),
        4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Validate secret rotation bounds through the Mojo secret-policy kernel.
pub fn secret_rotation_policy_decision(
    max_age_seconds: u64,
    overlap_seconds: u64,
) -> Result<SecretRotationPolicyDecision, MojoError> {
    let mut decision = -1_i64;
    let status = unsafe {
        prodex_mojo_secret_rotation_policy_validate_v1(
            SECRET_POLICY_ABI_VERSION,
            max_age_seconds,
            overlap_seconds,
            (&mut decision as *mut i64) as usize as u64,
        )
    };
    match status {
        0 => match decision {
            0 => Ok(SecretRotationPolicyDecision::Valid),
            1 => Ok(SecretRotationPolicyDecision::ZeroMaxAge),
            2 => Ok(SecretRotationPolicyDecision::OverlapNotShorterThanMaxAge),
            _ => Err(MojoError::InvalidOutput),
        },
        1 => Err(MojoError::InvalidInput),
        4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}
