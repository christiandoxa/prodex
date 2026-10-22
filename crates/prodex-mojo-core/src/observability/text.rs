use crate::MojoError;

/// Shared caller-owned allocation and status validation for observability text ABIs.
pub(super) fn render_measured_text(
    mut invoke: impl FnMut(i64, u64, i64, u64) -> i64,
) -> Result<String, MojoError> {
    let mut measured = -1_i64;
    status(invoke(1, 0, 0, &mut measured as *mut i64 as u64))?;
    let capacity = usize::try_from(measured).map_err(|_| MojoError::InvalidOutput)?;
    if capacity > 67_108_864 {
        return Err(MojoError::InvalidOutput);
    }
    let mut output = vec![0_u8; capacity];
    let mut written = -1_i64;
    status(invoke(
        0,
        output.as_mut_ptr() as u64,
        capacity as i64,
        &mut written as *mut i64 as u64,
    ))?;
    if written != measured {
        return Err(MojoError::InvalidOutput);
    }
    String::from_utf8(output).map_err(|_| MojoError::InvalidOutput)
}

fn status(code: i64) -> Result<(), MojoError> {
    match code {
        0 => Ok(()),
        1 => Err(MojoError::InvalidInput),
        2 => Err(MojoError::Capacity),
        4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measured_text_rejects_bad_lengths_status_and_utf8_without_recomputation() {
        assert_eq!(
            render_measured_text(|_, _, _, _| 4),
            Err(MojoError::AbiMismatch)
        );
        assert_eq!(
            render_measured_text(|_, _, _, _| 0),
            Err(MojoError::InvalidOutput)
        );
        assert_eq!(
            render_measured_text(|phase, _, _, written| unsafe {
                *(written as *mut i64) = if phase == 1 { 1 } else { 0 };
                0
            }),
            Err(MojoError::InvalidOutput)
        );
        assert_eq!(
            render_measured_text(|phase, output, _, written| unsafe {
                *(written as *mut i64) = 1;
                if phase == 0 {
                    *(output as *mut u8) = 0xff;
                }
                0
            }),
            Err(MojoError::InvalidOutput)
        );
    }
}
