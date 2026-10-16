use super::prodex_runtime_profile_active_backoff_max_v1;

/// Select the latest active backoff cutoff from host-owned deadline values.
/// Mojo alone owns strict deadline activation and max selection.
pub fn profile_active_backoff_max(
    deadlines: &[i64],
    now: i64,
) -> Result<Option<i64>, crate::MojoError> {
    if deadlines.len() > 1_000_000 {
        return Err(crate::MojoError::Capacity);
    }
    let mut output = [-1_i64; 2];
    let status = unsafe {
        prodex_runtime_profile_active_backoff_max_v1(
            1,
            deadlines.as_ptr() as u64,
            i64::try_from(deadlines.len()).map_err(|_| crate::MojoError::InvalidInput)?,
            now,
            output.as_mut_ptr() as u64,
        )
    };
    if status != 0 {
        return Err(match status {
            1 => crate::MojoError::InvalidInput,
            4 => crate::MojoError::AbiMismatch,
            _ => crate::MojoError::InvalidOutput,
        });
    }
    match output[0] {
        0 if output[1] == 0 => Ok(None),
        1 if output[1] > now => Ok(Some(output[1])),
        _ => Err(crate::MojoError::InvalidOutput),
    }
}
