use super::*;

fn candidate(id: &str, token_cost: u64) -> SmartContextContextCandidate {
    SmartContextContextCandidate {
        id: id.to_string(),
        kind: SmartContextCandidateKind::ArtifactExactRange,
        allocation: SmartContextCandidateAllocation::General,
        provenance: format!("artifact:{id}"),
        content_hash: smart_context_hash_text(id),
        token_cost,
        recency_rank: 20,
        critical_signal_count: 0,
        matching_paths: Vec::new(),
        matching_symbols: Vec::new(),
        matching_error_codes: Vec::new(),
        command_kind: None,
        changed_file_relationship: false,
        dependency_relationship: false,
        unresolved_failure_relationship: false,
        novelty_fingerprint: Some(id.to_string()),
        existing_context_overlap_percent: 0,
        confidence_percent: 90,
        rehydration_overhead_tokens: 0,
        mandatory: false,
    }
}

#[test]
fn candidate_score_rewards_relevance_confidence_criticality_and_cost() {
    let mut high = candidate("high", 100);
    high.critical_signal_count = 2;
    high.matching_paths = vec!["src/lib.rs".to_string()];
    high.matching_symbols = vec!["rewrite".to_string()];
    high.matching_error_codes = vec!["E0425".to_string()];
    high.changed_file_relationship = true;
    high.unresolved_failure_relationship = true;
    high.confidence_percent = 95;

    let mut low = candidate("low", 800);
    low.confidence_percent = 50;
    low.existing_context_overlap_percent = 80;

    assert!(
        smart_context_candidate_score(&high).utility_points
            > smart_context_candidate_score(&low).utility_points
    );
}

#[test]
fn candidate_selection_is_budgeted_deterministic_and_diversity_aware() {
    let mut rust_error = candidate("rust-error", 200);
    rust_error.allocation = SmartContextCandidateAllocation::ActiveFailure;
    rust_error.matching_paths = vec!["src/lib.rs".to_string()];
    rust_error.matching_error_codes = vec!["E0425".to_string()];
    rust_error.unresolved_failure_relationship = true;
    rust_error.critical_signal_count = 2;
    rust_error.novelty_fingerprint = Some("failure-a".to_string());
    rust_error.mandatory = true;

    let mut duplicate_error = rust_error.clone();
    duplicate_error.id = "duplicate-error".to_string();
    duplicate_error.token_cost = 190;
    duplicate_error.content_hash = smart_context_hash_text("duplicate-error");
    duplicate_error.confidence_percent = 1;
    duplicate_error.existing_context_overlap_percent = 95;
    duplicate_error.mandatory = false;

    let mut dependency = candidate("dependency", 220);
    dependency.allocation = SmartContextCandidateAllocation::DependencyClosure;
    dependency.matching_paths = vec!["src/dependency.rs".to_string()];
    dependency.matching_symbols = vec!["rewrite_helper".to_string()];
    dependency.dependency_relationship = true;
    dependency.changed_file_relationship = true;
    dependency.critical_signal_count = 1;
    dependency.novelty_fingerprint = Some("dependency".to_string());

    let selection = smart_context_select_context_candidates(SmartContextCandidateSelectionInput {
        candidates: vec![duplicate_error, dependency, rust_error],
        token_budget: 450,
        minimum_allocations: Vec::new(),
        debug_scores: true,
    });

    assert_eq!(selection.used_tokens, 420);
    assert_eq!(
        selection.selected_ids,
        vec!["rust-error".to_string(), "dependency".to_string()]
    );
    assert_eq!(selection.omitted.len(), 1);
    assert_eq!(selection.omitted[0].id, "duplicate-error");
    assert_eq!(
        selection.omitted[0].reason,
        SmartContextCandidateRejectionReason::BudgetExceeded
    );
    assert_eq!(selection.scores.len(), 2);
}

#[test]
fn candidate_selection_honors_mandatory_and_minimum_allocations() {
    let mut intent = candidate("intent", 100);
    intent.mandatory = true;
    intent.allocation = SmartContextCandidateAllocation::CurrentUserIntent;

    let mut command = candidate("command", 180);
    command.allocation = SmartContextCandidateAllocation::CurrentCommandOutput;
    command.command_kind = Some("test".to_string());

    let mut general = candidate("general", 150);
    general.matching_paths = vec!["src/other.rs".to_string()];

    let selection = smart_context_select_context_candidates(SmartContextCandidateSelectionInput {
        candidates: vec![general, command, intent],
        token_budget: 300,
        minimum_allocations: vec![SmartContextCandidateMinimumAllocation {
            allocation: SmartContextCandidateAllocation::CurrentCommandOutput,
            min_tokens: 100,
        }],
        debug_scores: false,
    });

    assert_eq!(
        selection.selected_ids,
        vec!["intent".to_string(), "command".to_string()]
    );
    assert_eq!(selection.used_tokens, 280);
    assert!(selection.scores.is_empty());
}

#[test]
fn candidate_selection_rejects_zero_cost_and_duplicate_ids() {
    let duplicate = candidate("same", 100);
    let mut duplicate_later = candidate("same", 90);
    duplicate_later.content_hash = smart_context_hash_text("same-later");
    let zero = candidate("zero", 0);

    let selection = smart_context_select_context_candidates(SmartContextCandidateSelectionInput {
        candidates: vec![duplicate, duplicate_later, zero],
        token_budget: 100,
        minimum_allocations: Vec::new(),
        debug_scores: false,
    });

    assert_eq!(selection.selected_ids, vec!["same".to_string()]);
    assert!(selection.omitted.iter().any(|omission| {
        omission.id == "same"
            && omission.reason == SmartContextCandidateRejectionReason::DuplicateId
    }));
    assert!(selection.omitted.iter().any(|omission| {
        omission.id == "zero"
            && omission.reason == SmartContextCandidateRejectionReason::ZeroTokenCost
    }));
}
