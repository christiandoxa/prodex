use super::{
    RichStringView, ensure_rich_abi, mojo_mut_pointer_address, mojo_pointer_address, view,
};
use crate::MojoError;

const RUNTIME_DOCTOR_MARKER_ABI_VERSION: i64 = 1;
const RUNTIME_DOCTOR_MARKER_SEMANTICS_ABI_VERSION: i64 = 2;
const RUNTIME_DOCTOR_MARKER_SUMMARY_COUNTS_ABI_VERSION: i64 = 1;
const RUNTIME_DOCTOR_MARKER_SUMMARY_COUNTS_MAX_BATCH: usize = 256;

unsafe extern "C" {
    fn prodex_mojo_runtime_doctor_marker_known_v1(abi_version: i64, marker: u64, known: u64)
    -> i64;
    fn prodex_mojo_runtime_doctor_marker_semantics_v2(
        abi_version: i64,
        marker: u64,
        output: u64,
    ) -> i64;
    fn prodex_mojo_runtime_doctor_marker_summary_counts_v1(
        abi_version: i64,
        marker_views: u64,
        counts: u64,
        count: i64,
        output: u64,
    ) -> i64;
    fn prodex_mojo_runtime_doctor_parse_message_v1(
        abi_version: i64,
        input_address: u64,
        input_length: i64,
        output_address: u64,
        output_capacity: i64,
    ) -> i64;
}

pub const RUNTIME_DOCTOR_MARKER_PHASE_NONE: i64 = 0;
pub const RUNTIME_DOCTOR_MARKER_PHASE_SELECTION: i64 = 1;
pub const RUNTIME_DOCTOR_MARKER_PHASE_PRE_SEND: i64 = 2;
pub const RUNTIME_DOCTOR_MARKER_PHASE_UPSTREAM: i64 = 3;
pub const RUNTIME_DOCTOR_MARKER_PHASE_COMMIT: i64 = 4;
pub const RUNTIME_DOCTOR_MARKER_PHASE_FAIL: i64 = 5;

pub const RUNTIME_DOCTOR_MARKER_SELECTION_NONE: i64 = 0;
pub const RUNTIME_DOCTOR_MARKER_SELECTION_PICKED: i64 = 1;
pub const RUNTIME_DOCTOR_MARKER_SELECTION_KEPT: i64 = 2;
pub const RUNTIME_DOCTOR_MARKER_SELECTION_SKIPPED: i64 = 3;
pub const RUNTIME_DOCTOR_MARKER_SELECTION_BLOCKED: i64 = 4;

pub const RUNTIME_DOCTOR_MARKER_ROUTE_NONE: i64 = 0;
pub const RUNTIME_DOCTOR_MARKER_ROUTE_SELECTED: i64 = 1;
pub const RUNTIME_DOCTOR_MARKER_ROUTE_SELECTION_SKIP: i64 = 2;
pub const RUNTIME_DOCTOR_MARKER_ROUTE_BLOCKED: i64 = 3;
pub const RUNTIME_DOCTOR_MARKER_ROUTE_HEALTH: i64 = 4;
pub const RUNTIME_DOCTOR_MARKER_ROUTE_TRANSPORT_HEALTH: i64 = 5;
pub const RUNTIME_DOCTOR_MARKER_ROUTE_TRANSPORT_FAILURE: i64 = 6;
pub const RUNTIME_DOCTOR_MARKER_ROUTE_QUOTA: i64 = 7;

pub const RUNTIME_DOCTOR_MARKER_FAILURE_CLASS_NONE: i64 = 0;
pub const RUNTIME_DOCTOR_MARKER_FAILURE_CLASS_ADMISSION: i64 = 1;
pub const RUNTIME_DOCTOR_MARKER_FAILURE_CLASS_AUTH: i64 = 2;
pub const RUNTIME_DOCTOR_MARKER_FAILURE_CLASS_CONTINUATION: i64 = 3;
pub const RUNTIME_DOCTOR_MARKER_FAILURE_CLASS_PERSISTENCE: i64 = 4;
pub const RUNTIME_DOCTOR_MARKER_FAILURE_CLASS_QUOTA: i64 = 5;
pub const RUNTIME_DOCTOR_MARKER_FAILURE_CLASS_TRANSPORT: i64 = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeDoctorMarkerSemantics {
    pub timeline_phase: i64,
    pub selection_bucket: i64,
    pub route_action: i64,
    pub failure_class: i64,
}

/// Fixed marker totals used by the runtime-doctor summary.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RuntimeDoctorMarkerSummaryCounts {
    pub selection_picked: usize,
    pub selection_kept: usize,
    pub selection_skipped: usize,
    pub selection_blocked: usize,
    pub failure_admission: usize,
    pub failure_auth: usize,
    pub failure_continuation: usize,
    pub failure_persistence: usize,
    pub failure_quota: usize,
    pub failure_transport: usize,
}

/// Count fixed selection and failure-class marker totals through the Mojo reducer.
pub fn runtime_doctor_marker_summary_counts<'a>(
    marker_counts: impl IntoIterator<Item = (&'a str, usize)>,
) -> Result<RuntimeDoctorMarkerSummaryCounts, MojoError> {
    ensure_rich_abi()?;
    let mut output = [0_i64; 10];
    let mut markers = Vec::with_capacity(RUNTIME_DOCTOR_MARKER_SUMMARY_COUNTS_MAX_BATCH);
    let mut counts = Vec::with_capacity(RUNTIME_DOCTOR_MARKER_SUMMARY_COUNTS_MAX_BATCH);
    for (marker, count) in marker_counts {
        markers.push(view(marker));
        counts.push(i64::try_from(count).map_err(|_| MojoError::InvalidInput)?);
        if markers.len() == RUNTIME_DOCTOR_MARKER_SUMMARY_COUNTS_MAX_BATCH {
            runtime_doctor_marker_summary_counts_batch(&markers, &counts, &mut output)?;
            markers.clear();
            counts.clear();
        }
    }
    if !markers.is_empty() {
        runtime_doctor_marker_summary_counts_batch(&markers, &counts, &mut output)?;
    }
    Ok(RuntimeDoctorMarkerSummaryCounts {
        selection_picked: usize::try_from(output[0]).map_err(|_| MojoError::InvalidOutput)?,
        selection_kept: usize::try_from(output[1]).map_err(|_| MojoError::InvalidOutput)?,
        selection_skipped: usize::try_from(output[2]).map_err(|_| MojoError::InvalidOutput)?,
        selection_blocked: usize::try_from(output[3]).map_err(|_| MojoError::InvalidOutput)?,
        failure_admission: usize::try_from(output[4]).map_err(|_| MojoError::InvalidOutput)?,
        failure_auth: usize::try_from(output[5]).map_err(|_| MojoError::InvalidOutput)?,
        failure_continuation: usize::try_from(output[6]).map_err(|_| MojoError::InvalidOutput)?,
        failure_persistence: usize::try_from(output[7]).map_err(|_| MojoError::InvalidOutput)?,
        failure_quota: usize::try_from(output[8]).map_err(|_| MojoError::InvalidOutput)?,
        failure_transport: usize::try_from(output[9]).map_err(|_| MojoError::InvalidOutput)?,
    })
}

fn runtime_doctor_marker_summary_counts_batch(
    markers: &[RichStringView],
    counts: &[i64],
    output: &mut [i64; 10],
) -> Result<(), MojoError> {
    let count = i64::try_from(markers.len()).map_err(|_| MojoError::InvalidInput)?;
    let status = unsafe {
        prodex_mojo_runtime_doctor_marker_summary_counts_v1(
            RUNTIME_DOCTOR_MARKER_SUMMARY_COUNTS_ABI_VERSION,
            mojo_pointer_address(markers.as_ptr()),
            mojo_pointer_address(counts.as_ptr()),
            count,
            mojo_mut_pointer_address(output.as_mut_ptr()),
        )
    };
    if status != 0 {
        return Err(match status {
            1 => MojoError::InvalidInput,
            2 | 5 => MojoError::InvalidOutput,
            3 => MojoError::Capacity,
            4 => MojoError::AbiMismatch,
            _ => MojoError::InvalidOutput,
        });
    }
    Ok(())
}

pub fn runtime_doctor_marker_semantics(
    marker: &str,
) -> Result<RuntimeDoctorMarkerSemantics, MojoError> {
    ensure_rich_abi()?;
    let marker = view(marker);
    let mut output = [0_i64; 4];
    let status = unsafe {
        prodex_mojo_runtime_doctor_marker_semantics_v2(
            RUNTIME_DOCTOR_MARKER_SEMANTICS_ABI_VERSION,
            mojo_pointer_address(&marker),
            mojo_mut_pointer_address(output.as_mut_ptr()),
        )
    };
    if status != 0 {
        return Err(match status {
            1 => MojoError::InvalidInput,
            2 => MojoError::InvalidOutput,
            _ => MojoError::InvalidOutput,
        });
    }
    if !(0..=RUNTIME_DOCTOR_MARKER_PHASE_FAIL).contains(&output[0])
        || !(0..=RUNTIME_DOCTOR_MARKER_SELECTION_BLOCKED).contains(&output[1])
        || !(0..=RUNTIME_DOCTOR_MARKER_ROUTE_QUOTA).contains(&output[2])
        || !(0..=RUNTIME_DOCTOR_MARKER_FAILURE_CLASS_TRANSPORT).contains(&output[3])
    {
        return Err(MojoError::InvalidOutput);
    }
    Ok(RuntimeDoctorMarkerSemantics {
        timeline_phase: output[0],
        selection_bucket: output[1],
        route_action: output[2],
        failure_class: output[3],
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeDoctorMessageFieldRange {
    pub key_start: usize,
    pub key_end: usize,
    pub value_start: usize,
    pub value_end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeDoctorMessageParsePlan {
    pub event: Option<(usize, usize)>,
    pub fields: Vec<RuntimeDoctorMessageFieldRange>,
}

pub fn runtime_doctor_parse_message_offsets(
    message: &str,
) -> Result<RuntimeDoctorMessageParsePlan, MojoError> {
    ensure_rich_abi()?;
    const MAX_BYTES: usize = 4 * 1024 * 1024;
    if message.len() > MAX_BYTES {
        return Err(MojoError::InvalidInput);
    }
    let max_fields = message.len().saturating_div(2).saturating_add(1);
    let output_capacity = max_fields
        .checked_mul(4)
        .and_then(|value| value.checked_add(3))
        .ok_or(MojoError::InvalidInput)?;
    let mut output = vec![0_i64; output_capacity];
    let status = unsafe {
        prodex_mojo_runtime_doctor_parse_message_v1(
            RUNTIME_DOCTOR_MARKER_ABI_VERSION,
            message.as_ptr() as u64,
            i64::try_from(message.len()).map_err(|_| MojoError::InvalidInput)?,
            output.as_mut_ptr() as u64,
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
        )
    };
    if status != 0 {
        return Err(match status {
            1 => MojoError::InvalidInput,
            2 | 3 => MojoError::InvalidOutput,
            4 => MojoError::AbiMismatch,
            _ => MojoError::InvalidOutput,
        });
    }
    let event = match (output[0], output[1]) {
        (-1, -1) => None,
        (start, end) if start >= 0 && end >= start => {
            let start = usize::try_from(start).map_err(|_| MojoError::InvalidOutput)?;
            let end = usize::try_from(end).map_err(|_| MojoError::InvalidOutput)?;
            if end > message.len()
                || !message.is_char_boundary(start)
                || !message.is_char_boundary(end)
            {
                return Err(MojoError::InvalidOutput);
            }
            Some((start, end))
        }
        _ => return Err(MojoError::InvalidOutput),
    };
    let field_count = usize::try_from(output[2]).map_err(|_| MojoError::InvalidOutput)?;
    if field_count > max_fields || 3 + field_count * 4 > output.len() {
        return Err(MojoError::InvalidOutput);
    }
    let mut fields = Vec::with_capacity(field_count);
    for index in 0..field_count {
        let base = 3 + index * 4;
        let key_start = usize::try_from(output[base]).map_err(|_| MojoError::InvalidOutput)?;
        let key_end = usize::try_from(output[base + 1]).map_err(|_| MojoError::InvalidOutput)?;
        let value_start =
            usize::try_from(output[base + 2]).map_err(|_| MojoError::InvalidOutput)?;
        let value_end = usize::try_from(output[base + 3]).map_err(|_| MojoError::InvalidOutput)?;
        if key_start > key_end
            || value_start > value_end
            || key_end > message.len()
            || value_end > message.len()
            || !message.is_char_boundary(key_start)
            || !message.is_char_boundary(key_end)
            || !message.is_char_boundary(value_start)
            || !message.is_char_boundary(value_end)
        {
            return Err(MojoError::InvalidOutput);
        }
        fields.push(RuntimeDoctorMessageFieldRange {
            key_start,
            key_end,
            value_start,
            value_end,
        });
    }
    Ok(RuntimeDoctorMessageParsePlan { event, fields })
}

pub fn runtime_doctor_marker_known(marker: &str) -> Result<bool, MojoError> {
    ensure_rich_abi()?;
    let marker = view(marker);
    let mut known = 0_i64;
    let status = unsafe {
        prodex_mojo_runtime_doctor_marker_known_v1(
            RUNTIME_DOCTOR_MARKER_ABI_VERSION,
            mojo_pointer_address(&marker),
            mojo_mut_pointer_address(&mut known),
        )
    };
    match (status, known) {
        (0, 0) => Ok(false),
        (0, 1) => Ok(true),
        (1, _) => Err(MojoError::InvalidInput),
        (2, _) => Err(MojoError::InvalidOutput),
        _ => Err(MojoError::InvalidOutput),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_doctor_marker_classifier_accepts_known_and_rejects_unknown() {
        assert!(runtime_doctor_marker_known("selection_pick").unwrap());
        assert!(runtime_doctor_marker_known("websocket_connect_overflow_rejected").unwrap());
        assert!(!runtime_doctor_marker_known("not_a_runtime_marker").unwrap());
    }

    #[test]
    fn runtime_doctor_marker_summary_counts_batch_and_chunk_marker_totals() {
        let long_marker = "x".repeat(257);
        let mut marker_counts = vec![
            ("selection_pick", 2),
            ("selection_keep_current", 3),
            ("selection_skip_current", 4),
            ("local_selection_blocked", 5),
            ("websocket_connect_overflow_rejected", 6),
            ("profile_auth_proactive_sync_failed", 7),
            ("compact_pressure_shed", 8),
            ("continuation_journal_queue_backpressure", 9),
            ("quota_critical_floor_before_send", 10),
            ("local_rewrite_gemini_live_sidecar_session_error", 11),
            (long_marker.as_str(), 12),
            ("unknown_marker", 0),
        ];
        marker_counts.extend((0..300).map(|_| ("selection_pick", 1)));

        let counts = runtime_doctor_marker_summary_counts(marker_counts).unwrap();

        assert_eq!(counts.selection_picked, 302);
        assert_eq!(counts.selection_kept, 3);
        assert_eq!(counts.selection_skipped, 4);
        assert_eq!(counts.selection_blocked, 15);
        assert_eq!(counts.failure_admission, 6);
        assert_eq!(counts.failure_auth, 7);
        assert_eq!(counts.failure_continuation, 8);
        assert_eq!(counts.failure_persistence, 9);
        assert_eq!(counts.failure_quota, 15);
        assert_eq!(counts.failure_transport, 11);
    }

    #[test]
    fn runtime_doctor_marker_summary_counts_rejects_unrepresentable_and_overflowing_totals() {
        assert_eq!(
            runtime_doctor_marker_summary_counts([("selection_pick", usize::MAX)]),
            Err(MojoError::InvalidInput),
        );
        assert_eq!(
            runtime_doctor_marker_summary_counts([
                ("selection_pick", i64::MAX as usize),
                ("selection_pick", 1),
            ]),
            Err(MojoError::InvalidOutput),
        );
    }
}
