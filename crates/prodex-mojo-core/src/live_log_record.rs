use crate::MojoError;

const ABI_VERSION: i64 = 1;
const MAX_RECORD_BYTES: usize = 128 * 1024;

const _: () = assert!(std::mem::size_of::<usize>() == std::mem::size_of::<u64>());

unsafe extern "C" {
    fn prodex_live_log_record_over_limit_v1(
        abi_version: i64,
        record_length: i64,
        output_address: u64,
    ) -> i64;
    fn prodex_live_log_string_clip_end_v1(
        abi_version: i64,
        input_address: u64,
        input_length: i64,
        output_address: u64,
    ) -> i64;
    fn prodex_live_log_json_plan_v1(
        abi_version: i64,
        serialized_length: i64,
        output_address: u64,
    ) -> i64;
    fn prodex_live_log_plain_text_truncate_v1(
        abi_version: i64,
        input_address: u64,
        input_length: i64,
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

fn input_length(length: usize) -> Result<i64, MojoError> {
    i64::try_from(length).map_err(|_| MojoError::InvalidInput)
}

fn pointer_address<T>(pointer: *const T) -> u64 {
    pointer as usize as u64
}

fn mutable_pointer_address<T>(pointer: *mut T) -> u64 {
    pointer as usize as u64
}

/// Whether a live-log record exceeds the Mojo-owned record limit.
pub fn record_exceeds_bound(length: usize) -> Result<bool, MojoError> {
    let mut output = [-1_i64];
    status(unsafe {
        prodex_live_log_record_over_limit_v1(
            ABI_VERSION,
            input_length(length)?,
            mutable_pointer_address(output.as_mut_ptr()),
        )
    })?;
    match output[0] {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Returns Mojo's UTF-8-safe nested-string prefix boundary.
pub fn nested_string_clip_end(value: &str) -> Result<usize, MojoError> {
    let mut output = [-1_i64];
    status(unsafe {
        prodex_live_log_string_clip_end_v1(
            ABI_VERSION,
            pointer_address(value.as_ptr()),
            input_length(value.len())?,
            mutable_pointer_address(output.as_mut_ptr()),
        )
    })?;
    let end = usize::try_from(output[0]).map_err(|_| MojoError::InvalidOutput)?;
    if end > value.len() || !value.is_char_boundary(end) {
        return Err(MojoError::InvalidOutput);
    }
    Ok(end)
}

/// Selected JSON fields to materialize if Mojo chooses compact metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LiveLogJsonPlan {
    /// Whether the clipped JSON exceeds the live-log record limit.
    pub compact_metadata: bool,
    /// Whether to preserve the top-level `timestamp` value.
    pub timestamp: bool,
    /// Whether to preserve the top-level `pid` value.
    pub pid: bool,
    /// Whether to preserve the top-level `event` value.
    pub event: bool,
}

/// Chooses full JSON or the compact metadata fallback from serialized bytes.
pub fn json_plan(serialized_length: usize) -> Result<LiveLogJsonPlan, MojoError> {
    let mut output = [-1_i64; 4];
    status(unsafe {
        prodex_live_log_json_plan_v1(
            ABI_VERSION,
            input_length(serialized_length)?,
            mutable_pointer_address(output.as_mut_ptr()),
        )
    })?;
    let compact_metadata = match output[0] {
        0 => false,
        1 => true,
        _ => return Err(MojoError::InvalidOutput),
    };
    let mut selected = [false; 3];
    for (index, value) in output[1..].iter().copied().enumerate() {
        selected[index] = match value {
            0 => false,
            1 => true,
            _ => return Err(MojoError::InvalidOutput),
        };
    }
    if selected
        .iter()
        .any(|selected| *selected != compact_metadata)
    {
        return Err(MojoError::InvalidOutput);
    }
    Ok(LiveLogJsonPlan {
        compact_metadata,
        timestamp: selected[0],
        pid: selected[1],
        event: selected[2],
    })
}

/// Truncates oversized plain text and materializes Mojo's marker and newline.
pub fn truncate_plain_text(value: &str) -> Result<String, MojoError> {
    let mut output = vec![0_u8; MAX_RECORD_BYTES];
    let mut written = [-1_i64];
    status(unsafe {
        prodex_live_log_plain_text_truncate_v1(
            ABI_VERSION,
            pointer_address(value.as_ptr()),
            input_length(value.len())?,
            output.as_mut_ptr() as usize as u64,
            input_length(output.len())?,
            mutable_pointer_address(written.as_mut_ptr()),
        )
    })?;
    let written = usize::try_from(written[0]).map_err(|_| MojoError::InvalidOutput)?;
    let output = output.get(..written).ok_or(MojoError::InvalidOutput)?;
    String::from_utf8(output.to_vec()).map_err(|_| MojoError::InvalidOutput)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_live_log_mojo_adapter_checks_bounds_and_selected_metadata() {
        assert!(!record_exceeds_bound(MAX_RECORD_BYTES).unwrap());
        assert!(record_exceeds_bound(MAX_RECORD_BYTES + 1).unwrap());
        assert_eq!(
            json_plan(MAX_RECORD_BYTES).unwrap(),
            LiveLogJsonPlan {
                compact_metadata: false,
                timestamp: false,
                pid: false,
                event: false,
            }
        );
        assert_eq!(
            json_plan(MAX_RECORD_BYTES + 1).unwrap(),
            LiveLogJsonPlan {
                compact_metadata: true,
                timestamp: true,
                pid: true,
                event: true,
            }
        );
    }

    #[test]
    fn runtime_live_log_mojo_adapter_returns_utf8_clip_boundaries() {
        let ascii = "a".repeat(8 * 1024 + 1);
        assert_eq!(nested_string_clip_end(&ascii).unwrap(), 8 * 1024);
        let unicode = format!("{}é-tail", "a".repeat(8 * 1024 - 1));
        assert_eq!(nested_string_clip_end(&unicode).unwrap(), 8 * 1024 + 1);
    }
}
