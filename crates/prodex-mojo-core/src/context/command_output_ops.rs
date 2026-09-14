use super::{
    CONTEXT_GIT_SEARCH_MAX_BYTES, CONTEXT_TEXT_ABI_VERSION, ProdexStringView, text_abi_is_ready,
};

unsafe extern "C" {
    fn prodex_context_normalize_command_output_v1(
        abi_version: i64,
        input: *const ProdexStringView,
        output: *mut u8,
        output_capacity: i64,
        written: *mut i64,
    ) -> i64;
    fn prodex_context_classify_command_output_kind_v1(
        abi_version: i64,
        input: *const ProdexStringView,
        hint: i64,
        output_kind: *mut i64,
    ) -> i64;
    fn prodex_context_truncate_command_output_v1(
        abi_version: i64,
        input: *const ProdexStringView,
        output: *mut u8,
        output_capacity: i64,
        max_lines: i64,
        head_lines: i64,
        tail_lines: i64,
        max_line_chars: i64,
        written: *mut i64,
    ) -> i64;
}

pub fn normalize_command_output(input: &str) -> Result<String, crate::MojoError> {
    if input.len() > CONTEXT_GIT_SEARCH_MAX_BYTES {
        return Err(crate::MojoError::InvalidInput);
    }
    if !text_abi_is_ready() {
        return Err(crate::MojoError::AbiMismatch);
    }
    let view = ProdexStringView {
        ptr: input.as_ptr(),
        len: input.len(),
    };
    let mut output = vec![0_u8; input.len()];
    let mut written = 0_i64;
    let status = unsafe {
        prodex_context_normalize_command_output_v1(
            CONTEXT_TEXT_ABI_VERSION,
            &view,
            output.as_mut_ptr(),
            i64::try_from(output.len()).map_err(|_| crate::MojoError::InvalidInput)?,
            &mut written,
        )
    };
    if status != 0 {
        return Err(match status {
            1 => crate::MojoError::InvalidInput,
            3 => crate::MojoError::Capacity,
            _ => crate::MojoError::InvalidOutput,
        });
    }
    output.truncate(usize::try_from(written).map_err(|_| crate::MojoError::InvalidOutput)?);
    String::from_utf8(output).map_err(|_| crate::MojoError::InvalidOutput)
}

pub fn classify_command_output_kind(
    input: &str,
    hint: Option<i64>,
) -> Result<Option<i64>, crate::MojoError> {
    if input.len() > CONTEXT_GIT_SEARCH_MAX_BYTES || !text_abi_is_ready() {
        return Err(crate::MojoError::InvalidInput);
    }
    let view = ProdexStringView {
        ptr: input.as_ptr(),
        len: input.len(),
    };
    let mut output_kind = 0_i64;
    let status = unsafe {
        prodex_context_classify_command_output_kind_v1(
            CONTEXT_TEXT_ABI_VERSION,
            &view,
            hint.unwrap_or_default(),
            &mut output_kind,
        )
    };
    if status != 0 {
        return Err(match status {
            4 => crate::MojoError::AbiMismatch,
            1 => crate::MojoError::InvalidInput,
            _ => crate::MojoError::InvalidOutput,
        });
    }
    (output_kind == 0 || (1..=9).contains(&output_kind))
        .then_some((output_kind != 0).then_some(output_kind))
        .ok_or(crate::MojoError::InvalidOutput)
}

pub fn truncate_command_output(
    input: &str,
    max_lines: usize,
    head_lines: usize,
    tail_lines: usize,
    max_line_chars: usize,
) -> Result<String, crate::MojoError> {
    if !text_abi_is_ready() || input.len() > CONTEXT_GIT_SEARCH_MAX_BYTES || max_lines == 0 {
        return Err(crate::MojoError::InvalidInput);
    }
    let view = ProdexStringView {
        ptr: input.as_ptr(),
        len: input.len(),
    };
    let capacity = input
        .len()
        .checked_add(4096)
        .ok_or(crate::MojoError::InvalidInput)?;
    let mut output = vec![0_u8; capacity];
    let mut written = 0_i64;
    let status = unsafe {
        prodex_context_truncate_command_output_v1(
            CONTEXT_TEXT_ABI_VERSION,
            &view,
            output.as_mut_ptr(),
            i64::try_from(output.len()).map_err(|_| crate::MojoError::InvalidInput)?,
            i64::try_from(max_lines.min(i64::MAX as usize))
                .map_err(|_| crate::MojoError::InvalidInput)?,
            i64::try_from(head_lines.min(i64::MAX as usize))
                .map_err(|_| crate::MojoError::InvalidInput)?,
            i64::try_from(tail_lines.min(i64::MAX as usize))
                .map_err(|_| crate::MojoError::InvalidInput)?,
            i64::try_from(max_line_chars.min(i64::MAX as usize))
                .map_err(|_| crate::MojoError::InvalidInput)?,
            &mut written,
        )
    };
    if status != 0 {
        return Err(match status {
            4 => crate::MojoError::AbiMismatch,
            1 => crate::MojoError::InvalidInput,
            3 => crate::MojoError::Capacity,
            _ => crate::MojoError::InvalidOutput,
        });
    }
    output.truncate(usize::try_from(written).map_err(|_| crate::MojoError::InvalidOutput)?);
    String::from_utf8(output).map_err(|_| crate::MojoError::InvalidOutput)
}
