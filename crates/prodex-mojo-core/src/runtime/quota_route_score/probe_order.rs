//! Cold-start probe ordering from acquired quota summaries.
use super::QuotaSummarySortKeyInput;
use crate::MojoError;

#[derive(Clone, Copy, Debug)]
pub struct QuotaProbeOrderInput {
    pub original_order: usize,
    pub summary: Option<QuotaSummarySortKeyInput>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum QuotaProbeOrderPlan {
    KeepInput,
    Reorder(Vec<usize>),
}

#[repr(C)]
struct Row {
    original_order: u64,
    present: i64,
    fields: [i64; 7],
}
const _: () = assert!(std::mem::size_of::<Row>() == 72);

unsafe extern "C" {
    fn prodex_runtime_quota_probe_order_v1(
        version: i64,
        rows: u64,
        count: i64,
        scratch: u64,
        scratch_words: i64,
        output: u64,
        output_words: i64,
    ) -> i64;
}

/// Acquires facts only when Mojo needs to reorder two or more probes.
/// IDs, provider values and filesystem paths remain owned by the caller.
pub fn quota_probe_order(
    inputs: impl ExactSizeIterator<Item = QuotaProbeOrderInput>,
) -> Result<QuotaProbeOrderPlan, MojoError> {
    let count = inputs.len();
    let count_abi = i64::try_from(count).map_err(|_| MojoError::InvalidInput)?;
    match unsafe { prodex_runtime_quota_probe_order_v1(1, 0, count_abi, 0, 0, 0, 0) } {
        0 if count <= 1 => return Ok(QuotaProbeOrderPlan::KeepInput),
        5 if count > 1 => {}
        -1 => return Err(MojoError::InvalidInput),
        -3 => return Err(MojoError::Capacity),
        -4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    let rows = inputs
        .map(|input| {
            Ok(Row {
                original_order: u64::try_from(input.original_order)
                    .map_err(|_| MojoError::InvalidInput)?,
                present: i64::from(input.summary.is_some()),
                fields: input.summary.map_or([0; 7], |summary| {
                    [
                        summary.route_band,
                        summary.weekly_status,
                        summary.five_hour_status,
                        summary.weekly_remaining,
                        summary.five_hour_remaining,
                        summary.weekly_reset_at,
                        summary.five_hour_reset_at,
                    ]
                }),
            })
        })
        .collect::<Result<Vec<_>, MojoError>>()?;
    if rows.len() != count {
        return Err(MojoError::InvalidInput);
    }
    let scratch_words = count.checked_mul(10).ok_or(MojoError::Capacity)?;
    let mut scratch = vec![0_i64; scratch_words];
    let mut output = vec![-1_i64; count];
    match unsafe {
        prodex_runtime_quota_probe_order_v1(
            1,
            rows.as_ptr() as u64,
            count_abi,
            scratch.as_mut_ptr() as u64,
            i64::try_from(scratch_words).map_err(|_| MojoError::Capacity)?,
            output.as_mut_ptr() as u64,
            count_abi,
        )
    } {
        0 => {}
        -1 => return Err(MojoError::InvalidInput),
        -3 => return Err(MojoError::Capacity),
        -4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    let mut seen = vec![false; count];
    let indices = output
        .into_iter()
        .map(|value| {
            let index = usize::try_from(value).map_err(|_| MojoError::InvalidOutput)?;
            if index >= count || std::mem::replace(&mut seen[index], true) {
                return Err(MojoError::InvalidOutput);
            }
            Ok(index)
        })
        .collect::<Result<_, _>>()?;
    Ok(QuotaProbeOrderPlan::Reorder(indices))
}

#[cfg(test)]
#[path = "probe_order_tests.rs"]
mod tests;
