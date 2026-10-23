use super::super::{ensure_rich_abi, mojo_mut_pointer_address};
use super::{SUPER_EXPOSE_ABI_VERSION, SUPER_EXPOSE_MAX_NAME_BYTES};
use crate::MojoError;

unsafe extern "C" {
    fn prodex_mojo_super_expose_tunnel_id_valid_v1(
        abi_version: i64,
        value_address: u64,
        value_length: i64,
        output_address: u64,
    ) -> i64;
    fn prodex_super_expose_tunnel_supported_version_v1(
        abi_version: i64,
        value_address: u64,
        value_length: i64,
        minimum_address: u64,
        minimum_length: i64,
        output_address: u64,
    ) -> i64;
}

pub fn super_expose_tunnel_id_valid(value: &str) -> Result<bool, MojoError> {
    ensure_rich_abi()?;
    if value.len() > SUPER_EXPOSE_MAX_NAME_BYTES {
        return Err(MojoError::InvalidInput);
    }
    let mut output = 0_i64;
    let status = unsafe {
        prodex_mojo_super_expose_tunnel_id_valid_v1(
            SUPER_EXPOSE_ABI_VERSION,
            value.as_ptr() as u64,
            i64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)?,
            mojo_mut_pointer_address(&mut output),
        )
    };
    if status != 0 {
        return Err(match status {
            1 | 2 => MojoError::InvalidInput,
            4 => MojoError::AbiMismatch,
            _ => MojoError::InvalidOutput,
        });
    }
    match output {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Select the first compatible official stable version using compiled policy.
/// The returned version borrows its original canonical bytes from the probe text.
/// Inputs are limited to 16 KiB of probe text and a 64-byte stable minimum.
pub fn super_expose_tunnel_supported_version<'a>(
    value: &'a str,
    minimum: &str,
) -> Result<Option<&'a str>, MojoError> {
    ensure_rich_abi()?;
    if value.len() > 16_384 || minimum.len() > 64 {
        return Err(MojoError::InvalidInput);
    }
    let mut output = [-1_i64; 3];
    let status = unsafe {
        prodex_super_expose_tunnel_supported_version_v1(
            SUPER_EXPOSE_ABI_VERSION,
            value.as_ptr() as u64,
            i64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)?,
            minimum.as_ptr() as u64,
            i64::try_from(minimum.len()).map_err(|_| MojoError::InvalidInput)?,
            output.as_mut_ptr() as u64,
        )
    };
    if status != 0 {
        return Err(match status {
            1 | 2 => MojoError::InvalidInput,
            4 => MojoError::AbiMismatch,
            _ => MojoError::InvalidOutput,
        });
    }
    match output {
        [0, 0, 0] => Ok(None),
        [1, start, end] => {
            let start = usize::try_from(start).map_err(|_| MojoError::InvalidOutput)?;
            let end = usize::try_from(end).map_err(|_| MojoError::InvalidOutput)?;
            value
                .get(start..end)
                .filter(|version| !version.is_empty())
                .map(Some)
                .ok_or(MojoError::InvalidOutput)
        }
        _ => Err(MojoError::InvalidOutput),
    }
}

#[cfg(test)]
#[path = "tunnel/tests.rs"]
mod tests;
