use super::constants::*;
use super::rehydration::{
    runtime_smart_context_append_missing_critical_ranges,
    runtime_smart_context_compact_line_refs_if_shorter, runtime_smart_context_line_excerpt,
    runtime_smart_context_progressive_critical_exact_ranges,
};
use super::types::{
    RuntimeSmartContextArtifactIndexes, RuntimeSmartContextDuplicateChunkSummaryPlan,
    RuntimeSmartContextIntentSignals, RuntimeSmartContextToolArgumentCandidate,
    RuntimeSmartContextToolArgumentDelta, RuntimeSmartContextToolCallMetadata,
    RuntimeSmartContextToolOutputCompactionMetadata, RuntimeSmartContextTransformStats,
};
use super::{
    runtime_smart_context_artifact_line_ref, runtime_smart_context_artifact_marker_line,
    runtime_smart_context_artifact_ref, runtime_smart_context_likely_blob_or_noise,
};
use crate::runtime_state_shared::{
    RuntimeSmartContextArtifactChunkIndex, RuntimeSmartContextArtifactDuplicateChunkFingerprint,
    RuntimeSmartContextArtifactStore,
};
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn runtime_smart_context_condense_tool_outputs(
    value: &mut serde_json::Value,
    store: &mut RuntimeSmartContextArtifactStore,
    request_id: u64,
    tier: runtime_proxy_crate::SmartContextTokenBudgetTier,
    inline_limit: usize,
    intent_signals: &RuntimeSmartContextIntentSignals,
    stats: &mut RuntimeSmartContextTransformStats,
) {
    let Some(input) = value
        .get_mut("input")
        .and_then(serde_json::Value::as_array_mut)
    else {
        return;
    };
    let tool_call_metadata = runtime_smart_context_tool_call_metadata_by_call_id(input);
    for item in input {
        let Some(object) = item.as_object_mut() else {
            continue;
        };
        let item_type = object
            .get("type")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        if !item_type.ends_with("_call_output") {
            continue;
        }
        let linked_metadata = runtime_smart_context_tool_call_id(object)
            .and_then(|call_id| tool_call_metadata.get(call_id));
        let compaction_metadata =
            runtime_smart_context_tool_output_compaction_metadata(object, linked_metadata);
        for field in ["output", "content"] {
            let Some(output) = object
                .get(field)
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
            else {
                continue;
            };
            if output.len() <= inline_limit.max(256) {
                continue;
            }
            let existing_artifact = store.artifact_ref_for_exact_text(&output);
            let Some(artifact) = existing_artifact
                .clone()
                .or_else(|| store.insert_text(request_id, &output))
            else {
                continue;
            };
            let replacement = if existing_artifact.is_some() {
                stats.repeat_tool_output_refs += 1;
                runtime_smart_context_repeat_tool_output_reference_summary(
                    &artifact,
                    &output,
                    runtime_smart_context_tool_call_id(object),
                )
            } else {
                let compacted = runtime_smart_context_progressive_tool_output_summary(
                    &artifact,
                    &output,
                    RuntimeSmartContextArtifactIndexes {
                        line_index: store.line_index(&artifact.id),
                        chunk_index: store.chunk_index(&artifact.id),
                    },
                    tier,
                    inline_limit,
                    &compaction_metadata,
                    &intent_signals.intent_terms,
                );
                runtime_smart_context_artifact_summary(&artifact, &compacted)
            };
            if replacement.len().saturating_mul(100) >= output.len().saturating_mul(90) {
                continue;
            }
            object.insert(field.to_string(), serde_json::Value::String(replacement));
            if existing_artifact.is_none() {
                stats.artifacts_stored += 1;
            }
            stats.tool_outputs_condensed += 1;
            if runtime_smart_context_likely_blob_or_noise(&output) {
                stats.blob_outputs_condensed += 1;
            }
            break;
        }
    }
}

pub(super) fn runtime_smart_context_condense_historical_tool_call_arguments(
    value: &mut serde_json::Value,
    store: &mut RuntimeSmartContextArtifactStore,
    request_id: u64,
    tier: runtime_proxy_crate::SmartContextTokenBudgetTier,
    inline_limit: usize,
    stats: &mut RuntimeSmartContextTransformStats,
) {
    let Some(input) = value
        .get_mut("input")
        .and_then(serde_json::Value::as_array_mut)
    else {
        return;
    };
    let completed_call_ids = input
        .iter()
        .filter_map(|item| {
            let object = item.as_object()?;
            let item_type = object
                .get("type")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default();
            item_type
                .ends_with("_call_output")
                .then(|| runtime_smart_context_tool_call_id(object))
                .flatten()
                .map(str::to_string)
        })
        .collect::<BTreeSet<_>>();
    if completed_call_ids.is_empty() {
        return;
    }

    let mut previous_arguments = Vec::<RuntimeSmartContextToolArgumentCandidate>::new();
    for item in input {
        let Some(object) = item.as_object_mut() else {
            continue;
        };
        let Some(call_id) = runtime_smart_context_tool_call_id(object) else {
            continue;
        };
        if !completed_call_ids.contains(call_id) {
            continue;
        }
        let item_type = object
            .get("type")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        if item_type.ends_with("_call_output") {
            continue;
        }
        for field in ["arguments", "input", "payload"] {
            let Some(arguments) = object.get(field) else {
                continue;
            };
            let Some(arguments_text) = runtime_smart_context_tool_argument_text(arguments) else {
                continue;
            };
            if arguments_text.len()
                <= inline_limit.clamp(256, SMART_CONTEXT_TOOL_ARGS_INLINE_MIN_BYTES)
            {
                continue;
            }
            let existing_artifact = store.artifact_ref_for_exact_text(&arguments_text);
            let Some(artifact) = existing_artifact
                .clone()
                .or_else(|| store.insert_text(request_id, &arguments_text))
            else {
                continue;
            };
            let replacement = runtime_smart_context_tool_argument_replacement(
                &artifact,
                &arguments_text,
                tier,
                existing_artifact.is_some(),
                &previous_arguments,
            );
            let candidate = RuntimeSmartContextToolArgumentCandidate {
                artifact: artifact.clone(),
                text: arguments_text.clone(),
            };
            if replacement.len().saturating_mul(100) >= arguments_text.len().saturating_mul(75) {
                previous_arguments.push(candidate);
                continue;
            }
            object.insert(field.to_string(), serde_json::Value::String(replacement));
            if existing_artifact.is_none() {
                stats.artifacts_stored = stats.artifacts_stored.saturating_add(1);
            }
            stats.tool_call_args_condensed = stats.tool_call_args_condensed.saturating_add(1);
            previous_arguments.push(candidate);
            break;
        }
    }
}

fn runtime_smart_context_tool_argument_text(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(text) if !text.trim().is_empty() => Some(text.clone()),
        serde_json::Value::Object(_) | serde_json::Value::Array(_) => {
            serde_json::to_string(value).ok()
        }
        _ => None,
    }
}

fn runtime_smart_context_tool_argument_replacement(
    artifact: &runtime_proxy_crate::SmartContextArtifactRef,
    text: &str,
    tier: runtime_proxy_crate::SmartContextTokenBudgetTier,
    already_stored: bool,
    previous_arguments: &[RuntimeSmartContextToolArgumentCandidate],
) -> String {
    if let Some(previous) = previous_arguments
        .iter()
        .find(|previous| previous.text == text)
    {
        return runtime_smart_context_tool_argument_repeat_summary(
            artifact,
            Some(&previous.artifact),
        );
    }
    if already_stored {
        return runtime_smart_context_tool_argument_repeat_summary(artifact, None);
    }
    if let Some(delta) = runtime_smart_context_tool_argument_delta(text, previous_arguments, tier) {
        return runtime_smart_context_tool_argument_delta_summary(artifact, &delta);
    }
    runtime_smart_context_tool_argument_summary(artifact, text, tier)
}

fn runtime_smart_context_tool_argument_summary(
    artifact: &runtime_proxy_crate::SmartContextArtifactRef,
    text: &str,
    tier: runtime_proxy_crate::SmartContextTokenBudgetTier,
) -> String {
    let reference = runtime_smart_context_artifact_ref(&artifact.id);
    let preview = text
        .chars()
        .take(runtime_smart_context_tool_args_preview_max_chars(tier))
        .collect::<String>();
    format!(
        "psc args {reference} b={} p:{}",
        artifact.byte_len,
        preview.trim()
    )
}

fn runtime_smart_context_tool_argument_repeat_summary(
    artifact: &runtime_proxy_crate::SmartContextArtifactRef,
    previous_artifact: Option<&runtime_proxy_crate::SmartContextArtifactRef>,
) -> String {
    let reference = runtime_smart_context_artifact_ref(&artifact.id);
    let base_reference = previous_artifact
        .filter(|previous| previous.id != artifact.id)
        .map(|previous| format!(" same={}", runtime_smart_context_artifact_ref(&previous.id)))
        .unwrap_or_default();
    format!(
        "psc args rep {reference} b={}{}",
        artifact.byte_len, base_reference
    )
}

fn runtime_smart_context_tool_argument_delta_summary(
    artifact: &runtime_proxy_crate::SmartContextArtifactRef,
    delta: &RuntimeSmartContextToolArgumentDelta,
) -> String {
    let reference = runtime_smart_context_artifact_ref(&artifact.id);
    let base_reference = runtime_smart_context_artifact_ref(&delta.base_artifact.id);
    let preview = if delta.inserted_preview.is_empty() {
        String::new()
    } else {
        format!(" p:{}", delta.inserted_preview.trim())
    };
    format!(
        "psc args d {reference} b={} base={base_reference} pre={} suf={} -{} +{} ih={}{}",
        artifact.byte_len,
        delta.prefix_bytes,
        delta.suffix_bytes,
        delta.removed_bytes,
        delta.inserted_bytes,
        delta.inserted_hash,
        preview
    )
}

fn runtime_smart_context_tool_argument_delta(
    text: &str,
    previous_arguments: &[RuntimeSmartContextToolArgumentCandidate],
    tier: runtime_proxy_crate::SmartContextTokenBudgetTier,
) -> Option<RuntimeSmartContextToolArgumentDelta> {
    previous_arguments
        .iter()
        .filter_map(|previous| {
            runtime_smart_context_tool_argument_delta_against(text, previous, tier)
        })
        .max_by(|left, right| {
            left.common_bytes
                .cmp(&right.common_bytes)
                .then_with(|| right.changed_bytes.cmp(&left.changed_bytes))
                .then_with(|| left.base_artifact.id.cmp(&right.base_artifact.id))
        })
}

fn runtime_smart_context_tool_argument_delta_against(
    text: &str,
    previous: &RuntimeSmartContextToolArgumentCandidate,
    tier: runtime_proxy_crate::SmartContextTokenBudgetTier,
) -> Option<RuntimeSmartContextToolArgumentDelta> {
    let base = previous.text.as_str();
    if text == base {
        return None;
    }

    let prefix_bytes = runtime_smart_context_common_prefix_boundary_len(text, base);
    let suffix_bytes = runtime_smart_context_common_suffix_boundary_len(text, base, prefix_bytes);
    let common_bytes = prefix_bytes.saturating_add(suffix_bytes);
    let max_len = text.len().max(base.len());
    if common_bytes < SMART_CONTEXT_TOOL_ARGS_DIFF_MIN_COMMON_BYTES
        || common_bytes.saturating_mul(100)
            < max_len.saturating_mul(SMART_CONTEXT_TOOL_ARGS_DIFF_MIN_COMMON_RATIO_PERCENT)
    {
        return None;
    }

    let removed_bytes = base
        .len()
        .saturating_sub(prefix_bytes)
        .saturating_sub(suffix_bytes);
    let inserted_bytes = text
        .len()
        .saturating_sub(prefix_bytes)
        .saturating_sub(suffix_bytes);
    let changed_bytes = removed_bytes.saturating_add(inserted_bytes);
    if changed_bytes.saturating_mul(100)
        > max_len.saturating_mul(SMART_CONTEXT_TOOL_ARGS_DIFF_MAX_CHANGED_RATIO_PERCENT)
    {
        return None;
    }

    let inserted_end = text.len().saturating_sub(suffix_bytes);
    let inserted = text.get(prefix_bytes..inserted_end)?;
    let inserted_preview = inserted
        .chars()
        .take(runtime_smart_context_tool_args_preview_max_chars(tier))
        .collect::<String>();
    Some(RuntimeSmartContextToolArgumentDelta {
        base_artifact: previous.artifact.clone(),
        prefix_bytes,
        suffix_bytes,
        removed_bytes,
        inserted_bytes,
        inserted_hash: runtime_proxy_crate::smart_context_hash_text(inserted),
        inserted_preview,
        common_bytes,
        changed_bytes,
    })
}

fn runtime_smart_context_common_prefix_boundary_len(left: &str, right: &str) -> usize {
    let max_len = left.len().min(right.len());
    let mut len = 0usize;
    let left_bytes = left.as_bytes();
    let right_bytes = right.as_bytes();
    while len < max_len && left_bytes[len] == right_bytes[len] {
        len += 1;
    }
    while len > 0 && (!left.is_char_boundary(len) || !right.is_char_boundary(len)) {
        len -= 1;
    }
    len
}

fn runtime_smart_context_common_suffix_boundary_len(
    left: &str,
    right: &str,
    prefix_bytes: usize,
) -> usize {
    let max_len = left.len().min(right.len()).saturating_sub(prefix_bytes);
    let mut len = 0usize;
    let left_bytes = left.as_bytes();
    let right_bytes = right.as_bytes();
    while len < max_len && left_bytes[left.len() - len - 1] == right_bytes[right.len() - len - 1] {
        len += 1;
    }
    while len > 0
        && (!left.is_char_boundary(left.len() - len) || !right.is_char_boundary(right.len() - len))
    {
        len -= 1;
    }
    len
}

fn runtime_smart_context_tool_args_preview_max_chars(
    tier: runtime_proxy_crate::SmartContextTokenBudgetTier,
) -> usize {
    match tier {
        runtime_proxy_crate::SmartContextTokenBudgetTier::Minimal => 160,
        runtime_proxy_crate::SmartContextTokenBudgetTier::Condensed => 240,
        runtime_proxy_crate::SmartContextTokenBudgetTier::Large => 360,
        runtime_proxy_crate::SmartContextTokenBudgetTier::Exact => 480,
    }
}

fn runtime_smart_context_progressive_tool_output_summary(
    artifact: &runtime_proxy_crate::SmartContextArtifactRef,
    text: &str,
    indexes: RuntimeSmartContextArtifactIndexes<'_>,
    tier: runtime_proxy_crate::SmartContextTokenBudgetTier,
    preview_byte_limit: usize,
    metadata: &RuntimeSmartContextToolOutputCompactionMetadata,
    intent_terms: &[String],
) -> String {
    let compacted = runtime_smart_context_compact_successful_tool_output(
        text,
        tier,
        preview_byte_limit,
        metadata,
    )
    .unwrap_or_else(|| {
        runtime_smart_context_compact_tool_output_preserving_critical(
            text,
            tier,
            preview_byte_limit,
            metadata.kind_hint,
            intent_terms,
        )
    });
    let summary = runtime_smart_context_progressive_summary_excerpt(&compacted);
    let summary = runtime_smart_context_dedupe_progressive_summary_chunks(
        &artifact.id,
        text,
        &summary,
        indexes.chunk_index,
    );
    let mut sections = Vec::new();
    if !summary.trim().is_empty() {
        sections.push(format!(
            "{SMART_CONTEXT_LABEL_SUMMARY}\n{}",
            summary.trim_end()
        ));
    }
    if let Some(critical_ranges) = runtime_smart_context_progressive_critical_exact_ranges(
        &artifact.id,
        text,
        indexes.line_index,
        &summary,
    ) {
        sections.push(critical_ranges);
    }
    if sections.is_empty() {
        sections.push(format!(
            "{SMART_CONTEXT_LABEL_SUMMARY}\nlarge output omitted; use ref/lines"
        ));
    }
    sections.join("\n\n")
}

fn runtime_smart_context_progressive_summary_excerpt(text: &str) -> String {
    if text.len() <= SMART_CONTEXT_TOOL_PROGRESSIVE_SUMMARY_MAX_BYTES {
        return text.to_string();
    }

    let mut summary = String::new();
    for line in text.lines() {
        let next_len = summary
            .len()
            .saturating_add((!summary.is_empty()) as usize)
            .saturating_add(line.len());
        if next_len > SMART_CONTEXT_TOOL_PROGRESSIVE_SUMMARY_MAX_BYTES {
            break;
        }
        if !summary.is_empty() {
            summary.push('\n');
        }
        summary.push_str(line);
    }
    if summary.is_empty() {
        summary.extend(
            text.chars()
                .take(SMART_CONTEXT_TOOL_PROGRESSIVE_SUMMARY_MAX_BYTES),
        );
    }
    summary.push_str("\n[trunc; use ref for full]");
    summary
}

pub(super) fn runtime_smart_context_dedupe_progressive_summary_chunks(
    artifact_id: &str,
    original: &str,
    summary: &str,
    chunk_index: Option<&RuntimeSmartContextArtifactChunkIndex>,
) -> String {
    let Some(chunk_index) = chunk_index.filter(|index| index.complete) else {
        return summary.to_string();
    };
    if chunk_index.duplicate_chunks.is_empty() {
        return summary.to_string();
    }
    let lines = original.lines().collect::<Vec<_>>();
    let plans = chunk_index
        .duplicate_chunks
        .iter()
        .filter_map(|duplicate| {
            runtime_smart_context_duplicate_chunk_summary_plan(artifact_id, &lines, duplicate)
        })
        .collect::<Vec<_>>();
    if plans.is_empty() {
        return summary.to_string();
    }

    let mut candidate = summary.to_string();
    let mut entries = Vec::new();
    for plan in plans {
        let matches = candidate.match_indices(&plan.text).count();
        if matches < 2 {
            continue;
        }
        let marker = format!("[psc dup h={} b={}]", plan.content_hash, plan.byte_len);
        let entry = format!(
            "- h={} b={} x={} refs={}",
            plan.content_hash,
            plan.byte_len,
            plan.occurrence_count,
            runtime_smart_context_compact_line_refs_if_shorter(&plan.refs)
        );
        let removed_bytes = plan.text.len().saturating_mul(matches.saturating_sub(1));
        let added_bytes = marker
            .len()
            .saturating_mul(matches.saturating_sub(1))
            .saturating_add(entry.len())
            .saturating_add(1);
        if added_bytes >= removed_bytes {
            continue;
        }
        candidate = runtime_smart_context_replace_repeated_exact_text_after_first(
            &candidate, &plan.text, &marker,
        );
        entries.push(entry);
    }

    if entries.is_empty() {
        return summary.to_string();
    }
    candidate.push_str("\n\n");
    candidate.push_str(SMART_CONTEXT_LABEL_DUPLICATE_CHUNKS);
    candidate.push('\n');
    candidate.push_str(&entries.join("\n"));
    if candidate.len() < summary.len() {
        candidate
    } else {
        summary.to_string()
    }
}

fn runtime_smart_context_duplicate_chunk_summary_plan(
    artifact_id: &str,
    lines: &[&str],
    duplicate: &RuntimeSmartContextArtifactDuplicateChunkFingerprint,
) -> Option<RuntimeSmartContextDuplicateChunkSummaryPlan> {
    if duplicate.occurrence_count < 2
        || duplicate.occurrence_count != duplicate.occurrences.len()
        || duplicate.byte_len == 0
    {
        return None;
    }
    let mut refs = Vec::new();
    let mut ranges = BTreeSet::<(usize, usize)>::new();
    let mut exact_text: Option<String> = None;
    for occurrence in &duplicate.occurrences {
        if occurrence.start == 0
            || occurrence.end < occurrence.start
            || !ranges.insert((occurrence.start, occurrence.end))
        {
            return None;
        }
        let text = runtime_smart_context_line_excerpt(lines, occurrence.start, occurrence.end)?;
        if text.len() != duplicate.byte_len
            || runtime_proxy_crate::smart_context_hash_text(&text) != duplicate.content_hash
        {
            return None;
        }
        if let Some(existing) = exact_text.as_ref() {
            if existing != &text {
                return None;
            }
        } else {
            exact_text = Some(text);
        }
        refs.push(runtime_smart_context_artifact_line_ref(
            artifact_id,
            occurrence.start,
            occurrence.end,
        ));
    }
    Some(RuntimeSmartContextDuplicateChunkSummaryPlan {
        text: exact_text?,
        content_hash: duplicate.content_hash.clone(),
        byte_len: duplicate.byte_len,
        occurrence_count: duplicate.occurrence_count,
        refs,
    })
}

fn runtime_smart_context_replace_repeated_exact_text_after_first(
    text: &str,
    needle: &str,
    replacement: &str,
) -> String {
    if needle.is_empty() {
        return text.to_string();
    }
    let mut rendered = String::with_capacity(text.len());
    let mut cursor = 0usize;
    let mut seen = 0usize;
    for (offset, _) in text.match_indices(needle) {
        rendered.push_str(&text[cursor..offset]);
        seen = seen.saturating_add(1);
        if seen == 1 {
            rendered.push_str(needle);
        } else {
            rendered.push_str(replacement);
        }
        cursor = offset.saturating_add(needle.len());
    }
    rendered.push_str(&text[cursor..]);
    rendered
}

fn runtime_smart_context_compact_successful_tool_output(
    text: &str,
    tier: runtime_proxy_crate::SmartContextTokenBudgetTier,
    preview_byte_limit: usize,
    metadata: &RuntimeSmartContextToolOutputCompactionMetadata,
) -> Option<String> {
    let max_lines =
        runtime_smart_context_tool_preview_max_lines(tier, preview_byte_limit).unwrap_or(40);
    let report = prodex_context::compact_successful_command_output_with_options(
        text,
        &prodex_context::CommandSuccessOutputCompactOptions {
            command: metadata.command.clone(),
            exit_code: metadata.exit_code,
            min_lines_to_compact: max_lines.saturating_mul(2).max(20),
            max_touched_files: max_lines.clamp(8, 48),
            max_key_lines: (max_lines / 2).clamp(4, 24),
            max_line_chars: SMART_CONTEXT_TOOL_PREVIEW_MAX_LINE_CHARS,
        },
    );
    if report.compacted && !report.failure_suspected && report.output.len() < text.len() {
        Some(report.output)
    } else {
        None
    }
}

fn runtime_smart_context_compact_tool_output_preserving_critical(
    text: &str,
    tier: runtime_proxy_crate::SmartContextTokenBudgetTier,
    preview_byte_limit: usize,
    command_kind_hint: Option<prodex_context::CommandOutputKind>,
    intent_terms: &[String],
) -> String {
    let compacted = runtime_smart_context_compact_tool_output(
        text,
        tier,
        preview_byte_limit,
        command_kind_hint,
        intent_terms,
    );
    runtime_smart_context_append_missing_critical_ranges(
        text,
        compacted,
        SMART_CONTEXT_SURGICAL_CRITICAL_MAX_RANGES,
    )
}

fn runtime_smart_context_compact_tool_output(
    text: &str,
    tier: runtime_proxy_crate::SmartContextTokenBudgetTier,
    preview_byte_limit: usize,
    command_kind_hint: Option<prodex_context::CommandOutputKind>,
    intent_terms: &[String],
) -> String {
    let Some(max_lines) = runtime_smart_context_tool_preview_max_lines(tier, preview_byte_limit)
    else {
        return text.to_string();
    };
    let options = prodex_context::CommandOutputCompactOptions {
        max_lines,
        head_lines: max_lines.saturating_mul(2) / 3,
        tail_lines: max_lines / 3,
        max_line_chars: SMART_CONTEXT_TOOL_PREVIEW_MAX_LINE_CHARS,
        ..prodex_context::CommandOutputCompactOptions::default()
    };
    prodex_context::compact_command_output_with_intent_options(
        text,
        &prodex_context::CommandOutputIntentCompactOptions::new(options, intent_terms.to_vec())
            .with_kind_hint(command_kind_hint),
    )
    .output
}

pub(super) fn runtime_smart_context_tool_preview_max_lines(
    tier: runtime_proxy_crate::SmartContextTokenBudgetTier,
    preview_byte_limit: usize,
) -> Option<usize> {
    if tier == runtime_proxy_crate::SmartContextTokenBudgetTier::Exact
        && preview_byte_limit == usize::MAX
    {
        return None;
    }

    let budget_lines = preview_byte_limit / SMART_CONTEXT_TOOL_PREVIEW_ESTIMATED_LINE_BYTES;
    let (floor, cap) = match tier {
        runtime_proxy_crate::SmartContextTokenBudgetTier::Minimal => (8, 24),
        runtime_proxy_crate::SmartContextTokenBudgetTier::Condensed => (24, 80),
        runtime_proxy_crate::SmartContextTokenBudgetTier::Large => (80, 240),
        runtime_proxy_crate::SmartContextTokenBudgetTier::Exact => (120, 360),
    };
    Some(budget_lines.max(floor).min(cap))
}

pub(super) fn runtime_smart_context_tool_call_metadata_by_call_id(
    input: &[serde_json::Value],
) -> BTreeMap<String, RuntimeSmartContextToolCallMetadata> {
    let mut metadata_by_call_id = BTreeMap::new();
    for item in input {
        let Some(object) = item.as_object() else {
            continue;
        };
        let Some(call_id) = runtime_smart_context_tool_call_id(object) else {
            continue;
        };
        let item_type = object
            .get("type")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        if item_type.ends_with("_call_output") {
            continue;
        }
        let metadata = runtime_smart_context_tool_item_metadata(object);
        if metadata.command.is_some()
            || metadata.exit_code.is_some()
            || metadata.kind_hint.is_some()
        {
            metadata_by_call_id.insert(call_id.to_string(), metadata);
        }
    }
    metadata_by_call_id
}

fn runtime_smart_context_tool_output_compaction_metadata(
    object: &serde_json::Map<String, serde_json::Value>,
    linked_metadata: Option<&RuntimeSmartContextToolCallMetadata>,
) -> RuntimeSmartContextToolOutputCompactionMetadata {
    let local_metadata = runtime_smart_context_tool_item_metadata(object);
    let command = linked_metadata
        .and_then(|metadata| metadata.command.clone())
        .or(local_metadata.command);
    let exit_code = local_metadata
        .exit_code
        .or_else(|| linked_metadata.and_then(|metadata| metadata.exit_code));
    let kind_hint = local_metadata
        .kind_hint
        .or_else(|| linked_metadata.and_then(|metadata| metadata.kind_hint))
        .or_else(|| {
            command
                .as_deref()
                .and_then(prodex_context::command_output_kind_hint_for_command)
        });
    RuntimeSmartContextToolOutputCompactionMetadata {
        kind_hint,
        command,
        exit_code,
    }
}

const SMART_CONTEXT_TOOL_COMMAND_HINT_MAX_CHARS: usize = 512;
const SMART_CONTEXT_TOOL_METADATA_SCAN_MAX_DEPTH: usize = 8;

pub(super) fn runtime_smart_context_tool_item_metadata(
    object: &serde_json::Map<String, serde_json::Value>,
) -> RuntimeSmartContextToolCallMetadata {
    let command = runtime_smart_context_tool_command_hint(object);
    let exit_code = runtime_smart_context_tool_exit_code_hint(object);
    let kind_hint = runtime_smart_context_tool_kind_hint(object, command.as_deref());
    RuntimeSmartContextToolCallMetadata {
        command,
        exit_code,
        kind_hint,
    }
}

fn runtime_smart_context_tool_command_hint(
    object: &serde_json::Map<String, serde_json::Value>,
) -> Option<String> {
    runtime_smart_context_tool_command_hint_from_object(object, 0)
}

fn runtime_smart_context_tool_command_hint_from_object(
    object: &serde_json::Map<String, serde_json::Value>,
    depth: usize,
) -> Option<String> {
    if depth > SMART_CONTEXT_TOOL_METADATA_SCAN_MAX_DEPTH {
        return None;
    }
    for key in runtime_smart_context_tool_command_keys() {
        if let Some(command) = object
            .get(key)
            .and_then(serde_json::Value::as_str)
            .and_then(runtime_smart_context_bounded_tool_command)
        {
            return Some(command);
        }
    }
    if let Some(command) = object.get("arguments").and_then(|value| {
        runtime_smart_context_tool_command_hint_from_value(value, depth + 1, true)
    }) {
        return Some(command);
    }
    for (field, child) in object {
        if field == "arguments" || runtime_smart_context_tool_output_payload_field(field) {
            continue;
        }
        if let Some(command) =
            runtime_smart_context_tool_command_hint_from_value(child, depth + 1, false)
        {
            return Some(command);
        }
    }
    None
}

fn runtime_smart_context_tool_command_hint_from_value(
    value: &serde_json::Value,
    depth: usize,
    allow_plain_command: bool,
) -> Option<String> {
    if depth > SMART_CONTEXT_TOOL_METADATA_SCAN_MAX_DEPTH {
        return None;
    }
    match value {
        serde_json::Value::String(text) => {
            if let Some(command) = runtime_smart_context_bounded_tool_command(text)
                && allow_plain_command
                && !text.trim_start().starts_with('{')
            {
                return Some(command);
            }
            serde_json::from_str::<serde_json::Value>(text)
                .ok()
                .and_then(|value| {
                    runtime_smart_context_tool_command_hint_from_value(
                        &value,
                        depth + 1,
                        allow_plain_command,
                    )
                })
        }
        serde_json::Value::Object(object) => {
            runtime_smart_context_tool_command_hint_from_object(object, depth + 1)
        }
        serde_json::Value::Array(items) => items.iter().find_map(|item| {
            runtime_smart_context_tool_command_hint_from_value(item, depth + 1, allow_plain_command)
        }),
        _ => None,
    }
}

fn runtime_smart_context_tool_command_keys() -> [&'static str; 4] {
    ["cmd", "command", "shell_command", "shellCommand"]
}

fn runtime_smart_context_bounded_tool_command(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(
        trimmed
            .chars()
            .take(SMART_CONTEXT_TOOL_COMMAND_HINT_MAX_CHARS)
            .collect(),
    )
}

fn runtime_smart_context_tool_exit_code_hint(
    object: &serde_json::Map<String, serde_json::Value>,
) -> Option<i32> {
    runtime_smart_context_tool_exit_code_hint_from_object(object, 0)
}

fn runtime_smart_context_tool_exit_code_hint_from_object(
    object: &serde_json::Map<String, serde_json::Value>,
    depth: usize,
) -> Option<i32> {
    if depth > SMART_CONTEXT_TOOL_METADATA_SCAN_MAX_DEPTH {
        return None;
    }
    for key in runtime_smart_context_tool_exit_code_keys() {
        if let Some(exit_code) = object
            .get(key)
            .and_then(runtime_smart_context_i32_from_json_value)
        {
            return Some(exit_code);
        }
    }
    if let Some(exit_code) = object.get("arguments").and_then(|value| {
        runtime_smart_context_tool_exit_code_hint_from_value(value, depth + 1, true)
    }) {
        return Some(exit_code);
    }
    for (field, child) in object {
        if field == "arguments" || runtime_smart_context_tool_output_payload_field(field) {
            continue;
        }
        if let Some(exit_code) =
            runtime_smart_context_tool_exit_code_hint_from_value(child, depth + 1, false)
        {
            return Some(exit_code);
        }
    }
    None
}

fn runtime_smart_context_tool_exit_code_hint_from_value(
    value: &serde_json::Value,
    depth: usize,
    allow_plain_exit_code: bool,
) -> Option<i32> {
    if depth > SMART_CONTEXT_TOOL_METADATA_SCAN_MAX_DEPTH {
        return None;
    }
    match value {
        serde_json::Value::String(text) => serde_json::from_str::<serde_json::Value>(text)
            .ok()
            .and_then(|value| {
                runtime_smart_context_tool_exit_code_hint_from_value(
                    &value,
                    depth + 1,
                    allow_plain_exit_code,
                )
            }),
        serde_json::Value::Object(object) => {
            runtime_smart_context_tool_exit_code_hint_from_object(object, depth + 1)
        }
        serde_json::Value::Array(items) => items.iter().find_map(|item| {
            runtime_smart_context_tool_exit_code_hint_from_value(
                item,
                depth + 1,
                allow_plain_exit_code,
            )
        }),
        _ => allow_plain_exit_code
            .then(|| runtime_smart_context_i32_from_json_value(value))
            .flatten(),
    }
}

fn runtime_smart_context_tool_exit_code_keys() -> [&'static str; 7] {
    [
        "exit_code",
        "exitCode",
        "exit_status",
        "exitStatus",
        "status_code",
        "statusCode",
        "status",
    ]
}

fn runtime_smart_context_tool_kind_hint(
    object: &serde_json::Map<String, serde_json::Value>,
    command: Option<&str>,
) -> Option<prodex_context::CommandOutputKind> {
    command
        .and_then(prodex_context::command_output_kind_hint_for_command)
        .or_else(|| runtime_smart_context_tool_kind_hint_from_object(object, 0))
}

fn runtime_smart_context_tool_kind_hint_from_object(
    object: &serde_json::Map<String, serde_json::Value>,
    depth: usize,
) -> Option<prodex_context::CommandOutputKind> {
    if depth > SMART_CONTEXT_TOOL_METADATA_SCAN_MAX_DEPTH {
        return None;
    }
    for key in runtime_smart_context_tool_kind_keys() {
        if let Some(kind) = object
            .get(key)
            .and_then(serde_json::Value::as_str)
            .and_then(runtime_smart_context_tool_kind_hint_from_text)
        {
            return Some(kind);
        }
    }
    if let Some(kind) = object
        .get("arguments")
        .and_then(|value| runtime_smart_context_tool_kind_hint_from_value(value, depth + 1, true))
    {
        return Some(kind);
    }
    for (field, child) in object {
        if field == "arguments" || runtime_smart_context_tool_output_payload_field(field) {
            continue;
        }
        if let Some(kind) = runtime_smart_context_tool_kind_hint_from_value(child, depth + 1, false)
        {
            return Some(kind);
        }
    }
    None
}

fn runtime_smart_context_tool_kind_hint_from_value(
    value: &serde_json::Value,
    depth: usize,
    allow_plain_kind: bool,
) -> Option<prodex_context::CommandOutputKind> {
    if depth > SMART_CONTEXT_TOOL_METADATA_SCAN_MAX_DEPTH {
        return None;
    }
    match value {
        serde_json::Value::String(text) => serde_json::from_str::<serde_json::Value>(text)
            .ok()
            .and_then(|value| {
                runtime_smart_context_tool_kind_hint_from_value(&value, depth + 1, allow_plain_kind)
            })
            .or_else(|| {
                allow_plain_kind
                    .then(|| runtime_smart_context_tool_kind_hint_from_text(text))
                    .flatten()
            }),
        serde_json::Value::Object(object) => {
            runtime_smart_context_tool_kind_hint_from_object(object, depth + 1)
        }
        serde_json::Value::Array(items) => items.iter().find_map(|item| {
            runtime_smart_context_tool_kind_hint_from_value(item, depth + 1, allow_plain_kind)
        }),
        _ => None,
    }
}

fn runtime_smart_context_tool_kind_hint_from_text(
    text: &str,
) -> Option<prodex_context::CommandOutputKind> {
    if text.len() > SMART_CONTEXT_TOOL_COMMAND_HINT_MAX_CHARS {
        return None;
    }
    let normalized = text.trim().to_ascii_lowercase().replace('_', "-");
    match normalized.as_str() {
        "git-status" | "status" => Some(prodex_context::CommandOutputKind::GitStatus),
        "git-diff" | "diff" | "git-show" => Some(prodex_context::CommandOutputKind::GitDiff),
        "rust-diagnostics" | "cargo-test" | "cargo-check" | "cargo-clippy" | "rust" => {
            Some(prodex_context::CommandOutputKind::RustDiagnostics)
        }
        "diagnostics" | "test" | "typecheck" | "type-check" => {
            Some(prodex_context::CommandOutputKind::Diagnostics)
        }
        "git-log" => Some(prodex_context::CommandOutputKind::GitLog),
        "search" | "rg" | "grep" => Some(prodex_context::CommandOutputKind::Search),
        "file-list" | "find" | "tree" => Some(prodex_context::CommandOutputKind::FileList),
        "log-stream" => Some(prodex_context::CommandOutputKind::LogStream),
        "noisy-success" => Some(prodex_context::CommandOutputKind::NoisySuccess),
        "plain" => Some(prodex_context::CommandOutputKind::Plain),
        _ => prodex_context::infer_command_output_kind_from_metadata(&normalized),
    }
}

fn runtime_smart_context_tool_kind_keys() -> [&'static str; 8] {
    [
        "kind",
        "output_kind",
        "outputKind",
        "command_kind",
        "commandKind",
        "detected_kind",
        "detectedKind",
        "name",
    ]
}

fn runtime_smart_context_i32_from_json_value(value: &serde_json::Value) -> Option<i32> {
    match value {
        serde_json::Value::Number(number) => {
            number.as_i64().and_then(|value| value.try_into().ok())
        }
        serde_json::Value::String(text) => text.trim().parse::<i32>().ok(),
        _ => None,
    }
}

pub(super) fn runtime_smart_context_tool_call_id(
    object: &serde_json::Map<String, serde_json::Value>,
) -> Option<&str> {
    ["call_id", "tool_call_id", "id"]
        .into_iter()
        .find_map(|key| object.get(key).and_then(serde_json::Value::as_str))
        .filter(|value| !value.trim().is_empty())
}

fn runtime_smart_context_tool_output_payload_field(key: &str) -> bool {
    matches!(key, "output" | "content")
}

fn runtime_smart_context_artifact_summary(
    artifact: &runtime_proxy_crate::SmartContextArtifactRef,
    compacted: &str,
) -> String {
    let marker = runtime_smart_context_artifact_marker_line("artifact", artifact);
    if compacted.is_empty() {
        return marker;
    }
    format!("{marker}\n{compacted}")
}

fn runtime_smart_context_artifact_reference_summary(
    artifact: &runtime_proxy_crate::SmartContextArtifactRef,
) -> String {
    runtime_smart_context_artifact_ref(&artifact.id)
}

fn runtime_smart_context_repeat_tool_output_reference_summary(
    artifact: &runtime_proxy_crate::SmartContextArtifactRef,
    text: &str,
    call_id: Option<&str>,
) -> String {
    let reference = runtime_smart_context_artifact_reference_summary(artifact);
    if runtime_proxy_crate::smart_context_command_output_critical_signals(text).count == 0 {
        return reference;
    }

    let previous_record =
        runtime_proxy_crate::smart_context_command_output_cache_record(reference.clone(), text);
    let rewrite = runtime_proxy_crate::smart_context_command_output_cache_rewrite(
        runtime_proxy_crate::SmartContextCommandOutputCacheInput {
            id: call_id.unwrap_or(&artifact.id).to_string(),
            text: text.to_string(),
            previous_records: vec![previous_record],
            min_replacement_bytes:
                runtime_proxy_crate::SMART_CONTEXT_COMMAND_OUTPUT_CACHE_MIN_BYTES,
        },
    );
    match rewrite.action {
        runtime_proxy_crate::SmartContextCommandOutputCacheAction::ReplaceWithUnchangedSummary {
            ..
        } => {
            let mut output = rewrite.output;
            output.push_str("\nref ");
            output.push_str(&reference);
            output
        }
        runtime_proxy_crate::SmartContextCommandOutputCacheAction::KeepExact { .. } => reference,
    }
}
