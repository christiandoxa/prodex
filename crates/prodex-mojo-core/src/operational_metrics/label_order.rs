use super::OPERATIONAL_METRICS_ABI_VERSION;
use crate::MojoError;

unsafe extern "C" {
    fn prodex_mojo_operational_metric_label_order_v1(
        abi_version: i64,
        fields_address: u64,
        label_count: i64,
        indices_address: u64,
        indices_capacity: i64,
    ) -> i64;
}

/// Original indices for stable key-then-value UTF-8 metric-label ordering.
/// The result is a complete permutation; duplicate pairs remain separate.
/// Accepts at most 65,536 pairs with each view within the telemetry byte limit.
pub fn operational_metric_label_order(labels: &[(&str, &str)]) -> Result<Vec<usize>, MojoError> {
    if labels.len() > 65_536 {
        return Err(MojoError::InvalidInput);
    }
    let fields: Vec<_> = labels
        .iter()
        .flat_map(|(key, value)| [key, value].map(|text| [text.as_ptr() as u64, text.len() as u64]))
        .collect();
    let mut indices = vec![-1_i64; labels.len()];
    match unsafe {
        prodex_mojo_operational_metric_label_order_v1(
            OPERATIONAL_METRICS_ABI_VERSION,
            fields.as_ptr() as u64,
            labels.len() as i64,
            indices.as_mut_ptr() as u64,
            indices.len() as i64,
        )
    } {
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        2 => return Err(MojoError::Capacity),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    let mut seen = vec![false; labels.len()];
    indices
        .into_iter()
        .map(|raw| {
            let index = usize::try_from(raw).map_err(|_| MojoError::InvalidOutput)?;
            let used = seen.get_mut(index).ok_or(MojoError::InvalidOutput)?;
            if *used {
                return Err(MojoError::InvalidOutput);
            }
            *used = true;
            Ok(index)
        })
        .collect()
}

#[cfg(test)]
#[path = "label_order_tests.rs"]
mod tests;
