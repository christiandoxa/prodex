use super::super::{
    RuntimeSmartContextArtifactLineIndex, RuntimeSmartContextArtifactSemanticLineRange,
    RuntimeSmartContextSelectiveDiffHunkTerm, RuntimeSmartContextSelectiveRehydrateTerms,
    SMART_CONTEXT_SEMANTIC_REHYDRATE_GLOBAL_MAX_RANGES,
    SMART_CONTEXT_SEMANTIC_REHYDRATE_NARROW_MAX_RANGES, runtime_smart_context_artifact_line_ref,
};
use std::cmp::Reverse;
use std::collections::BTreeSet;

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_matching_semantic_ranges<
    'a,
>(
    artifact_id: &str,
    line_index: &'a RuntimeSmartContextArtifactLineIndex,
    current_text: &str,
    terms: &RuntimeSmartContextSelectiveRehydrateTerms,
) -> Vec<&'a RuntimeSmartContextArtifactSemanticLineRange> {
    let mut ranges = Vec::new();
    let mut seen = BTreeSet::<(usize, usize, String)>::new();
    for range in line_index
        .file_location_ranges
        .iter()
        .chain(line_index.diff_hunk_ranges.iter())
        .chain(line_index.test_failure_ranges.iter())
        .chain(line_index.error_ranges.iter())
        .chain(line_index.symbol_ranges.iter())
    {
        if !runtime_smart_context_artifact_semantic_range_valid(range)
            || !runtime_smart_context_semantic_range_matches_terms(range, terms)
            || current_text.contains(&runtime_smart_context_artifact_line_ref(
                artifact_id,
                range.start,
                range.end,
            ))
            || current_text.contains(&format!(
                "prodex-artifact:{artifact_id}#L{}-L{}",
                range.start, range.end
            ))
            || current_text.contains(&range.text)
        {
            continue;
        }
        if seen.insert((range.start, range.end, range.content_hash.clone())) {
            ranges.push(range);
        }
    }
    ranges.sort_by_key(|range| {
        (
            Reverse(runtime_smart_context_semantic_range_score_with_command(
                range,
                terms,
                line_index.command_kind.as_deref(),
            )),
            range.start,
            range.end,
        )
    });
    ranges
}

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_semantic_range_score_with_command(
    range: &RuntimeSmartContextArtifactSemanticLineRange,
    terms: &RuntimeSmartContextSelectiveRehydrateTerms,
    command_kind: Option<&str>,
) -> usize {
    let mut score = prodex_context::count_critical_signals(&range.text)
        .total()
        .saturating_mul(20);
    if range
        .code
        .as_deref()
        .is_some_and(|code| runtime_smart_context_error_code_matches_terms(code, terms))
    {
        score = score.saturating_add(120);
    }
    if range
        .symbol
        .as_deref()
        .is_some_and(|symbol| runtime_smart_context_symbol_matches_terms(symbol, terms))
    {
        score = score.saturating_add(110);
    }
    if range
        .path
        .as_deref()
        .is_some_and(|path| runtime_smart_context_path_matches_terms(path, terms))
    {
        score = score.saturating_add(90);
    }
    if command_kind.is_some_and(|kind| terms.command_kinds.contains(kind)) {
        score = score.saturating_add(match range.label.as_deref() {
            Some("test_failure" | "error") => 70,
            Some("diff_hunk") => 50,
            Some("file_location") => 30,
            _ => 20,
        });
    }
    if terms
        .diff_hunks
        .iter()
        .any(|term| runtime_smart_context_diff_hunk_range_matches_term(range, term))
    {
        score = score.saturating_add(80);
    }
    score.saturating_add(10_000usize.saturating_sub(range.byte_len.min(10_000)) / 100)
}

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_symbol_matches_terms(
    symbol: &str,
    terms: &RuntimeSmartContextSelectiveRehydrateTerms,
) -> bool {
    terms
        .test_symbols
        .iter()
        .any(|term| runtime_smart_context_symbol_matches_term(symbol, term))
}

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_symbol_matches_term(
    symbol: &str,
    term: &str,
) -> bool {
    let symbol = symbol.trim_end_matches("()");
    let term = term.trim_end_matches("()");
    symbol == term
        || term
            .rsplit("::")
            .next()
            .is_some_and(|suffix| suffix == symbol)
        || symbol
            .rsplit("::")
            .next()
            .is_some_and(|suffix| suffix == term)
        || term
            .rsplit('#')
            .next()
            .is_some_and(|suffix| suffix == symbol)
        || symbol
            .rsplit('#')
            .next()
            .is_some_and(|suffix| suffix == term)
        || term
            .rsplit('.')
            .next()
            .is_some_and(|suffix| suffix == symbol)
        || symbol
            .rsplit('.')
            .next()
            .is_some_and(|suffix| suffix == term)
}

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_path_matches_terms(
    path: &str,
    terms: &RuntimeSmartContextSelectiveRehydrateTerms,
) -> bool {
    terms
        .file_paths
        .iter()
        .any(|term| runtime_smart_context_path_matches_term(path, term))
}

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_path_matches_term(
    path: &str,
    term: &str,
) -> bool {
    let Some(path) = runtime_smart_context_normalized_intent_path(path) else {
        return false;
    };
    let Some(term) = runtime_smart_context_normalized_intent_path(term) else {
        return false;
    };
    path == term
        || path.ends_with(&format!("/{term}"))
        || term.ends_with(&format!("/{path}"))
        || !term.contains('/') && path.rsplit('/').next() == Some(term.as_str())
}

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_normalized_intent_path(
    value: &str,
) -> Option<String> {
    let normalized = value.trim().replace('\\', "/");
    let path = normalized
        .as_str()
        .trim_start_matches("a/")
        .trim_start_matches("b/")
        .trim_start_matches("./")
        .trim_matches(|ch| matches!(ch, '"' | '\'' | '`' | '/' | ':' | ',' | ';'))
        .to_string();
    (!path.is_empty()).then_some(path)
}

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_error_code_matches_terms(
    code: &str,
    terms: &RuntimeSmartContextSelectiveRehydrateTerms,
) -> bool {
    terms
        .error_codes
        .iter()
        .any(|term| code.eq_ignore_ascii_case(term))
}

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_semantic_rehydrate_range_cap(
    terms: &RuntimeSmartContextSelectiveRehydrateTerms,
) -> usize {
    if runtime_smart_context_selective_rehydrate_terms_narrow(terms) {
        SMART_CONTEXT_SEMANTIC_REHYDRATE_NARROW_MAX_RANGES
    } else {
        SMART_CONTEXT_SEMANTIC_REHYDRATE_GLOBAL_MAX_RANGES
    }
}

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_selective_rehydrate_terms_narrow(
    terms: &RuntimeSmartContextSelectiveRehydrateTerms,
) -> bool {
    let term_count = terms
        .file_paths
        .len()
        .saturating_add(terms.error_codes.len())
        .saturating_add(terms.test_symbols.len())
        .saturating_add(terms.diff_hunks.len());
    term_count == 1
}

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_selective_rehydrate_terms_empty(
    terms: &RuntimeSmartContextSelectiveRehydrateTerms,
) -> bool {
    terms.file_paths.is_empty()
        && terms.error_codes.is_empty()
        && terms.test_symbols.is_empty()
        && terms.command_kinds.is_empty()
        && terms.diff_hunks.is_empty()
}

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_selective_rehydrate_terms_strong(
    terms: &RuntimeSmartContextSelectiveRehydrateTerms,
) -> bool {
    !terms.file_paths.is_empty()
        || !terms.error_codes.is_empty()
        || !terms.test_symbols.is_empty()
        || !terms.diff_hunks.is_empty()
}

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_semantic_range_matches_terms(
    range: &RuntimeSmartContextArtifactSemanticLineRange,
    terms: &RuntimeSmartContextSelectiveRehydrateTerms,
) -> bool {
    range
        .path
        .as_deref()
        .is_some_and(|path| runtime_smart_context_path_matches_terms(path, terms))
        || range
            .code
            .as_deref()
            .is_some_and(|code| runtime_smart_context_error_code_matches_terms(code, terms))
        || range
            .symbol
            .as_deref()
            .is_some_and(|symbol| runtime_smart_context_symbol_matches_terms(symbol, terms))
        || terms
            .diff_hunks
            .iter()
            .any(|term| runtime_smart_context_diff_hunk_range_matches_term(range, term))
}

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_diff_hunk_range_matches_term(
    range: &RuntimeSmartContextArtifactSemanticLineRange,
    term: &RuntimeSmartContextSelectiveDiffHunkTerm,
) -> bool {
    range.label.as_deref() == Some("diff_hunk")
        && term
            .path
            .as_deref()
            .map(|path| range.path.as_deref() == Some(path))
            .unwrap_or(true)
        && term
            .old_start
            .map(|old_start| range.old_start == Some(old_start))
            .unwrap_or(true)
        && term
            .new_start
            .map(|new_start| range.new_start == Some(new_start))
            .unwrap_or(true)
}

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_artifact_semantic_range_valid(
    range: &RuntimeSmartContextArtifactSemanticLineRange,
) -> bool {
    range.start > 0
        && range.end >= range.start
        && range.byte_len == range.text.len()
        && range.content_hash == runtime_proxy_crate::smart_context_hash_text(&range.text)
}
