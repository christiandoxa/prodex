//! Windows session path text; filesystem interpretation stays with the host.
use crate::{MojoError, json::render_native_text};

unsafe extern "C" {
    fn prodex_shared_session_path_codec_v1(
        abi_version: i64,
        mode: i64,
        value_address: u64,
        value_length: i64,
        template_address: u64,
        template_length: i64,
        measuring: i64,
        output_address: u64,
        capacity: i64,
        metadata_address: u64,
    ) -> i64;
}

/// Collapse each escaped backslash run using the shortest nonempty source run.
pub fn decode_session_path(raw_path: &str) -> Result<String, MojoError> {
    path_codec(0, raw_path, raw_path)
}

/// Match the source session's backslash escaping without interpreting its path.
pub fn render_session_path(rendered: &str, raw_path: &str) -> Result<String, MojoError> {
    path_codec(1, rendered, raw_path)
}

fn path_codec(mode: i64, value: &str, template: &str) -> Result<String, MojoError> {
    let value_length = i64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)?;
    let template_length = i64::try_from(template.len()).map_err(|_| MojoError::InvalidInput)?;
    render_native_text(|measuring, output, capacity, metadata| unsafe {
        prodex_shared_session_path_codec_v1(
            1,
            mode,
            value.as_ptr() as u64,
            value_length,
            template.as_ptr() as u64,
            template_length,
            measuring,
            output,
            capacity,
            metadata,
        )
    })
}

#[cfg(test)]
mod tests;
