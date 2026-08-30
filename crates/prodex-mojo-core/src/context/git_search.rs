use super::{
    CONTEXT_GIT_SEARCH_DIRECT_MATCH, CONTEXT_GIT_SEARCH_HEADING_MATCH,
    CONTEXT_GIT_SEARCH_HEADING_PATH, CONTEXT_GIT_SEARCH_JSON_LINE, CONTEXT_GIT_SEARCH_JSON_MATCH,
    CONTEXT_GIT_SEARCH_MAX_BYTES, CONTEXT_GIT_SEARCH_RESULT_WIDTH, CONTEXT_TEXT_ABI_VERSION,
    ProdexStringView, text_abi_is_ready,
};

unsafe extern "C" {
    fn prodex_context_classify_git_search_line_v1(
        abi_version: i64,
        line: *const ProdexStringView,
        heading_path: *const ProdexStringView,
        heading_present: i64,
        path_output: *mut u8,
        path_output_capacity: i64,
        text_output: *mut u8,
        text_output_capacity: i64,
        output: *mut i64,
        output_count: i64,
    ) -> i64;
}

/// Classifies one bounded Git-search output line into caller-owned buffers.
pub fn classify_git_search_line(
    line: &str,
    heading_path: Option<&str>,
    path_output: &mut [u8],
    text_output: &mut [u8],
) -> Result<[i64; CONTEXT_GIT_SEARCH_RESULT_WIDTH], crate::MojoError> {
    if !git_search_input_is_valid(line, heading_path, path_output, text_output) {
        return Err(crate::MojoError::InvalidInput);
    }
    if !text_abi_is_ready() {
        return Err(crate::MojoError::AbiMismatch);
    }
    let line_view = ProdexStringView {
        ptr: line.as_ptr(),
        len: line.len(),
    };
    let heading_view = heading_path.map_or(
        ProdexStringView {
            ptr: std::ptr::null(),
            len: 0,
        },
        |path| ProdexStringView {
            ptr: path.as_ptr(),
            len: path.len(),
        },
    );
    let mut output = [-1_i64; CONTEXT_GIT_SEARCH_RESULT_WIDTH];
    let status = unsafe {
        prodex_context_classify_git_search_line_v1(
            CONTEXT_TEXT_ABI_VERSION,
            &line_view,
            &heading_view,
            i64::from(heading_path.is_some()),
            path_output.as_mut_ptr(),
            i64::try_from(path_output.len()).map_err(|_| crate::MojoError::InvalidInput)?,
            text_output.as_mut_ptr(),
            i64::try_from(text_output.len()).map_err(|_| crate::MojoError::InvalidInput)?,
            output.as_mut_ptr(),
            CONTEXT_GIT_SEARCH_RESULT_WIDTH as i64,
        )
    };
    if status != 0 {
        return Err(match status {
            1 | 2 => crate::MojoError::InvalidInput,
            3 => crate::MojoError::Capacity,
            4 => crate::MojoError::AbiMismatch,
            _ => crate::MojoError::InvalidOutput,
        });
    }

    let primary = validate_git_search_flags(&output)?;
    validate_git_search_buffers(&output, path_output, text_output)?;
    let has_path = output[2] >= 0;
    let has_text = output[3] >= 0;
    git_search_shape_is_valid(primary, output[1], has_path, has_text)
        .then_some(output)
        .ok_or(crate::MojoError::InvalidOutput)
}

fn git_search_input_is_valid(
    line: &str,
    heading_path: Option<&str>,
    path_output: &[u8],
    text_output: &[u8],
) -> bool {
    line.len() <= CONTEXT_GIT_SEARCH_MAX_BYTES
        && heading_path.is_none_or(|path| path.len() <= CONTEXT_GIT_SEARCH_MAX_BYTES)
        && !path_output.is_empty()
        && !text_output.is_empty()
        && path_output.len() <= CONTEXT_GIT_SEARCH_MAX_BYTES
        && text_output.len() <= CONTEXT_GIT_SEARCH_MAX_BYTES
}

fn validate_git_search_flags(
    output: &[i64; CONTEXT_GIT_SEARCH_RESULT_WIDTH],
) -> Result<i64, crate::MojoError> {
    let allowed = CONTEXT_GIT_SEARCH_DIRECT_MATCH
        | CONTEXT_GIT_SEARCH_JSON_MATCH
        | CONTEXT_GIT_SEARCH_JSON_LINE
        | CONTEXT_GIT_SEARCH_HEADING_PATH
        | CONTEXT_GIT_SEARCH_HEADING_MATCH;
    let primary = output[0]
        & (CONTEXT_GIT_SEARCH_DIRECT_MATCH
            | CONTEXT_GIT_SEARCH_JSON_MATCH
            | CONTEXT_GIT_SEARCH_HEADING_PATH
            | CONTEXT_GIT_SEARCH_HEADING_MATCH);
    let known_primary = matches!(
        primary,
        0 | CONTEXT_GIT_SEARCH_DIRECT_MATCH
            | CONTEXT_GIT_SEARCH_JSON_MATCH
            | CONTEXT_GIT_SEARCH_HEADING_PATH
            | CONTEXT_GIT_SEARCH_HEADING_MATCH
    );
    if output[0] < 0
        || output[0] & !allowed != 0
        || !known_primary
        || (output[0] & CONTEXT_GIT_SEARCH_JSON_MATCH != 0
            && primary != CONTEXT_GIT_SEARCH_JSON_MATCH)
        || output[1] < -1
    {
        return Err(crate::MojoError::InvalidOutput);
    }
    Ok(primary)
}

fn validate_git_search_buffers(
    output: &[i64; CONTEXT_GIT_SEARCH_RESULT_WIDTH],
    path_output: &[u8],
    text_output: &[u8],
) -> Result<(), crate::MojoError> {
    for (length, buffer) in [(output[2], path_output), (output[3], text_output)] {
        if length < -1 {
            return Err(crate::MojoError::InvalidOutput);
        }
        let Some(length) = usize::try_from(length).ok() else {
            continue;
        };
        if length > buffer.len() || std::str::from_utf8(&buffer[..length]).is_err() {
            return Err(crate::MojoError::InvalidOutput);
        }
    }
    Ok(())
}

fn git_search_shape_is_valid(primary: i64, kind: i64, has_path: bool, has_text: bool) -> bool {
    match primary {
        0 => !has_path && !has_text && kind == -1,
        CONTEXT_GIT_SEARCH_DIRECT_MATCH | CONTEXT_GIT_SEARCH_JSON_MATCH => has_path && has_text,
        CONTEXT_GIT_SEARCH_HEADING_PATH => has_path && !has_text && kind == -1,
        CONTEXT_GIT_SEARCH_HEADING_MATCH => !has_path && has_text,
        _ => false,
    }
}
