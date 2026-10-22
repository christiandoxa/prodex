use super::{LogStringView, STRUCTURED_LOG_ABI_VERSION, StructuredLogFieldPolicy};
use crate::{MojoError, text_output::render_measured_text};

unsafe extern "C" {
    fn prodex_mojo_structured_log_message_field_policy_v1(
        abi_version: i64,
        key_address: u64,
        key_length: i64,
        value_address: u64,
        value_length: i64,
        output_address: u64,
    ) -> i64;
    fn prodex_mojo_structured_log_message_v1(
        abi_version: i64,
        event_address: u64,
        event_length: i64,
        fields_address: u64,
        field_count: i64,
        measuring: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
    ) -> i64;
}

/// Select message fields using sanitized keys, including empty-key suppression.
pub fn structured_log_message_field_policy(
    key: &str,
    value: &str,
) -> Result<StructuredLogFieldPolicy, MojoError> {
    super::structured_log_field_policy_with(
        key,
        value,
        prodex_mojo_structured_log_message_field_policy_v1,
    )
}

/// Join selected fields in input order after host redaction and value quoting.
/// Keys must be sanitized and selected by `structured_log_message_field_policy`.
pub fn render_structured_log_message(
    event: &str,
    fields: &[(String, String)],
) -> Result<String, MojoError> {
    if fields.len() > 65_536 {
        return Err(MojoError::InvalidInput);
    }
    let views: Vec<_> = fields
        .iter()
        .flat_map(|(key, value)| {
            [key, value].map(|text| LogStringView {
                ptr: text.as_ptr() as u64,
                len: text.len() as u64,
            })
        })
        .collect();
    let length = i64::try_from(event.len()).map_err(|_| MojoError::InvalidInput)?;
    render_measured_text(|measuring, output, capacity, written| unsafe {
        prodex_mojo_structured_log_message_v1(
            STRUCTURED_LOG_ABI_VERSION,
            event.as_ptr() as u64,
            length,
            views.as_ptr() as u64,
            fields.len() as i64,
            measuring,
            output,
            capacity,
            written,
        )
    })
}

#[cfg(test)]
#[path = "structured_message_tests.rs"]
mod tests;
