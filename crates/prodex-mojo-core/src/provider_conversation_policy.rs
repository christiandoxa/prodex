//! Borrowed provider-cache facts and checked plans; JSON and synchronization stay with the host.
use crate::MojoError;

/// A cache entry in ascending, unique storage-key order.
#[derive(Clone, Copy)]
pub struct ConversationEntry<'a> {
    pub key: &'a str,
    pub sequence: u64,
    pub estimated_bytes: usize,
}

/// Complete retention decisions, indexed by the original entry order.
#[derive(Debug, PartialEq, Eq)]
pub struct ConversationRetention {
    pub keep: Vec<bool>,
    pub keep_incoming: bool,
    pub next_sequence: u64,
    pub estimated_bytes: usize,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct StringView {
    address: u64,
    length: u64,
}

impl From<&str> for StringView {
    fn from(value: &str) -> Self {
        Self {
            address: value.as_ptr() as u64,
            length: value.len() as u64,
        }
    }
}

#[repr(C)]
struct Row {
    key: StringView,
    sequence: u64,
    estimated_bytes: u64,
}

const _: () = {
    assert!(std::mem::size_of::<StringView>() == 16);
    assert!(std::mem::size_of::<Row>() == 32);
    assert!(std::mem::align_of::<Row>() == 8);
};

impl From<&ConversationEntry<'_>> for Row {
    fn from(value: &ConversationEntry<'_>) -> Self {
        Self {
            key: value.key.into(),
            sequence: value.sequence,
            estimated_bytes: value.estimated_bytes as u64,
        }
    }
}

fn borrowed_rows(entries: &[ConversationEntry<'_>]) -> Result<Vec<Row>, MojoError> {
    const MAX_ABI_ROWS: usize = 2048;
    if entries.len() > MAX_ABI_ROWS {
        return Err(MojoError::InvalidInput);
    }
    Ok(entries.iter().map(Row::from).collect())
}

unsafe extern "C" {
    fn prodex_provider_conversation_history_v1(
        version: i64,
        mode: i64,
        scope_address: u64,
        scope_length: i64,
        scoped: i64,
        rows_address: u64,
        count: i64,
        mask_address: u64,
        mask_capacity: i64,
        selected_address: u64,
    ) -> i64;
    fn prodex_provider_conversation_retention_v1(
        version: i64,
        scope_address: u64,
        scope_length: i64,
        scoped: i64,
        rows_address: u64,
        count: i64,
        keep_address: u64,
        keep_capacity: i64,
        scratch_address: u64,
        scratch_capacity: i64,
        output_address: u64,
    ) -> i64;
}

fn check_status(status: i64) -> Result<(), MojoError> {
    match status {
        0 => Ok(()),
        1 => Err(MojoError::InvalidInput),
        3 => Err(MojoError::Capacity),
        4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

fn flag(value: u8) -> Result<bool, MojoError> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Calls the opaque predicate only for in-scope entries, in original key order.
pub fn latest_matching(
    entries: &[ConversationEntry<'_>],
    scope: Option<&str>,
    mut predicate: impl FnMut(usize) -> bool,
) -> Result<Option<usize>, MojoError> {
    let rows = borrowed_rows(entries)?;
    let count = i64::try_from(rows.len()).map_err(|_| MojoError::InvalidInput)?;
    let scope_view = StringView::from(scope.unwrap_or_default());
    let scope_len = i64::try_from(scope_view.length).map_err(|_| MojoError::InvalidInput)?;
    let mut mask = vec![0_u8; rows.len()];
    let mut selected = -1_i64;
    for mode in 0..=1 {
        check_status(unsafe {
            prodex_provider_conversation_history_v1(
                1,
                mode,
                scope_view.address,
                scope_len,
                i64::from(scope.is_some()),
                rows.as_ptr() as u64,
                count,
                mask.as_mut_ptr() as u64,
                count,
                (&raw mut selected) as u64,
            )
        })?;
        if mode == 0 {
            for (index, eligible) in mask.iter_mut().enumerate() {
                *eligible = u8::from(flag(*eligible)? && predicate(index));
            }
        }
    }
    match selected {
        -1 => Ok(None),
        value => {
            let index = usize::try_from(value).map_err(|_| MojoError::InvalidOutput)?;
            if index >= rows.len() || !flag(mask[index])? {
                return Err(MojoError::InvalidOutput);
            }
            Ok(Some(index))
        }
    }
}

/// Plans replacement, oversize rejection, per-scope eviction and global retention atomically.
pub fn retention_plan(
    entries: &[ConversationEntry<'_>],
    scope: Option<&str>,
    incoming_key: &str,
    incoming_bytes: usize,
    next_sequence: u64,
) -> Result<ConversationRetention, MojoError> {
    let mut rows = borrowed_rows(entries)?;
    let count = i64::try_from(rows.len()).map_err(|_| MojoError::InvalidInput)?;
    rows.push(Row {
        key: incoming_key.into(),
        sequence: next_sequence,
        estimated_bytes: incoming_bytes as u64,
    });
    let capacity = i64::try_from(rows.len()).map_err(|_| MojoError::Capacity)?;
    let scope_view = StringView::from(scope.unwrap_or_default());
    let scope_len = i64::try_from(scope_view.length).map_err(|_| MojoError::InvalidInput)?;
    let mut keep = vec![0_u8; rows.len()];
    let mut scratch = vec![0_i64; rows.len()];
    let mut output = [0_u64; 2];
    check_status(unsafe {
        prodex_provider_conversation_retention_v1(
            1,
            scope_view.address,
            scope_len,
            i64::from(scope.is_some()),
            rows.as_ptr() as u64,
            count,
            keep.as_mut_ptr() as u64,
            capacity,
            scratch.as_mut_ptr() as u64,
            capacity,
            output.as_mut_ptr() as u64,
        )
    })?;
    let keep_incoming = flag(keep.pop().ok_or(MojoError::InvalidOutput)?)?;
    Ok(ConversationRetention {
        keep: keep.into_iter().map(flag).collect::<Result<_, _>>()?,
        keep_incoming,
        next_sequence: output[0],
        estimated_bytes: usize::try_from(output[1]).map_err(|_| MojoError::InvalidOutput)?,
    })
}

#[cfg(test)]
#[path = "provider_conversation_policy_tests.rs"]
mod tests;
