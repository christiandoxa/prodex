use super::{signed, status};
use crate::{MojoError, text_output::render_measured_string};

/// Validate the shared [present, length] metadata of native text kernels.
/// Their input-specific call sites retain ownership and the standard JSON status codes.
pub(crate) fn render_native_text(
    mut invoke: impl FnMut(i64, u64, i64, u64) -> i64,
) -> Result<String, MojoError> {
    render_measured_string(|output| {
        let (phase, address, capacity) = match output {
            None => (1, 0, 0),
            Some(output) => (0, output.as_mut_ptr() as u64, signed(output.len())?),
        };
        let mut metadata = [-1_i64; 2];
        status(invoke(
            phase,
            address,
            capacity,
            metadata.as_mut_ptr() as u64,
        ))?;
        if metadata[0] != 1 {
            return Err(MojoError::InvalidOutput);
        }
        usize::try_from(metadata[1]).map_err(|_| MojoError::InvalidOutput)
    })
}

#[cfg(test)]
#[path = "native_text_tests.rs"]
mod tests;
