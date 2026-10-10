use crate::runtime_state_shared::{
    RUNTIME_SMART_CONTEXT_MAX_LINE_INDEX_EXCERPT_BYTES,
    RuntimeSmartContextArtifactSemanticLineRange,
};

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

pub(in crate::runtime_state_shared) fn runtime_smart_context_materialize_semantic_range(
    lines: &[&str],
    plan: &prodex_mojo_core::smart_context_markers::SemanticRangePlan,
) -> RuntimeSmartContextArtifactSemanticLineRange {
    let text = runtime_smart_context_line_excerpt(lines, plan.start_line, plan.end_line)
        .expect("Mojo semantic-index planner returned an invalid bounded range");
    assert!(text.len() <= RUNTIME_SMART_CONTEXT_MAX_LINE_INDEX_EXCERPT_BYTES);
    let label = match plan.kind {
        prodex_mojo_core::smart_context_markers::SemanticRangeKind::FileLocation => "file_location",
        prodex_mojo_core::smart_context_markers::SemanticRangeKind::DiffHunk => "diff_hunk",
        prodex_mojo_core::smart_context_markers::SemanticRangeKind::TestFailure => "test_failure",
        prodex_mojo_core::smart_context_markers::SemanticRangeKind::Error => "error",
    };
    RuntimeSmartContextArtifactSemanticLineRange {
        start: plan.start_line,
        end: plan.end_line,
        byte_len: text.len(),
        content_hash: runtime_proxy_crate::smart_context_hash_text(&text),
        text,
        label: Some(label.to_string()),
        path: plan.path.clone(),
        line: plan.line,
        column: plan.column,
        old_start: plan.old_start,
        old_count: plan.old_count,
        new_start: plan.new_start,
        new_count: plan.new_count,
        code: plan.code.clone(),
        symbol: plan.symbol.clone(),
    }
}
