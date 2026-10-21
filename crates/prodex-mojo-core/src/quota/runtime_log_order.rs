//! Borrowed filesystem metadata for runtime quota-profile log priority.
//! Compiled Mojo alone sorts Option<SystemTime> ascending and path bytes
//! ascending, so the host may apply later log events in chronological order.

use crate::MojoError;
use std::time::{SystemTime, UNIX_EPOCH};

const ABI_VERSION: i64 = 1;
const MAX_FILES: usize = 65_536;

unsafe extern "C" {
    fn prodex_quota_runtime_log_order_v1(
        abi: i64,
        rows_address: u64,
        count: i64,
        output_address: u64,
        output_capacity: i64,
    ) -> i64;
}

#[derive(Clone, Copy, Debug)]
pub struct QuotaRuntimeLogOrderInput<'a> {
    pub path: &'a [u8],
    pub modified_at: Option<SystemTime>,
}

/// Return a validated permutation of all original log paths. Missing
/// modified-time metadata precedes any real timestamp.
pub fn quota_runtime_log_order(
    input: &[QuotaRuntimeLogOrderInput<'_>],
) -> Result<Vec<usize>, MojoError> {
    if input.len() > MAX_FILES {
        return Err(MojoError::InvalidInput);
    }
    let mut rows = Vec::<u64>::new();
    rows.try_reserve_exact(input.len().checked_mul(6).ok_or(MojoError::Capacity)?)
        .map_err(|_| MojoError::Capacity)?;
    for entry in input {
        let (present, negative, absolute) = match entry.modified_at {
            Some(value) => match value.duration_since(UNIX_EPOCH) {
                Ok(value) => (1_u64, 0_u64, value),
                Err(error) => (1, 1, error.duration()),
            },
            None => (0, 0, std::time::Duration::ZERO),
        };
        rows.extend([
            entry.path.as_ptr() as usize as u64,
            u64::try_from(entry.path.len()).map_err(|_| MojoError::InvalidInput)?,
            present,
            negative,
            absolute.as_secs(),
            u64::from(absolute.subsec_nanos()),
        ]);
    }
    let mut indices = vec![-1_i64; input.len()];
    match unsafe {
        prodex_quota_runtime_log_order_v1(
            ABI_VERSION,
            rows.as_ptr() as usize as u64,
            i64::try_from(input.len()).map_err(|_| MojoError::InvalidInput)?,
            indices.as_mut_ptr() as usize as u64,
            i64::try_from(indices.len()).map_err(|_| MojoError::InvalidInput)?,
        )
    } {
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    let mut seen = vec![false; input.len()];
    let mut ordered = Vec::with_capacity(input.len());
    for index in indices {
        let index = usize::try_from(index).map_err(|_| MojoError::InvalidOutput)?;
        let slot = seen.get_mut(index).ok_or(MojoError::InvalidOutput)?;
        if *slot {
            return Err(MojoError::InvalidOutput);
        }
        *slot = true;
        ordered.push(index);
    }
    Ok(ordered)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn entry<'a>(path: &'a str, modified_at: Option<SystemTime>) -> QuotaRuntimeLogOrderInput<'a> {
        QuotaRuntimeLogOrderInput {
            path: path.as_bytes(),
            modified_at,
        }
    }

    #[test]
    fn compiled_quota_runtime_log_order_honors_missing_negative_and_unicode_ties() {
        let epoch = UNIX_EPOCH;
        let sample = [
            entry("雪", Some(epoch + std::time::Duration::from_secs(3))),
            entry("z", Some(epoch + std::time::Duration::from_secs(3))),
            entry("none-z", None),
            entry("old", Some(epoch - std::time::Duration::from_secs(2))),
            entry("early", Some(epoch + std::time::Duration::from_secs(1))),
            entry("none-a", None),
            entry("a", Some(epoch + std::time::Duration::from_secs(3))),
            entry("z", Some(epoch + std::time::Duration::from_secs(3))),
        ];
        assert_eq!(
            quota_runtime_log_order(&sample),
            Ok(vec![5, 2, 3, 4, 6, 1, 7, 0]),
        );
        assert_eq!(quota_runtime_log_order(&[]), Ok(vec![]));
        // Use subsecond increments representable by SystemTime on Windows too.
        let finer = [
            entry("newer", Some(epoch + std::time::Duration::from_nanos(200))),
            entry("older", Some(epoch + std::time::Duration::from_nanos(100))),
            entry("pre", Some(epoch - std::time::Duration::from_nanos(100))),
        ];
        assert_eq!(quota_runtime_log_order(&finer), Ok(vec![2, 1, 0]));
        let many = (0..1025)
            .map(|index| QuotaRuntimeLogOrderInput {
                path: "stable".as_bytes(),
                modified_at: Some(epoch + std::time::Duration::from_secs(index)),
            })
            .collect::<Vec<_>>();
        assert_eq!(quota_runtime_log_order(&many), Ok((0..1025).collect()));
    }

    #[test]
    fn compiled_quota_runtime_log_order_abi_rejects_invalid_late_row_without_writes() {
        let mut rows = [0_u64; 12];
        let mut output = [91_i64; 2];
        let source = rows.as_ptr() as usize as u64;
        let target = output.as_mut_ptr() as usize as u64;
        for (abi, address, count, capacity, expected) in [
            (2, source, 2, 2, 4),
            (1, 0, 2, 2, 1),
            (1, source, -1, 2, 1),
            (1, source, 65_537, 2, 1),
            (1, source, 2, 1, 1),
        ] {
            assert_eq!(
                unsafe { prodex_quota_runtime_log_order_v1(abi, address, count, target, capacity) },
                expected,
            );
            assert_eq!(output, [91; 2]);
        }
        rows[8] = 2;
        assert_eq!(
            unsafe {
                prodex_quota_runtime_log_order_v1(1, rows.as_ptr() as usize as u64, 2, target, 2)
            },
            1,
        );
        assert_eq!(output, [91; 2]);
        assert_eq!(
            unsafe { prodex_quota_runtime_log_order_v1(1, 0, 0, target, 2) },
            0,
        );
        assert_eq!(output, [91; 2]);
    }
}
