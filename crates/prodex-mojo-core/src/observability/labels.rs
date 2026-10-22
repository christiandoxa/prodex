use super::{OBSERVABILITY_LABEL_ABI_VERSION, text::render_measured_text};
use crate::MojoError;

unsafe extern "C" {
    fn prodex_mojo_log_event_label_v1(
        abi_version: i64,
        source_address: u64,
        source_length: i64,
        measuring: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
    ) -> i64;
}

/// Render a human log source label through the compiled catalog and alias policy.
pub fn render_log_event_label(source: &str) -> Result<String, MojoError> {
    let length = i64::try_from(source.len()).map_err(|_| MojoError::InvalidInput)?;
    render_measured_text(|measuring, output, capacity, written| unsafe {
        prodex_mojo_log_event_label_v1(
            OBSERVABILITY_LABEL_ABI_VERSION,
            source.as_ptr() as u64,
            length,
            measuring,
            output,
            capacity,
            written,
        )
    })
}

#[cfg(test)]
#[path = "labels_tests.rs"]
mod tests;
