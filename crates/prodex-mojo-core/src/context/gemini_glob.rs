use super::{CONTEXT_TEXT_ABI_VERSION, ProdexStringView, text_abi_is_ready};

const GEMINI_GLOB_MAX_BYTES: usize = 131_072;

unsafe extern "C" {
    fn prodex_context_gemini_glob_matches_v1(
        abi_version: i64,
        pattern: *const ProdexStringView,
        path: *const ProdexStringView,
        output: *mut i64,
    ) -> i64;
}

/// Matches one normalized Gemini context glob against one normalized path.
///
/// Rust owns path normalization and filesystem policy; Mojo owns only the
/// bounded wildcard matching semantics. Inputs must be UTF-8 and use `/` as
/// the path separator.
pub fn gemini_glob_matches(pattern: &str, path: &str) -> Result<bool, crate::MojoError> {
    if pattern.len() > GEMINI_GLOB_MAX_BYTES || path.len() > GEMINI_GLOB_MAX_BYTES {
        return Err(crate::MojoError::InvalidInput);
    }
    if !text_abi_is_ready() {
        return Err(crate::MojoError::AbiMismatch);
    }
    let pattern_view = ProdexStringView {
        ptr: pattern.as_ptr(),
        len: pattern.len(),
    };
    let path_view = ProdexStringView {
        ptr: path.as_ptr(),
        len: path.len(),
    };
    let mut output = -1_i64;
    let status = unsafe {
        prodex_context_gemini_glob_matches_v1(
            CONTEXT_TEXT_ABI_VERSION,
            &pattern_view,
            &path_view,
            &mut output,
        )
    };
    match status {
        0 => match output {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(crate::MojoError::InvalidOutput),
        },
        4 => Err(crate::MojoError::AbiMismatch),
        1 | 2 => Err(crate::MojoError::InvalidInput),
        _ => Err(crate::MojoError::InvalidOutput),
    }
}
