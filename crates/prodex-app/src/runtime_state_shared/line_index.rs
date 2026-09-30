use super::{
    RUNTIME_SMART_CONTEXT_CHUNK_WINDOW_LINES, RUNTIME_SMART_CONTEXT_MAX_CHUNK_FINGERPRINTS,
    RUNTIME_SMART_CONTEXT_MAX_DUPLICATE_CHUNK_FINGERPRINTS,
    RUNTIME_SMART_CONTEXT_MAX_DUPLICATE_CHUNK_OCCURRENCES,
    RUNTIME_SMART_CONTEXT_MAX_LINE_INDEX_EXCERPT_BYTES,
    RUNTIME_SMART_CONTEXT_MAX_LINE_INDEX_RANGES, RUNTIME_SMART_CONTEXT_SEMANTIC_SCHEMA_VERSION,
    RuntimeSmartContextArtifactChunkFingerprint, RuntimeSmartContextArtifactChunkIndex,
    RuntimeSmartContextArtifactChunkOccurrence,
    RuntimeSmartContextArtifactDuplicateChunkFingerprint, RuntimeSmartContextArtifactLineIndex,
    RuntimeSmartContextArtifactLineRange, RuntimeSmartContextArtifactRepoMap,
    RuntimeSmartContextArtifactRepoMapEntry, RuntimeSmartContextArtifactRepoMapEntryKind,
    RuntimeSmartContextArtifactRepoMapKey, RuntimeSmartContextArtifactSemanticLineRange,
    runtime_smart_context_artifact_semantic_line_index, runtime_smart_context_bounded_string,
    runtime_smart_context_infer_command_kind, runtime_smart_context_line_excerpt,
};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

pub(super) fn runtime_smart_context_artifact_line_index(
    text: &str,
) -> RuntimeSmartContextArtifactLineIndex {
    let critical_signals_available = prodex_context::critical_signal_available();
    let mut ranges = if critical_signals_available {
        prodex_context::critical_signal_lost_line_ranges_with_options(
            text,
            "",
            prodex_context::CriticalSignalLineRangeOptions {
                context_lines: 1,
                max_ranges: RUNTIME_SMART_CONTEXT_MAX_LINE_INDEX_RANGES.saturating_add(1),
                max_range_lines: 6,
            },
        )
    } else {
        Vec::new()
    };
    let complete =
        critical_signals_available && ranges.len() <= RUNTIME_SMART_CONTEXT_MAX_LINE_INDEX_RANGES;
    ranges.truncate(RUNTIME_SMART_CONTEXT_MAX_LINE_INDEX_RANGES);

    let lines = text.lines().collect::<Vec<_>>();
    let mut indexed_ranges = Vec::new();
    let mut index_complete = complete;
    for range in ranges {
        if range.start == 0 || range.start > lines.len() || range.end < range.start {
            index_complete = false;
            continue;
        }
        let end = range.end.min(lines.len());
        let excerpt = lines[range.start - 1..end].join("\n");
        if excerpt.len() > RUNTIME_SMART_CONTEXT_MAX_LINE_INDEX_EXCERPT_BYTES {
            index_complete = false;
            continue;
        }
        indexed_ranges.push(RuntimeSmartContextArtifactLineRange {
            start: range.start,
            end,
            byte_len: excerpt.len(),
            content_hash: runtime_proxy_crate::smart_context_hash_text(&excerpt),
            text: excerpt,
        });
    }

    let semantic_index = runtime_smart_context_artifact_semantic_line_index(&lines);

    RuntimeSmartContextArtifactLineIndex {
        complete: index_complete,
        semantic_schema_version: RUNTIME_SMART_CONTEXT_SEMANTIC_SCHEMA_VERSION,
        semantic_complete: semantic_index.complete && semantic_index.symbol_complete,
        symbol_complete: semantic_index.symbol_complete,
        critical_ranges: indexed_ranges,
        file_location_ranges: semantic_index.file_location_ranges,
        diff_hunk_ranges: semantic_index.diff_hunk_ranges,
        test_failure_ranges: semantic_index.test_failure_ranges,
        error_ranges: semantic_index.error_ranges,
        symbol_ranges: semantic_index.symbol_ranges,
        command_kind: runtime_smart_context_infer_command_kind(&lines),
    }
}

pub(super) fn runtime_smart_context_artifact_line_index_needs_refresh(
    line_index: Option<&RuntimeSmartContextArtifactLineIndex>,
) -> bool {
    match line_index {
        Some(line_index) => {
            line_index.semantic_schema_version < RUNTIME_SMART_CONTEXT_SEMANTIC_SCHEMA_VERSION
        }
        None => true,
    }
}

pub(super) fn runtime_smart_context_artifact_chunk_index(
    text: &str,
    line_index: &RuntimeSmartContextArtifactLineIndex,
) -> RuntimeSmartContextArtifactChunkIndex {
    let lines = text.lines().collect::<Vec<_>>();
    let mut chunks = Vec::new();
    let mut complete = line_index.semantic_complete && line_index.symbol_complete;

    for range in &line_index.file_location_ranges {
        runtime_smart_context_push_chunk_fingerprint(&mut chunks, &mut complete, "file", range);
    }
    for range in &line_index.diff_hunk_ranges {
        runtime_smart_context_push_chunk_fingerprint(&mut chunks, &mut complete, "diff", range);
    }
    for range in &line_index.test_failure_ranges {
        runtime_smart_context_push_chunk_fingerprint(&mut chunks, &mut complete, "test", range);
    }
    for range in &line_index.error_ranges {
        runtime_smart_context_push_chunk_fingerprint(&mut chunks, &mut complete, "error", range);
    }
    for range in &line_index.symbol_ranges {
        runtime_smart_context_push_chunk_fingerprint(&mut chunks, &mut complete, "symbol", range);
    }

    if chunks.is_empty() {
        runtime_smart_context_push_window_chunk_fingerprints(&mut chunks, &mut complete, &lines);
    }

    let (duplicate_chunks, duplicate_metadata_complete) =
        runtime_smart_context_duplicate_chunk_fingerprints(&chunks);
    RuntimeSmartContextArtifactChunkIndex {
        complete: complete && duplicate_metadata_complete,
        chunks,
        duplicate_chunks,
    }
}

pub(super) fn runtime_smart_context_push_chunk_fingerprint(
    chunks: &mut Vec<RuntimeSmartContextArtifactChunkFingerprint>,
    complete: &mut bool,
    kind: &str,
    range: &RuntimeSmartContextArtifactSemanticLineRange,
) {
    let plan = prodex_mojo_core::runtime_repo_map::repo_chunk_plan(
        chunks.len(),
        RUNTIME_SMART_CONTEXT_MAX_CHUNK_FINGERPRINTS,
        range.byte_len,
        range.text.len(),
        range.content_hash == runtime_proxy_crate::smart_context_hash_text(&range.text),
    )
    .expect("Mojo repo-map chunk policy returned invalid output");
    match plan {
        prodex_mojo_core::runtime_repo_map::RepoChunkPlan::Accept => {}
        prodex_mojo_core::runtime_repo_map::RepoChunkPlan::RejectCapacity
        | prodex_mojo_core::runtime_repo_map::RepoChunkPlan::RejectInvalid => {
            *complete = false;
            return;
        }
    }
    chunks.push(RuntimeSmartContextArtifactChunkFingerprint {
        start: range.start,
        end: range.end,
        byte_len: range.byte_len,
        content_hash: range.content_hash.clone(),
        kind: kind.to_string(),
        label: range.label.clone(),
        path: range.path.clone(),
        code: range.code.clone(),
        symbol: range.symbol.clone(),
    });
}

pub(super) fn runtime_smart_context_push_window_chunk_fingerprints(
    chunks: &mut Vec<RuntimeSmartContextArtifactChunkFingerprint>,
    complete: &mut bool,
    lines: &[&str],
) {
    let mut start = 1usize;
    while start <= lines.len() {
        if chunks.len() >= RUNTIME_SMART_CONTEXT_MAX_CHUNK_FINGERPRINTS {
            *complete = false;
            return;
        }
        let end = (start + RUNTIME_SMART_CONTEXT_CHUNK_WINDOW_LINES - 1).min(lines.len());
        let Some(text) = runtime_smart_context_line_excerpt(lines, start, end) else {
            *complete = false;
            start = end.saturating_add(1);
            continue;
        };
        chunks.push(RuntimeSmartContextArtifactChunkFingerprint {
            start,
            end,
            byte_len: text.len(),
            content_hash: runtime_proxy_crate::smart_context_hash_text(&text),
            kind: "window".to_string(),
            label: None,
            path: None,
            code: None,
            symbol: None,
        });
        start = end.saturating_add(1);
    }
}

pub(super) fn runtime_smart_context_duplicate_chunk_fingerprints(
    chunks: &[RuntimeSmartContextArtifactChunkFingerprint],
) -> (
    Vec<RuntimeSmartContextArtifactDuplicateChunkFingerprint>,
    bool,
) {
    let mut grouped =
        BTreeMap::<(String, usize), Vec<&RuntimeSmartContextArtifactChunkFingerprint>>::new();
    for chunk in chunks {
        grouped
            .entry((chunk.content_hash.clone(), chunk.byte_len))
            .or_default()
            .push(chunk);
    }

    let mut duplicates = Vec::new();
    let mut complete = true;
    for ((content_hash, byte_len), occurrences) in grouped {
        let plan = prodex_mojo_core::runtime_repo_map::repo_duplicate_plan(
            occurrences.len(),
            duplicates.len(),
            RUNTIME_SMART_CONTEXT_MAX_DUPLICATE_CHUNK_FINGERPRINTS,
            RUNTIME_SMART_CONTEXT_MAX_DUPLICATE_CHUNK_OCCURRENCES,
        )
        .expect("Mojo repo-map duplicate policy returned invalid output");
        let occurrences_complete = match plan {
            prodex_mojo_core::runtime_repo_map::RepoDuplicatePlan::Skip => continue,
            prodex_mojo_core::runtime_repo_map::RepoDuplicatePlan::StopCapacity => {
                complete = false;
                break;
            }
            prodex_mojo_core::runtime_repo_map::RepoDuplicatePlan::Append {
                occurrences_complete,
            } => occurrences_complete,
        };
        complete &= occurrences_complete;
        duplicates.push(RuntimeSmartContextArtifactDuplicateChunkFingerprint {
            byte_len,
            content_hash,
            occurrence_count: occurrences.len(),
            occurrences: occurrences
                .into_iter()
                .take(RUNTIME_SMART_CONTEXT_MAX_DUPLICATE_CHUNK_OCCURRENCES)
                .map(|chunk| RuntimeSmartContextArtifactChunkOccurrence {
                    start: chunk.start,
                    end: chunk.end,
                    kind: chunk.kind.clone(),
                })
                .collect(),
        });
    }
    (duplicates, complete)
}

pub(super) fn runtime_smart_context_limited_repo_map(
    mut repo_map: RuntimeSmartContextArtifactRepoMap,
    limit: usize,
) -> RuntimeSmartContextArtifactRepoMap {
    if repo_map.entries.len() > limit {
        repo_map.entries.truncate(limit);
        repo_map.complete = false;
    }
    repo_map
}

pub(super) fn runtime_smart_context_push_projection_source_ranges(
    source: &mut String,
    kind: &str,
    ranges: &[RuntimeSmartContextArtifactSemanticLineRange],
) {
    for range in ranges {
        let _ = writeln!(
            source,
            "{kind}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            range.start,
            range.end,
            range.content_hash,
            range.label.as_deref().unwrap_or_default(),
            range.path.as_deref().unwrap_or_default(),
            range.line.unwrap_or_default(),
            range.column.unwrap_or_default(),
            range.old_start.unwrap_or_default(),
            range.new_start.unwrap_or_default(),
            range.code.as_deref().unwrap_or_default(),
            range.symbol.as_deref().unwrap_or_default(),
            range.old_count.unwrap_or_default(),
            range.new_count.unwrap_or_default()
        );
    }
}

pub(super) fn runtime_smart_context_repo_map_paths(
    line_index: &RuntimeSmartContextArtifactLineIndex,
) -> Vec<String> {
    let mut paths = BTreeSet::new();
    for range in line_index
        .file_location_ranges
        .iter()
        .chain(line_index.diff_hunk_ranges.iter())
    {
        if let Some(path) = range.path.as_ref().filter(|path| !path.trim().is_empty()) {
            paths.insert(path.clone());
        }
    }
    paths.into_iter().collect()
}

pub(super) fn runtime_smart_context_repo_map_first_path_range<'a>(
    line_index: &'a RuntimeSmartContextArtifactLineIndex,
    path: &str,
) -> Option<&'a RuntimeSmartContextArtifactSemanticLineRange> {
    line_index
        .file_location_ranges
        .iter()
        .chain(line_index.diff_hunk_ranges.iter())
        .find(|range| range.path.as_deref() == Some(path))
}

pub(super) fn runtime_smart_context_repo_map_nearest_path(
    line_index: &RuntimeSmartContextArtifactLineIndex,
    line: usize,
) -> Option<String> {
    line_index
        .file_location_ranges
        .iter()
        .chain(line_index.diff_hunk_ranges.iter())
        .filter_map(|range| {
            let path = range.path.as_ref()?.clone();
            let distance = prodex_mojo_core::runtime_repo_map::repo_path_distance(
                range.start,
                range.end,
                line,
            )
            .expect("Mojo repo-map path distance returned invalid output");
            Some((distance, range.start, path))
        })
        .min_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)))
        .map(|(_, _, path)| path)
}

pub(super) fn runtime_smart_context_repo_map_symbol_kind(
    range: &RuntimeSmartContextArtifactSemanticLineRange,
) -> RuntimeSmartContextArtifactRepoMapEntryKind {
    match prodex_mojo_core::runtime_repo_map::repo_symbol_kind(range.label.as_deref(), &range.text)
        .expect("Mojo repo-map symbol-kind policy returned invalid output")
    {
        prodex_mojo_core::runtime_repo_map::RepoSymbolKind::Module => {
            RuntimeSmartContextArtifactRepoMapEntryKind::Module
        }
        prodex_mojo_core::runtime_repo_map::RepoSymbolKind::Symbol => {
            RuntimeSmartContextArtifactRepoMapEntryKind::Symbol
        }
        prodex_mojo_core::runtime_repo_map::RepoSymbolKind::Test => {
            RuntimeSmartContextArtifactRepoMapEntryKind::Test
        }
    }
}

pub(super) fn runtime_smart_context_repo_map_symbol_module(
    kind: RuntimeSmartContextArtifactRepoMapEntryKind,
    path: Option<&str>,
    symbol: &str,
) -> Option<String> {
    if kind == RuntimeSmartContextArtifactRepoMapEntryKind::Module {
        return runtime_smart_context_bounded_string(symbol);
    }
    path.and_then(runtime_smart_context_repo_map_module_from_path)
}

pub(super) fn runtime_smart_context_repo_map_module_from_path(path: &str) -> Option<String> {
    prodex_mojo_core::runtime_repo_map::repo_module_from_path(path)
        .expect("Mojo repo-map path normalizer returned invalid output")
}

pub(super) fn runtime_smart_context_insert_repo_map_entry(
    entries: &mut BTreeMap<
        RuntimeSmartContextArtifactRepoMapKey,
        RuntimeSmartContextArtifactRepoMapEntry,
    >,
    entry: RuntimeSmartContextArtifactRepoMapEntry,
) {
    let key = (
        entry.kind,
        entry.path.clone(),
        entry.module.clone(),
        entry.symbol.clone(),
        entry.code.clone(),
    );
    entries
        .entry(key)
        .and_modify(|current| {
            if prodex_mojo_core::runtime_repo_map::repo_entry_should_replace(
                entry.order,
                current.order,
                &entry.artifact_id,
                &current.artifact_id,
            )
            .expect("Mojo repo-map entry replacement policy returned invalid output")
            {
                *current = entry.clone();
            }
        })
        .or_insert(entry);
}
