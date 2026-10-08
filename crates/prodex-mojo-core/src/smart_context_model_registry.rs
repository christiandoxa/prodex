use crate::MojoError;

const ABI_VERSION: i64 = 1;

unsafe extern "C" {
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
