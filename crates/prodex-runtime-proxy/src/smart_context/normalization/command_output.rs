use super::*;

pub(in crate::smart_context) fn smart_context_command_output_keep_exact(
    record: SmartContextCommandOutputCacheRecord,
    output: String,
    reason: SmartContextCommandOutputCacheKeepReason,
    summary: Option<String>,
) -> SmartContextCommandOutputCacheRewrite {
    SmartContextCommandOutputCacheRewrite {
        record,
        output,
        action: SmartContextCommandOutputCacheAction::KeepExact { reason, summary },
    }
}

pub(in crate::smart_context) fn smart_context_command_output_cache_record_valid(
    record: &SmartContextCommandOutputCacheRecord,
) -> bool {
    non_empty(&record.content_hash) && record.byte_len > 0
}

pub(in crate::smart_context) fn smart_context_command_output_unchanged_summary(
    current: &SmartContextCommandOutputCacheRecord,
    previous: &SmartContextCommandOutputCacheRecord,
    text: &str,
) -> String {
    let mut summary = format!(
        "psc co same id={} ref={} h={} b={} tok={} vn-repeat omitted",
        smart_context_command_output_label(&current.id),
        smart_context_command_output_label(&previous.id),
        current.content_hash,
        current.byte_len,
        current.estimated_tokens,
    );

    let critical_signals = smart_context_command_output_critical_signals(text);
    if critical_signals.count > 0 {
        summary.push('\n');
        summary.push_str(&format!(
            "psc co crit n={} h={}",
            critical_signals.count, current.content_hash
        ));
        for sample in critical_signals.samples {
            summary.push('\n');
            summary.push_str("sig: ");
            summary.push_str(&sample);
        }
    }

    summary
}

pub(in crate::smart_context) fn smart_context_command_output_changed_summary(
    current: &SmartContextCommandOutputCacheRecord,
    previous: &SmartContextCommandOutputCacheRecord,
) -> String {
    let byte_delta = smart_context_signed_delta(current.byte_len, previous.byte_len);
    let token_delta =
        smart_context_signed_delta_u64(current.estimated_tokens, previous.estimated_tokens);
    format!(
        "psc co delta id={} ref={} old_h={} new_h={} old_b={} new_b={} old_tok={} new_tok={} db={} dtok={} exact kept",
        smart_context_command_output_label(&current.id),
        smart_context_command_output_label(&previous.id),
        previous.content_hash,
        current.content_hash,
        previous.byte_len,
        current.byte_len,
        previous.estimated_tokens,
        current.estimated_tokens,
        byte_delta,
        token_delta,
    )
}

pub(in crate::smart_context) fn smart_context_command_output_label(value: &str) -> String {
    let mut label = String::new();
    for value in value.trim().chars() {
        if label.len() >= 96 {
            break;
        }
        let replacement = if value.is_ascii_alphanumeric()
            || matches!(value, ':' | '_' | '-' | '.' | '/' | '#' | '@')
        {
            value
        } else {
            '_'
        };
        label.push(replacement);
    }

    if label.is_empty() {
        "unknown".to_string()
    } else {
        label
    }
}

pub(in crate::smart_context) fn smart_context_command_output_line_has_critical_signal(
    line: &str,
) -> bool {
    let line = line.to_ascii_lowercase();
    [
        "error",
        "failed",
        "failure",
        "panic",
        "exception",
        "traceback",
        "fatal",
        "denied",
        "not found",
        "segmentation fault",
        "abort",
        "timeout",
    ]
    .iter()
    .any(|signal| line.contains(signal))
}

pub(in crate::smart_context) fn smart_context_command_output_signal_sample(line: &str) -> String {
    let line = line.trim();
    let mut sample =
        smart_context_summary_prefix(line, SMART_CONTEXT_COMMAND_OUTPUT_CRITICAL_SAMPLE_BYTES);
    if sample.len() < line.len() {
        sample.push_str("...");
    }
    sample
}

pub(in crate::smart_context) fn smart_context_signed_delta(
    current: usize,
    previous: usize,
) -> i128 {
    let current = i128::try_from(current).unwrap_or(i128::MAX);
    let previous = i128::try_from(previous).unwrap_or(i128::MAX);
    current.saturating_sub(previous)
}

pub(in crate::smart_context) fn smart_context_signed_delta_u64(
    current: u64,
    previous: u64,
) -> i128 {
    let current = i128::from(current);
    let previous = i128::from(previous);
    current.saturating_sub(previous)
}
