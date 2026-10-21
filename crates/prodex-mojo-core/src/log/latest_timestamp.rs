//! Newest-log timestamp tie and ordering policy owned by compiled Mojo.
use crate::MojoError;

const ABI_VERSION: i64 = 1;
const MAX_TIMESTAMP_BYTES: usize = 1_048_576;

unsafe extern "C" {
    fn prodex_mojo_log_latest_timestamp_v1(
        abi_version: i64,
        current_address: u64,
        current_length: i64,
        current_present: i64,
        incoming_address: u64,
        incoming_length: i64,
        output_address: u64,
    ) -> i64;
}

/// Select incoming if no incumbent exists or the timestamp is lexically
/// greater-or-equal. Invalid ABI input/output is never compared in Rust.
pub fn log_timestamp_should_replace(
    current: Option<&str>,
    incoming: &str,
) -> Result<bool, MojoError> {
    let current_text = current.unwrap_or_default();
    if current_text.len() > MAX_TIMESTAMP_BYTES || incoming.len() > MAX_TIMESTAMP_BYTES {
        return Err(MojoError::InvalidInput);
    }
    let mut selected = -1_i64;
    let status = unsafe {
        prodex_mojo_log_latest_timestamp_v1(
            ABI_VERSION,
            current_text.as_ptr() as usize as u64,
            i64::try_from(current_text.len()).map_err(|_| MojoError::InvalidInput)?,
            i64::from(current.is_some()),
            incoming.as_ptr() as usize as u64,
            i64::try_from(incoming.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut selected as *mut i64) as usize as u64,
        )
    };
    match status {
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    match selected {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiled_newest_timestamp_ties_choose_last_event() {
        assert_eq!(log_timestamp_should_replace(None, ""), Ok(true));
        assert_eq!(log_timestamp_should_replace(None, "2026-10-21"), Ok(true));
        assert_eq!(log_timestamp_should_replace(Some(""), ""), Ok(true));
        assert_eq!(
            log_timestamp_should_replace(Some("2026-10-20"), "2026-10-21"),
            Ok(true)
        );
        assert_eq!(
            log_timestamp_should_replace(Some("2026-10-21"), "2026-10-21"),
            Ok(true)
        );
        assert_eq!(
            log_timestamp_should_replace(Some("2026-10-22"), "2026-10-21"),
            Ok(false)
        );
        assert_eq!(log_timestamp_should_replace(Some("🚀"), "🧭"), Ok(true));
        assert_eq!(log_timestamp_should_replace(Some("🧭"), "🚀"), Ok(false));
    }

    #[test]
    fn compiled_newest_timestamp_rejects_unbounded_names() {
        assert_eq!(
            log_timestamp_should_replace(Some(&"a".repeat(MAX_TIMESTAMP_BYTES + 1)), "x"),
            Err(MojoError::InvalidInput)
        );
    }
}
