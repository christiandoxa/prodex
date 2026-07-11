use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SmartContextCandidateKind {
    ArtifactExactRange,
    StaticPromptSection,
    HistoricalMessage,
    ToolOutputChunk,
    ToolArgumentDelta,
    RepoStateFact,
    DiffHunk,
    DependencyRange,
    PriorFailure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SmartContextCandidateAllocation {
    CurrentUserIntent,
    ActiveFailure,
    RecentlyChangedFiles,
    CurrentCommandOutput,
    DependencyClosure,
    ExactCriticalEvidence,
    General,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SmartContextCandidateRejectionReason {
    DuplicateId,
    ZeroTokenCost,
    BudgetExceeded,
    MinimumAllocationBudgetExceeded,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartContextContextCandidate {
    pub id: String,
    pub kind: SmartContextCandidateKind,
    pub allocation: SmartContextCandidateAllocation,
    pub provenance: String,
    pub content_hash: String,
    pub token_cost: u64,
    pub recency_rank: u32,
    pub critical_signal_count: u16,
    pub matching_paths: Vec<String>,
    pub matching_symbols: Vec<String>,
    pub matching_error_codes: Vec<String>,
    pub command_kind: Option<String>,
    pub changed_file_relationship: bool,
    pub dependency_relationship: bool,
    pub unresolved_failure_relationship: bool,
    pub novelty_fingerprint: Option<String>,
    pub existing_context_overlap_percent: u8,
    pub confidence_percent: u8,
    pub rehydration_overhead_tokens: u64,
    pub mandatory: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartContextCandidateScore {
    pub id: String,
    pub utility_points: u64,
    pub relevance_points: u64,
    pub token_cost_with_overhead: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmartContextCandidateMinimumAllocation {
    pub allocation: SmartContextCandidateAllocation,
    pub min_tokens: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SmartContextCandidateSelectionInput {
    pub candidates: Vec<SmartContextContextCandidate>,
    pub token_budget: u64,
    pub minimum_allocations: Vec<SmartContextCandidateMinimumAllocation>,
    pub debug_scores: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartContextCandidateSelection {
    pub selected_ids: Vec<String>,
    pub omitted: Vec<SmartContextCandidateOmission>,
    pub used_tokens: u64,
    pub scores: Vec<SmartContextCandidateScore>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartContextCandidateOmission {
    pub id: String,
    pub reason: SmartContextCandidateRejectionReason,
}

pub fn smart_context_candidate_score(
    candidate: &SmartContextContextCandidate,
) -> SmartContextCandidateScore {
    let token_cost_with_overhead = candidate
        .token_cost
        .saturating_add(candidate.rehydration_overhead_tokens)
        .max(1);
    let relevance_points = smart_context_candidate_relevance_points(candidate);
    let confidence = u64::from(candidate.confidence_percent.clamp(1, 100));
    let criticality = 100u64.saturating_add(u64::from(candidate.critical_signal_count) * 35);
    let recency =
        100u64.saturating_add(100u64.saturating_sub(u64::from(candidate.recency_rank.min(100))));
    let unresolved_failure = if candidate.unresolved_failure_relationship {
        180
    } else {
        100
    };
    let dependency = if candidate.dependency_relationship {
        145
    } else {
        100
    };
    let novelty = 100u64.saturating_sub(u64::from(
        candidate.existing_context_overlap_percent.min(95),
    ));
    let mandatory = if candidate.mandatory { 200 } else { 100 };
    let utility_points = relevance_points
        .saturating_mul(confidence)
        .saturating_mul(criticality)
        .saturating_mul(recency)
        .saturating_mul(unresolved_failure)
        .saturating_mul(dependency)
        .saturating_mul(novelty)
        .saturating_mul(mandatory)
        / 100_000_000
        / token_cost_with_overhead;

    SmartContextCandidateScore {
        id: candidate.id.clone(),
        utility_points,
        relevance_points,
        token_cost_with_overhead,
    }
}

pub fn smart_context_select_context_candidates(
    input: SmartContextCandidateSelectionInput,
) -> SmartContextCandidateSelection {
    let mut candidates_by_id = BTreeMap::<String, SmartContextContextCandidate>::new();
    let mut omitted = Vec::new();
    for candidate in input.candidates {
        if candidate.token_cost == 0 {
            omitted.push(SmartContextCandidateOmission {
                id: candidate.id,
                reason: SmartContextCandidateRejectionReason::ZeroTokenCost,
            });
            continue;
        }
        if candidates_by_id
            .insert(candidate.id.clone(), candidate.clone())
            .is_some()
        {
            omitted.push(SmartContextCandidateOmission {
                id: candidate.id,
                reason: SmartContextCandidateRejectionReason::DuplicateId,
            });
        }
    }

    let mut state = SmartContextCandidateSelectionState::default();
    let mut scores_by_id = candidates_by_id
        .values()
        .map(|candidate| {
            (
                candidate.id.clone(),
                smart_context_candidate_score(candidate),
            )
        })
        .collect::<BTreeMap<_, _>>();

    for candidate in candidates_by_id
        .values()
        .filter(|candidate| candidate.mandatory)
    {
        if !state.try_select(candidate, input.token_budget) {
            omitted.push(SmartContextCandidateOmission {
                id: candidate.id.clone(),
                reason: SmartContextCandidateRejectionReason::BudgetExceeded,
            });
        }
    }

    for minimum in &input.minimum_allocations {
        let mut allocation_used = smart_context_selected_allocation_tokens(
            *minimum,
            &state.selected_set,
            &candidates_by_id,
        );
        while allocation_used < minimum.min_tokens {
            let Some(candidate) = smart_context_best_candidate_for_allocation(
                &candidates_by_id,
                &scores_by_id,
                &state,
                minimum.allocation,
            ) else {
                break;
            };
            if !state.try_select(candidate, input.token_budget) {
                omitted.push(SmartContextCandidateOmission {
                    id: candidate.id.clone(),
                    reason: SmartContextCandidateRejectionReason::MinimumAllocationBudgetExceeded,
                });
                state.rejected_set.insert(candidate.id.clone());
                break;
            }
            allocation_used = allocation_used.saturating_add(candidate.token_cost);
        }
    }

    while let Some(candidate) =
        smart_context_best_candidate(&candidates_by_id, &scores_by_id, &state)
    {
        if !state.try_select(candidate, input.token_budget) {
            omitted.push(SmartContextCandidateOmission {
                id: candidate.id.clone(),
                reason: SmartContextCandidateRejectionReason::BudgetExceeded,
            });
            state.rejected_set.insert(candidate.id.clone());
            scores_by_id.remove(&candidate.id);
        }
    }

    for id in candidates_by_id.keys() {
        if !state.selected_set.contains(id) && !omitted.iter().any(|omission| omission.id == *id) {
            omitted.push(SmartContextCandidateOmission {
                id: id.clone(),
                reason: SmartContextCandidateRejectionReason::BudgetExceeded,
            });
        }
    }
    let mut scores = if input.debug_scores {
        scores_by_id.into_values().collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    scores.sort_by(|left, right| left.id.cmp(&right.id));

    SmartContextCandidateSelection {
        selected_ids: state.selected_ids,
        omitted,
        used_tokens: state.used_tokens,
        scores,
    }
}

fn smart_context_candidate_relevance_points(candidate: &SmartContextContextCandidate) -> u64 {
    let mut relevance = 100u64;
    relevance = relevance.saturating_add(candidate.matching_paths.len() as u64 * 80);
    relevance = relevance.saturating_add(candidate.matching_symbols.len() as u64 * 70);
    relevance = relevance.saturating_add(candidate.matching_error_codes.len() as u64 * 90);
    if candidate
        .command_kind
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty())
    {
        relevance = relevance.saturating_add(40);
    }
    if candidate.changed_file_relationship {
        relevance = relevance.saturating_add(120);
    }
    if candidate.dependency_relationship {
        relevance = relevance.saturating_add(90);
    }
    if candidate.unresolved_failure_relationship {
        relevance = relevance.saturating_add(140);
    }
    relevance
}

fn smart_context_best_candidate<'a>(
    candidates: &'a BTreeMap<String, SmartContextContextCandidate>,
    scores: &BTreeMap<String, SmartContextCandidateScore>,
    state: &SmartContextCandidateSelectionState,
) -> Option<&'a SmartContextContextCandidate> {
    candidates
        .values()
        .filter(|candidate| {
            !state.selected_set.contains(&candidate.id)
                && !state.rejected_set.contains(&candidate.id)
        })
        .max_by_key(|candidate| {
            smart_context_candidate_diversity_score(
                candidate,
                scores,
                &state.selected_fingerprints,
                &state.selected_paths,
                &state.selected_symbols,
            )
        })
}

fn smart_context_best_candidate_for_allocation<'a>(
    candidates: &'a BTreeMap<String, SmartContextContextCandidate>,
    scores: &BTreeMap<String, SmartContextCandidateScore>,
    state: &SmartContextCandidateSelectionState,
    allocation: SmartContextCandidateAllocation,
) -> Option<&'a SmartContextContextCandidate> {
    candidates
        .values()
        .filter(|candidate| {
            candidate.allocation == allocation
                && !state.selected_set.contains(&candidate.id)
                && !state.rejected_set.contains(&candidate.id)
        })
        .max_by_key(|candidate| {
            smart_context_candidate_diversity_score(
                candidate,
                scores,
                &state.selected_fingerprints,
                &state.selected_paths,
                &state.selected_symbols,
            )
        })
}

fn smart_context_candidate_diversity_score(
    candidate: &SmartContextContextCandidate,
    scores: &BTreeMap<String, SmartContextCandidateScore>,
    selected_fingerprints: &BTreeSet<String>,
    selected_paths: &BTreeSet<String>,
    selected_symbols: &BTreeSet<String>,
) -> (u64, std::cmp::Reverse<u64>, std::cmp::Reverse<String>) {
    let utility = scores
        .get(&candidate.id)
        .map(|score| score.utility_points)
        .unwrap_or_default();
    let diversity_divisor = smart_context_candidate_overlap_divisor(
        candidate,
        selected_fingerprints,
        selected_paths,
        selected_symbols,
    );
    let diversity_adjusted = utility / diversity_divisor;
    (
        diversity_adjusted,
        std::cmp::Reverse(candidate.token_cost),
        std::cmp::Reverse(candidate.id.clone()),
    )
}

fn smart_context_candidate_overlap_divisor(
    candidate: &SmartContextContextCandidate,
    selected_fingerprints: &BTreeSet<String>,
    selected_paths: &BTreeSet<String>,
    selected_symbols: &BTreeSet<String>,
) -> u64 {
    let mut divisor = 1u64;
    if candidate
        .novelty_fingerprint
        .as_ref()
        .is_some_and(|fingerprint| selected_fingerprints.contains(fingerprint))
    {
        divisor = divisor.saturating_add(4);
    }
    if candidate
        .matching_paths
        .iter()
        .any(|path| selected_paths.contains(path))
    {
        divisor = divisor.saturating_add(2);
    }
    if candidate
        .matching_symbols
        .iter()
        .any(|symbol| selected_symbols.contains(symbol))
    {
        divisor = divisor.saturating_add(2);
    }
    divisor
}

fn smart_context_selected_allocation_tokens(
    minimum: SmartContextCandidateMinimumAllocation,
    selected_set: &BTreeSet<String>,
    candidates: &BTreeMap<String, SmartContextContextCandidate>,
) -> u64 {
    selected_set
        .iter()
        .filter_map(|id| candidates.get(id))
        .filter(|candidate| candidate.allocation == minimum.allocation)
        .map(|candidate| candidate.token_cost)
        .sum()
}

#[derive(Debug, Default)]
struct SmartContextCandidateSelectionState {
    selected_ids: Vec<String>,
    used_tokens: u64,
    selected_set: BTreeSet<String>,
    rejected_set: BTreeSet<String>,
    selected_fingerprints: BTreeSet<String>,
    selected_paths: BTreeSet<String>,
    selected_symbols: BTreeSet<String>,
}

impl SmartContextCandidateSelectionState {
    fn try_select(&mut self, candidate: &SmartContextContextCandidate, token_budget: u64) -> bool {
        if self.selected_set.contains(&candidate.id) {
            return true;
        }
        let next_used = self.used_tokens.saturating_add(candidate.token_cost);
        if next_used > token_budget {
            return false;
        }
        self.used_tokens = next_used;
        self.selected_set.insert(candidate.id.clone());
        self.selected_ids.push(candidate.id.clone());
        if let Some(fingerprint) = &candidate.novelty_fingerprint {
            self.selected_fingerprints.insert(fingerprint.clone());
        }
        self.selected_paths
            .extend(candidate.matching_paths.iter().cloned());
        self.selected_symbols
            .extend(candidate.matching_symbols.iter().cloned());
        true
    }
}
