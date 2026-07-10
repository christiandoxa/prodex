use super::constants::*;
use super::*;
use std::collections::{BTreeMap, BTreeSet};

mod items;
mod sections;

pub(super) use self::items::{
    runtime_smart_context_replace_static_context_duplicate_texts,
    runtime_smart_context_replace_static_context_item_texts,
    runtime_smart_context_replace_static_context_texts,
    runtime_smart_context_static_context_delta_prompt_cache_hash,
    runtime_smart_context_static_context_items, runtime_smart_context_static_prompt_field_key,
    runtime_smart_context_static_role_is_prompt_prefix,
    runtime_smart_context_value_is_static_context_item,
};
use self::items::{
    runtime_smart_context_static_context_chunk_dup_marker,
    runtime_smart_context_static_context_dup_marker,
};
pub(super) use self::sections::{
    RuntimeSmartContextStaticHeadingSection, RuntimeSmartContextStaticSectionFingerprint,
    runtime_smart_context_apply_static_context_delta,
    runtime_smart_context_apply_static_context_persistent_section_dedupe,
    runtime_smart_context_apply_static_context_section_dedupe,
    runtime_smart_context_fingerprint_change_is_substantive,
    runtime_smart_context_merge_persisted_static_section_fingerprints,
    runtime_smart_context_static_context_heading_sections,
    runtime_smart_context_static_heading_section_body,
    runtime_smart_context_static_prompt_cache_key_from_body,
    runtime_smart_context_static_section_fingerprint_state_from_persisted,
    runtime_smart_context_static_section_fingerprints_from_value,
    runtime_smart_context_substantive_static_context_changed_item_ids,
};

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(super) struct RuntimeSmartContextStaticContextObservation {
    pub(super) seen_before: bool,
    pub(super) changed: bool,
    pub(super) item_count: usize,
    pub(super) delta_count: usize,
    pub(super) prompt_cache_hash: Option<String>,
    pub(super) changed_item_ids: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeSmartContextStaticChunkSeen {
    source_id: String,
    body: String,
}

pub(super) fn runtime_smart_context_apply_static_context_cross_field_dedupe(
    value: &mut serde_json::Value,
    exactness_guard: &runtime_proxy_crate::SmartContextExactnessGuard,
    stats: &mut RuntimeSmartContextTransformStats,
) {
    if exactness_guard.decision != runtime_proxy_crate::SmartContextExactnessDecision::Allow {
        return;
    }
    let items = runtime_smart_context_static_context_items(value);
    if items.len() < 2 {
        return;
    }
    let mut first_by_hash = BTreeMap::<String, String>::new();
    let mut duplicate_ids = BTreeMap::<String, String>::new();
    for item in items {
        if item.text.len() < SMART_CONTEXT_DUPLICATE_TEXT_MIN_BYTES {
            continue;
        }
        let content_hash = runtime_proxy_crate::smart_context_hash_text(&item.text);
        if let Some(first_id) = first_by_hash.get(&content_hash) {
            duplicate_ids.insert(
                item.id.clone(),
                runtime_smart_context_static_context_dup_marker(first_id),
            );
        } else {
            first_by_hash.insert(content_hash, item.id);
        }
    }
    if duplicate_ids.is_empty() {
        return;
    }
    stats.static_context_deltas = stats.static_context_deltas.saturating_add(
        runtime_smart_context_replace_static_context_duplicate_texts(value, &duplicate_ids),
    );
}

pub(super) fn runtime_smart_context_apply_static_context_chunk_dedupe(
    value: &mut serde_json::Value,
    exactness_guard: &runtime_proxy_crate::SmartContextExactnessGuard,
    stats: &mut RuntimeSmartContextTransformStats,
) {
    if exactness_guard.decision != runtime_proxy_crate::SmartContextExactnessDecision::Allow {
        return;
    }
    let items = runtime_smart_context_static_context_items(value);
    if items.len() < 2 {
        return;
    }
    let mut seen_chunks =
        BTreeMap::<(String, usize), Vec<RuntimeSmartContextStaticChunkSeen>>::new();
    let mut replacements = BTreeMap::<String, String>::new();
    for item in items {
        let next_text = runtime_smart_context_static_context_chunk_deduped_text(
            &item.id,
            &item.text,
            &seen_chunks,
        );
        let source_text = next_text.as_deref().unwrap_or(&item.text);
        runtime_smart_context_record_static_context_chunks(&item.id, source_text, &mut seen_chunks);
        if let Some(next_text) = next_text {
            replacements.insert(item.id, next_text);
        }
    }
    if replacements.is_empty() {
        return;
    }
    stats.static_context_deltas = stats.static_context_deltas.saturating_add(
        runtime_smart_context_replace_static_context_item_texts(value, &replacements),
    );
}

fn runtime_smart_context_static_context_chunk_deduped_text(
    item_id: &str,
    text: &str,
    seen_chunks: &BTreeMap<(String, usize), Vec<RuntimeSmartContextStaticChunkSeen>>,
) -> Option<String> {
    let mut candidate = text.to_string();
    let mut changed = false;
    for chunk in runtime_smart_context_static_context_dedupe_chunks(text) {
        let content_hash = runtime_proxy_crate::smart_context_hash_text(chunk);
        let Some(seen) = seen_chunks.get(&(content_hash.clone(), chunk.len())) else {
            continue;
        };
        let Some(first_seen) = seen
            .iter()
            .find(|seen| seen.body == chunk && seen.source_id != item_id)
        else {
            continue;
        };
        let marker = runtime_smart_context_static_context_chunk_dup_marker(
            &first_seen.source_id,
            &content_hash,
        );
        if marker.len() >= chunk.len() || !candidate.contains(chunk) {
            continue;
        }
        let next = candidate.replacen(chunk, &marker, 1);
        if next.len() < candidate.len()
            && prodex_context::critical_signal_self_check(text, &next).passed()
        {
            candidate = next;
            changed = true;
        }
    }
    changed.then_some(candidate)
}

fn runtime_smart_context_static_context_dedupe_chunks(text: &str) -> Vec<&str> {
    text.split("\n\n")
        .map(str::trim)
        .filter(|chunk| chunk.len() >= SMART_CONTEXT_STATIC_CONTEXT_CHUNK_MIN_BYTES)
        .filter(|chunk| {
            !chunk.starts_with(SMART_CONTEXT_STATIC_CONTEXT_DELTA_MARKER_PREFIX)
                && !chunk.starts_with(SMART_CONTEXT_STATIC_CONTEXT_DELTA_MARKER_PREFIX_LEGACY)
                && !chunk.starts_with(SMART_CONTEXT_STATIC_CONTEXT_DUP_MARKER_PREFIX)
                && !chunk.starts_with(SMART_CONTEXT_STATIC_CONTEXT_CHUNK_DUP_MARKER_PREFIX)
        })
        .collect()
}

fn runtime_smart_context_record_static_context_chunks(
    item_id: &str,
    text: &str,
    seen_chunks: &mut BTreeMap<(String, usize), Vec<RuntimeSmartContextStaticChunkSeen>>,
) {
    for chunk in runtime_smart_context_static_context_dedupe_chunks(text) {
        let content_hash = runtime_proxy_crate::smart_context_hash_text(chunk);
        let entry = seen_chunks.entry((content_hash, chunk.len())).or_default();
        if entry
            .iter()
            .any(|seen| seen.source_id == item_id && seen.body == chunk)
        {
            continue;
        }
        entry.push(RuntimeSmartContextStaticChunkSeen {
            source_id: item_id.to_string(),
            body: chunk.to_string(),
        });
    }
}
