use super::super::{
    RuntimeSmartContextArtifactLineIndex, RuntimeSmartContextArtifactStore,
    RuntimeSmartContextExactAppendixRange, RuntimeSmartContextScoredExactAppendixRange,
    RuntimeSmartContextSelectiveRehydrateTerms, SMART_CONTEXT_BUDGET_AWARE_IMPORT_MAX_RANGES,
    SMART_CONTEXT_BUDGET_AWARE_IMPORT_SCAN_MAX_LINES,
    SMART_CONTEXT_BUDGET_AWARE_REHYDRATE_MAX_RANGES, SMART_CONTEXT_LABEL_REHYDRATE_PLAN_EXACT,
    SMART_CONTEXT_LABEL_SEMANTIC_EXACT, runtime_smart_context_artifact_line_index_range_valid,
    runtime_smart_context_artifact_line_ref, runtime_smart_context_critical_exact_appendix_score,
    runtime_smart_context_line_excerpt,
    runtime_smart_context_render_budgeted_scored_exact_appendix,
};
use super::{
    runtime_smart_context_artifact_semantic_range_valid,
    runtime_smart_context_matching_semantic_ranges,
    runtime_smart_context_semantic_range_matches_terms,
    runtime_smart_context_semantic_range_score_with_command,
    runtime_smart_context_semantic_rehydrate_range_cap,
};

mod selection;

use selection::{
    runtime_smart_context_select_semantic_ranges, runtime_smart_context_semantic_range_candidate_id,
};

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_deferred_read_plan_appendix(
    artifact_id: &str,
    store: &RuntimeSmartContextArtifactStore,
    terms: &RuntimeSmartContextSelectiveRehydrateTerms,
    current_text: &str,
    token_budget: usize,
) -> Option<(String, usize, usize)> {
    let line_index = store.line_index(artifact_id)?;
    let artifact_text = store.get_text(artifact_id);
    let mut ranges = Vec::<RuntimeSmartContextScoredExactAppendixRange>::new();

    for range in line_index
        .symbol_ranges
        .iter()
        .chain(line_index.test_failure_ranges.iter())
        .chain(line_index.error_ranges.iter())
        .chain(line_index.file_location_ranges.iter())
        .chain(line_index.diff_hunk_ranges.iter())
    {
        if !runtime_smart_context_artifact_semantic_range_valid(range)
            || !runtime_smart_context_semantic_range_matches_terms(range, terms)
        {
            continue;
        }
        runtime_smart_context_push_scored_exact_range(
            &mut ranges,
            current_text,
            RuntimeSmartContextExactAppendixRange {
                reference: runtime_smart_context_artifact_line_ref(
                    artifact_id,
                    range.start,
                    range.end,
                ),
                body: range.text.clone(),
            },
            runtime_smart_context_semantic_range_score_with_command(
                range,
                terms,
                line_index.command_kind.as_deref(),
            )
            .saturating_add(200),
        );
    }

    for range in &line_index.critical_ranges {
        if !runtime_smart_context_artifact_line_index_range_valid(range) {
            continue;
        }
        runtime_smart_context_push_scored_exact_range(
            &mut ranges,
            current_text,
            RuntimeSmartContextExactAppendixRange {
                reference: runtime_smart_context_artifact_line_ref(
                    artifact_id,
                    range.start,
                    range.end,
                ),
                body: range.text.clone(),
            },
            runtime_smart_context_critical_exact_appendix_score(
                &RuntimeSmartContextExactAppendixRange {
                    reference: String::new(),
                    body: range.text.clone(),
                },
            )
            .saturating_add(100),
        );
    }

    if let Some(artifact_text) = artifact_text.as_deref() {
        ranges.extend(runtime_smart_context_import_read_plan_ranges(
            artifact_id,
            artifact_text,
            current_text,
            terms,
        ));
    }

    if ranges.is_empty() {
        return None;
    }
    let exact_ranges = ranges
        .iter()
        .map(|range| range.range.clone())
        .collect::<Vec<_>>();
    runtime_smart_context_render_budgeted_scored_exact_appendix(
        SMART_CONTEXT_LABEL_REHYDRATE_PLAN_EXACT,
        exact_ranges,
        SMART_CONTEXT_BUDGET_AWARE_REHYDRATE_MAX_RANGES,
        token_budget,
        |range| {
            ranges
                .iter()
                .find(|candidate| candidate.range.eq(range))
                .map_or(0, |candidate| candidate.score)
        },
    )
}

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_push_scored_exact_range(
    ranges: &mut Vec<RuntimeSmartContextScoredExactAppendixRange>,
    current_text: &str,
    range: RuntimeSmartContextExactAppendixRange,
    score: usize,
) {
    if range.body.trim().is_empty()
        || current_text.contains(&range.reference)
        || current_text.contains(&range.body)
        || ranges.iter().any(|candidate| candidate.range == range)
    {
        return;
    }
    ranges.push(RuntimeSmartContextScoredExactAppendixRange { range, score });
}

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_import_read_plan_ranges(
    artifact_id: &str,
    artifact_text: &str,
    current_text: &str,
    terms: &RuntimeSmartContextSelectiveRehydrateTerms,
) -> Vec<RuntimeSmartContextScoredExactAppendixRange> {
    let lines = artifact_text.lines().collect::<Vec<_>>();
    let mut ranges = Vec::new();
    let mut start: Option<usize> = None;
    let mut end = 0usize;
    for (index, line) in lines
        .iter()
        .enumerate()
        .take(SMART_CONTEXT_BUDGET_AWARE_IMPORT_SCAN_MAX_LINES)
    {
        let line_number = index + 1;
        if runtime_smart_context_line_is_import(line) {
            start.get_or_insert(line_number);
            end = line_number;
            continue;
        }
        if let Some(range_start) = start.take() {
            runtime_smart_context_push_import_read_plan_range(
                artifact_id,
                &lines,
                range_start,
                end,
                current_text,
                terms,
                &mut ranges,
            );
            if ranges.len() >= SMART_CONTEXT_BUDGET_AWARE_IMPORT_MAX_RANGES {
                return ranges;
            }
        }
        if !line.trim().is_empty()
            && !line.trim_start().starts_with("//")
            && !line.trim_start().starts_with('#')
        {
            continue;
        }
    }
    if let Some(range_start) = start {
        runtime_smart_context_push_import_read_plan_range(
            artifact_id,
            &lines,
            range_start,
            end,
            current_text,
            terms,
            &mut ranges,
        );
    }
    ranges
}

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_push_import_read_plan_range(
    artifact_id: &str,
    lines: &[&str],
    start: usize,
    end: usize,
    current_text: &str,
    terms: &RuntimeSmartContextSelectiveRehydrateTerms,
    ranges: &mut Vec<RuntimeSmartContextScoredExactAppendixRange>,
) {
    if ranges.len() >= SMART_CONTEXT_BUDGET_AWARE_IMPORT_MAX_RANGES {
        return;
    }
    let Some(body) = runtime_smart_context_line_excerpt(lines, start, end) else {
        return;
    };
    let mut score = 40usize;
    for term in terms
        .file_paths
        .iter()
        .chain(terms.test_symbols.iter())
        .chain(terms.error_codes.iter())
    {
        if body.contains(term) {
            score = score.saturating_add(40);
        }
    }
    runtime_smart_context_push_scored_exact_range(
        ranges,
        current_text,
        RuntimeSmartContextExactAppendixRange {
            reference: runtime_smart_context_artifact_line_ref(artifact_id, start, end),
            body,
        },
        score,
    );
}

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_line_is_import(
    line: &str,
) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("use ")
        || trimmed.starts_with("pub use ")
        || trimmed.starts_with("extern crate ")
        || trimmed.starts_with("import ")
        || trimmed.starts_with("from ") && trimmed.contains(" import ")
}

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_matching_semantic_range_appendix_with_budget(
    artifact_id: &str,
    line_index: &RuntimeSmartContextArtifactLineIndex,
    current_text: &str,
    terms: &RuntimeSmartContextSelectiveRehydrateTerms,
    token_budget: usize,
) -> Option<(
    String,
    usize,
    usize,
    runtime_proxy_crate::SmartContextCandidateSelection,
)> {
    let ranges = runtime_smart_context_matching_semantic_ranges(
        artifact_id,
        line_index,
        current_text,
        terms,
    );
    if ranges.is_empty() {
        return None;
    }
    let selection = runtime_smart_context_select_semantic_ranges(
        artifact_id,
        line_index,
        terms,
        &ranges,
        token_budget,
        runtime_smart_context_semantic_rehydrate_range_cap(terms),
    );
    let selected_ids = selection
        .selected_ids
        .iter()
        .take(runtime_smart_context_semantic_rehydrate_range_cap(terms))
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();
    let ranges = ranges
        .into_iter()
        .filter(|range| {
            selected_ids.contains(&runtime_smart_context_semantic_range_candidate_id(
                artifact_id,
                range,
            ))
        })
        .collect::<Vec<_>>();
    if ranges.is_empty() {
        return None;
    }

    let exact_ranges = ranges
        .iter()
        .map(|range| RuntimeSmartContextExactAppendixRange {
            reference: runtime_smart_context_artifact_line_ref(artifact_id, range.start, range.end),
            body: range.text.clone(),
        })
        .collect::<Vec<_>>();
    let scored_ranges = ranges
        .iter()
        .zip(exact_ranges.iter())
        .map(
            |(range, exact)| RuntimeSmartContextScoredExactAppendixRange {
                range: exact.clone(),
                score: runtime_smart_context_semantic_range_score_with_command(
                    range,
                    terms,
                    line_index.command_kind.as_deref(),
                ),
            },
        )
        .collect::<Vec<_>>();

    runtime_smart_context_render_budgeted_scored_exact_appendix(
        SMART_CONTEXT_LABEL_SEMANTIC_EXACT,
        exact_ranges,
        usize::MAX,
        token_budget,
        |range| {
            scored_ranges
                .iter()
                .find(|candidate| candidate.range.eq(range))
                .map_or(0, |candidate| candidate.score)
        },
    )
    .map(|(appendix, range_count, token_cost)| (appendix, range_count, token_cost, selection))
}
