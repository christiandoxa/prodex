//! Typed Mojo-owned Gemini retry and continuation-history decisions.

use crate::MojoError;

#[repr(i64)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeminiFailureOperation {
    RetryableFinishReason = 1,
    TerminalWithoutHistory = 2,
}

unsafe extern "C" {
    fn prodex_gemini_failure_policy_v1(
        version: i64,
        operation: i64,
        text_address: u64,
        text_length: i64,
        text_present: i64,
        has_error: i64,
        result_address: u64,
    ) -> i64;
}

pub fn gemini_failure_policy(
    operation: GeminiFailureOperation,
    status_or_reason: Option<&str>,
    has_error: bool,
) -> Result<bool, MojoError> {
    let (address, length, present) = match status_or_reason {
        Some(text) => (
            text.as_ptr() as usize as u64,
            i64::try_from(text.len()).map_err(|_| MojoError::InvalidInput)?,
            1,
        ),
        None => (0, 0, 0),
    };
    let mut result = -1_i64;
    let status = unsafe {
        prodex_gemini_failure_policy_v1(
            1,
            operation as i64,
            address,
            length,
            present,
            i64::from(has_error),
            (&mut result as *mut i64) as usize as u64,
        )
    };
    if status == 4 {
        return Err(MojoError::AbiMismatch);
    }
    if status != 0 {
        return Err(MojoError::InvalidInput);
    }
    match result {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}
