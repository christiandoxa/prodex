use crate::MojoError;

const ABI_VERSION: i64 = 1;
const MODEL_NAME_NORMALIZE_ABI_VERSION: i64 = 1;
pub const SMART_CONTEXT_MODEL_NAME_MAX_BYTES: usize = 128;

unsafe extern "C" {
    fn prodex_smart_context_model_name_normalize_v1(
        abi_version: i64,
        address: u64,
        length: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
    ) -> i64;
    fn prodex_smart_context_model_context_window_v1(
        abi_version: i64,
        address: u64,
        length: i64,
    ) -> i64;
}

pub fn context_window_tokens(model: &str) -> Result<Option<u64>, MojoError> {
    let length = i64::try_from(model.len()).map_err(|_| MojoError::InvalidInput)?;
    let value = unsafe {
        prodex_smart_context_model_context_window_v1(
            ABI_VERSION,
            model.as_ptr() as usize as u64,
            length,
        )
    };
    match value {
        0 => Ok(None),
        -2 => Err(MojoError::InvalidInput),
        -4 => Err(MojoError::AbiMismatch),
        value if value > 0 => u64::try_from(value)
            .map(Some)
            .map_err(|_| MojoError::InvalidOutput),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Normalize a Smart Context model identifier through the Mojo registry policy.
pub fn normalize_model_name(value: &str) -> Result<Option<String>, MojoError> {
    let length = i64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)?;
    let mut output = vec![0_u8; value.len().clamp(1, SMART_CONTEXT_MODEL_NAME_MAX_BYTES)];
    let mut written = 0_i64;
    let status = unsafe {
        prodex_smart_context_model_name_normalize_v1(
            MODEL_NAME_NORMALIZE_ABI_VERSION,
            value.as_ptr() as usize as u64,
            length,
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut written as *mut i64) as usize as u64,
        )
    };
    match status {
        0 => {}
        1 => return Ok(None),
        -2 => return Err(MojoError::InvalidInput),
        3 => return Err(MojoError::Capacity),
        -4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    if written > output.len() || written > SMART_CONTEXT_MODEL_NAME_MAX_BYTES {
        return Err(MojoError::InvalidOutput);
    }
    String::from_utf8(output[..written].to_vec())
        .map(Some)
        .map_err(|_| MojoError::InvalidOutput)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_context_window_kernel_preserves_known_and_unknown_shapes() {
        assert_eq!(
            context_window_tokens("GPT-5.1-Codex").unwrap(),
            Some(200_000)
        );
        assert_eq!(
            context_window_tokens("gemini-custom").unwrap(),
            Some(1_048_576)
        );
        assert_eq!(context_window_tokens("local-test").unwrap(), None);
    }
}
