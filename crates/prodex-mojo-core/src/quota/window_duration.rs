//! Checked scalar ABI for OpenAI quota window duration normalization.
//! Serde owns option acquisition; compiled Mojo owns precedence and math.
use crate::MojoError;

unsafe extern "C" {
    fn prodex_quota_window_duration_seconds_v1(
        abi_version: i64,
        seconds_present: i64,
        seconds: i64,
        minutes_present: i64,
        minutes: i64,
        output_address: u64,
    ) -> i64;
}

/// Normalize upstream window durations using the compiled quota policy.
pub fn quota_window_duration_seconds(
    seconds: Option<i64>,
    minutes: Option<i64>,
) -> Result<Option<i64>, MojoError> {
    let mut output = [-1_i64; 2];
    match unsafe {
        prodex_quota_window_duration_seconds_v1(
            1,
            i64::from(seconds.is_some()),
            seconds.unwrap_or_default(),
            i64::from(minutes.is_some()),
            minutes.unwrap_or_default(),
            output.as_mut_ptr() as usize as u64,
        )
    } {
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    match output {
        [0, 0] => Ok(None),
        [1, value] => Ok(Some(value)),
        _ => Err(MojoError::InvalidOutput),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiled_quota_window_duration_preserves_seconds_presence_and_checked_multiplication() {
        for (seconds, minutes, expected) in [
            (None, None, None),
            (Some(0), Some(10), Some(0)),
            (Some(-1), Some(10), Some(-1)),
            (Some(i64::MIN), Some(i64::MAX), Some(i64::MIN)),
            (Some(i64::MAX), None, Some(i64::MAX)),
            (None, Some(0), Some(0)),
            (None, Some(-1), Some(-60)),
            (None, Some(300), Some(18_000)),
            (None, Some(10_080), Some(604_800)),
            (None, Some(153722867280912930), Some(9223372036854775800)),
            (None, Some(-153722867280912930), Some(-9223372036854775800)),
            (None, Some(153722867280912931), None),
            (None, Some(-153722867280912931), None),
            (None, Some(i64::MIN), None),
            (None, Some(i64::MAX), None),
        ] {
            assert_eq!(
                quota_window_duration_seconds(seconds, minutes),
                Ok(expected),
                "seconds={seconds:?}, minutes={minutes:?}",
            );
        }
    }

    #[test]
    fn compiled_quota_window_duration_abi_rejects_bad_flags_version_and_missing_result() {
        let mut output = [77_i64; 2];
        let dest = output.as_mut_ptr() as usize as u64;
        for (abi, seconds_present, minutes_present, ptr, expected) in [
            (2, 1, 0, dest, 4),
            (1, -1, 0, dest, 1),
            (1, 2, 0, dest, 1),
            (1, 0, 2, dest, 1),
            (1, 0, -1, dest, 1),
            (1, 1, 1, 0, 1),
        ] {
            assert_eq!(
                unsafe {
                    prodex_quota_window_duration_seconds_v1(
                        abi,
                        seconds_present,
                        18_000,
                        minutes_present,
                        300,
                        ptr,
                    )
                },
                expected,
            );
            assert_eq!(output, [77; 2]);
        }
    }
}
