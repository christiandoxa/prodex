use crate::MojoError;

const ABI_VERSION: i64 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThroughputSamplePlan {
    pub counter_reset: bool,
    pub append_sample: bool,
}

unsafe extern "C" {
    fn prodex_log_throughput_sample_plan_v1(
        abi_version: i64,
        previous_present: i64,
        previous_tokens: u64,
        previous_generation_ms: u64,
        current_tokens: u64,
        current_generation_ms: u64,
        output_address: u64,
    ) -> i64;

    fn prodex_log_throughput_completed_rate_v1(
        abi_version: i64,
        output_tokens: u64,
        generation_ms: u64,
        valid_address: u64,
        rate_address: u64,
    ) -> i64;

    fn prodex_log_retention_policy_v1(
        abi_version: i64,
        operation: i64,
        input0: u64,
        input1: u64,
        input2: u64,
        input3: u64,
        input4: u64,
        input5: u64,
        output_address: u64,
    ) -> i64;

    fn prodex_log_throughput_stream_rate_v1(
        abi_version: i64,
        first_tokens: u64,
        first_generation_ms: u64,
        last_tokens: u64,
        last_generation_ms: u64,
        valid_address: u64,
        rate_address: u64,
    ) -> i64;
}

fn status(value: i64) -> Result<(), MojoError> {
    match value {
        0 => Ok(()),
        1 => Err(MojoError::InvalidInput),
        4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

fn bool_output(value: i64) -> Result<bool, MojoError> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

const RETENTION_BOUNDED_VALUE: i64 = 1;
const RETENTION_ROTATION: i64 = 2;
const RETENTION_EXPIRED: i64 = 3;
const RETENTION_OVER_BUDGET: i64 = 4;
const RETENTION_BOUNDED_TEXT: i64 = 5;

fn retention_call(operation: i64, input: [u64; 6]) -> Result<[u64; 4], MojoError> {
    let mut output = [0_u64; 4];
    status(unsafe {
        prodex_log_retention_policy_v1(
            ABI_VERSION,
            operation,
            input[0],
            input[1],
            input[2],
            input[3],
            input[4],
            input[5],
            output.as_mut_ptr() as usize as u64,
        )
    })?;
    Ok(output)
}

pub fn bounded_text_policy_value(
    value: Option<&str>,
    default: u64,
    min: u64,
    max: u64,
) -> Result<u64, MojoError> {
    let (address, length, present) = match value {
        Some(value) => (
            value.as_ptr() as usize as u64,
            u64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)?,
            1_u64,
        ),
        None => (0, 0, 0),
    };
    Ok(retention_call(
        RETENTION_BOUNDED_TEXT,
        [address, length, present, default, min, max],
    )?[0])
}

pub fn bounded_policy_value(
    value: Option<u64>,
    default: u64,
    min: u64,
    max: u64,
) -> Result<u64, MojoError> {
    Ok(retention_call(
        RETENTION_BOUNDED_VALUE,
        [
            u64::from(value.is_some()),
            value.unwrap_or_default(),
            default,
            min,
            max,
            0,
        ],
    )?[0])
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LogRotationPlan {
    pub rotate_before_write: bool,
    pub rotate_after_oversized_line: bool,
}

pub fn log_rotation_plan(
    current_size: u64,
    line_len: u64,
    max_file_bytes: u64,
) -> Result<LogRotationPlan, MojoError> {
    let output = retention_call(
        RETENTION_ROTATION,
        [current_size, line_len, max_file_bytes, 0, 0, 0],
    )?;
    Ok(LogRotationPlan {
        rotate_before_write: bool_output(
            i64::try_from(output[0]).map_err(|_| MojoError::InvalidOutput)?,
        )?,
        rotate_after_oversized_line: bool_output(
            i64::try_from(output[1]).map_err(|_| MojoError::InvalidOutput)?,
        )?,
    })
}

fn signed_order_key(value: i64) -> u64 {
    (value as u64) ^ (1_u64 << 63)
}

pub fn log_expired_removal_allowed(
    modified_epoch_seconds: i64,
    oldest_allowed: i64,
    removable: bool,
) -> Result<bool, MojoError> {
    let output = retention_call(
        RETENTION_EXPIRED,
        [
            signed_order_key(modified_epoch_seconds),
            signed_order_key(oldest_allowed),
            u64::from(removable),
            0,
            0,
            0,
        ],
    )?;
    bool_output(i64::try_from(output[0]).map_err(|_| MojoError::InvalidOutput)?)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LogOverBudgetPlan {
    pub within_budget: bool,
    pub remove_current: bool,
}

pub fn log_over_budget_plan(
    remaining_count: usize,
    max_files: usize,
    total_bytes: u64,
    total_budget: u64,
    already_removed: bool,
    removable: bool,
) -> Result<LogOverBudgetPlan, MojoError> {
    let output = retention_call(
        RETENTION_OVER_BUDGET,
        [
            u64::try_from(remaining_count).map_err(|_| MojoError::InvalidInput)?,
            u64::try_from(max_files).map_err(|_| MojoError::InvalidInput)?,
            total_bytes,
            total_budget,
            u64::from(already_removed),
            u64::from(removable),
        ],
    )?;
    Ok(LogOverBudgetPlan {
        within_budget: bool_output(
            i64::try_from(output[0]).map_err(|_| MojoError::InvalidOutput)?,
        )?,
        remove_current: bool_output(
            i64::try_from(output[1]).map_err(|_| MojoError::InvalidOutput)?,
        )?,
    })
}

pub fn sample_plan(
    previous: Option<(u64, u64)>,
    current_tokens: u64,
    current_generation_ms: u64,
) -> Result<ThroughputSamplePlan, MojoError> {
    let (previous_present, previous_tokens, previous_generation_ms) = previous
        .map(|(tokens, generation_ms)| (1_i64, tokens, generation_ms))
        .unwrap_or((0_i64, 0, 0));
    let mut output = [-1_i64; 2];
    status(unsafe {
        prodex_log_throughput_sample_plan_v1(
            ABI_VERSION,
            previous_present,
            previous_tokens,
            previous_generation_ms,
            current_tokens,
            current_generation_ms,
            output.as_mut_ptr() as usize as u64,
        )
    })?;
    Ok(ThroughputSamplePlan {
        counter_reset: bool_output(output[0])?,
        append_sample: bool_output(output[1])?,
    })
}

fn rate_output(status_code: i64, valid: i64, rate: f64) -> Result<Option<f64>, MojoError> {
    status(status_code)?;
    match valid {
        0 => Ok(None),
        1 if rate.is_finite() && rate > 0.0 => Ok(Some(rate)),
        1 => Err(MojoError::InvalidOutput),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn completed_rate(output_tokens: u64, generation_ms: u64) -> Result<Option<f64>, MojoError> {
    let mut valid = -1_i64;
    let mut rate = 0.0_f64;
    let status_code = unsafe {
        prodex_log_throughput_completed_rate_v1(
            ABI_VERSION,
            output_tokens,
            generation_ms,
            (&mut valid as *mut i64) as usize as u64,
            (&mut rate as *mut f64) as usize as u64,
        )
    };
    rate_output(status_code, valid, rate)
}

pub fn stream_rate(
    first_tokens: u64,
    first_generation_ms: u64,
    last_tokens: u64,
    last_generation_ms: u64,
) -> Result<Option<f64>, MojoError> {
    let mut valid = -1_i64;
    let mut rate = 0.0_f64;
    let status_code = unsafe {
        prodex_log_throughput_stream_rate_v1(
            ABI_VERSION,
            first_tokens,
            first_generation_ms,
            last_tokens,
            last_generation_ms,
            (&mut valid as *mut i64) as usize as u64,
            (&mut rate as *mut f64) as usize as u64,
        )
    };
    rate_output(status_code, valid, rate)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn throughput_policy_kernel_preserves_counter_and_rate_contract() {
        assert_eq!(
            sample_plan(Some((100, 1000)), 99, 1100).unwrap(),
            ThroughputSamplePlan {
                counter_reset: true,
                append_sample: true,
            }
        );
        assert_eq!(
            sample_plan(Some((100, 1000)), 100, 1100).unwrap(),
            ThroughputSamplePlan {
                counter_reset: false,
                append_sample: false,
            }
        );
        assert_eq!(
            sample_plan(Some((100, 1000)), 101, 900).unwrap(),
            ThroughputSamplePlan {
                counter_reset: true,
                append_sample: true,
            }
        );
        assert_eq!(completed_rate(0, 1000).unwrap(), None);
        assert_eq!(completed_rate(100, 0).unwrap(), None);
        assert_eq!(completed_rate(100, 2000).unwrap(), Some(50.0));
        assert_eq!(stream_rate(100, 1000, 200, 3000).unwrap(), Some(50.0));
        assert_eq!(stream_rate(100, 1000, 110, 1200).unwrap(), None);
        assert_eq!(stream_rate(200, 1000, 100, 3000).unwrap(), None);
        assert_eq!(stream_rate(100, 3000, 200, 1000).unwrap(), None);
        assert_eq!(bounded_policy_value(Some(9), 5, 1, 8).unwrap(), 5);
        assert_eq!(bounded_policy_value(Some(7), 5, 1, 8).unwrap(), 7);
        assert_eq!(bounded_text_policy_value(None, 5, 1, 8).unwrap(), 5);
        assert_eq!(bounded_text_policy_value(Some("7"), 5, 1, 8).unwrap(), 7);
        assert_eq!(bounded_text_policy_value(Some("+7"), 5, 1, 8).unwrap(), 7);
        assert_eq!(bounded_text_policy_value(Some(" 7"), 5, 1, 8).unwrap(), 5);
        assert_eq!(bounded_text_policy_value(Some("7 "), 5, 1, 8).unwrap(), 5);
        assert_eq!(bounded_text_policy_value(Some("-1"), 5, 1, 8).unwrap(), 5);
        assert_eq!(bounded_text_policy_value(Some(""), 5, 1, 8).unwrap(), 5);
        assert_eq!(bounded_text_policy_value(Some("09"), 5, 1, 10).unwrap(), 9);
        assert_eq!(
            bounded_text_policy_value(Some("18446744073709551615"), 5, 1, u64::MAX).unwrap(),
            u64::MAX
        );
        assert_eq!(
            bounded_text_policy_value(Some("18446744073709551616"), 5, 1, u64::MAX).unwrap(),
            5
        );
        assert_eq!(
            log_rotation_plan(10, 5, 12).unwrap(),
            LogRotationPlan {
                rotate_before_write: true,
                rotate_after_oversized_line: false,
            }
        );
        assert!(log_expired_removal_allowed(10, 20, true).unwrap());
        assert_eq!(
            log_over_budget_plan(6, 5, 100, 200, false, true).unwrap(),
            LogOverBudgetPlan {
                within_budget: false,
                remove_current: true,
            }
        );
    }
}
