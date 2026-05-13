use super::*;

#[path = "semantic_index/markers.rs"]
mod markers;
#[path = "semantic_index/ranges.rs"]
mod ranges;
#[path = "semantic_index/symbols.rs"]
mod symbols;
#[path = "semantic_index/types.rs"]
mod types;
#[path = "semantic_index/util.rs"]
mod util;

pub(super) use markers::*;
pub(super) use ranges::*;
pub(super) use symbols::*;
pub(super) use types::*;
pub(super) use util::*;

pub(super) fn runtime_smart_context_artifact_semantic_line_index(
    lines: &[&str],
) -> RuntimeSmartContextArtifactSemanticLineIndexParts {
    let mut parts = RuntimeSmartContextArtifactSemanticLineIndexParts {
        complete: true,
        symbol_complete: true,
        ..Default::default()
    };
    let mut remaining = RUNTIME_SMART_CONTEXT_MAX_SEMANTIC_LINE_INDEX_RANGES;
    let mut current_diff_path: Option<String> = None;

    for (index, line) in lines.iter().enumerate() {
        let line_number = index + 1;

        if let Some(path) = runtime_smart_context_parse_diff_file_path(line) {
            current_diff_path = Some(path);
        }

        if let Some(hunk) = runtime_smart_context_parse_diff_hunk(line) {
            let end = runtime_smart_context_diff_hunk_end(lines, index);
            let metadata = RuntimeSmartContextSemanticRangeMetadata {
                label: Some("diff_hunk".to_string()),
                path: current_diff_path.clone(),
                old_start: Some(hunk.old_start),
                old_count: Some(hunk.old_count),
                new_start: Some(hunk.new_start),
                new_count: Some(hunk.new_count),
                ..Default::default()
            };
            runtime_smart_context_push_semantic_range(
                &mut parts.diff_hunk_ranges,
                &mut remaining,
                &mut parts.complete,
                lines,
                line_number,
                end,
                metadata,
            );
        }

        if let Some(location) = runtime_smart_context_parse_file_location(line) {
            let metadata = RuntimeSmartContextSemanticRangeMetadata {
                label: Some("file_location".to_string()),
                path: Some(location.path),
                line: Some(location.line),
                column: location.column,
                ..Default::default()
            };
            runtime_smart_context_push_semantic_range(
                &mut parts.file_location_ranges,
                &mut remaining,
                &mut parts.complete,
                lines,
                line_number,
                line_number,
                metadata,
            );
        }

        if runtime_smart_context_is_test_failure_line(line) {
            let metadata = RuntimeSmartContextSemanticRangeMetadata {
                label: Some("test_failure".to_string()),
                symbol: runtime_smart_context_parse_test_symbol(line),
                ..Default::default()
            };
            runtime_smart_context_push_semantic_range(
                &mut parts.test_failure_ranges,
                &mut remaining,
                &mut parts.complete,
                lines,
                line_number.saturating_sub(1).max(1),
                (line_number + 1).min(lines.len()),
                metadata,
            );
        }

        if let Some(code) = runtime_smart_context_parse_error_code(line) {
            let metadata = RuntimeSmartContextSemanticRangeMetadata {
                label: Some("error".to_string()),
                code: Some(code),
                ..Default::default()
            };
            runtime_smart_context_push_semantic_range(
                &mut parts.error_ranges,
                &mut remaining,
                &mut parts.complete,
                lines,
                line_number,
                line_number,
                metadata,
            );
        }
    }

    for (index, _) in lines.iter().enumerate() {
        let Some(symbol) = runtime_smart_context_parse_symbol_line(lines, index) else {
            continue;
        };
        let (start, end) = runtime_smart_context_symbol_range_bounds(lines, index, symbol.style);
        let metadata = RuntimeSmartContextSemanticRangeMetadata {
            label: Some(symbol.label.to_string()),
            line: Some(index + 1),
            symbol: Some(symbol.symbol),
            ..Default::default()
        };
        runtime_smart_context_push_semantic_range(
            &mut parts.symbol_ranges,
            &mut remaining,
            &mut parts.symbol_complete,
            lines,
            start,
            end,
            metadata,
        );
    }

    parts
}
