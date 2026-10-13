use crate::MojoError;
use std::collections::BTreeMap;

const ABI_VERSION: i64 = 1;

/// Runtime log output format tags owned by the Mojo renderer.
pub const RUNTIME_LOG_FORMAT_TEXT: i64 = 0;
/// Runtime log output format tag owned by the Mojo renderer.
pub const RUNTIME_LOG_FORMAT_JSON: i64 = 1;

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
struct RuntimeLogStringView {
    ptr: u64,
    len: u64,
}

unsafe extern "C" {
    fn prodex_mojo_runtime_log_render_v1(
        abi_version: i64,
        format: i64,
        timestamp_address: u64,
        timestamp_length: i64,
        pid: u64,
        message_address: u64,
        message_length: i64,
        event_address: u64,
        event_length: i64,
        event_present: i64,
        fields_address: u64,
        field_count: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
    ) -> i64;
}

fn status(value: i64) -> Result<(), MojoError> {
    match value {
        0 => Ok(()),
        1 => Err(MojoError::InvalidInput),
        2 => Err(MojoError::Capacity),
        4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

fn view(value: &str) -> RuntimeLogStringView {
    RuntimeLogStringView {
        ptr: value.as_ptr() as usize as u64,
        len: value.len() as u64,
    }
}

fn output_capacity(
    format: i64,
    timestamp: &str,
    message: &str,
    event: Option<&str>,
    fields: &BTreeMap<String, String>,
) -> Result<usize, MojoError> {
    let mut input_bytes = timestamp
        .len()
        .checked_add(message.len())
        .ok_or(MojoError::InvalidInput)?;
    if let Some(event) = event {
        input_bytes = input_bytes
            .checked_add(event.len())
            .ok_or(MojoError::InvalidInput)?;
    }
    for (key, value) in fields {
        input_bytes = input_bytes
            .checked_add(key.len())
            .and_then(|size| size.checked_add(value.len()))
            .ok_or(MojoError::InvalidInput)?;
    }
    let escaped = if format == RUNTIME_LOG_FORMAT_JSON {
        input_bytes.checked_mul(6).ok_or(MojoError::InvalidInput)?
    } else {
        input_bytes
    };
    escaped
        .checked_add(256)
        .and_then(|size| size.checked_add(fields.len().checked_mul(8)?))
        .ok_or(MojoError::InvalidInput)
}

/// Render one already-redacted runtime-log record through the authoritative Mojo formatter.
///
/// Rust retains redaction, log parsing, ordered field ownership, and the output buffer. Mojo
/// owns text normalization, JSON field ordering, escaping, and the final record templates.
pub fn render_runtime_log_line(
    format: i64,
    timestamp: &str,
    pid: u32,
    message: &str,
    event: Option<&str>,
    fields: &BTreeMap<String, String>,
) -> Result<String, MojoError> {
    if !matches!(format, RUNTIME_LOG_FORMAT_TEXT | RUNTIME_LOG_FORMAT_JSON) {
        return Err(MojoError::InvalidInput);
    }
    let mut text = Vec::with_capacity(fields.len().saturating_mul(2));
    for (key, value) in fields {
        text.push(view(key));
        text.push(view(value));
    }
    let capacity = output_capacity(format, timestamp, message, event, fields)?;
    let mut output = vec![0_u8; capacity.max(1)];
    let mut written = -1_i64;
    let (event_address, event_length, event_present) = event
        .map(|event| (event.as_ptr() as usize as u64, event.len() as i64, 1_i64))
        .unwrap_or((0, 0, 0));
    status(unsafe {
        prodex_mojo_runtime_log_render_v1(
            ABI_VERSION,
            format,
            timestamp.as_ptr() as usize as u64,
            i64::try_from(timestamp.len()).map_err(|_| MojoError::InvalidInput)?,
            u64::from(pid),
            message.as_ptr() as usize as u64,
            i64::try_from(message.len()).map_err(|_| MojoError::InvalidInput)?,
            event_address,
            event_length,
            event_present,
            text.as_ptr() as usize as u64,
            i64::try_from(fields.len()).map_err(|_| MojoError::InvalidInput)?,
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut written as *mut i64) as usize as u64,
        )
    })?;
    let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    let output = output.get(..written).ok_or(MojoError::InvalidOutput)?;
    String::from_utf8(output.to_vec()).map_err(|_| MojoError::InvalidOutput)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_log_renderer_rejects_wrong_mojo_abi_and_invalid_wire_format() {
        let mut output = [0_u8; 128];
        let mut written = -1_i64;
        let code = unsafe {
            prodex_mojo_runtime_log_render_v1(
                999,
                RUNTIME_LOG_FORMAT_TEXT,
                0,
                0,
                42,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                output.as_mut_ptr() as usize as u64,
                output.len() as i64,
                (&mut written as *mut i64) as usize as u64,
            )
        };
        assert_eq!(code, 4, "Mojo ABI mismatch must fail without fallback");
    }

    #[test]
    fn runtime_log_renderer_rejects_unknown_format_without_fallback() {
        assert_eq!(
            render_runtime_log_line(99, "now", 7, "event", None, &BTreeMap::new()),
            Err(MojoError::InvalidInput)
        );
    }
}
