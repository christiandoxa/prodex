use super::{
    RichStringView, ensure_rich_abi, mojo_mut_pointer_address, mojo_pointer_address, view,
};
use crate::MojoError;

const RUNTIME_DOCTOR_MARKER_ABI_VERSION: i64 = 1;
const RUNTIME_DOCTOR_SMART_CONTEXT_DECISION_ABI_VERSION: i64 = 1;
const RUNTIME_DOCTOR_LOG_VALUE_ABI_VERSION: i64 = 1;
const RUNTIME_DOCTOR_FALLBACK_REASON_SOURCE_ABI_VERSION: i64 = 1;
const RUNTIME_DOCTOR_MARKER_SEMANTICS_ABI_VERSION: i64 = 2;
const RUNTIME_DOCTOR_MESSAGE_PARSE_ABI_VERSION: i64 = 2;
const RUNTIME_DOCTOR_MARKER_SUMMARY_COUNTS_ABI_VERSION: i64 = 1;
const RUNTIME_DOCTOR_MARKER_SUMMARY_COUNTS_MAX_BATCH: usize = 256;
const RUNTIME_DOCTOR_COMPACT_EXIT_COUNTS_ABI_VERSION: i64 = 1;
const RUNTIME_DOCTOR_COMPACT_EXIT_COUNTS_MAX_BATCH: usize = 256;
const RUNTIME_DOCTOR_COMPACT_EXIT_COUNT_LABELS: [&str; 11] = [
    "candidate_exhausted",
    "committed",
    "committed_owner",
    "followup_owner",
    "lineage_released",
    "owner_retry",
    "precommit_budget",
    "pressure_shed",
    "quota_misc",
    "retryable_failure",
    "transport_failure",
];

unsafe extern "C" {
    fn prodex_mojo_runtime_doctor_marker_known_v1(abi_version: i64, marker: u64, known: u64)
    -> i64;
    fn prodex_mojo_runtime_doctor_smart_context_decision_is_fallback_v1(
        abi_version: i64,
        decision: u64,
        is_fallback: u64,
    ) -> i64;
    fn prodex_mojo_runtime_doctor_log_value_is_ignored_v1(
        abi_version: i64,
        value: u64,
        is_ignored: u64,
    ) -> i64;
    fn prodex_mojo_runtime_doctor_smart_context_fallback_reason_source_v1(
        abi_version: i64,
        decision: u64,
        has_self_check: i64,
        reason_count: i64,
        source: u64,
    ) -> i64;
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
    fn prodex_mojo_runtime_doctor_compact_exit_counts_v1(
        abi_version: i64,
        marker_views: u64,
        counts: u64,
        count: i64,
        output: u64,
    ) -> i64;
    fn prodex_mojo_runtime_doctor_parse_message_v2(
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

pub const RUNTIME_DOCTOR_FALLBACK_REASON_SOURCE_EVENT_REASONS: i64 = 1;
pub const RUNTIME_DOCTOR_FALLBACK_REASON_SOURCE_SELF_CHECK: i64 = 2;
pub const RUNTIME_DOCTOR_FALLBACK_REASON_SOURCE_DECISION: i64 = 3;

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

/// Aggregate compact-exit marker aliases into the stable Mojo-owned label order.
pub fn runtime_doctor_compact_exit_counts<'a>(
    marker_counts: impl IntoIterator<Item = (&'a str, usize)>,
) -> Result<Vec<(String, usize)>, MojoError> {
    ensure_rich_abi()?;
    let mut output = [0_i64; RUNTIME_DOCTOR_COMPACT_EXIT_COUNT_LABELS.len()];
    let mut markers = Vec::with_capacity(RUNTIME_DOCTOR_COMPACT_EXIT_COUNTS_MAX_BATCH);
    let mut counts = Vec::with_capacity(RUNTIME_DOCTOR_COMPACT_EXIT_COUNTS_MAX_BATCH);
    for (marker, count) in marker_counts {
        markers.push(view(marker));
        counts.push(i64::try_from(count).map_err(|_| MojoError::InvalidInput)?);
        if markers.len() == RUNTIME_DOCTOR_COMPACT_EXIT_COUNTS_MAX_BATCH {
            runtime_doctor_compact_exit_counts_batch(&markers, &counts, &mut output)?;
            markers.clear();
            counts.clear();
        }
    }
    if !markers.is_empty() {
        runtime_doctor_compact_exit_counts_batch(&markers, &counts, &mut output)?;
    }
    RUNTIME_DOCTOR_COMPACT_EXIT_COUNT_LABELS
        .into_iter()
        .zip(output)
        .filter(|(_, count)| *count > 0)
        .map(|(label, count)| {
            usize::try_from(count)
                .map(|count| (label.to_string(), count))
                .map_err(|_| MojoError::InvalidOutput)
        })
        .collect()
}

fn runtime_doctor_compact_exit_counts_batch(
    markers: &[RichStringView],
    counts: &[i64],
    output: &mut [i64; RUNTIME_DOCTOR_COMPACT_EXIT_COUNT_LABELS.len()],
) -> Result<(), MojoError> {
    let count = i64::try_from(markers.len()).map_err(|_| MojoError::InvalidInput)?;
    let status = unsafe {
        prodex_mojo_runtime_doctor_compact_exit_counts_v1(
            RUNTIME_DOCTOR_COMPACT_EXIT_COUNTS_ABI_VERSION,
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
    pub marker: Option<(usize, usize)>,
    pub fields: Vec<RuntimeDoctorMessageFieldRange>,
}

fn runtime_doctor_message_optional_span(
    message: &str,
    start: i64,
    end: i64,
) -> Result<Option<(usize, usize)>, MojoError> {
    match (start, end) {
        (-1, -1) => Ok(None),
        (start, end) if start >= 0 && end >= start => {
            let start = usize::try_from(start).map_err(|_| MojoError::InvalidOutput)?;
            let end = usize::try_from(end).map_err(|_| MojoError::InvalidOutput)?;
            if end > message.len()
                || !message.is_char_boundary(start)
                || !message.is_char_boundary(end)
            {
                return Err(MojoError::InvalidOutput);
            }
            Ok(Some((start, end)))
        }
        _ => Err(MojoError::InvalidOutput),
    }
}

fn runtime_doctor_message_field_range(
    message: &str,
    values: &[i64],
) -> Result<RuntimeDoctorMessageFieldRange, MojoError> {
    let [key_start, key_end, value_start, value_end] = values else {
        return Err(MojoError::InvalidOutput);
    };
    let key_start = usize::try_from(*key_start).map_err(|_| MojoError::InvalidOutput)?;
    let key_end = usize::try_from(*key_end).map_err(|_| MojoError::InvalidOutput)?;
    let value_start = usize::try_from(*value_start).map_err(|_| MojoError::InvalidOutput)?;
    let value_end = usize::try_from(*value_end).map_err(|_| MojoError::InvalidOutput)?;
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
    Ok(RuntimeDoctorMessageFieldRange {
        key_start,
        key_end,
        value_start,
        value_end,
    })
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
        .and_then(|value| value.checked_add(5))
        .ok_or(MojoError::InvalidInput)?;
    let mut output = vec![0_i64; output_capacity];
    let status = unsafe {
        prodex_mojo_runtime_doctor_parse_message_v2(
            RUNTIME_DOCTOR_MESSAGE_PARSE_ABI_VERSION,
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
    let event = runtime_doctor_message_optional_span(message, output[0], output[1])?;
    let marker = runtime_doctor_message_optional_span(message, output[2], output[3])?;
    let field_count = usize::try_from(output[4]).map_err(|_| MojoError::InvalidOutput)?;
    if field_count > max_fields || 5 + field_count * 4 > output.len() {
        return Err(MojoError::InvalidOutput);
    }
    let mut fields = Vec::with_capacity(field_count);
    for index in 0..field_count {
        let base = 5 + index * 4;
        fields.push(runtime_doctor_message_field_range(
            message,
            &output[base..base + 4],
        )?);
    }
    Ok(RuntimeDoctorMessageParsePlan {
        event,
        marker,
        fields,
    })
}

/// Decide whether a parsed runtime-doctor log value is semantically absent.
pub fn runtime_doctor_log_value_is_ignored(value: &str) -> Result<bool, MojoError> {
    ensure_rich_abi()?;
    let value = view(value);
    let mut is_ignored = 0_i64;
    let status = unsafe {
        prodex_mojo_runtime_doctor_log_value_is_ignored_v1(
            RUNTIME_DOCTOR_LOG_VALUE_ABI_VERSION,
            mojo_pointer_address(&value),
            mojo_mut_pointer_address(&mut is_ignored),
        )
    };
    match (status, is_ignored) {
        (0, 0) => Ok(false),
        (0, 1) => Ok(true),
        (1, _) => Err(MojoError::InvalidInput),
        (2, _) => Err(MojoError::InvalidOutput),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Select which parsed Smart Context field supplies fallback reason labels.
pub fn runtime_doctor_smart_context_fallback_reason_source(
    decision: &str,
    has_self_check: bool,
    reason_count: usize,
) -> Result<i64, MojoError> {
    ensure_rich_abi()?;
    let decision = view(decision);
    let reason_count = i64::try_from(reason_count).map_err(|_| MojoError::InvalidInput)?;
    let mut source = 0_i64;
    let status = unsafe {
        prodex_mojo_runtime_doctor_smart_context_fallback_reason_source_v1(
            RUNTIME_DOCTOR_FALLBACK_REASON_SOURCE_ABI_VERSION,
            mojo_pointer_address(&decision),
            i64::from(has_self_check),
            reason_count,
            mojo_mut_pointer_address(&mut source),
        )
    };
    if status != 0 {
        return Err(match status {
            1 => MojoError::InvalidInput,
            2 => MojoError::InvalidOutput,
            _ => MojoError::InvalidOutput,
        });
    }
    match source {
        RUNTIME_DOCTOR_FALLBACK_REASON_SOURCE_EVENT_REASONS
        | RUNTIME_DOCTOR_FALLBACK_REASON_SOURCE_SELF_CHECK
        | RUNTIME_DOCTOR_FALLBACK_REASON_SOURCE_DECISION => Ok(source),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Classify Smart Context autopilot decision labels through the Mojo authority.
pub fn runtime_doctor_smart_context_decision_is_fallback(
    decision: &str,
) -> Result<bool, MojoError> {
    ensure_rich_abi()?;
    let decision = view(decision);
    let mut is_fallback = 0_i64;
    let status = unsafe {
        prodex_mojo_runtime_doctor_smart_context_decision_is_fallback_v1(
            RUNTIME_DOCTOR_SMART_CONTEXT_DECISION_ABI_VERSION,
            mojo_pointer_address(&decision),
            mojo_mut_pointer_address(&mut is_fallback),
        )
    };
    match (status, is_fallback) {
        (0, 0) => Ok(false),
        (0, 1) => Ok(true),
        (1, _) => Err(MojoError::InvalidInput),
        (2, _) => Err(MojoError::InvalidOutput),
        _ => Err(MojoError::InvalidOutput),
    }
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
    fn runtime_doctor_log_value_filter_is_mojo_owned() {
        assert!(runtime_doctor_log_value_is_ignored("").unwrap());
        assert!(runtime_doctor_log_value_is_ignored("-").unwrap());
        assert!(!runtime_doctor_log_value_is_ignored(" - ").unwrap());
        assert!(!runtime_doctor_log_value_is_ignored("value").unwrap());
    }

    #[test]
    fn runtime_doctor_smart_context_fallback_reason_source_is_mojo_owned() {
        assert_eq!(
            runtime_doctor_smart_context_fallback_reason_source("self_check_passthrough", true, 1,)
                .unwrap(),
            RUNTIME_DOCTOR_FALLBACK_REASON_SOURCE_EVENT_REASONS,
        );
        assert_eq!(
            runtime_doctor_smart_context_fallback_reason_source("self_check_passthrough", true, 0,)
                .unwrap(),
            RUNTIME_DOCTOR_FALLBACK_REASON_SOURCE_SELF_CHECK,
        );
        assert_eq!(
            runtime_doctor_smart_context_fallback_reason_source("require_exact", true, 0).unwrap(),
            RUNTIME_DOCTOR_FALLBACK_REASON_SOURCE_DECISION,
        );
        assert_eq!(
            runtime_doctor_smart_context_fallback_reason_source(
                "self_check_passthrough",
                false,
                0,
            )
            .unwrap(),
            RUNTIME_DOCTOR_FALLBACK_REASON_SOURCE_DECISION,
        );
    }

    #[test]
    fn runtime_doctor_smart_context_decision_fallback_classification_is_mojo_owned() {
        assert!(!runtime_doctor_smart_context_decision_is_fallback("rewritten").unwrap());
        assert!(!runtime_doctor_smart_context_decision_is_fallback("pass_through").unwrap());
        assert!(
            runtime_doctor_smart_context_decision_is_fallback("self_check_passthrough").unwrap()
        );
        assert!(runtime_doctor_smart_context_decision_is_fallback("require_exact").unwrap());
        assert!(runtime_doctor_smart_context_decision_is_fallback("").unwrap());
    }

    #[test]
    fn runtime_doctor_message_marker_selection_preserves_event_and_fallback_precedence() {
        let exact = "selection_pick profile=alpha";
        assert_eq!(
            runtime_doctor_parse_message_offsets(exact)
                .unwrap()
                .marker
                .map(|(start, end)| &exact[start..end]),
            Some("selection_pick")
        );

        let prefixed = "notice request_id=req-1 selection_pick profile=alpha";
        assert_eq!(
            runtime_doctor_parse_message_offsets(prefixed)
                .unwrap()
                .marker
                .map(|(start, end)| &prefixed[start..end]),
            Some("selection_pick")
        );

        let fields_first = "request=7 selection_pick profile=alpha";
        assert_eq!(
            runtime_doctor_parse_message_offsets(fields_first)
                .unwrap()
                .marker
                .map(|(start, end)| &fields_first[start..end]),
            Some("selection_pick")
        );

        assert_eq!(
            runtime_doctor_parse_message_offsets("notice unknown=value")
                .unwrap()
                .marker,
            None
        );
    }

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
