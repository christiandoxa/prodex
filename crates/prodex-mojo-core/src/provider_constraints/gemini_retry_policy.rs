//! Mojo-owned Gemini retry timing policy.

use crate::MojoError;

const ABI_VERSION: i64 = 1;
const OP_DELAY: i64 = 0;
const OP_INLINE: i64 = 1;
const OP_INVALID_STREAM: i64 = 2;
const OP_INLINE_THRESHOLD: i64 = 3;

unsafe extern "C" {
    fn prodex_gemini_retry_policy_v1(
        abi_version: i64,
        operation: i64,
        retry_index: i64,
        header_present: i64,
        header_ms: u64,
        body_present: i64,
        body_ms: u64,
        delay_ms: u64,
        output_address: u64,
    ) -> i64;
}

fn call(
    operation: i64,
    retry_index: usize,
    header_ms: Option<u64>,
    body_ms: Option<u64>,
    delay_ms: u64,
) -> Result<u64, MojoError> {
    call_status(operation, retry_index, header_ms, body_ms, delay_ms).and_then(
        |(status, output)| {
            if status == 0 {
                Ok(output)
            } else {
                Err(MojoError::InvalidOutput)
            }
        },
    )
}

fn call_status(
    operation: i64,
    retry_index: usize,
    header_ms: Option<u64>,
    body_ms: Option<u64>,
    delay_ms: u64,
) -> Result<(i64, u64), MojoError> {
    let retry_index = i64::try_from(retry_index).unwrap_or(i64::MAX);
    let mut output = 0_u64;
    let status = unsafe {
        prodex_gemini_retry_policy_v1(
            ABI_VERSION,
            operation,
            retry_index,
            i64::from(header_ms.is_some()),
            header_ms.unwrap_or_default(),
            i64::from(body_ms.is_some()),
            body_ms.unwrap_or_default(),
            delay_ms,
            (&mut output as *mut u64) as usize as u64,
        )
    };
    match status {
        0 | 2 => Ok((status, output)),
        1 => Err(MojoError::InvalidInput),
        4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Apply Gemini's bounded server/default retry schedule through Mojo.
pub fn retry_delay_ms(
    header_ms: Option<u64>,
    body_ms: Option<u64>,
    retry_index: usize,
) -> Result<Option<u64>, MojoError> {
    let (status, output) = call_status(OP_DELAY, retry_index, header_ms, body_ms, 0)?;
    Ok((status == 0).then_some(output))
}

/// Return the Mojo-owned inline-retry decision for a computed delay.
pub fn should_inline(delay_ms: u64) -> Result<bool, MojoError> {
    match call(OP_INLINE, 0, None, None, delay_ms)? {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Return the current Mojo-owned inline retry threshold for diagnostics.
pub fn inline_threshold_ms() -> Result<u64, MojoError> {
    call(OP_INLINE_THRESHOLD, 0, None, None, 0)
}

/// Return the bounded exponential delay used before retrying an invalid stream.
pub fn invalid_stream_retry_delay_ms(retry_index: usize) -> Result<u64, MojoError> {
    call(OP_INVALID_STREAM, retry_index, None, None, 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retry_policy_preserves_schedule_maximum_cap_and_bounds() {
        assert_eq!(retry_delay_ms(None, None, 0).unwrap(), Some(5_000));
        assert_eq!(retry_delay_ms(None, None, 8).unwrap(), Some(30_000));
        assert_eq!(retry_delay_ms(None, None, 9).unwrap(), None);
        assert_eq!(
            retry_delay_ms(Some(3_000), Some(7_000), 0).unwrap(),
            Some(7_000)
        );
        assert_eq!(
            retry_delay_ms(Some(u64::MAX), None, 0).unwrap(),
            Some(300_000)
        );
        assert_eq!(retry_delay_ms(Some(0), Some(0), 0).unwrap(), Some(5_000));
    }

    #[test]
    fn retry_policy_preserves_inline_and_invalid_stream_decisions() {
        assert_eq!(inline_threshold_ms().unwrap(), 30_000);
        assert!(!should_inline(0).unwrap());
        assert!(should_inline(30_000).unwrap());
        assert!(!should_inline(30_001).unwrap());
        assert_eq!(invalid_stream_retry_delay_ms(0).unwrap(), 1_000);
        assert_eq!(invalid_stream_retry_delay_ms(3).unwrap(), 8_000);
        assert_eq!(invalid_stream_retry_delay_ms(99).unwrap(), 256_000);
    }

    #[test]
    fn retry_policy_rejects_bad_abi_and_operation() {
        let mut output = 0_u64;
        assert_eq!(
            unsafe {
                prodex_gemini_retry_policy_v1(
                    0,
                    OP_DELAY,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                    (&mut output as *mut u64) as u64,
                )
            },
            4
        );
        assert_eq!(call(99, 0, None, None, 0), Err(MojoError::InvalidInput));
    }
}
