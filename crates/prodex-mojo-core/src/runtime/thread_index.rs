use crate::MojoError;
use crate::json::JsonNode;

const ABI_VERSION: i64 = 1;
const MAX_JSON_BYTES: usize = 64 * 1024 * 1024;
const MAX_JSON_NODES: usize = 1_048_576;
// ponytail: cap cursor history at 65k entries / 16 MiB; raise both ABI bounds if scans exceed it.
const MAX_SEEN_CURSORS: usize = 65_536;
const MAX_SEEN_CURSOR_BYTES: usize = 16 * 1024 * 1024;

const OP_START: i64 = 0;
const OP_RESPONSE: i64 = 1;
const OP_EOF: i64 = 2;
const OP_INVALID_JSON: i64 = 3;

const ACTION_IGNORE: i64 = 0;
const ACTION_SEND: i64 = 1;
const ACTION_DONE: i64 = 2;
const ACTION_ERROR: i64 = 3;

const CURSORS_KEEP: i64 = 0;
const CURSORS_CLEAR: i64 = 1;
const CURSORS_APPEND: i64 = 2;

unsafe extern "C" {
    fn prodex_runtime_thread_index_protocol_v1(
        abi_version: i64,
        operation: i64,
        scope: i64,
        phase: i64,
        expected_id: i64,
        archived: i64,
        seen_cursors_address: u64,
        seen_cursors_count: i64,
        nodes_address: u64,
        nodes_count: i64,
        raw_address: u64,
        raw_length: i64,
        version_address: u64,
        version_length: i64,
        output_address: u64,
        output_capacity: i64,
        cursor_output_address: u64,
        cursor_output_capacity: i64,
        cursor_written_address: u64,
        result_address: u64,
    ) -> i64;
    fn prodex_runtime_thread_index_dirty_marker_v1(
        abi_version: i64,
        operation: i64,
        parse_valid: i64,
        target_address: u64,
        target_length: i64,
        nodes_address: u64,
        nodes_count: i64,
        raw_address: u64,
        raw_length: i64,
        output_address: u64,
        output_capacity: i64,
        result_address: u64,
    ) -> i64;
    fn prodex_runtime_thread_index_dirty_marker_contents_v1(
        abi_version: i64,
        path_address: u64,
        path_length: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
    ) -> i64;
    fn prodex_runtime_thread_index_state_combine_v1(
        abi_version: i64,
        current_state: i64,
        observed_state: i64,
        result_address: u64,
    ) -> i64;
    fn prodex_runtime_thread_index_repair_action_v1(
        abi_version: i64,
        initial_state: i64,
        progress: i64,
        database_files_exist: i64,
        reconciliation_succeeded: i64,
        verified_state: i64,
        marker_matches: i64,
        result_address: u64,
    ) -> i64;
}

/// SQLite observations reduced by Mojo into the latest thread-index state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum ThreadIndexState {
    Present = 0,
    Stale = 1,
    Missing = 2,
    Unavailable = 3,
}

/// Selects the page limit and archive coverage for app-server reconciliation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum ThreadIndexScope {
    Full = 0,
    Latest = 1,
}

/// Next host operation selected by the Mojo repair state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum ThreadIndexRepairAction {
    CheckDatabaseFiles = 0,
    Reconcile = 1,
    CheckDirtyMarker = 2,
    ClearDirtyMarker = 3,
    SaveDirtyMarker = 4,
    Noop = 5,
}

/// Inputs describing the current host-side repair progress.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThreadIndexRepairProgress {
    Initial,
    DatabaseFilesChecked {
        exist: bool,
    },
    ReconciliationFinished {
        succeeded: bool,
        verified_state: ThreadIndexState,
    },
    DirtyMarkerChecked {
        matches: bool,
        after_reconciliation: bool,
    },
}

/// One app-server protocol action returned by Mojo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThreadIndexProtocolStep {
    Ignore,
    Send(Vec<Vec<u8>>),
    Done,
    Error(String),
}

/// Carries transport state between calls to the Mojo-owned protocol machine.
pub struct ThreadIndexProtocol {
    scope: ThreadIndexScope,
    phase: i64,
    expected_id: i64,
    archived: bool,
    seen_cursors: Vec<String>,
    seen_cursor_bytes: usize,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct StringViewFfi {
    address: u64,
    length: u64,
}

impl From<&str> for StringViewFfi {
    fn from(value: &str) -> Self {
        Self {
            address: value.as_ptr() as u64,
            length: value.len() as u64,
        }
    }
}

#[repr(C)]
struct NodeFfi {
    kind: i64,
    first_child: i64,
    next_sibling: i64,
    parent: i64,
    key: StringViewFfi,
    text: StringViewFfi,
    raw_start: i64,
    raw_length: i64,
}

const _: () = {
    assert!(std::mem::size_of::<NodeFfi>() == 80);
    assert!(std::mem::align_of::<NodeFfi>() == 8);
    assert!(std::mem::size_of::<StringViewFfi>() == 16);
};

fn ffi_nodes(nodes: &[JsonNode<'_>]) -> Result<Vec<NodeFfi>, MojoError> {
    nodes
        .iter()
        .map(|node| {
            let index = |value: Option<usize>| -> Result<i64, MojoError> {
                value.map_or(Ok(-1), |index| {
                    i64::try_from(index).map_err(|_| MojoError::InvalidInput)
                })
            };
            Ok(NodeFfi {
                kind: node.kind as i64,
                first_child: index(node.first_child)?,
                next_sibling: index(node.next_sibling)?,
                parent: index(node.parent)?,
                key: node.key.into(),
                text: node.text.into(),
                raw_start: i64::try_from(node.raw_start).map_err(|_| MojoError::InvalidInput)?,
                raw_length: i64::try_from(node.raw_length).map_err(|_| MojoError::InvalidInput)?,
            })
        })
        .collect()
}

fn status(code: i64) -> Result<(), MojoError> {
    match code {
        0 => Ok(()),
        1 | 2 => Err(MojoError::InvalidInput),
        3 => Err(MojoError::Capacity),
        4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

fn state_code(state: ThreadIndexState) -> i64 {
    state as i64
}

/// Reduce one SQLite observation into the Mojo-owned latest-index state.
pub fn combine_state(
    current: ThreadIndexState,
    observed: ThreadIndexState,
) -> Result<ThreadIndexState, MojoError> {
    let mut output = -1_i64;
    status(unsafe {
        prodex_runtime_thread_index_state_combine_v1(
            ABI_VERSION,
            state_code(current),
            state_code(observed),
            &mut output as *mut i64 as u64,
        )
    })?;
    decode_state(output)
}

fn decode_state(value: i64) -> Result<ThreadIndexState, MojoError> {
    match value {
        0 => Ok(ThreadIndexState::Present),
        1 => Ok(ThreadIndexState::Stale),
        2 => Ok(ThreadIndexState::Missing),
        3 => Ok(ThreadIndexState::Unavailable),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Select the next IO action for a repair without reimplementing its policy in Rust.
pub fn repair_action(
    initial_state: ThreadIndexState,
    progress: ThreadIndexRepairProgress,
) -> Result<ThreadIndexRepairAction, MojoError> {
    let (
        progress_code,
        database_files_exist,
        reconciliation_succeeded,
        verified_state,
        marker_matches,
    ) = match progress {
        ThreadIndexRepairProgress::Initial => (0, 0, 0, 3, 0),
        ThreadIndexRepairProgress::DatabaseFilesChecked { exist } => (1, i64::from(exist), 0, 3, 0),
        ThreadIndexRepairProgress::ReconciliationFinished {
            succeeded,
            verified_state,
        } => (2, 0, i64::from(succeeded), state_code(verified_state), 0),
        ThreadIndexRepairProgress::DirtyMarkerChecked {
            matches,
            after_reconciliation,
        } => (3, 0, i64::from(after_reconciliation), 0, i64::from(matches)),
    };
    let mut output = -1_i64;
    status(unsafe {
        prodex_runtime_thread_index_repair_action_v1(
            ABI_VERSION,
            state_code(initial_state),
            progress_code,
            database_files_exist,
            reconciliation_succeeded,
            verified_state,
            marker_matches,
            &mut output as *mut i64 as u64,
        )
    })?;
    match output {
        0 => Ok(ThreadIndexRepairAction::CheckDatabaseFiles),
        1 => Ok(ThreadIndexRepairAction::Reconcile),
        2 => Ok(ThreadIndexRepairAction::CheckDirtyMarker),
        3 => Ok(ThreadIndexRepairAction::ClearDirtyMarker),
        4 => Ok(ThreadIndexRepairAction::SaveDirtyMarker),
        5 => Ok(ThreadIndexRepairAction::Noop),
        _ => Err(MojoError::InvalidOutput),
    }
}

fn protocol_call(
    operation: i64,
    state: &ThreadIndexProtocol,
    tree: Option<(&[JsonNode<'_>], &[u8])>,
    client_version: &str,
    input_len: usize,
) -> Result<ProtocolOutput, MojoError> {
    if input_len > MAX_JSON_BYTES
        || client_version.len() > 256
        || state.seen_cursors.len() > MAX_SEEN_CURSORS
        || state
            .seen_cursors
            .iter()
            .try_fold(0_usize, |total, cursor| total.checked_add(cursor.len()))
            .is_none_or(|total| total > MAX_SEEN_CURSOR_BYTES)
    {
        return Err(MojoError::Capacity);
    }
    let nodes = tree
        .map(|(nodes, _)| ffi_nodes(nodes))
        .transpose()?
        .unwrap_or_default();
    let raw = tree.map_or(&[][..], |(_, raw)| raw);
    if nodes.len() > MAX_JSON_NODES || raw.len() > MAX_JSON_BYTES {
        return Err(MojoError::Capacity);
    }
    let seen = state
        .seen_cursors
        .iter()
        .map(|cursor| StringViewFfi::from(cursor.as_str()))
        .collect::<Vec<_>>();
    let output_capacity = raw
        .len()
        .checked_add(client_version.len())
        .and_then(|length| length.checked_add(1024))
        .ok_or(MojoError::InvalidInput)?;
    let cursor_capacity = raw.len().max(1);
    let mut output = Vec::new();
    output
        .try_reserve_exact(output_capacity)
        .map_err(|_| MojoError::Capacity)?;
    output.resize(output_capacity, 0);
    let mut cursor_output = Vec::new();
    cursor_output
        .try_reserve_exact(cursor_capacity)
        .map_err(|_| MojoError::Capacity)?;
    cursor_output.resize(cursor_capacity, 0);
    let mut cursor_written = -1_i64;
    let mut result = [-1_i64; 12];
    status(unsafe {
        // SAFETY: every view points to a live immutable Rust allocation; outputs
        // are distinct caller-owned buffers for this synchronous ABI call.
        prodex_runtime_thread_index_protocol_v1(
            ABI_VERSION,
            operation,
            state.scope as i64,
            state.phase,
            state.expected_id,
            i64::from(state.archived),
            seen.as_ptr() as u64,
            i64::try_from(seen.len()).map_err(|_| MojoError::InvalidInput)?,
            nodes.as_ptr() as u64,
            i64::try_from(nodes.len()).map_err(|_| MojoError::InvalidInput)?,
            raw.as_ptr() as u64,
            i64::try_from(raw.len()).map_err(|_| MojoError::InvalidInput)?,
            client_version.as_ptr() as u64,
            i64::try_from(client_version.len()).map_err(|_| MojoError::InvalidInput)?,
            output.as_mut_ptr() as u64,
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            cursor_output.as_mut_ptr() as u64,
            i64::try_from(cursor_output.len()).map_err(|_| MojoError::InvalidInput)?,
            &mut cursor_written as *mut i64 as u64,
            result.as_mut_ptr() as u64,
        )
    })?;
    let written = checked_length(result[11], output.len())?;
    output.truncate(written);
    let cursor_written = checked_length(cursor_written, cursor_output.len())?;
    cursor_output.truncate(cursor_written);
    Ok(ProtocolOutput {
        action: result[0],
        phase: result[1],
        archived: bool_value(result[2])?,
        expected_id: result[3],
        message_count: result[4],
        message_0: checked_span(result[5], result[6], output.len())?,
        message_1: checked_span(result[7], result[8], output.len())?,
        cursor_action: result[9],
        cursor_output,
        output,
    })
}

struct ProtocolOutput {
    action: i64,
    phase: i64,
    archived: bool,
    expected_id: i64,
    message_count: i64,
    message_0: (usize, usize),
    message_1: (usize, usize),
    cursor_action: i64,
    cursor_output: Vec<u8>,
    output: Vec<u8>,
}

fn checked_length(value: i64, capacity: usize) -> Result<usize, MojoError> {
    usize::try_from(value)
        .ok()
        .filter(|value| *value <= capacity)
        .ok_or(MojoError::InvalidOutput)
}

fn checked_span(start: i64, length: i64, capacity: usize) -> Result<(usize, usize), MojoError> {
    let start = usize::try_from(start).map_err(|_| MojoError::InvalidOutput)?;
    let length = usize::try_from(length).map_err(|_| MojoError::InvalidOutput)?;
    let end = start.checked_add(length).ok_or(MojoError::InvalidOutput)?;
    if end > capacity {
        return Err(MojoError::InvalidOutput);
    }
    Ok((start, end))
}

fn bool_value(value: i64) -> Result<bool, MojoError> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

fn protocol_step(output: ProtocolOutput) -> Result<ThreadIndexProtocolStep, MojoError> {
    match output.action {
        ACTION_IGNORE => Ok(ThreadIndexProtocolStep::Ignore),
        ACTION_DONE => Ok(ThreadIndexProtocolStep::Done),
        ACTION_ERROR => {
            let (start, end) = checked_span(0, output.output.len() as i64, output.output.len())?;
            String::from_utf8(output.output[start..end].to_vec())
                .map(ThreadIndexProtocolStep::Error)
                .map_err(|_| MojoError::InvalidOutput)
        }
        ACTION_SEND => {
            if !(1..=2).contains(&output.message_count) {
                return Err(MojoError::InvalidOutput);
            }
            let mut messages = vec![output.message_0];
            if output.message_count == 2 {
                messages.push(output.message_1);
            }
            messages
                .into_iter()
                .map(|(start, end)| {
                    output
                        .output
                        .get(start..end)
                        .map(<[u8]>::to_vec)
                        .ok_or(MojoError::InvalidOutput)
                })
                .collect::<Result<Vec<_>, _>>()
                .map(ThreadIndexProtocolStep::Send)
        }
        _ => Err(MojoError::InvalidOutput),
    }
}

impl ThreadIndexProtocol {
    /// Start reconciliation and return Mojo's initialize request.
    pub fn start(
        scope: ThreadIndexScope,
        client_version: &str,
    ) -> Result<(Self, ThreadIndexProtocolStep), MojoError> {
        let protocol = Self {
            scope,
            phase: 0,
            expected_id: 1,
            archived: false,
            seen_cursors: Vec::new(),
            seen_cursor_bytes: 0,
        };
        let output = protocol_call(OP_START, &protocol, None, client_version, 0)?;
        let step = protocol_step(output)?;
        if !matches!(step, ThreadIndexProtocolStep::Send(_)) {
            return Err(MojoError::InvalidOutput);
        }
        Ok((protocol, step))
    }

    /// Apply one validated app-server JSON tree and return the Mojo-selected action.
    pub fn response(
        &mut self,
        nodes: &[JsonNode<'_>],
        raw: &[u8],
        input_len: usize,
    ) -> Result<ThreadIndexProtocolStep, MojoError> {
        if input_len > MAX_JSON_BYTES {
            return Err(MojoError::Capacity);
        }
        let mut output = protocol_call(OP_RESPONSE, self, Some((nodes, raw)), "", input_len)?;
        if output.action == ACTION_IGNORE {
            return Ok(ThreadIndexProtocolStep::Ignore);
        }
        self.phase = output.phase;
        self.archived = output.archived;
        self.expected_id = output.expected_id;
        match output.cursor_action {
            CURSORS_KEEP => {}
            CURSORS_CLEAR => {
                self.seen_cursors.clear();
                self.seen_cursor_bytes = 0;
            }
            CURSORS_APPEND => {
                let cursor = String::from_utf8(std::mem::take(&mut output.cursor_output))
                    .map_err(|_| MojoError::InvalidOutput)?;
                self.seen_cursor_bytes = self
                    .seen_cursor_bytes
                    .checked_add(cursor.len())
                    .ok_or(MojoError::InvalidOutput)?;
                if self.seen_cursor_bytes > MAX_SEEN_CURSOR_BYTES
                    || self.seen_cursors.len() >= MAX_SEEN_CURSORS
                {
                    return Err(MojoError::Capacity);
                }
                self.seen_cursors.push(cursor);
            }
            _ => return Err(MojoError::InvalidOutput),
        }
        protocol_step(output)
    }

    /// Map malformed JSON through Mojo's protocol error policy.
    pub fn invalid_json(&self, input_len: usize) -> Result<ThreadIndexProtocolStep, MojoError> {
        protocol_step(protocol_call(OP_INVALID_JSON, self, None, "", input_len)?)
    }

    /// Map app-server EOF through the same Mojo-owned protocol error policy.
    pub fn eof(&self) -> Result<ThreadIndexProtocolStep, MojoError> {
        protocol_step(protocol_call(OP_EOF, self, None, "", 0)?)
    }
}

const MARKER_READ: i64 = 0;
const MARKER_MATCH: i64 = 1;

fn marker_call(
    operation: i64,
    parse_valid: bool,
    tree: Option<(&[JsonNode<'_>], &[u8])>,
    target: &str,
    input_len: usize,
) -> Result<(i64, Vec<u8>), MojoError> {
    if target.len() > MAX_JSON_BYTES || input_len > MAX_JSON_BYTES {
        return Err(MojoError::Capacity);
    }
    let nodes = tree
        .map(|(nodes, _)| ffi_nodes(nodes))
        .transpose()?
        .unwrap_or_default();
    let raw = tree.map_or(&[][..], |(_, raw)| raw);
    if nodes.len() > MAX_JSON_NODES || raw.len() > MAX_JSON_BYTES {
        return Err(MojoError::Capacity);
    }
    let output_capacity = input_len.checked_add(16).ok_or(MojoError::InvalidInput)?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(output_capacity.max(1))
        .map_err(|_| MojoError::Capacity)?;
    output.resize(output_capacity.max(1), 0);
    let mut result = [-1_i64; 2];
    status(unsafe {
        prodex_runtime_thread_index_dirty_marker_v1(
            ABI_VERSION,
            operation,
            i64::from(parse_valid),
            target.as_ptr() as u64,
            i64::try_from(target.len()).map_err(|_| MojoError::InvalidInput)?,
            nodes.as_ptr() as u64,
            i64::try_from(nodes.len()).map_err(|_| MojoError::InvalidInput)?,
            raw.as_ptr() as u64,
            i64::try_from(raw.len()).map_err(|_| MojoError::InvalidInput)?,
            output.as_mut_ptr() as u64,
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            result.as_mut_ptr() as u64,
        )
    })?;
    let written = checked_length(result[1], output.len())?;
    output.truncate(written);
    Ok((result[0], output))
}

/// Return the schema-valid relative rollout path from a dirty marker.
pub fn dirty_marker_path(
    tree: Option<(&[JsonNode<'_>], &[u8])>,
    input_len: usize,
) -> Result<Option<String>, MojoError> {
    match tree {
        Some((nodes, raw)) => {
            let (kind, output) = marker_call(MARKER_READ, true, Some((nodes, raw)), "", input_len)?;
            match kind {
                0 => Ok(None),
                1 => String::from_utf8(output)
                    .map(Some)
                    .map_err(|_| MojoError::InvalidOutput),
                _ => Err(MojoError::InvalidOutput),
            }
        }
        None => {
            let (kind, _) = marker_call(MARKER_READ, false, None, "", input_len)?;
            if kind == 0 {
                Ok(None)
            } else {
                Err(MojoError::InvalidOutput)
            }
        }
    }
}

/// Compare a dirty marker's schema and rollout path against the requested session.
pub fn dirty_marker_targets(
    tree: Option<(&[JsonNode<'_>], &[u8])>,
    target: &str,
    input_len: usize,
) -> Result<bool, MojoError> {
    match tree {
        Some((nodes, raw)) => {
            let (kind, _) = marker_call(MARKER_MATCH, true, Some((nodes, raw)), target, input_len)?;
            bool_value(kind)
        }
        None => {
            let (kind, _) = marker_call(MARKER_MATCH, false, None, target, input_len)?;
            bool_value(kind)
        }
    }
}

/// Serialize the versioned dirty marker in Mojo before Rust persists it.
pub fn dirty_marker_contents(path: &str) -> Result<Vec<u8>, MojoError> {
    if path.len() > MAX_JSON_BYTES {
        return Err(MojoError::Capacity);
    }
    let capacity = path
        .len()
        .checked_mul(6)
        .and_then(|length| length.checked_add(64))
        .ok_or(MojoError::InvalidInput)?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(capacity)
        .map_err(|_| MojoError::Capacity)?;
    output.resize(capacity, 0);
    let mut written = -1_i64;
    status(unsafe {
        prodex_runtime_thread_index_dirty_marker_contents_v1(
            ABI_VERSION,
            path.as_ptr() as u64,
            i64::try_from(path.len()).map_err(|_| MojoError::InvalidInput)?,
            output.as_mut_ptr() as u64,
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            &mut written as *mut i64 as u64,
        )
    })?;
    output.truncate(checked_length(written, output.len())?);
    Ok(output)
}
