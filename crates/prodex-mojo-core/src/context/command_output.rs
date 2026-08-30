use super::{
    CONTEXT_GIT_SEARCH_MAX_BYTES, CONTEXT_TEXT_ABI_VERSION, ProdexStringView, text_abi_is_ready,
};

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CommandOutputLineClassification {
    pub flags: i64,
    pub noisy_label: i64,
    pub diagnostic_label: i64,
}

const _: () = assert!(
    std::mem::size_of::<CommandOutputLineClassification>() == 3 * std::mem::size_of::<i64>()
);

unsafe extern "C" {
    fn prodex_context_classify_command_output_line_v1(
        abi_version: i64,
        line: *const ProdexStringView,
        result: *mut CommandOutputLineClassification,
    ) -> i64;
}

pub fn classify_command_output_line(
    line: &str,
) -> Result<CommandOutputLineClassification, crate::MojoError> {
    if line.len() > CONTEXT_GIT_SEARCH_MAX_BYTES {
        return Err(crate::MojoError::InvalidInput);
    }
    if !text_abi_is_ready() {
        return Err(crate::MojoError::AbiMismatch);
    }
    let view = ProdexStringView {
        ptr: line.as_ptr(),
        len: line.len(),
    };
    let mut result = CommandOutputLineClassification::default();
    let status = unsafe {
        prodex_context_classify_command_output_line_v1(CONTEXT_TEXT_ABI_VERSION, &view, &mut result)
    };
    match status {
        0 if result.flags >= 0
            && result.noisy_label >= 0
            && result.diagnostic_label >= 0
            && result.noisy_label <= 100
            && result.diagnostic_label <= 100 =>
        {
            Ok(result)
        }
        4 => Err(crate::MojoError::AbiMismatch),
        1 | 2 => Err(crate::MojoError::InvalidInput),
        _ => Err(crate::MojoError::InvalidOutput),
    }
}
