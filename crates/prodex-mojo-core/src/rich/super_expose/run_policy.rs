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

/// Redacted text and host-owned buffer facts for one output chunk.
pub struct SuperRunOutputInput<'a> {
    pub text: &'a str,
    pub output_bytes: usize,
    pub output_truncated: bool,
    pub next_seq: u64,
    pub marker_flags: &'a [bool],
    pub event_is_marker: bool,
    pub event_text_limit: usize,
    pub output_limit: usize,
}

/// Byte prefixes and event mutations selected before any host storage changes.
#[derive(Debug, PartialEq, Eq)]
pub struct SuperRunOutputPlan {
    pub text_end: usize,
    pub append_end: usize,
    pub truncated: bool,
    pub event_push: Option<SuperRunEventPushPlan>,
    pub marker_push: Option<SuperRunEventPushPlan>,
}

#[repr(C)]
struct OutputRow {
    text_address: u64,
    text_length: i64,
    output_bytes: u64,
    output_truncated: i64,
    next_seq: u64,
    markers_address: u64,
    marker_count: i64,
    event_is_marker: i64,
    event_text_limit: u64,
    output_limit: u64,
}

const _: () = {
    assert!(std::mem::size_of::<OutputRow>() == 80);
    assert!(std::mem::align_of::<OutputRow>() == 8);
};

unsafe extern "C" {
    fn prodex_super_run_output_plan_v1(version: i64, input: u64, output: u64) -> i64;
}

/// Plans both output prefixes and post-eviction marker insertion in one compiled call.
pub fn super_run_output_plan(
    input: SuperRunOutputInput<'_>,
) -> Result<SuperRunOutputPlan, MojoError> {
    let marker_count =
        i64::try_from(input.marker_flags.len()).map_err(|_| MojoError::InvalidInput)?;
    if marker_count > i64::MAX / 8 {
        return Err(MojoError::InvalidInput);
    }
    let markers = input
        .marker_flags
        .iter()
        .map(|flag| u8::from(*flag))
        .collect::<Vec<_>>();
    let row = OutputRow {
        text_address: input.text.as_ptr() as u64,
        text_length: i64::try_from(input.text.len()).map_err(|_| MojoError::InvalidInput)?,
        output_bytes: u64::try_from(input.output_bytes).map_err(|_| MojoError::InvalidInput)?,
        output_truncated: i64::from(input.output_truncated),
        next_seq: input.next_seq,
        markers_address: markers.as_ptr() as u64,
        marker_count,
        event_is_marker: i64::from(input.event_is_marker),
        event_text_limit: u64::try_from(input.event_text_limit)
            .map_err(|_| MojoError::InvalidInput)?,
        output_limit: u64::try_from(input.output_limit).map_err(|_| MojoError::InvalidInput)?,
    };
    let mut output = [u64::MAX; 9];
    check_run_status(unsafe {
        prodex_super_run_output_plan_v1(
            1,
            (&row as *const OutputRow) as u64,
            output.as_mut_ptr() as u64,
        )
    })?;
    let text_end = usize::try_from(output[0]).map_err(|_| MojoError::InvalidOutput)?;
    let append_end = usize::try_from(output[1]).map_err(|_| MojoError::InvalidOutput)?;
    if text_end > input.text.len()
        || text_end > input.event_text_limit
        || append_end > text_end
        || append_end > input.output_limit
        || !input.text.is_char_boundary(text_end)
        || !input.text.is_char_boundary(append_end)
    {
        return Err(MojoError::InvalidOutput);
    }
    let flag = |raw| match raw {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    };
    let truncated = flag(output[2])?;
    let emit_output = flag(output[3])?;
    let emit_marker = flag(output[6])?;
    if emit_marker && !emit_output {
        return Err(MojoError::InvalidOutput);
    }
    let drop_front = usize::try_from(output[5]).map_err(|_| MojoError::InvalidOutput)?;
    let marker_drop = usize::try_from(output[8]).map_err(|_| MojoError::InvalidOutput)?;
    if drop_front > input.marker_flags.len()
        || marker_drop > input.marker_flags.len() - drop_front + 1
    {
        return Err(MojoError::InvalidOutput);
    }
    Ok(SuperRunOutputPlan {
        text_end,
        append_end,
        truncated,
        event_push: emit_output.then_some(SuperRunEventPushPlan {
            next_seq: output[4],
            drop_front,
        }),
        marker_push: emit_marker.then_some(SuperRunEventPushPlan {
            next_seq: output[7],
            drop_front: marker_drop,
        }),
    })
}

#[cfg(test)]
#[path = "run_policy_tests.rs"]
mod tests;
