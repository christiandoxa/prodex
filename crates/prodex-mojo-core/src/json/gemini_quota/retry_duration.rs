//! Gemini retry text plans with host-only standard float acquisition.
use super::{MojoError, signed, status};

#[repr(i64)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeminiRetryTextKind {
    Duration,
    Message,
    HeaderSeconds,
}

unsafe extern "C" {
    fn prodex_gemini_retry_text_plan_v1(
        version: i64,
        operation: i64,
        address: u64,
        length: i64,
        output_address: u64,
        capacity: i64,
    ) -> i64;
    fn prodex_gemini_retry_text_finish_v1(
        version: i64,
        count: i64,
        parsed: u64,
        first: f64,
        second: f64,
        first_multiplier: u64,
        second_multiplier: u64,
        output_address: u64,
        capacity: i64,
    ) -> i64;
}

/// Uses Mojo's grammar, marker precedence, positivity, rounding and saturation.
/// Rust acquires float values because the pinned Mojo text parser requires
/// CompilerRT heap/error symbols unavailable in the native object ABI.
pub fn gemini_retry_text_delay_ms(
    kind: GeminiRetryTextKind,
    text: &str,
) -> Result<Option<u64>, MojoError> {
    let mut plan = [u64::MAX; 9];
    status(unsafe {
        prodex_gemini_retry_text_plan_v1(
            1,
            kind as i64,
            text.as_ptr() as u64,
            signed(text.len())?,
            plan.as_mut_ptr() as u64,
            9,
        )
    })?;
    if plan == [0; 9] {
        return Ok(None);
    }
    if let [1, value, 0, 0, 0, 0, 0, 0, 0] = plan {
        return (value > 0)
            .then_some(Some(value))
            .ok_or(MojoError::InvalidOutput);
    }
    let [2, 0, count @ 1..=2, ..] = plan else {
        return Err(MojoError::InvalidOutput);
    };
    if count == 1 && plan[6..] != [0; 3] {
        return Err(MojoError::InvalidOutput);
    }
    let mut parsed = 0_u64;
    let mut values = [0.0; 2];
    for index in 0..count as usize {
        let start = usize::try_from(plan[3 + index * 3]).map_err(|_| MojoError::InvalidOutput)?;
        let end = usize::try_from(plan[4 + index * 3]).map_err(|_| MojoError::InvalidOutput)?;
        if !matches!(plan[5 + index * 3], 1 | 1000) || start >= end {
            return Err(MojoError::InvalidOutput);
        }
        let number = text.get(start..end).ok_or(MojoError::InvalidOutput)?;
        // Float64(StringSlice) imports unsupported CompilerRT allocation/error
        // symbols. This is acquisition only; Mojo decides which value is usable.
        if let Ok(value) = number.parse::<f64>() {
            values[index] = value;
            parsed |= 1 << index;
        }
    }
    let mut result = [u64::MAX; 2];
    status(unsafe {
        prodex_gemini_retry_text_finish_v1(
            1,
            count as i64,
            parsed,
            values[0],
            values[1],
            plan[5],
            plan[8],
            result.as_mut_ptr() as u64,
            2,
        )
    })?;
    match result {
        [0, 0] => Ok(None),
        [1, value] if value > 0 => Ok(Some(value)),
        _ => Err(MojoError::InvalidOutput),
    }
}

#[cfg(test)]
#[path = "retry_duration_tests.rs"]
mod tests;
