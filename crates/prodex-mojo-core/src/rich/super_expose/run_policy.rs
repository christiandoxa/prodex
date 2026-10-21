//! Checked retention masks for host-owned Super run records.
use crate::MojoError;

/// Run facts in ascending unique run-ID order; active runs are never evicted.
#[derive(Clone, Copy, Debug)]
pub struct SuperRunRetentionInput {
    pub terminal: bool,
    pub created_at: u64,
    pub finished_at: Option<u64>,
}

#[repr(C)]
struct Row {
    terminal: i64,
    created_at: u64,
    finished_at: u64,
    finished_present: i64,
}

const _: () = {
    assert!(std::mem::size_of::<Row>() == 32);
    assert!(std::mem::align_of::<Row>() == 8);
};

unsafe extern "C" {
    fn prodex_super_run_retention_v1(
        version: i64,
        rows: u64,
        count: i64,
        keep: u64,
        keep_capacity: i64,
        scratch: u64,
        scratch_capacity: i64,
    ) -> i64;
}

/// Plans oldest-terminal eviction without moving IDs, output or child handles over the ABI.
pub fn super_run_retention_mask(inputs: &[SuperRunRetentionInput]) -> Result<Vec<bool>, MojoError> {
    let count = i64::try_from(inputs.len()).map_err(|_| MojoError::InvalidInput)?;
    if count > i64::MAX / 32 {
        return Err(MojoError::InvalidInput);
    }
    let rows = inputs
        .iter()
        .map(|input| Row {
            terminal: i64::from(input.terminal),
            created_at: input.created_at,
            finished_at: input.finished_at.unwrap_or_default(),
            finished_present: i64::from(input.finished_at.is_some()),
        })
        .collect::<Vec<_>>();
    let mut keep = vec![u8::MAX; inputs.len()];
    let mut scratch = vec![0_i64; inputs.len()];
    let status = unsafe {
        prodex_super_run_retention_v1(
            1,
            rows.as_ptr() as u64,
            count,
            keep.as_mut_ptr() as u64,
            count,
            scratch.as_mut_ptr() as u64,
            count,
        )
    };
    match status {
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        3 => return Err(MojoError::Capacity),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    keep.into_iter()
        .map(|value| match value {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(MojoError::InvalidOutput),
        })
        .collect()
}

#[cfg(test)]
#[path = "run_policy_tests.rs"]
mod tests;
