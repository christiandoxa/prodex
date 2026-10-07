use crate::MojoError;

unsafe extern "C" {
    fn prodex_mojo_log_event_name_v1(
        abi_version: i64,
        event_address: u64,
        event_length: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
    ) -> i64;
}

const LOG_EVENT_NAME_ABI_VERSION: i64 = 1;

pub fn render_log_event_name(event: &str) -> Result<String, MojoError> {
    let capacity = event
        .len()
        .checked_add(64)
        .ok_or(MojoError::InvalidInput)?
        .max(1);
    let mut output = vec![0_u8; capacity];
    let mut written = -1_i64;
    let status = unsafe {
        prodex_mojo_log_event_name_v1(
            LOG_EVENT_NAME_ABI_VERSION,
            event.as_ptr() as usize as u64,
            i64::try_from(event.len()).map_err(|_| MojoError::InvalidInput)?,
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut written as *mut i64) as usize as u64,
        )
    };
    match status {
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        2 => return Err(MojoError::Capacity),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    if written > output.len() {
        return Err(MojoError::InvalidOutput);
    }
    String::from_utf8(output[..written].to_vec()).map_err(|_| MojoError::InvalidOutput)
}
