use super::constants::*;
use super::*;
use std::collections::{BTreeMap, BTreeSet};

mod items;

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
    runtime_smart_context_static_context_delta_marker,
    runtime_smart_context_static_context_dup_marker,
    runtime_smart_context_static_context_section_dup_marker,
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

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct RuntimeSmartContextStaticSectionFingerprint {
    pub(super) item_id: String,
    pub(super) heading: String,
    pub(super) ordinal: usize,
    pub(super) content_hash: String,
    pub(super) byte_len: usize,
}

pub(super) fn runtime_smart_context_static_prompt_cache_key_from_body(
    body: &[u8],
) -> Option<String> {
    if body.is_empty() {
        return None;
    }
    let value = serde_json::from_slice::<serde_json::Value>(body).ok()?;
    if let Some(prompt_cache_hash) =
        runtime_smart_context_static_context_delta_prompt_cache_hash(&value)
    {
        return Some(prompt_cache_hash);
    }
    let cache = runtime_proxy_crate::smart_context_static_context_prompt_cache_fingerprint(
        runtime_smart_context_static_context_items(&value),
    );
    (!cache.items.is_empty()).then_some(cache.content_hash)
}

pub(super) fn runtime_smart_context_fingerprint_change_is_substantive(
    change: &runtime_proxy_crate::SmartContextFingerprintChange,
) -> bool {
    !matches!(
        change,
        runtime_proxy_crate::SmartContextFingerprintChange::Unchanged { .. }
    )
}

pub(super) fn runtime_smart_context_substantive_static_context_changed_item_ids(
    changes: &[runtime_proxy_crate::SmartContextFingerprintChange],
) -> BTreeSet<String> {
    changes
        .iter()
        .filter_map(|change| match change {
            runtime_proxy_crate::SmartContextFingerprintChange::Added { fingerprint }
            | runtime_proxy_crate::SmartContextFingerprintChange::Changed {
                after: fingerprint,
                ..
            } => Some(fingerprint.id.clone()),
            runtime_proxy_crate::SmartContextFingerprintChange::Removed { .. }
            | runtime_proxy_crate::SmartContextFingerprintChange::Unchanged { .. } => None,
        })
        .collect()
}

pub(super) fn runtime_smart_context_apply_static_context_delta(
    value: &mut serde_json::Value,
    observation: &RuntimeSmartContextStaticContextObservation,
    exactness_guard: &runtime_proxy_crate::SmartContextExactnessGuard,
    stats: &mut RuntimeSmartContextTransformStats,
) {
    if exactness_guard.decision != runtime_proxy_crate::SmartContextExactnessDecision::Allow
        || !observation.seen_before
        || observation.item_count == 0
    {
        return;
    }
    let Some(prompt_cache_hash) = observation.prompt_cache_hash.as_deref() else {
        return;
    };
    let marker = runtime_smart_context_static_context_delta_marker(prompt_cache_hash);
    stats.static_context_deltas = stats.static_context_deltas.saturating_add(
        runtime_smart_context_replace_static_context_texts(value, observation, &marker),
    );
}

pub(super) fn runtime_smart_context_apply_static_context_section_dedupe(
    value: &mut serde_json::Value,
    exactness_guard: &runtime_proxy_crate::SmartContextExactnessGuard,
    stats: &mut RuntimeSmartContextTransformStats,
) {
    if exactness_guard.decision != runtime_proxy_crate::SmartContextExactnessDecision::Allow {
        return;
    }
    let items = runtime_smart_context_static_context_items(value);
    if items.is_empty() {
        return;
    }
    let mut first_by_heading_hash = BTreeMap::<(String, String, usize), String>::new();
    let mut replacements = BTreeMap::<String, String>::new();
    for item in items {
        let Some(next_text) = runtime_smart_context_static_context_section_deduped_text(
            &item.id,
            &item.text,
            &mut first_by_heading_hash,
        ) else {
            continue;
        };
        replacements.insert(item.id, next_text);
    }
    if replacements.is_empty() {
        return;
    }
    stats.static_context_deltas = stats.static_context_deltas.saturating_add(
        runtime_smart_context_replace_static_context_item_texts(value, &replacements),
    );
}

pub(super) fn runtime_smart_context_apply_static_context_persistent_section_dedupe(
    value: &mut serde_json::Value,
    shared: &RuntimeRotationProxyShared,
    exactness_guard: &runtime_proxy_crate::SmartContextExactnessGuard,
    stats: &mut RuntimeSmartContextTransformStats,
) {
    if exactness_guard.decision != runtime_proxy_crate::SmartContextExactnessDecision::Allow {
        return;
    }
    let current = runtime_smart_context_static_section_fingerprints_from_value(value);
    if current.is_empty() {
        return;
    }
    let Some((replacements, changed)) = with_runtime_smart_context_proxy_state(shared, |state| {
        let replacements = runtime_smart_context_persistent_static_section_replacements(
            value,
            &state.static_section_fingerprints,
            &current,
        );
        let next = current
            .iter()
            .cloned()
            .map(|fingerprint| {
                (
                    runtime_smart_context_static_section_fingerprint_key(&fingerprint),
                    fingerprint,
                )
            })
            .collect::<BTreeMap<_, _>>();
        let changed = state.static_section_fingerprints != next;
        if changed {
            state.static_section_fingerprints = next;
        }
        (replacements, changed)
    }) else {
        return;
    };

    if !replacements.is_empty() {
        stats.static_context_deltas = stats.static_context_deltas.saturating_add(
            runtime_smart_context_replace_static_context_item_texts(value, &replacements),
        );
    }
    if changed {
        persist_runtime_smart_context_token_calibration_metadata(
            shared,
            "smart_context_static_sections",
        );
    }
}

fn runtime_smart_context_persistent_static_section_replacements(
    value: &serde_json::Value,
    previous: &BTreeMap<String, RuntimeSmartContextStaticSectionFingerprint>,
    current: &[RuntimeSmartContextStaticSectionFingerprint],
) -> BTreeMap<String, String> {
    if previous.is_empty() {
        return BTreeMap::new();
    }
    let current_by_key = current
        .iter()
        .map(|fingerprint| {
            (
                runtime_smart_context_static_section_fingerprint_key(fingerprint),
                fingerprint,
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut replacements = BTreeMap::new();
    for item in runtime_smart_context_static_context_items(value) {
        let Some(next_text) = runtime_smart_context_static_context_persistent_section_deduped_text(
            &item.id,
            &item.text,
            previous,
            &current_by_key,
        ) else {
            continue;
        };
        replacements.insert(item.id, next_text);
    }
    replacements
}

fn runtime_smart_context_static_context_persistent_section_deduped_text(
    item_id: &str,
    text: &str,
    previous: &BTreeMap<String, RuntimeSmartContextStaticSectionFingerprint>,
    current_by_key: &BTreeMap<String, &RuntimeSmartContextStaticSectionFingerprint>,
) -> Option<String> {
    let sections = runtime_smart_context_static_context_heading_sections(text);
    if sections.is_empty() {
        return None;
    }
    let mut candidate = String::new();
    let mut cursor = 0usize;
    let mut changed = false;
    for section in sections {
        if section.start < cursor {
            continue;
        }
        let Some(body) = runtime_smart_context_static_heading_section_body(text, &section) else {
            continue;
        };
        candidate.push_str(&text[cursor..section.start]);
        let key =
            runtime_smart_context_static_section_key(item_id, &section.heading, section.ordinal);
        let marker = previous
            .get(&key)
            .zip(current_by_key.get(&key))
            .filter(|(previous, current)| {
                previous.content_hash == current.content_hash
                    && previous.byte_len == current.byte_len
                    && previous.heading == current.heading
            })
            .map(|(previous, _)| {
                runtime_smart_context_static_context_section_dup_marker(
                    &format!("{}:{}", previous.item_id, previous.ordinal),
                    &previous.content_hash,
                    &section.heading,
                )
            });
        if let Some(marker) = marker
            && marker.len() < body.len()
        {
            candidate.push_str(&marker);
            changed = true;
        } else {
            candidate.push_str(body);
        }
        cursor = section.end;
    }
    candidate.push_str(&text[cursor..]);
    if !changed || candidate.len() >= text.len() {
        return None;
    }
    prodex_context::critical_signal_self_check(text, &candidate)
        .passed()
        .then_some(candidate)
}

fn runtime_smart_context_static_context_section_deduped_text(
    item_id: &str,
    text: &str,
    first_by_heading_hash: &mut BTreeMap<(String, String, usize), String>,
) -> Option<String> {
    let sections = runtime_smart_context_static_context_heading_sections(text);
    if sections.len() < 2 {
        return None;
    }
    let mut candidate = String::new();
    let mut cursor = 0usize;
    let mut changed = false;
    for section in sections {
        if section.start < cursor {
            continue;
        }
        let Some(body) = runtime_smart_context_static_heading_section_body(text, &section) else {
            continue;
        };
        candidate.push_str(&text[cursor..section.start]);
        let body_key = body.trim();
        let content_hash = runtime_proxy_crate::smart_context_hash_text(body_key);
        let key = (
            section.heading.to_ascii_lowercase(),
            content_hash.clone(),
            body_key.len(),
        );
        let marker = if let Some(first_id) = first_by_heading_hash.get(&key) {
            runtime_smart_context_static_context_section_dup_marker(
                first_id,
                &content_hash,
                &section.heading,
            )
        } else {
            first_by_heading_hash.insert(key, format!("{item_id}:{}", section.ordinal));
            String::new()
        };
        if !marker.is_empty()
            && marker.len() < body.len()
            && let Some(suffix) = text.get(section.end..)
            && prodex_context::critical_signal_self_check(
                text,
                &format!("{candidate}{marker}{suffix}"),
            )
            .passed()
        {
            candidate.push_str(&marker);
            changed = true;
        } else {
            candidate.push_str(body);
        }
        cursor = section.end;
    }
    candidate.push_str(&text[cursor..]);
    (changed && candidate.len() < text.len()).then_some(candidate)
}

pub(super) fn runtime_smart_context_static_section_fingerprints_from_value(
    value: &serde_json::Value,
) -> Vec<RuntimeSmartContextStaticSectionFingerprint> {
    let mut fingerprints = Vec::new();
    for item in runtime_smart_context_static_context_items(value) {
        for section in runtime_smart_context_static_context_heading_sections(&item.text) {
            let Some(body) =
                runtime_smart_context_static_heading_section_body(&item.text, &section)
            else {
                continue;
            };
            let body = body.trim();
            if body.is_empty() {
                continue;
            }
            fingerprints.push(RuntimeSmartContextStaticSectionFingerprint {
                item_id: item.id.clone(),
                heading: section.heading.clone(),
                ordinal: section.ordinal,
                content_hash: runtime_proxy_crate::smart_context_hash_text(body),
                byte_len: body.len(),
            });
        }
    }
    fingerprints.sort_by_key(runtime_smart_context_static_section_fingerprint_key);
    fingerprints.truncate(SMART_CONTEXT_PERSISTED_STATIC_SECTION_LIMIT);
    fingerprints
}

pub(super) fn runtime_smart_context_static_section_fingerprint_state_from_persisted(
    persisted: Vec<RuntimeSmartContextPersistedStaticSectionFingerprint>,
) -> BTreeMap<String, RuntimeSmartContextStaticSectionFingerprint> {
    runtime_smart_context_merge_persisted_static_section_fingerprints(Vec::new(), persisted)
        .into_iter()
        .map(RuntimeSmartContextStaticSectionFingerprint::from)
        .map(|fingerprint| {
            (
                runtime_smart_context_static_section_fingerprint_key(&fingerprint),
                fingerprint,
            )
        })
        .collect()
}

pub(super) fn runtime_smart_context_merge_persisted_static_section_fingerprints(
    existing: Vec<RuntimeSmartContextPersistedStaticSectionFingerprint>,
    incoming: Vec<RuntimeSmartContextPersistedStaticSectionFingerprint>,
) -> Vec<RuntimeSmartContextPersistedStaticSectionFingerprint> {
    let mut by_key = BTreeMap::<String, RuntimeSmartContextStaticSectionFingerprint>::new();
    for fingerprint in existing.into_iter().chain(incoming) {
        let fingerprint = RuntimeSmartContextStaticSectionFingerprint::from(fingerprint);
        if !runtime_smart_context_static_section_fingerprint_valid(&fingerprint) {
            continue;
        }
        by_key.insert(
            runtime_smart_context_static_section_fingerprint_key(&fingerprint),
            fingerprint,
        );
    }
    by_key
        .into_values()
        .take(SMART_CONTEXT_PERSISTED_STATIC_SECTION_LIMIT)
        .map(RuntimeSmartContextPersistedStaticSectionFingerprint::from)
        .collect()
}

fn runtime_smart_context_static_section_fingerprint_valid(
    fingerprint: &RuntimeSmartContextStaticSectionFingerprint,
) -> bool {
    !fingerprint.item_id.trim().is_empty()
        && !fingerprint.heading.trim().is_empty()
        && !fingerprint.content_hash.trim().is_empty()
        && fingerprint.byte_len >= SMART_CONTEXT_STATIC_CONTEXT_CHUNK_MIN_BYTES
}

pub(super) fn runtime_smart_context_static_section_fingerprint_key(
    fingerprint: &RuntimeSmartContextStaticSectionFingerprint,
) -> String {
    runtime_smart_context_static_section_key(
        &fingerprint.item_id,
        &fingerprint.heading,
        fingerprint.ordinal,
    )
}

fn runtime_smart_context_static_section_key(
    item_id: &str,
    heading: &str,
    ordinal: usize,
) -> String {
    format!("{item_id}\n{}\n{ordinal}", heading.to_ascii_lowercase())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeSmartContextStaticHeadingSection {
    pub(super) heading: String,
    pub(super) start: usize,
    pub(super) end: usize,
    pub(super) ordinal: usize,
}

pub(super) fn runtime_smart_context_static_heading_section_body<'a>(
    text: &'a str,
    section: &RuntimeSmartContextStaticHeadingSection,
) -> Option<&'a str> {
    if section.start >= section.end
        || section.end > text.len()
        || !text.is_char_boundary(section.start)
        || !text.is_char_boundary(section.end)
    {
        return None;
    }
    text.get(section.start..section.end)
}

pub(super) fn runtime_smart_context_static_context_heading_sections(
    text: &str,
) -> Vec<RuntimeSmartContextStaticHeadingSection> {
    let mut headings = Vec::<(String, usize)>::new();
    let mut offset = 0usize;
    for line in text.split_inclusive('\n') {
        let line_without_newline = line.trim_end_matches('\n').trim_end_matches('\r');
        if let Some(heading) = runtime_smart_context_static_context_heading(line_without_newline) {
            headings.push((heading, offset));
        }
        offset = offset.saturating_add(line.len());
    }
    if !text.ends_with('\n')
        && let Some(last_line) = text.rsplit('\n').next()
        && let Some(heading) = runtime_smart_context_static_context_heading(last_line)
    {
        let start = text.len().saturating_sub(last_line.len());
        if !headings
            .iter()
            .any(|(_, existing_start)| *existing_start == start)
        {
            headings.push((heading, start));
        }
    }
    let mut sections = Vec::new();
    for (index, (heading, start)) in headings.iter().enumerate() {
        let end = headings
            .get(index + 1)
            .map(|(_, next_start)| *next_start)
            .unwrap_or(text.len());
        if end.saturating_sub(*start) < SMART_CONTEXT_STATIC_CONTEXT_CHUNK_MIN_BYTES {
            continue;
        }
        let Some(body) = text.get(*start..end).map(str::trim) else {
            continue;
        };
        if body.starts_with(SMART_CONTEXT_STATIC_CONTEXT_DELTA_MARKER_PREFIX)
            || body.starts_with(SMART_CONTEXT_STATIC_CONTEXT_DELTA_MARKER_PREFIX_LEGACY)
            || body.starts_with(SMART_CONTEXT_STATIC_CONTEXT_DUP_MARKER_PREFIX)
            || body.starts_with(SMART_CONTEXT_STATIC_CONTEXT_CHUNK_DUP_MARKER_PREFIX)
            || body.starts_with(SMART_CONTEXT_STATIC_CONTEXT_SECTION_DUP_MARKER_PREFIX)
        {
            continue;
        }
        sections.push(RuntimeSmartContextStaticHeadingSection {
            heading: heading.clone(),
            start: *start,
            end,
            ordinal: index,
        });
    }
    sections
}

fn runtime_smart_context_static_context_heading(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if !trimmed.starts_with('#') {
        return None;
    }
    let level = trimmed.chars().take_while(|ch| *ch == '#').count();
    if level == 0 || level > 6 || !trimmed.chars().nth(level).is_some_and(char::is_whitespace) {
        return None;
    }
    Some(trimmed.to_string())
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
