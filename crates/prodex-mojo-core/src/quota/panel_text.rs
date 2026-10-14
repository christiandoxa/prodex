unsafe extern "C" {
    fn prodex_quota_panel_first_line_v1(
        abi_version: i64,
        input_address: u64,
        input_length: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
    ) -> i64;
}

pub fn quota_panel_first_line(value: &str) -> Result<String, crate::MojoError> {
    const ABI_VERSION: i64 = 1;
    let mut output = vec![0_u8; value.len().max(1)];
    let mut written = 0_i64;
    let status = unsafe {
        prodex_quota_panel_first_line_v1(
            ABI_VERSION,
            value.as_ptr() as usize as u64,
            i64::try_from(value.len()).map_err(|_| crate::MojoError::InvalidInput)?,
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output.len()).map_err(|_| crate::MojoError::InvalidInput)?,
            (&mut written as *mut i64) as usize as u64,
        )
    };
    match status {
        0 => {
            let written = usize::try_from(written).map_err(|_| crate::MojoError::InvalidOutput)?;
            if written > output.len() {
                return Err(crate::MojoError::InvalidOutput);
            }
            String::from_utf8(output[..written].to_vec())
                .map_err(|_| crate::MojoError::InvalidOutput)
        }
        1 => Err(crate::MojoError::InvalidInput),
        2 => Err(crate::MojoError::Capacity),
        4 => Err(crate::MojoError::AbiMismatch),
        _ => Err(crate::MojoError::InvalidOutput),
    }
}
