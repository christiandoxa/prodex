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
