use super::*;

pub const SMART_CONTEXT_COMMAND_OUTPUT_CACHE_MIN_BYTES: usize = 4 * 1024;
pub const SMART_CONTEXT_COMMAND_OUTPUT_CRITICAL_SAMPLE_LIMIT: usize = 3;
pub(super) const SMART_CONTEXT_COMMAND_OUTPUT_CRITICAL_SAMPLE_BYTES: usize = 160;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartContextCommandOutputCacheRecord {
    pub id: String,
    pub content_hash: String,
    pub byte_len: usize,
    pub estimated_tokens: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartContextCommandOutputCacheInput {
    pub id: String,
    pub text: String,
    pub previous_records: Vec<SmartContextCommandOutputCacheRecord>,
    pub min_replacement_bytes: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SmartContextCommandOutputCacheKeepReason {
    BelowMinByteThreshold,
    NoMatchingPreviousOutput,
    ChangedSincePreviousOutput,
    SummaryWouldNotSaveTokens,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SmartContextCommandOutputCacheAction {
    KeepExact {
        reason: SmartContextCommandOutputCacheKeepReason,
        summary: Option<String>,
    },
    ReplaceWithUnchangedSummary {
        ref_id: String,
        saved_tokens: u64,
        critical_signal_count: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartContextCommandOutputCacheRewrite {
    pub record: SmartContextCommandOutputCacheRecord,
    pub output: String,
    pub action: SmartContextCommandOutputCacheAction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartContextCommandOutputCriticalSignals {
    pub count: usize,
    pub samples: Vec<String>,
}

pub fn smart_context_command_output_cache_record(
    id: impl Into<String>,
    text: &str,
) -> SmartContextCommandOutputCacheRecord {
    SmartContextCommandOutputCacheRecord {
        id: id.into(),
        content_hash: smart_context_normalized_command_output_hash_text(text),
        byte_len: text.len(),
        estimated_tokens: smart_context_estimate_tokens_from_body(text.as_bytes()),
    }
}

pub fn smart_context_command_output_cache_rewrite(
    input: SmartContextCommandOutputCacheInput,
) -> SmartContextCommandOutputCacheRewrite {
    let record = smart_context_command_output_cache_record(input.id, &input.text);
    let min_replacement_bytes = if input.min_replacement_bytes == 0 {
        SMART_CONTEXT_COMMAND_OUTPUT_CACHE_MIN_BYTES
    } else {
        input.min_replacement_bytes
    };

    if record.byte_len < min_replacement_bytes {
        return smart_context_command_output_keep_exact(
            record,
            input.text,
            SmartContextCommandOutputCacheKeepReason::BelowMinByteThreshold,
            None,
        );
    }

    let mut previous_records = input
        .previous_records
        .into_iter()
        .filter(smart_context_command_output_cache_record_valid)
        .collect::<Vec<_>>();
    previous_records.sort_by(|left, right| {
        (left.id != record.id)
            .cmp(&(right.id != record.id))
            .then_with(|| left.id.cmp(&right.id))
            .then_with(|| left.content_hash.cmp(&right.content_hash))
            .then_with(|| left.byte_len.cmp(&right.byte_len))
    });

    if let Some(previous) = previous_records
        .iter()
        .find(|previous| previous.content_hash == record.content_hash)
    {
        let summary =
            smart_context_command_output_unchanged_summary(&record, previous, &input.text);
        let summary_estimated_tokens = smart_context_estimate_tokens_from_body(summary.as_bytes());
        if summary.len() >= record.byte_len || summary_estimated_tokens >= record.estimated_tokens {
            return smart_context_command_output_keep_exact(
                record,
                input.text,
                SmartContextCommandOutputCacheKeepReason::SummaryWouldNotSaveTokens,
                None,
            );
        }

        return SmartContextCommandOutputCacheRewrite {
            output: summary,
            action: SmartContextCommandOutputCacheAction::ReplaceWithUnchangedSummary {
                ref_id: previous.id.clone(),
                saved_tokens: record
                    .estimated_tokens
                    .saturating_sub(summary_estimated_tokens),
                critical_signal_count: smart_context_command_output_critical_signals(&input.text)
                    .count,
            },
            record,
        };
    }

    let changed_summary = previous_records
        .iter()
        .find(|previous| previous.id == record.id)
        .map(|previous| smart_context_command_output_changed_summary(&record, previous));
    let reason = if changed_summary.is_some() {
        SmartContextCommandOutputCacheKeepReason::ChangedSincePreviousOutput
    } else {
        SmartContextCommandOutputCacheKeepReason::NoMatchingPreviousOutput
    };

    smart_context_command_output_keep_exact(record, input.text, reason, changed_summary)
}

pub fn smart_context_command_output_critical_signals(
    text: &str,
) -> SmartContextCommandOutputCriticalSignals {
    let mut count = 0usize;
    let mut samples = Vec::new();

    for line in text.lines() {
        if !smart_context_command_output_line_has_critical_signal(line) {
            continue;
        }
        count += 1;
        if samples.len() < SMART_CONTEXT_COMMAND_OUTPUT_CRITICAL_SAMPLE_LIMIT {
            samples.push(smart_context_command_output_signal_sample(line));
        }
    }

    SmartContextCommandOutputCriticalSignals { count, samples }
}
