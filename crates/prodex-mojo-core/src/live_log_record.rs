use crate::MojoError;
use crate::json::{JsonNode, transform_json_with_scratch};

const ABI_VERSION: i64 = 1;
const MAX_RECORD_BYTES: usize = 128 * 1024;
pub const FOLLOW_MAX_PENDING_BYTES: usize = 1024 * 1024;
pub const FOLLOW_MAX_LINE_BYTES: usize = 2 * 1024 * 1024;
const FOLLOW_MAX_INPUT_LINES: usize = 2 * 1024 * 1024;

const _: () = assert!(std::mem::size_of::<usize>() == std::mem::size_of::<u64>());

unsafe extern "C" {
    fn prodex_live_log_follow_last_newline_v1(
        abi_version: i64,
        buffer_address: u64,
        buffer_length: i64,
        result_address: u64,
    ) -> i64;
    fn prodex_live_log_follow_line_lengths_v1(
        abi_version: i64,
        text_address: u64,
        text_length: i64,
        output_address: u64,
        output_capacity: i64,
        measuring: i64,
        count_address: u64,
    ) -> i64;
    fn prodex_live_log_json_projection_v1(
        abi: i64,
        operation: i64,
        flag: i64,
        nodes_address: u64,
        node_count: i64,
        raw_address: u64,
        raw_length: i64,
        scratch_address: u64,
        scratch_count: i64,
        measuring: i64,
        output_address: u64,
        output_capacity: i64,
        metadata_address: u64,
    ) -> i64;
    fn prodex_live_log_compact_json_v1(
        abi_version: i64,
        fields_address: u64,
        field_count: i64,
        output_address: u64,
        output_capacity: i64,
        measuring: i64,
        written_address: u64,
    ) -> i64;
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

/// Render compact live-log JSON from existing Serde-canonical raw field
/// values. Key ordering, omission and truncation-marker formatting belong
/// entirely to compiled Mojo; caller keeps the values and their lifetimes.
pub fn compact_json_metadata(
    event: Option<&str>,
    pid: Option<&str>,
    timestamp: Option<&str>,
) -> Result<String, MojoError> {
    let mut fields = [0_u64; 9];
    for (index, raw) in [event, pid, timestamp].into_iter().enumerate() {
        if let Some(raw) = raw {
            fields[index * 3] = 1;
            fields[index * 3 + 1] = pointer_address(raw.as_ptr());
            fields[index * 3 + 2] =
                u64::try_from(raw.len()).map_err(|_| MojoError::InvalidInput)?;
        }
    }
    let mut written = -1_i64;
    let length_address = mutable_pointer_address(&mut written as *mut i64);
    status(unsafe {
        prodex_live_log_compact_json_v1(
            ABI_VERSION,
            pointer_address(fields.as_ptr()),
            3,
            0,
            0,
            1,
            length_address,
        )
    })?;
    let size = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(size)
        .map_err(|_| MojoError::Capacity)?;
    output.resize(size, 0_u8);
    written = -1;
    status(unsafe {
        prodex_live_log_compact_json_v1(
            ABI_VERSION,
            pointer_address(fields.as_ptr()),
            3,
            mutable_pointer_address(output.as_mut_ptr()),
            input_length(output.len())?,
            0,
            length_address,
        )
    })?;
    if usize::try_from(written).map_err(|_| MojoError::InvalidOutput)? != size {
        return Err(MojoError::InvalidOutput);
    }
    String::from_utf8(output).map_err(|_| MojoError::InvalidOutput)
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

/// Serialize one borrowed Serde JSON tree in compiled Mojo, truncating
/// oversized nested string values at Mojo-owned UTF-8-safe byte boundaries.
/// Numeric lexemes are acquired canonically by the host in the raw buffer.
pub fn clipped_json_projection(nodes: &[JsonNode<'_>], raw: &str) -> Result<String, MojoError> {
    let output =
        transform_json_with_scratch(nodes, raw, 0, false, prodex_live_log_json_projection_v1, 1)?
            .ok_or(MojoError::InvalidOutput)?;
    String::from_utf8(output).map_err(|_| MojoError::InvalidOutput)
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

/// Find the byte immediately after the last complete binary LF line.
/// A split UTF-8 scalar in a trailing partial line remains untouched.
pub fn follow_last_complete_line_end(buffer: &[u8]) -> Result<usize, MojoError> {
    let mut result = -1_i64;
    status(unsafe {
        prodex_live_log_follow_last_newline_v1(
            ABI_VERSION,
            pointer_address(buffer.as_ptr()),
            input_length(buffer.len())?,
            mutable_pointer_address(&mut result as *mut i64),
        )
    })?;
    let end = usize::try_from(result).map_err(|_| MojoError::InvalidOutput)?;
    if end > buffer.len() || (end > 0 && buffer[end - 1] != b'\n') {
        return Err(MojoError::InvalidOutput);
    }
    Ok(end)
}

/// Extract original post-lossy UTF-8 split-inclusive line lengths using
/// a read-only Mojo measure call followed by the Mojo materialization.
pub fn follow_utf8_line_lengths(text: &str) -> Result<Vec<usize>, MojoError> {
    let mut count = -1_i64;
    let pointer = pointer_address(text.as_ptr());
    let length = input_length(text.len())?;
    let count_ptr = mutable_pointer_address(&mut count as *mut i64);
    status(unsafe {
        prodex_live_log_follow_line_lengths_v1(ABI_VERSION, pointer, length, 0, 0, 1, count_ptr)
    })?;
    let size = usize::try_from(count).map_err(|_| MojoError::InvalidOutput)?;
    if size > FOLLOW_MAX_INPUT_LINES || size > text.len() {
        return Err(MojoError::InvalidOutput);
    }
    let mut lengths = Vec::<i64>::new();
    lengths
        .try_reserve_exact(size)
        .map_err(|_| MojoError::Capacity)?;
    lengths.resize(size, -1);
    count = -1;
    status(unsafe {
        prodex_live_log_follow_line_lengths_v1(
            ABI_VERSION,
            pointer,
            length,
            mutable_pointer_address(lengths.as_mut_ptr()),
            input_length(lengths.len())?,
            0,
            count_ptr,
        )
    })?;
    if usize::try_from(count).map_err(|_| MojoError::InvalidOutput)? != size {
        return Err(MojoError::InvalidOutput);
    }
    let mut verified = Vec::with_capacity(size);
    let mut total = 0usize;
    for len in lengths {
        let len = usize::try_from(len).map_err(|_| MojoError::InvalidOutput)?;
        if len == 0 {
            return Err(MojoError::InvalidOutput);
        }
        total = total.checked_add(len).ok_or(MojoError::InvalidOutput)?;
        verified.push(len);
    }
    if total != text.len() {
        return Err(MojoError::InvalidOutput);
    }
    Ok(verified)
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
#[path = "../tests/src/live_log_record/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "../tests/src/live_log_record/compiled_follow_framing_tests.rs"]
mod compiled_follow_framing_tests;
