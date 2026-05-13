use super::*;

pub(in crate::runtime_state_shared) fn runtime_smart_context_push_semantic_range(
    target: &mut Vec<RuntimeSmartContextArtifactSemanticLineRange>,
    remaining: &mut usize,
    complete: &mut bool,
    lines: &[&str],
    start: usize,
    end: usize,
    metadata: RuntimeSmartContextSemanticRangeMetadata,
) {
    if *remaining == 0 {
        *complete = false;
        return;
    }
    let Some(text) = runtime_smart_context_line_excerpt(lines, start, end) else {
        *complete = false;
        return;
    };
    if target.iter().any(|range| {
        range.start == start
            && range.end == end
            && range.label == metadata.label
            && range.path == metadata.path
            && range.code == metadata.code
            && range.symbol == metadata.symbol
    }) {
        return;
    }
    let byte_len = text.len();
    target.push(RuntimeSmartContextArtifactSemanticLineRange {
        start,
        end,
        byte_len,
        content_hash: runtime_proxy_crate::smart_context_hash_text(&text),
        text,
        label: metadata.label,
        path: metadata.path,
        line: metadata.line,
        column: metadata.column,
        old_start: metadata.old_start,
        old_count: metadata.old_count,
        new_start: metadata.new_start,
        new_count: metadata.new_count,
        code: metadata.code,
        symbol: metadata.symbol,
    });
    *remaining = remaining.saturating_sub(1);
}

pub(in crate::runtime_state_shared) fn runtime_smart_context_line_excerpt(
    lines: &[&str],
    start: usize,
    end: usize,
) -> Option<String> {
    if start == 0 || start > lines.len() || end < start {
        return None;
    }
    let end = end.min(lines.len());
    let excerpt = lines[start - 1..end].join("\n");
    (excerpt.len() <= RUNTIME_SMART_CONTEXT_MAX_LINE_INDEX_EXCERPT_BYTES).then_some(excerpt)
}
