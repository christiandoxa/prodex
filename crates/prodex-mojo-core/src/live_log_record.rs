use crate::MojoError;

const ABI_VERSION: i64 = 1;
const MAX_RECORD_BYTES: usize = 128 * 1024;
pub const FOLLOW_MAX_PENDING_BYTES: usize = 1024 * 1024;
pub const FOLLOW_MAX_LINE_BYTES: usize = 2 * 1024 * 1024;
const FOLLOW_MAX_INPUT_LINES: usize = 2 * 1024 * 1024;

const _: () = assert!(std::mem::size_of::<usize>() == std::mem::size_of::<u64>());

unsafe extern "C" {
    fn prodex_live_log_retention_trim_v1(
        abi_version: i64,
        scope: i64,
        entries: u64,
        bytes: u64,
        paths: u64,
        output_address: u64,
    ) -> i64;
    fn prodex_live_log_oldest_path_v1(
        abi_version: i64,
        sequences_address: u64,
        sequence_count: i64,
        output_address: u64,
    ) -> i64;
    fn prodex_live_log_record_over_limit_v1(
        abi_version: i64,
        record_length: i64,
        output_address: u64,
    ) -> i64;
    fn prodex_live_log_string_clip_end_v1(
        abi_version: i64,
        input_address: u64,
        input_length: i64,
        output_address: u64,
    ) -> i64;
    fn prodex_live_log_json_plan_v1(
        abi_version: i64,
        serialized_length: i64,
        output_address: u64,
    ) -> i64;
    fn prodex_live_log_plain_text_truncate_v1(
        abi_version: i64,
        input_address: u64,
        input_length: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
    ) -> i64;
    fn prodex_live_log_snapshot_plan_v1(
        abi_version: i64,
        sequences_address: u64,
        sequence_count: i64,
        after: u64,
        limit: i64,
        max_entries: i64,
        next_sequence: u64,
        dropped: u64,
        output_address: u64,
    ) -> i64;
    fn prodex_live_log_follow_plan_v1(
        abi_version: i64,
        line_lengths_address: u64,
        line_count: i64,
        pending_length: i64,
        keep_address: u64,
        dropped_address: u64,
        pending_gap_address: u64,
    ) -> i64;
}

fn status(value: i64) -> Result<(), MojoError> {
    match value {
        0 => Ok(()),
        1 => Err(MojoError::InvalidInput),
        2 => Err(MojoError::Capacity),
        4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

fn input_length(length: usize) -> Result<i64, MojoError> {
    i64::try_from(length).map_err(|_| MojoError::InvalidInput)
}

fn pointer_address<T>(pointer: *const T) -> u64 {
    pointer as usize as u64
}

fn mutable_pointer_address<T>(pointer: *mut T) -> u64 {
    pointer as usize as u64
}

/// In-memory retention scope for the shared compiled live-log policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum LiveLogRetentionScope {
    Path = 0,
    Store = 1,
}

/// Decide whether a bounded path or the global store requires eviction.
/// The Rust host retains the mutex, collections, and deletion effects.
pub fn retention_trim_required(
    scope: LiveLogRetentionScope,
    entries: usize,
    bytes: usize,
    paths: usize,
) -> Result<bool, MojoError> {
    let mut decision = -1_i64;
    status(unsafe {
        prodex_live_log_retention_trim_v1(
            ABI_VERSION,
            scope as i64,
            u64::try_from(entries).map_err(|_| MojoError::InvalidInput)?,
            u64::try_from(bytes).map_err(|_| MojoError::InvalidInput)?,
            u64::try_from(paths).map_err(|_| MojoError::InvalidInput)?,
            mutable_pointer_address(&mut decision as *mut i64),
        )
    })?;
    match decision {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Choose the least-recently updated path; ties preserve host key order.
pub fn oldest_retention_path(sequences: &[u64]) -> Result<usize, MojoError> {
    let mut index = -1_i64;
    status(unsafe {
        prodex_live_log_oldest_path_v1(
            ABI_VERSION,
            pointer_address(sequences.as_ptr()),
            input_length(sequences.len())?,
            mutable_pointer_address(&mut index as *mut i64),
        )
    })?;
    let index = usize::try_from(index).map_err(|_| MojoError::InvalidOutput)?;
    if index >= sequences.len() {
        return Err(MojoError::InvalidOutput);
    }
    Ok(index)
}

/// Whether a live-log record exceeds the Mojo-owned record limit.
pub fn record_exceeds_bound(length: usize) -> Result<bool, MojoError> {
    let mut output = [-1_i64];
    status(unsafe {
        prodex_live_log_record_over_limit_v1(
            ABI_VERSION,
            input_length(length)?,
            mutable_pointer_address(output.as_mut_ptr()),
        )
    })?;
    match output[0] {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Returns Mojo's UTF-8-safe nested-string prefix boundary.
pub fn nested_string_clip_end(value: &str) -> Result<usize, MojoError> {
    let mut output = [-1_i64];
    status(unsafe {
        prodex_live_log_string_clip_end_v1(
            ABI_VERSION,
            pointer_address(value.as_ptr()),
            input_length(value.len())?,
            mutable_pointer_address(output.as_mut_ptr()),
        )
    })?;
    let end = usize::try_from(output[0]).map_err(|_| MojoError::InvalidOutput)?;
    if end > value.len() || !value.is_char_boundary(end) {
        return Err(MojoError::InvalidOutput);
    }
    Ok(end)
}

/// Selected JSON fields to materialize if Mojo chooses compact metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LiveLogJsonPlan {
    /// Whether the clipped JSON exceeds the live-log record limit.
    pub compact_metadata: bool,
    /// Whether to preserve the top-level `timestamp` value.
    pub timestamp: bool,
    /// Whether to preserve the top-level `pid` value.
    pub pid: bool,
    /// Whether to preserve the top-level `event` value.
    pub event: bool,
}

/// Chooses full JSON or the compact metadata fallback from serialized bytes.
pub fn json_plan(serialized_length: usize) -> Result<LiveLogJsonPlan, MojoError> {
    let mut output = [-1_i64; 4];
    status(unsafe {
        prodex_live_log_json_plan_v1(
            ABI_VERSION,
            input_length(serialized_length)?,
            mutable_pointer_address(output.as_mut_ptr()),
        )
    })?;
    let compact_metadata = match output[0] {
        0 => false,
        1 => true,
        _ => return Err(MojoError::InvalidOutput),
    };
    let mut selected = [false; 3];
    for (index, value) in output[1..].iter().copied().enumerate() {
        selected[index] = match value {
            0 => false,
            1 => true,
            _ => return Err(MojoError::InvalidOutput),
        };
    }
    if selected
        .iter()
        .any(|selected| *selected != compact_metadata)
    {
        return Err(MojoError::InvalidOutput);
    }
    Ok(LiveLogJsonPlan {
        compact_metadata,
        timestamp: selected[0],
        pid: selected[1],
        event: selected[2],
    })
}

/// Truncates oversized plain text and materializes Mojo's marker and newline.
pub fn truncate_plain_text(value: &str) -> Result<String, MojoError> {
    let mut output = vec![0_u8; MAX_RECORD_BYTES];
    let mut written = [-1_i64];
    status(unsafe {
        prodex_live_log_plain_text_truncate_v1(
            ABI_VERSION,
            pointer_address(value.as_ptr()),
            input_length(value.len())?,
            output.as_mut_ptr() as usize as u64,
            input_length(output.len())?,
            mutable_pointer_address(written.as_mut_ptr()),
        )
    })?;
    let written = usize::try_from(written[0]).map_err(|_| MojoError::InvalidOutput)?;
    let output = output.get(..written).ok_or(MojoError::InvalidOutput)?;
    String::from_utf8(output.to_vec()).map_err(|_| MojoError::InvalidOutput)
}

/// The bounded page selected from a live-log sequence list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LiveLogSnapshotPlan {
    /// Index of the first entry whose sequence is greater than the cursor.
    pub start: usize,
    /// Number of entries to materialize from `start`.
    pub count: usize,
    /// Cursor to return after this page.
    pub cursor: u64,
    /// Number of entries dropped from the path's bounded history.
    pub dropped: u64,
}

/// Mojo's bounded classification for one file-follow read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveLogFollowPlan {
    /// Whether each complete line can be materialized for the viewer.
    pub keep_lines: Vec<bool>,
    /// Number of complete lines dropped for exceeding the line bound.
    pub dropped_lines: usize,
    /// Whether the remaining partial line must be discarded as oversized.
    pub pending_gap: bool,
}

/// Classifies complete lines and pending-buffer overflow without crossing log text into Mojo.
pub fn follow_line_plan(
    line_lengths: &[usize],
    pending_length: usize,
) -> Result<LiveLogFollowPlan, MojoError> {
    if line_lengths.len() > FOLLOW_MAX_INPUT_LINES {
        return Err(MojoError::InvalidInput);
    }
    let line_lengths = line_lengths
        .iter()
        .copied()
        .map(input_length)
        .collect::<Result<Vec<_>, _>>()?;
    let line_count = input_length(line_lengths.len())?;
    let mut keep = vec![0_i64; line_lengths.len()];
    let mut dropped = [-1_i64];
    let mut pending_gap = [-1_i64];
    status(unsafe {
        prodex_live_log_follow_plan_v1(
            ABI_VERSION,
            pointer_address(line_lengths.as_ptr()),
            line_count,
            input_length(pending_length)?,
            mutable_pointer_address(keep.as_mut_ptr()),
            mutable_pointer_address(dropped.as_mut_ptr()),
            mutable_pointer_address(pending_gap.as_mut_ptr()),
        )
    })?;
    let keep_lines = keep
        .into_iter()
        .map(|value| match value {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(MojoError::InvalidOutput),
        })
        .collect::<Result<Vec<_>, _>>()?;
    let dropped_lines = usize::try_from(dropped[0]).map_err(|_| MojoError::InvalidOutput)?;
    if dropped_lines > keep_lines.len()
        || dropped_lines != keep_lines.iter().filter(|keep_line| !**keep_line).count()
    {
        return Err(MojoError::InvalidOutput);
    }
    let pending_gap = match pending_gap[0] {
        0 => false,
        1 => true,
        _ => return Err(MojoError::InvalidOutput),
    };
    Ok(LiveLogFollowPlan {
        keep_lines,
        dropped_lines,
        pending_gap,
    })
}

/// Plans a bounded page after a cursor without crossing the ABI with log text.
pub fn snapshot_plan(
    sequences: &[u64],
    after: u64,
    limit: usize,
    max_entries: usize,
    next_sequence: u64,
    dropped: u64,
) -> Result<LiveLogSnapshotPlan, MojoError> {
    let sequence_count = input_length(sequences.len())?;
    let limit = i64::try_from(limit).unwrap_or(i64::MAX);
    let max_entries_abi = input_length(max_entries)?;
    let mut output = [u64::MAX; 4];
    status(unsafe {
        prodex_live_log_snapshot_plan_v1(
            ABI_VERSION,
            pointer_address(sequences.as_ptr()),
            sequence_count,
            after,
            limit,
            max_entries_abi,
            next_sequence,
            dropped,
            mutable_pointer_address(output.as_mut_ptr()),
        )
    })?;
    let start = usize::try_from(output[0]).map_err(|_| MojoError::InvalidOutput)?;
    let count = usize::try_from(output[1]).map_err(|_| MojoError::InvalidOutput)?;
    if start > sequences.len()
        || count > sequences.len().saturating_sub(start)
        || count > max_entries
    {
        return Err(MojoError::InvalidOutput);
    }
    let expected_cursor = sequences
        .get(start..start.saturating_add(count))
        .and_then(|page| page.last().copied())
        .unwrap_or(next_sequence);
    if output[2] != expected_cursor || output[3] != dropped {
        return Err(MojoError::InvalidOutput);
    }
    Ok(LiveLogSnapshotPlan {
        start,
        count,
        cursor: output[2],
        dropped: output[3],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiled_live_log_retention_bounds_keep_exact_thresholds_and_ties() {
        use LiveLogRetentionScope::{Path, Store};
        for (scope, entries, bytes, paths, expected) in [
            (Path, 512, 2 * 1024 * 1024, 100, false),
            (Path, 513, 0, 0, true),
            (Path, 0, 2 * 1024 * 1024 + 1, 0, true),
            (Store, 2048, 8 * 1024 * 1024, 64, false),
            (Store, 2049, 0, 0, true),
            (Store, 0, 8 * 1024 * 1024 + 1, 0, true),
            (Store, 0, 0, 65, true),
            (Store, usize::MAX, usize::MAX, usize::MAX, true),
        ] {
            assert_eq!(
                retention_trim_required(scope, entries, bytes, paths).unwrap(),
                expected
            );
        }
        assert_eq!(oldest_retention_path(&[12, 7, 7, 9]).unwrap(), 1);
        assert_eq!(oldest_retention_path(&[u64::MAX, 0, 1]).unwrap(), 1);
        assert_eq!(oldest_retention_path(&[7]).unwrap(), 0);
        assert_eq!(oldest_retention_path(&[]), Err(MojoError::InvalidInput));
        assert_eq!(
            oldest_retention_path(&vec![1_u64; 66]),
            Err(MojoError::InvalidInput)
        );
    }

    #[test]
    fn compiled_live_log_retention_abi_rejects_bad_input_before_output_writes() {
        let mut decision = 91_i64;
        let result = (&mut decision as *mut i64) as usize as u64;
        for (abi, scope, address, expected) in [
            (2, 0, result, 4),
            (1, -1, result, 1),
            (1, 2, result, 1),
            (1, 0, 0, 1),
        ] {
            assert_eq!(
                unsafe { prodex_live_log_retention_trim_v1(abi, scope, 512, 0, 0, address) },
                expected
            );
            assert_eq!(decision, 91);
        }
        let mut oldest = 72_i64;
        let output = (&mut oldest as *mut i64) as usize as u64;
        for (abi, count, sequences, expected) in
            [(2, 1, 8, 4), (1, 0, 8, 1), (1, 66, 8, 1), (1, 1, 0, 1)]
        {
            assert_eq!(
                unsafe { prodex_live_log_oldest_path_v1(abi, sequences, count, output) },
                expected
            );
            assert_eq!(oldest, 72);
        }
    }

    #[test]
    fn runtime_live_log_mojo_adapter_checks_bounds_and_selected_metadata() {
        assert!(!record_exceeds_bound(MAX_RECORD_BYTES).unwrap());
        assert!(record_exceeds_bound(MAX_RECORD_BYTES + 1).unwrap());
        assert_eq!(
            json_plan(MAX_RECORD_BYTES).unwrap(),
            LiveLogJsonPlan {
                compact_metadata: false,
                timestamp: false,
                pid: false,
                event: false,
            }
        );
        assert_eq!(
            json_plan(MAX_RECORD_BYTES + 1).unwrap(),
            LiveLogJsonPlan {
                compact_metadata: true,
                timestamp: true,
                pid: true,
                event: true,
            }
        );
    }

    #[test]
    fn runtime_live_log_mojo_adapter_returns_utf8_clip_boundaries() {
        let ascii = "a".repeat(8 * 1024 + 1);
        assert_eq!(nested_string_clip_end(&ascii).unwrap(), 8 * 1024);
        let unicode = format!("{}é-tail", "a".repeat(8 * 1024 - 1));
        assert_eq!(nested_string_clip_end(&unicode).unwrap(), 8 * 1024 + 1);
    }

    #[test]
    fn runtime_live_log_mojo_adapter_pages_after_cursor_and_clamps_limit() {
        assert_eq!(
            snapshot_plan(&[7, 9, 11], 7, usize::MAX, 2, 20, 3).unwrap(),
            LiveLogSnapshotPlan {
                start: 1,
                count: 2,
                cursor: 11,
                dropped: 3,
            }
        );
        assert_eq!(
            snapshot_plan(&[7, 9, 11], 11, 2, 2, 20, 3).unwrap(),
            LiveLogSnapshotPlan {
                start: 3,
                count: 0,
                cursor: 20,
                dropped: 3,
            }
        );
    }

    #[test]
    fn runtime_live_log_mojo_adapter_rejects_unsorted_sequences() {
        assert_eq!(
            snapshot_plan(&[9, 7], 0, 1, 2, 9, 0),
            Err(MojoError::InvalidInput)
        );
    }

    #[test]
    fn runtime_log_follow_mojo_adapter_classifies_line_and_pending_bounds() {
        const { assert!(crate::MOJO_REQUIRED) };

        let plan = follow_line_plan(
            &[FOLLOW_MAX_LINE_BYTES, FOLLOW_MAX_LINE_BYTES + 1],
            FOLLOW_MAX_PENDING_BYTES + 1,
        )
        .unwrap();
        assert_eq!(plan.keep_lines, [true, false]);
        assert_eq!(plan.dropped_lines, 1);
        assert!(plan.pending_gap);
        assert_eq!(
            follow_line_plan(&vec![0; 2 * 1024 * 1024 + 1], 0),
            Err(MojoError::InvalidInput)
        );
    }
}
