//! Typed, versioned ABI for secure upstream payload logging byte limits.
//! Borrowed lengths and payload bytes remain host-owned; compiled Mojo owns
//! budget allocation, omission decisions and UTF-8 prefix truncation.

use crate::MojoError;

const ABI_VERSION: i64 = 1;
const MAX_STRINGS: usize = 1_048_576;

unsafe extern "C" {
    fn prodex_mojo_upstream_payload_string_budget_v1(
        abi_version: i64,
        lengths_address: u64,
        count: i64,
        budget: u64,
        flags_address: u64,
        flags_capacity: i64,
        remaining_address: u64,
    ) -> i64;
    fn prodex_mojo_upstream_payload_truncate_v1(
        abi_version: i64,
        input_address: u64,
        input_length: i64,
        limit: i64,
        result_address: u64,
    ) -> i64;
}

/// Mojo-owned decisions for each original Serde string and unused capacity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpstreamPayloadStringBudgetPlan {
    pub omit: Vec<bool>,
    pub remaining: usize,
}

fn check_status(code: i64) -> Result<(), MojoError> {
    match code {
        0 => Ok(()),
        1 => Err(MojoError::InvalidInput),
        3 => Err(MojoError::Capacity),
        4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn upstream_payload_string_budget_plan(
    string_lengths: &[usize],
    initial_budget: usize,
) -> Result<UpstreamPayloadStringBudgetPlan, MojoError> {
    if string_lengths.len() > MAX_STRINGS {
        return Err(MojoError::InvalidInput);
    }
    let mut lengths = Vec::<u64>::new();
    lengths
        .try_reserve_exact(string_lengths.len())
        .map_err(|_| MojoError::Capacity)?;
    for &length in string_lengths {
        lengths.push(u64::try_from(length).map_err(|_| MojoError::InvalidInput)?);
    }
    let mut flags = Vec::<i64>::new();
    flags
        .try_reserve_exact(string_lengths.len())
        .map_err(|_| MojoError::Capacity)?;
    flags.resize(lengths.len(), -1);
    let mut remaining = u64::MAX;
    check_status(unsafe {
        prodex_mojo_upstream_payload_string_budget_v1(
            ABI_VERSION,
            lengths.as_ptr() as usize as u64,
            i64::try_from(lengths.len()).map_err(|_| MojoError::InvalidInput)?,
            u64::try_from(initial_budget).map_err(|_| MojoError::InvalidInput)?,
            flags.as_mut_ptr() as usize as u64,
            i64::try_from(flags.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut remaining as *mut u64) as usize as u64,
        )
    })?;
    if remaining > u64::try_from(initial_budget).map_err(|_| MojoError::InvalidInput)? {
        return Err(MojoError::InvalidOutput);
    }
    let mut omit = Vec::with_capacity(flags.len());
    for flag in flags {
        match flag {
            0 => omit.push(false),
            1 => omit.push(true),
            _ => return Err(MojoError::InvalidOutput),
        }
    }
    Ok(UpstreamPayloadStringBudgetPlan {
        omit,
        remaining: usize::try_from(remaining).map_err(|_| MojoError::InvalidOutput)?,
    })
}

/// Mojo returns the source byte prefix and whether the payload was capped.
/// No redaction, JSON parsing or disk operations enter this scalar ABI.
pub fn upstream_payload_log_prefix(
    payload: &[u8],
    max_bytes: usize,
) -> Result<(usize, bool), MojoError> {
    let mut result = [-1_i64; 2];
    check_status(unsafe {
        prodex_mojo_upstream_payload_truncate_v1(
            ABI_VERSION,
            payload.as_ptr() as usize as u64,
            i64::try_from(payload.len()).map_err(|_| MojoError::InvalidInput)?,
            i64::try_from(max_bytes).map_err(|_| MojoError::InvalidInput)?,
            result.as_mut_ptr() as usize as u64,
        )
    })?;
    let end = usize::try_from(result[0]).map_err(|_| MojoError::InvalidOutput)?;
    let truncated = match result[1] {
        0 => false,
        1 => true,
        _ => return Err(MojoError::InvalidOutput),
    };
    if end > payload.len() || end > max_bytes || truncated != (payload.len() > max_bytes) {
        return Err(MojoError::InvalidOutput);
    }
    if !truncated && end != payload.len() {
        return Err(MojoError::InvalidOutput);
    }
    Ok((end, truncated))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiled_upstream_payload_string_budget_preserves_order_and_skipped_items() {
        assert_eq!(
            upstream_payload_string_budget_plan(&[200, 500, 400, 100, 300, 0], 1024),
            Ok(UpstreamPayloadStringBudgetPlan {
                omit: vec![false, false, true, false, true, false],
                remaining: 224,
            })
        );
        assert_eq!(
            upstream_payload_string_budget_plan(&[], 1024),
            Ok(UpstreamPayloadStringBudgetPlan {
                omit: vec![],
                remaining: 1024
            }),
        );
        assert_eq!(
            upstream_payload_string_budget_plan(&[0, 1, 0], 0),
            Ok(UpstreamPayloadStringBudgetPlan {
                omit: vec![false, true, false],
                remaining: 0,
            })
        );
        assert_eq!(
            upstream_payload_string_budget_plan(&[usize::MAX, 1, 0], 1024),
            Ok(UpstreamPayloadStringBudgetPlan {
                omit: vec![true, false, false],
                remaining: 1023,
            })
        );
    }

    #[test]
    fn compiled_upstream_payload_prefix_preserves_valid_utf8_and_legacy_ascii_fallback() {
        let text = "abé🙂".as_bytes();
        for (cap, expected) in [
            (0, (0, true)),
            (1, (1, true)),
            (2, (2, true)),
            (3, (2, true)),
            (4, (4, true)),
            (6, (2, true)),
            (7, (2, true)),
            (8, (8, false)),
            (9, (8, false)),
        ] {
            assert_eq!(
                upstream_payload_log_prefix(text, cap),
                Ok(expected),
                "{cap}"
            );
        }
        let raw = [255_u8, 254, b'a', 192, 240];
        assert_eq!(upstream_payload_log_prefix(&raw, 4), Ok((3, true)));
        assert_eq!(upstream_payload_log_prefix(&raw, 5), Ok((5, false)));
        assert_eq!(upstream_payload_log_prefix(b"abc", 2), Ok((2, true)));
        assert_eq!(upstream_payload_log_prefix(&[], 0), Ok((0, false)));
    }

    #[test]
    fn compiled_upstream_payload_limits_reject_abi_and_capacity_without_writes() {
        let data = [11_u64, 22];
        let mut flags = [73_i64; 2];
        let mut remaining = 91_u64;
        let source = data.as_ptr() as usize as u64;
        let output = flags.as_mut_ptr() as usize as u64;
        let rest = (&mut remaining as *mut u64) as usize as u64;
        for (abi, input, count, capacity, rest_ptr, expected) in [
            (2, source, 2, 2, rest, 4),
            (1, 0, 2, 2, rest, 1),
            (1, source, -1, 2, rest, 1),
            (1, source, 1_048_577, 2, rest, 1),
            (1, source, 2, 1, rest, 1),
            (1, source, 2, 2, 0, 1),
        ] {
            assert_eq!(
                unsafe {
                    prodex_mojo_upstream_payload_string_budget_v1(
                        abi, input, count, 1024, output, capacity, rest_ptr,
                    )
                },
                expected
            );
            assert_eq!(flags, [73; 2]);
            assert_eq!(remaining, 91);
        }
        let mut span = [37_i64; 2];
        let span_ptr = span.as_mut_ptr() as usize as u64;
        let input = b"long enough";
        for (abi, source, size, cap, dest, expected) in [
            (2, input.as_ptr() as usize as u64, 11, 4, span_ptr, 4),
            (1, 0, 11, 4, span_ptr, 1),
            (1, input.as_ptr() as usize as u64, -1, 4, span_ptr, 1),
            (1, input.as_ptr() as usize as u64, 11, -1, span_ptr, 1),
            (1, input.as_ptr() as usize as u64, 11, 4, 0, 1),
        ] {
            assert_eq!(
                unsafe { prodex_mojo_upstream_payload_truncate_v1(abi, source, size, cap, dest,) },
                expected
            );
            assert_eq!(span, [37; 2]);
        }
    }
}
