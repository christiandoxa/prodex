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
    }
}
