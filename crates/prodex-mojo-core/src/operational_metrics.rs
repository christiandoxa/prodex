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
