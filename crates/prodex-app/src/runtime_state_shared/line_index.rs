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
    runtime_smart_context_line_excerpt,
};
use std::collections::BTreeMap;
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

    let semantic_index = runtime_smart_context_artifact_semantic_line_index(text, &lines);

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
        command_kind: semantic_index.command_kind.clone(),
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
    let facts = chunks
        .iter()
        .map(
            |chunk| prodex_mojo_core::runtime_repo_map::RepoDuplicateChunkInput {
                hash: &chunk.content_hash,
                byte_len: chunk.byte_len,
            },
        )
        .collect::<Vec<_>>();
    let plan = prodex_mojo_core::runtime_repo_map::repo_duplicate_chunk_groups(
        &facts,
        RUNTIME_SMART_CONTEXT_MAX_DUPLICATE_CHUNK_FINGERPRINTS,
        RUNTIME_SMART_CONTEXT_MAX_DUPLICATE_CHUNK_OCCURRENCES,
    )
    .expect("compiled Mojo repo-map duplicate chunk grouping returned invalid output");
    let duplicates = plan
        .groups
        .into_iter()
        .map(|group| {
            let original = &chunks[group.first_source];
            RuntimeSmartContextArtifactDuplicateChunkFingerprint {
                byte_len: original.byte_len,
                content_hash: original.content_hash.clone(),
                occurrence_count: group.occurrence_count,
                occurrences: group
                    .occurrence_indices
                    .into_iter()
                    .map(|index| {
                        let original = &chunks[index];
                        RuntimeSmartContextArtifactChunkOccurrence {
                            start: original.start,
                            end: original.end,
                            kind: original.kind.clone(),
                        }
                    })
                    .collect(),
            }
        })
        .collect();
    (duplicates, plan.complete)
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
    let paths = line_index
        .file_location_ranges
        .iter()
        .chain(line_index.diff_hunk_ranges.iter())
        .map(|range| range.path.as_deref())
        .collect::<Vec<_>>();
    let chosen = prodex_mojo_core::runtime_repo_map::repo_map_unique_path_indices(&paths)
        .expect("compiled Mojo repo-map canonical path selection returned invalid output");
    chosen
        .into_iter()
        .map(|index| {
            paths[index]
                .expect("validated Mojo selected a present path")
                .to_owned()
        })
        .collect()
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
    let ranges = line_index
        .file_location_ranges
        .iter()
        .chain(line_index.diff_hunk_ranges.iter())
        .collect::<Vec<_>>();
    let facts = ranges
        .iter()
        .map(
            |range| prodex_mojo_core::runtime_repo_map::RepoNearestPathInput {
                start: range.start,
                end: range.end,
                has_path: range.path.is_some(),
            },
        )
        .collect::<Vec<_>>();
    let index = prodex_mojo_core::runtime_repo_map::repo_nearest_path_index(&facts, line)
        .expect("compiled Mojo repo-map nearest-path selection returned invalid output")?;
    ranges[index].path.clone()
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

#[cfg(test)]
mod compiled_repo_nearest_path_caller_tests {
    use super::*;

    fn path_range(
        start: usize,
        end: usize,
        path: Option<&str>,
    ) -> RuntimeSmartContextArtifactSemanticLineRange {
        RuntimeSmartContextArtifactSemanticLineRange {
            start,
            end,
            byte_len: 0,
            content_hash: String::new(),
            text: String::new(),
            label: None,
            path: path.map(str::to_owned),
            line: None,
            column: None,
            old_start: None,
            old_count: None,
            new_start: None,
            new_count: None,
            code: None,
            symbol: None,
        }
    }

    #[test]
    fn compiled_nearest_repo_path_considers_file_and_diff_ranges_and_start_ties() {
        let index = RuntimeSmartContextArtifactLineIndex {
            file_location_ranges: vec![
                path_range(5, 5, Some("src/first-雪.rs")),
                path_range(8, 9, Some("src/later.rs")),
                path_range(3, 20, None),
            ],
            diff_hunk_ranges: vec![
                path_range(3, 3, Some("src/earliest.rs")),
                path_range(5, 5, Some("src/tie.rs")),
            ],
            ..Default::default()
        };
        assert_eq!(
            runtime_smart_context_repo_map_nearest_path(&index, 4).as_deref(),
            Some("src/earliest.rs"),
            "equal distance prefers lower start even across range collections",
        );
        assert_eq!(
            runtime_smart_context_repo_map_nearest_path(&index, 8).as_deref(),
            Some("src/later.rs"),
        );
        assert_eq!(
            runtime_smart_context_repo_map_nearest_path(&index, 5).as_deref(),
            Some("src/first-雪.rs"),
            "equal start and distance must preserve the first input range",
        );
    }

    #[test]
    fn compiled_nearest_repo_path_skips_missing_paths_but_preserves_empty_strings() {
        let mut index = RuntimeSmartContextArtifactLineIndex {
            file_location_ranges: vec![path_range(1, 10, None)],
            diff_hunk_ranges: vec![path_range(20, 22, None)],
            ..Default::default()
        };
        assert_eq!(runtime_smart_context_repo_map_nearest_path(&index, 4), None);
        index.diff_hunk_ranges.push(path_range(30, 33, Some("")));
        assert_eq!(
            runtime_smart_context_repo_map_nearest_path(&index, 4),
            Some(String::new()),
            "the previous filter_map accepted Some(empty path)",
        );
        index.file_location_ranges.clear();
        index.diff_hunk_ranges.clear();
        assert_eq!(runtime_smart_context_repo_map_nearest_path(&index, 4), None);
    }
}

#[cfg(test)]
mod compiled_repo_map_path_set_caller_tests {
    use super::*;

    fn semantic_path(path: Option<&str>) -> RuntimeSmartContextArtifactSemanticLineRange {
        RuntimeSmartContextArtifactSemanticLineRange {
            start: 1,
            end: 1,
            byte_len: 0,
            content_hash: String::new(),
            text: String::new(),
            label: None,
            path: path.map(str::to_owned),
            line: None,
            column: None,
            old_start: None,
            old_count: None,
            new_start: None,
            new_count: None,
            code: None,
            symbol: None,
        }
    }

    #[test]
    fn compiled_repo_map_path_set_keeps_unique_sorted_originals_across_file_and_diff_ranges() {
        let index = RuntimeSmartContextArtifactLineIndex {
            file_location_ranges: vec![
                semantic_path(Some("src/雪.rs")),
                semantic_path(Some("src/a.rs")),
                semantic_path(Some("\u{2003}")),
                semantic_path(None),
                semantic_path(Some(" src/a.rs ")),
            ],
            diff_hunk_ranges: vec![
                semantic_path(Some("src/a.rs")),
                semantic_path(Some("src/🦀.rs")),
                semantic_path(Some("")),
                semantic_path(Some("src/雪.rs")),
            ],
            ..Default::default()
        };
        assert_eq!(
            runtime_smart_context_repo_map_paths(&index),
            [" src/a.rs ", "src/a.rs", "src/雪.rs", "src/🦀.rs",]
        );
        assert_eq!(
            runtime_smart_context_repo_map_paths(&RuntimeSmartContextArtifactLineIndex::default()),
            Vec::<String>::new(),
        );
    }

    #[test]
    fn compiled_repo_map_path_set_filters_only_whitespace_not_internal_or_surrounding_spaces() {
        let index = RuntimeSmartContextArtifactLineIndex {
            file_location_ranges: vec![
                semantic_path(Some("\t")),
                semantic_path(Some("  path with spaces  ")),
                semantic_path(Some("\u{3000}")),
                semantic_path(Some("x  y")),
                semantic_path(Some("x  y")),
            ],
            ..Default::default()
        };
        assert_eq!(
            runtime_smart_context_repo_map_paths(&index),
            ["  path with spaces  ", "x  y"],
        );
    }
}

#[cfg(test)]
mod compiled_duplicate_chunk_batch_caller_tests {
    use super::*;

    fn fingerprint(
        hash: &str,
        byte_len: usize,
        start: usize,
        kind: &str,
    ) -> RuntimeSmartContextArtifactChunkFingerprint {
        RuntimeSmartContextArtifactChunkFingerprint {
            start,
            end: start,
            byte_len,
            content_hash: hash.to_owned(),
            kind: kind.to_owned(),
            label: None,
            path: None,
            code: None,
            symbol: None,
        }
    }

    #[test]
    fn compiled_duplicate_chunk_batch_groups_hash_size_and_preserves_occurrences() {
        let chunks = [
            fingerprint("b", 1, 10, "window"),
            fingerprint("a", 2, 1, "error"),
            fingerprint("b", 1, 11, "file"),
            fingerprint("a", 2, 2, "diff"),
            fingerprint("a", 4, 3, "symbol"),
            fingerprint("a", 4, 4, "test"),
            fingerprint("solo", 1, 5, "window"),
        ];
        let (groups, complete) = runtime_smart_context_duplicate_chunk_fingerprints(&chunks);
        assert!(complete);
        assert_eq!(groups.len(), 3);
        assert_eq!(
            groups
                .iter()
                .map(|group| (
                    group.content_hash.as_str(),
                    group.byte_len,
                    group.occurrence_count
                ))
                .collect::<Vec<_>>(),
            [("a", 2, 2), ("a", 4, 2), ("b", 1, 2)],
        );
        assert_eq!(
            groups[0]
                .occurrences
                .iter()
                .map(|item| (item.start, item.kind.as_str()))
                .collect::<Vec<_>>(),
            [(1, "error"), (2, "diff")],
        );
        assert_eq!(
            groups[1]
                .occurrences
                .iter()
                .map(|item| item.start)
                .collect::<Vec<_>>(),
            [3, 4],
        );
        assert_eq!(
            groups[2]
                .occurrences
                .iter()
                .map(|item| item.start)
                .collect::<Vec<_>>(),
            [10, 11],
        );
    }

    #[test]
    fn compiled_duplicate_chunk_batch_reports_truncated_metadata_without_data_loss() {
        let repeated = (0..9)
            .map(|index| fingerprint("duplicate", 1, index, "error"))
            .collect::<Vec<_>>();
        let (groups, complete) = runtime_smart_context_duplicate_chunk_fingerprints(&repeated);
        assert_eq!(groups.len(), 1);
        assert!(!complete);
        assert_eq!(groups[0].occurrence_count, 9);
        assert_eq!(groups[0].occurrences.len(), 8);

        let mut many = Vec::new();
        for index in 0..65 {
            for repeat in 0..2 {
                many.push(fingerprint(
                    &format!("hash-{index:04}"),
                    1,
                    index * 2 + repeat,
                    "test",
                ));
            }
        }
        let (groups, complete) = runtime_smart_context_duplicate_chunk_fingerprints(&many);
        assert_eq!(groups.len(), 64);
        assert!(!complete);
        assert_eq!(groups[0].content_hash, "hash-0000");
        assert_eq!(groups[63].content_hash, "hash-0063");
        assert_eq!(
            runtime_smart_context_duplicate_chunk_fingerprints(&[])
                .0
                .len(),
            0,
        );
        assert!(runtime_smart_context_duplicate_chunk_fingerprints(&[]).1);
    }
}
