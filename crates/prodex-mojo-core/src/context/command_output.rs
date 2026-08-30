use super::{CONTEXT_TEXT_ABI_VERSION, ProdexStringView, text_abi_is_ready};

unsafe extern "C" {
    fn prodex_context_classify_dot_reporter_success_line_v1(
        abi_version: i64,
        line: *const ProdexStringView,
        output: *mut i64,
    ) -> i64;
    fn prodex_context_classify_noisy_success_line_v1(
        abi_version: i64,
        line: *const ProdexStringView,
        output: *mut i64,
    ) -> i64;
}

/// Classifies one command-output dot-progress line through the Mojo text ABI.
pub fn classify_dot_reporter_success_line(line: &str) -> Result<bool, crate::MojoError> {
    if !text_abi_is_ready() {
        return Err(crate::MojoError::AbiMismatch);
    }
    let view = ProdexStringView {
        ptr: line.as_ptr(),
        len: line.len(),
    };
    let mut output = 0_i64;
    let status = unsafe {
        prodex_context_classify_dot_reporter_success_line_v1(
            CONTEXT_TEXT_ABI_VERSION,
            &view,
            &mut output,
        )
    };
    match status {
        0 if matches!(output, 0 | 1) => Ok(output == 1),
        4 => Err(crate::MojoError::AbiMismatch),
        1 | 2 => Err(crate::MojoError::InvalidInput),
        _ => Err(crate::MojoError::InvalidOutput),
    }
}

/// Classifies one deterministic command-output success/noise label through Mojo.
pub fn classify_noisy_success_line(line: &str) -> Result<Option<i64>, crate::MojoError> {
    if !text_abi_is_ready() {
        return Err(crate::MojoError::AbiMismatch);
    }
    let view = ProdexStringView {
        ptr: line.as_ptr(),
        len: line.len(),
    };
    let mut output = -1_i64;
    let status = unsafe {
        prodex_context_classify_noisy_success_line_v1(CONTEXT_TEXT_ABI_VERSION, &view, &mut output)
    };
    match status {
        0 => match output {
            -1 | 0 => Ok(None),
            1..=72 => Ok(Some(output)),
            _ => Err(crate::MojoError::InvalidOutput),
        },
        4 => Err(crate::MojoError::AbiMismatch),
        1 | 2 => Err(crate::MojoError::InvalidInput),
        _ => Err(crate::MojoError::InvalidOutput),
    }
}

#[cfg(test)]
mod tests {
    use super::classify_dot_reporter_success_line;

    #[test]
    fn dot_reporter_adapter_matches_rust_shape() {
        for line in ["....", "...", "....x", "....🙂", ""] {
            let expected = line.len() >= 4 && line.chars().all(|ch| ch == '.');
            assert_eq!(
                classify_dot_reporter_success_line(line),
                Ok(expected),
                "{line:?}"
            );
        }
    }
}
