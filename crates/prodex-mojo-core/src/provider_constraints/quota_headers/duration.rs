//! Quota duration grammar and arithmetic; Rust only acquires standard floats.
use super::{ABI_VERSION, len, status};
use crate::MojoError;

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Number {
    start: i64,
    end: i64,
    unit: i64,
}

#[repr(C)]
struct AcquiredNumber {
    unit: i64,
    parsed: i64,
    amount: f64,
}
const _: () = assert!(std::mem::size_of::<Number>() == 24);
const _: () = assert!(std::mem::size_of::<AcquiredNumber>() == 24);

unsafe extern "C" {
    fn prodex_provider_quota_duration_plan_v1(
        version: i64,
        address: u64,
        length: i64,
        numbers_address: u64,
        numbers_capacity: i64,
        rows_address: u64,
        rows_capacity: i64,
        result_address: u64,
    ) -> i64;
    fn prodex_provider_quota_duration_finish_v1(
        version: i64,
        values_address: u64,
        count: i64,
        output_address: u64,
        output_capacity: i64,
    ) -> i64;
}

/// Mojo owns whitespace, unit grammar, summation, ceiling and saturation.
/// Standard float acquisition remains native because the pinned Mojo parser
/// requires CompilerRT heap/error symbols unavailable in the object ABI.
pub fn provider_quota_duration_seconds(text: &str) -> Result<Option<i64>, MojoError> {
    let length = len(text)?;
    let mut plan = [-1_i64; 3];
    status(unsafe {
        prodex_provider_quota_duration_plan_v1(
            ABI_VERSION,
            text.as_ptr() as u64,
            length,
            0,
            0,
            0,
            0,
            plan.as_mut_ptr() as u64,
        )
    })?;
    if plan == [0; 3] {
        return Ok(None);
    }
    let [1, count, bytes] = plan else {
        return Err(MojoError::InvalidOutput);
    };
    if count <= 0 || count > bytes || bytes > length {
        return Err(MojoError::InvalidOutput);
    }
    let count_native = usize::try_from(count).map_err(|_| MojoError::InvalidOutput)?;
    let mut numbers = vec![0_u8; bytes as usize];
    let mut rows = vec![Number::default(); count_native];
    let mut written = [-1_i64; 3];
    status(unsafe {
        prodex_provider_quota_duration_plan_v1(
            ABI_VERSION,
            text.as_ptr() as u64,
            length,
            numbers.as_mut_ptr() as u64,
            bytes,
            rows.as_mut_ptr() as u64,
            count,
            written.as_mut_ptr() as u64,
        )
    })?;
    if written != plan {
        return Err(MojoError::InvalidOutput);
    }
    let numbers = std::str::from_utf8(&numbers).map_err(|_| MojoError::InvalidOutput)?;
    let acquired = rows
        .into_iter()
        .map(|row| {
            let start = usize::try_from(row.start).map_err(|_| MojoError::InvalidOutput)?;
            let end = usize::try_from(row.end).map_err(|_| MojoError::InvalidOutput)?;
            if start >= end || !(0..=4).contains(&row.unit) {
                return Err(MojoError::InvalidOutput);
            }
            let text = numbers.get(start..end).ok_or(MojoError::InvalidOutput)?;
            let amount = text.parse::<f64>().ok();
            Ok(AcquiredNumber {
                unit: row.unit,
                parsed: i64::from(amount.is_some()),
                amount: amount.unwrap_or(0.0),
            })
        })
        .collect::<Result<Vec<_>, MojoError>>()?;
    let mut output = [-1_i64; 2];
    status(unsafe {
        prodex_provider_quota_duration_finish_v1(
            ABI_VERSION,
            acquired.as_ptr() as u64,
            count,
            output.as_mut_ptr() as u64,
            2,
        )
    })?;
    match output {
        [0, 0] => Ok(None),
        [1, seconds] if seconds >= 0 => Ok(Some(seconds)),
        _ => Err(MojoError::InvalidOutput),
    }
}

#[cfg(test)]
#[path = "duration_tests.rs"]
mod tests;
