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
    check_run_status(status)?;
    keep.into_iter()
        .map(|value| match value {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(MojoError::InvalidOutput),
        })
        .collect()
}

fn check_run_status(value: i64) -> Result<(), MojoError> {
    match value {
        0 => Ok(()),
        1 => Err(MojoError::InvalidInput),
        3 => Err(MojoError::Capacity),
        4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Host mutations selected for one appended event.
#[derive(Debug, PartialEq, Eq)]
pub struct SuperRunEventPushPlan {
    pub next_seq: u64,
    pub drop_front: usize,
}

/// Original event positions selected for a cursor page.
#[derive(Debug, PartialEq, Eq)]
pub struct SuperRunEventPagePlan {
    pub indices: Vec<usize>,
    pub truncated: bool,
}

unsafe extern "C" {
    fn prodex_super_run_event_push_v1(
        version: i64,
        event_count: i64,
        next_seq: u64,
        output: u64,
    ) -> i64;
    fn prodex_super_run_events_page_v1(
        version: i64,
        sequences: u64,
        count: i64,
        after_seq: u64,
        next_seq: u64,
        limit: u64,
        indices: u64,
        indices_capacity: i64,
        output: u64,
    ) -> i64;
}

/// Plans the bounded deque eviction and full-width saturating sequence increment.
pub fn super_run_event_push_plan(
    event_count: usize,
    next_seq: u64,
) -> Result<SuperRunEventPushPlan, MojoError> {
    let mut output = [0_u64; 2];
    check_run_status(unsafe {
        prodex_super_run_event_push_v1(
            1,
            i64::try_from(event_count).map_err(|_| MojoError::InvalidInput)?,
            next_seq,
            output.as_mut_ptr() as u64,
        )
    })?;
    let drop_front = usize::try_from(output[1]).map_err(|_| MojoError::InvalidOutput)?;
    if drop_front > event_count {
        return Err(MojoError::InvalidOutput);
    }
    Ok(SuperRunEventPushPlan {
        next_seq: output[0],
        drop_front,
    })
}

/// Selects a page in original deque order, including gaps and saturated sequence ties.
pub fn super_run_events_page_plan(
    sequences: &[u64],
    after_seq: u64,
    next_seq: u64,
    limit: usize,
) -> Result<SuperRunEventPagePlan, MojoError> {
    let count = i64::try_from(sequences.len()).map_err(|_| MojoError::InvalidInput)?;
    if count > i64::MAX / 8 {
        return Err(MojoError::InvalidInput);
    }
    let mut raw_indices = vec![-1_i64; sequences.len()];
    let mut output = [-1_i64; 2];
    check_run_status(unsafe {
        prodex_super_run_events_page_v1(
            1,
            sequences.as_ptr() as u64,
            count,
            after_seq,
            next_seq,
            u64::try_from(limit).map_err(|_| MojoError::InvalidInput)?,
            raw_indices.as_mut_ptr() as u64,
            count,
            output.as_mut_ptr() as u64,
        )
    })?;
    let written = usize::try_from(output[0]).map_err(|_| MojoError::InvalidOutput)?;
    if written > raw_indices.len() {
        return Err(MojoError::InvalidOutput);
    }
    let truncated = match output[1] {
        0 => false,
        1 => true,
        _ => return Err(MojoError::InvalidOutput),
    };
    let mut indices = Vec::with_capacity(written);
    for raw in raw_indices.into_iter().take(written) {
        let index = usize::try_from(raw).map_err(|_| MojoError::InvalidOutput)?;
        if index >= sequences.len() || indices.last().is_some_and(|previous| *previous >= index) {
            return Err(MojoError::InvalidOutput);
        }
        indices.push(index);
    }
    Ok(SuperRunEventPagePlan { indices, truncated })
}

#[cfg(test)]
#[path = "run_policy_tests.rs"]
mod tests;
