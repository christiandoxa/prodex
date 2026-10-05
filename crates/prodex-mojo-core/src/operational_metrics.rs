use crate::MojoError;

const OPERATIONAL_METRICS_ABI_VERSION: i64 = 1;
const MAX_HISTOGRAM_BUCKETS: usize = 16;

unsafe extern "C" {
    fn prodex_mojo_operational_histogram_bounds_v1(
        abi_version: i64,
        name_address: u64,
        name_length: i64,
        output_address: u64,
        output_capacity: i64,
        output_count_address: u64,
    ) -> i64;
}

/// Plans bounded operational histogram bucket limits from a metric name.
pub fn histogram_bucket_bounds(metric_name: &str) -> Result<Vec<u64>, MojoError> {
    let name_length = i64::try_from(metric_name.len()).map_err(|_| MojoError::InvalidInput)?;
    let mut output = [0_i64; MAX_HISTOGRAM_BUCKETS];
    let mut output_count = -1_i64;
    let status = unsafe {
        prodex_mojo_operational_histogram_bounds_v1(
            OPERATIONAL_METRICS_ABI_VERSION,
            metric_name.as_ptr() as usize as u64,
            name_length,
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut output_count as *mut i64) as usize as u64,
        )
    };
    match status {
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        2 => return Err(MojoError::Capacity),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    let output_count = usize::try_from(output_count)
        .ok()
        .filter(|count| (1..=output.len()).contains(count))
        .ok_or(MojoError::InvalidOutput)?;
    output[..output_count]
        .iter()
        .map(|bound| u64::try_from(*bound).map_err(|_| MojoError::InvalidOutput))
        .collect()
}

unsafe extern "C" {
    fn prodex_mojo_operational_histogram_observe_v1(
        abi_version: i64,
        observation: u64,
        bounds_address: u64,
        counts_address: u64,
        bucket_count: i64,
        count_address: u64,
        sum_address: u64,
    ) -> i64;
}

/// Updates cumulative histogram buckets, count, and sum with saturating arithmetic.
/// Bounds and counts must have equal lengths of at most 16; errors leave state unchanged.
pub fn observe_histogram(
    observation: u64,
    bounds: &[u64],
    counts: &mut [u64],
    count: &mut u64,
    sum: &mut u64,
) -> Result<(), MojoError> {
    if bounds.len() != counts.len() || bounds.len() > MAX_HISTOGRAM_BUCKETS {
        return Err(MojoError::InvalidInput);
    }
    let status = unsafe {
        prodex_mojo_operational_histogram_observe_v1(
            OPERATIONAL_METRICS_ABI_VERSION,
            observation,
            bounds.as_ptr() as usize as u64,
            counts.as_mut_ptr() as usize as u64,
            i64::try_from(bounds.len()).map_err(|_| MojoError::InvalidInput)?,
            (count as *mut u64) as usize as u64,
            (sum as *mut u64) as usize as u64,
        )
    };
    match status {
        0 => Ok(()),
        1 => Err(MojoError::InvalidInput),
        4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}
