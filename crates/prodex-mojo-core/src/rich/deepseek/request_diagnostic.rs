//! Provider-labelled diagnostics for compiled DeepSeek request-policy plans.

use super::DeepSeekRequestPolicyOperation;
use crate::MojoError;

unsafe extern "C" {
    fn prodex_deepseek_request_diagnostic_v1(
        abi: i64,
        operation: i64,
        tag: i64,
        provider_address: u64,
        provider_length: i64,
        detail_address: u64,
        detail_length: i64,
        output_address: u64,
        capacity: i64,
        metadata_address: u64,
        metadata_count: i64,
    ) -> i64;
}

/// Returns the Mojo-owned rejection message, or `None` for a successful plan.
/// Unsupported operations and unknown plan tags fail explicitly.
pub fn deepseek_request_diagnostic(
    operation: DeepSeekRequestPolicyOperation,
    tag: i64,
    provider_label: &str,
    detail: &str,
) -> Result<Option<String>, MojoError> {
    let capacity = provider_label
        .len()
        .checked_add(detail.len())
        .and_then(|length| length.checked_add(256))
        .ok_or(MojoError::Capacity)?;
    // Successful plans should not allocate an error-message buffer on request paths.
    let mut inline = [0_u8; 512];
    let mut overflow = (capacity > inline.len()).then(|| vec![0_u8; capacity]);
    let output = overflow.as_deref_mut().unwrap_or(&mut inline);
    let mut metadata = [-1_i64; 2];
    let status = unsafe {
        prodex_deepseek_request_diagnostic_v1(
            1,
            operation as i64,
            tag,
            provider_label.as_ptr() as usize as u64,
            i64::try_from(provider_label.len()).map_err(|_| MojoError::InvalidInput)?,
            detail.as_ptr() as usize as u64,
            i64::try_from(detail.len()).map_err(|_| MojoError::InvalidInput)?,
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output.len()).map_err(|_| MojoError::Capacity)?,
            metadata.as_mut_ptr() as usize as u64,
            metadata.len() as i64,
        )
    };
    match status {
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        3 => return Err(MojoError::Capacity),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    let written = usize::try_from(metadata[1]).map_err(|_| MojoError::InvalidOutput)?;
    if written > output.len() {
        return Err(MojoError::InvalidOutput);
    }
    match (metadata[0], written) {
        (0, 0) => Ok(None),
        (1, _) => String::from_utf8(output[..written].to_vec())
            .map(Some)
            .map_err(|_| MojoError::InvalidOutput),
        _ => Err(MojoError::InvalidOutput),
    }
}

#[cfg(test)]
#[path = "request_diagnostic_tests.rs"]
mod tests;
