//! Typed adapter for Mojo-owned log snapshot ordering.

use crate::MojoError;

const LOG_SNAPSHOT_ABI_VERSION: i64 = 1;

unsafe extern "C" {
    fn prodex_mojo_log_snapshot_order_v1(
        abi_version: i64,
        transcript_present: i64,
        upstream_present: i64,
        token_usage_present: i64,
        output_address: u64,
    ) -> i64;
}

/// Canonical item kinds used by the `prodex log --last` report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogSnapshotItemKind {
    Transcript,
    UpstreamPayload,
    TokenUsage,
}

/// Return Mojo's stable report order for the available snapshot items.
pub fn snapshot_item_order(
    transcript_present: bool,
    upstream_present: bool,
    token_usage_present: bool,
) -> Result<Vec<LogSnapshotItemKind>, MojoError> {
    let presence = [transcript_present, upstream_present, token_usage_present];
    let mut output = [-1_i64; 4];
    let status = unsafe {
        prodex_mojo_log_snapshot_order_v1(
            LOG_SNAPSHOT_ABI_VERSION,
            i64::from(transcript_present),
            i64::from(upstream_present),
            i64::from(token_usage_present),
            output.as_mut_ptr() as usize as u64,
        )
    };
    match status {
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    let count = usize::try_from(output[0]).map_err(|_| MojoError::InvalidOutput)?;
    if count > presence.len() || count != presence.iter().filter(|present| **present).count() {
        return Err(MojoError::InvalidOutput);
    }
    let mut seen = [false; 3];
    let mut order = Vec::with_capacity(count);
    for raw_kind in output[1..=count].iter().copied() {
        let kind = usize::try_from(raw_kind).map_err(|_| MojoError::InvalidOutput)?;
        if kind >= presence.len() || !presence[kind] || seen[kind] {
            return Err(MojoError::InvalidOutput);
        }
        seen[kind] = true;
        order.push(match kind {
            0 => LogSnapshotItemKind::Transcript,
            1 => LogSnapshotItemKind::UpstreamPayload,
            2 => LogSnapshotItemKind::TokenUsage,
            _ => return Err(MojoError::InvalidOutput),
        });
    }
    if seen != presence {
        return Err(MojoError::InvalidOutput);
    }
    Ok(order)
}
