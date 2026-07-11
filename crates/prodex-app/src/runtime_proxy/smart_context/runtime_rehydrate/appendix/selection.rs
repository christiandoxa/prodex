//! Candidate selection helpers for semantic exact-range appendix rehydration.

use super::super::super::{
    RuntimeSmartContextArtifactLineIndex, RuntimeSmartContextArtifactSemanticLineRange,
    RuntimeSmartContextSelectiveRehydrateTerms,
};
use super::super::{
    runtime_smart_context_error_code_matches_terms, runtime_smart_context_path_matches_terms,
    runtime_smart_context_semantic_range_score_with_command,
    runtime_smart_context_symbol_matches_terms,
};

pub(super) fn runtime_smart_context_select_semantic_ranges(
    artifact_id: &str,
    line_index: &RuntimeSmartContextArtifactLineIndex,
    terms: &RuntimeSmartContextSelectiveRehydrateTerms,
    ranges: &[&RuntimeSmartContextArtifactSemanticLineRange],
    token_budget: usize,
    range_cap: usize,
) -> runtime_proxy_crate::SmartContextCandidateSelection {
    let candidates = ranges
        .iter()
        .map(|range| {
            runtime_smart_context_semantic_range_candidate(artifact_id, line_index, terms, range)
        })
        .collect::<Vec<_>>();
    runtime_proxy_crate::smart_context_select_context_candidates(
        runtime_proxy_crate::SmartContextCandidateSelectionInput {
            candidates,
            token_budget: token_budget as u64,
            minimum_allocations: runtime_smart_context_semantic_minimum_allocations(
                token_budget,
                range_cap,
            ),
            debug_scores: false,
        },
    )
}

fn runtime_smart_context_semantic_range_candidate(
    artifact_id: &str,
    line_index: &RuntimeSmartContextArtifactLineIndex,
    terms: &RuntimeSmartContextSelectiveRehydrateTerms,
    range: &RuntimeSmartContextArtifactSemanticLineRange,
) -> runtime_proxy_crate::SmartContextContextCandidate {
    let score = runtime_smart_context_semantic_range_score_with_command(
        range,
        terms,
        line_index.command_kind.as_deref(),
    );
    runtime_proxy_crate::SmartContextContextCandidate {
        id: runtime_smart_context_semantic_range_candidate_id(artifact_id, range),
        kind: runtime_smart_context_semantic_candidate_kind(range),
        allocation: runtime_smart_context_semantic_candidate_allocation(range),
        provenance: artifact_id.to_string(),
        content_hash: range.content_hash.clone(),
        token_cost: runtime_proxy_crate::smart_context_estimate_tokens_from_body_bytes(
            range.byte_len,
        ),
        recency_rank: range.start.min(u32::MAX as usize) as u32,
        critical_signal_count: prodex_context::count_critical_signals(&range.text)
            .total()
            .min(u16::MAX as usize) as u16,
        matching_paths: range
            .path
            .iter()
            .filter(|path| runtime_smart_context_path_matches_terms(path, terms))
            .cloned()
            .collect(),
        matching_symbols: range
            .symbol
            .iter()
            .filter(|symbol| runtime_smart_context_symbol_matches_terms(symbol, terms))
            .cloned()
            .collect(),
        matching_error_codes: range
            .code
            .iter()
            .filter(|code| runtime_smart_context_error_code_matches_terms(code, terms))
            .cloned()
            .collect(),
        command_kind: line_index.command_kind.clone(),
        changed_file_relationship: range
            .path
            .as_deref()
            .is_some_and(|path| runtime_smart_context_path_matches_terms(path, terms)),
        dependency_relationship: range.label.as_deref() == Some("import")
            || range.label.as_deref() == Some("file_location")
            || range
                .symbol
                .as_ref()
                .is_some_and(|symbol| runtime_smart_context_symbol_matches_terms(symbol, terms)),
        unresolved_failure_relationship: range.label.as_deref() == Some("test_failure")
            || range.label.as_deref() == Some("error")
            || range
                .code
                .as_deref()
                .is_some_and(|code| runtime_smart_context_error_code_matches_terms(code, terms)),
        novelty_fingerprint: runtime_smart_context_semantic_range_novelty_fingerprint(range),
        existing_context_overlap_percent: 0,
        confidence_percent: runtime_smart_context_semantic_candidate_confidence_percent(
            range, score,
        ),
        rehydration_overhead_tokens: 8,
        mandatory: false,
    }
}

pub(super) fn runtime_smart_context_semantic_range_candidate_id(
    artifact_id: &str,
    range: &RuntimeSmartContextArtifactSemanticLineRange,
) -> String {
    format!(
        "{artifact_id}:{}:{}:{}",
        range.start, range.end, range.content_hash
    )
}

fn runtime_smart_context_semantic_candidate_kind(
    range: &RuntimeSmartContextArtifactSemanticLineRange,
) -> runtime_proxy_crate::SmartContextCandidateKind {
    match range.label.as_deref() {
        Some("diff_hunk") => runtime_proxy_crate::SmartContextCandidateKind::DiffHunk,
        Some("file_location") => runtime_proxy_crate::SmartContextCandidateKind::DependencyRange,
        Some("test_failure" | "error") => {
            runtime_proxy_crate::SmartContextCandidateKind::PriorFailure
        }
        _ => runtime_proxy_crate::SmartContextCandidateKind::ArtifactExactRange,
    }
}

fn runtime_smart_context_semantic_candidate_allocation(
    range: &RuntimeSmartContextArtifactSemanticLineRange,
) -> runtime_proxy_crate::SmartContextCandidateAllocation {
    match range.label.as_deref() {
        Some("test_failure" | "error") => {
            runtime_proxy_crate::SmartContextCandidateAllocation::ActiveFailure
        }
        Some("diff_hunk") => {
            runtime_proxy_crate::SmartContextCandidateAllocation::RecentlyChangedFiles
        }
        Some("file_location") => {
            runtime_proxy_crate::SmartContextCandidateAllocation::DependencyClosure
        }
        _ if range.symbol.is_some() => {
            runtime_proxy_crate::SmartContextCandidateAllocation::DependencyClosure
        }
        _ => runtime_proxy_crate::SmartContextCandidateAllocation::General,
    }
}

fn runtime_smart_context_semantic_range_novelty_fingerprint(
    range: &RuntimeSmartContextArtifactSemanticLineRange,
) -> Option<String> {
    range
        .path
        .clone()
        .or_else(|| range.symbol.clone())
        .or_else(|| range.code.clone())
        .or_else(|| range.label.clone())
}

fn runtime_smart_context_semantic_candidate_confidence_percent(
    range: &RuntimeSmartContextArtifactSemanticLineRange,
    score: usize,
) -> u8 {
    let mut confidence = 70u8;
    if range.content_hash == runtime_proxy_crate::smart_context_hash_text(&range.text) {
        confidence = confidence.saturating_add(10);
    }
    if range.path.is_some() || range.symbol.is_some() || range.code.is_some() {
        confidence = confidence.saturating_add(10);
    }
    if score >= 120 {
        confidence = confidence.saturating_add(10);
    }
    confidence.min(100)
}

fn runtime_smart_context_semantic_minimum_allocations(
    token_budget: usize,
    range_cap: usize,
) -> Vec<runtime_proxy_crate::SmartContextCandidateMinimumAllocation> {
    if token_budget == 0 || range_cap == 0 {
        return Vec::new();
    }
    let min_tokens = (token_budget as u64 / range_cap as u64).max(1);
    vec![
        runtime_proxy_crate::SmartContextCandidateMinimumAllocation {
            allocation: runtime_proxy_crate::SmartContextCandidateAllocation::ActiveFailure,
            min_tokens,
        },
        runtime_proxy_crate::SmartContextCandidateMinimumAllocation {
            allocation: runtime_proxy_crate::SmartContextCandidateAllocation::DependencyClosure,
            min_tokens,
        },
        runtime_proxy_crate::SmartContextCandidateMinimumAllocation {
            allocation: runtime_proxy_crate::SmartContextCandidateAllocation::RecentlyChangedFiles,
            min_tokens,
        },
    ]
}
