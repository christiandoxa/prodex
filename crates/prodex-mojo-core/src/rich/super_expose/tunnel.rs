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
    fn prodex_mojo_super_expose_tunnel_client_version_valid_v1(
        abi_version: i64,
        value_address: u64,
        value_length: i64,
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

pub fn super_expose_tunnel_client_version_output_valid(value: &str) -> Result<bool, MojoError> {
    ensure_rich_abi()?;
    if value.len() > 16_384 {
        return Err(MojoError::InvalidInput);
    }
    let mut output = 0_i64;
    let status = unsafe {
        prodex_mojo_super_expose_tunnel_client_version_valid_v1(
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
