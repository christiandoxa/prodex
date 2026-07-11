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

mod arguments;
mod compaction;
mod dedupe;
mod diff;
mod metadata;

use arguments::*;
#[cfg(test)]
pub(in crate::runtime_proxy::smart_context) use compaction::runtime_smart_context_tool_preview_max_lines;
use compaction::*;
#[cfg(test)]
pub(in crate::runtime_proxy::smart_context) use dedupe::runtime_smart_context_dedupe_progressive_summary_chunks;
use diff::*;
pub(in crate::runtime_proxy::smart_context) use metadata::*;

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
