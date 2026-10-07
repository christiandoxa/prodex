use super::{LogStringView, mutable_pointer_address, pointer_address};
use crate::MojoError;

const LOG_LEVEL_ABI_VERSION: i64 = 1;
const LOG_LEVEL_MAX_BYTES: usize = 4096;

unsafe extern "C" {
    fn prodex_mojo_log_level_classify_v1(abi_version: i64, event: u64, level: u64) -> i64;
}

/// Classifies an already-normalized, length-bounded log line by its level.
pub fn classify_log_level(line: &str) -> Result<Option<&'static str>, MojoError> {
    if line.len() > LOG_LEVEL_MAX_BYTES {
        return Err(MojoError::InvalidInput);
    }
    let mut level = -1_i64;
    let line_view = LogStringView {
        ptr: line.as_ptr() as usize as u64,
        len: line.len() as u64,
    };
    let status = unsafe {
        prodex_mojo_log_level_classify_v1(
            LOG_LEVEL_ABI_VERSION,
            pointer_address(&line_view),
            mutable_pointer_address(&mut level),
        )
    };
    if status == 2 {
        return Err(MojoError::InvalidInput);
    }
    if status != 0 {
        return Err(MojoError::AbiMismatch);
    }
    match level {
        0 => Ok(None),
        1 => Ok(Some("fatal")),
        2 => Ok(Some("error")),
        3 => Ok(Some("warn")),
        4 => Ok(Some("info")),
        5 => Ok(Some("debug")),
        6 => Ok(Some("trace")),
        _ => Err(MojoError::InvalidOutput),
    }
}
