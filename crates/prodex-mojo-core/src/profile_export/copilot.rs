use crate::{MojoError, rich::ensure_rich_abi};

const ABI_VERSION: i64 = 1;

unsafe extern "C" {
    fn prodex_profile_export_copilot_strip_json_line_comments_v1(
        abi_version: i64,
        input_address: u64,
        input_length: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
    ) -> i64;
}

pub fn strip_copilot_json_line_comments(raw: &str) -> Result<String, MojoError> {
    ensure_rich_abi()?;
    let input_length = i64::try_from(raw.len()).map_err(|_| MojoError::InvalidInput)?;
    let mut output = vec![0_u8; raw.len().max(1)];
    let mut written = 0_i64;
    let status = unsafe {
        prodex_profile_export_copilot_strip_json_line_comments_v1(
            ABI_VERSION,
            raw.as_ptr() as usize as u64,
            input_length,
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut written as *mut i64) as usize as u64,
        )
    };
    match status {
        0 => {
            let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
            if written > raw.len() {
                return Err(MojoError::InvalidOutput);
            }
            String::from_utf8(output[..written].to_vec()).map_err(|_| MojoError::InvalidOutput)
        }
        2 => Err(MojoError::Capacity),
        99 => Err(MojoError::InvalidInput),
        100 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}
