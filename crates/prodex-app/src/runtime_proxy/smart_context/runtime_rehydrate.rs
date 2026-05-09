use super::*;

pub(super) fn runtime_smart_context_exact_header(request: &RuntimeProxyRequest) -> bool {
    runtime_proxy_request_header_value(&request.headers, "x-prodex-smart-context")
        .is_some_and(|value| value.eq_ignore_ascii_case("exact"))
}

pub(super) fn runtime_smart_context_missing_artifact_refs(
    value: &serde_json::Value,
    shared: &RuntimeRotationProxyShared,
) -> Vec<String> {
    let ref_ids = runtime_smart_context_collect_rehydratable_artifact_ref_ids(value);
    if ref_ids.is_empty() {
        return Vec::new();
    }
    with_runtime_smart_context_artifacts(shared, |store| {
        ref_ids
            .into_iter()
            .filter(|id| !store.contains(id))
            .collect::<Vec<_>>()
    })
    .unwrap_or_default()
}

pub(super) fn runtime_smart_context_auto_rehydrate_plan(
    value: &serde_json::Value,
    store: &RuntimeSmartContextArtifactStore,
    token_budget: usize,
    tier: runtime_proxy_crate::SmartContextTokenBudgetTier,
) -> runtime_proxy_crate::SmartContextRehydratePlan {
    let mut refs_by_id = BTreeMap::<String, usize>::new();
    let mut available_ids = BTreeSet::<String>::new();
    for reference in runtime_smart_context_collect_rehydratable_artifact_refs(value) {
        if store.contains(&reference.id) {
            available_ids.insert(reference.id.clone());
        }
        let token_cost =
            runtime_smart_context_rehydrate_ref_token_cost(&reference, store).unwrap_or(0);
        refs_by_id
            .entry(reference.id)
            .and_modify(|cost| *cost = cost.saturating_add(token_cost))
            .or_insert(token_cost);
    }

    runtime_proxy_crate::smart_context_auto_rehydrate_plan(
        refs_by_id.into_iter().map(|(id, token_cost)| {
            runtime_proxy_crate::SmartContextRehydrateRef {
                id,
                token_cost,
                required: true,
            }
        }),
        available_ids,
        token_budget,
        tier,
    )
}

pub(super) fn runtime_smart_context_rehydrate_ref_token_cost(
    reference: &RuntimeSmartContextArtifactReference,
    store: &RuntimeSmartContextArtifactStore,
) -> Option<usize> {
    let artifact_text = store.get_text(&reference.id)?;
    let rehydrated =
        runtime_smart_context_rehydrated_artifact_reference_text(&artifact_text, reference)?;
    runtime_proxy_crate::smart_context_estimate_tokens_from_body_bytes(rehydrated.len())
        .try_into()
        .ok()
}

pub(super) fn runtime_smart_context_deferred_rehydrate_refs(
    plan: &runtime_proxy_crate::SmartContextRehydratePlan,
) -> Vec<String> {
    plan.actions
        .iter()
        .filter_map(|action| match action {
            runtime_proxy_crate::SmartContextRehydrateAction::Defer { id, .. } => Some(id.clone()),
            runtime_proxy_crate::SmartContextRehydrateAction::Rehydrate { .. } => None,
        })
        .collect()
}

#[cfg(test)]
pub(super) fn runtime_smart_context_rehydrate_value(
    value: &mut serde_json::Value,
    store: &RuntimeSmartContextArtifactStore,
    stats: &mut RuntimeSmartContextTransformStats,
) {
    let plan = runtime_smart_context_auto_rehydrate_plan(
        value,
        store,
        usize::MAX,
        runtime_proxy_crate::SmartContextTokenBudgetTier::Exact,
    );
    runtime_smart_context_rehydrate_value_with_plan(value, store, &plan, stats);
}

pub(super) fn runtime_smart_context_rehydrate_value_with_plan(
    value: &mut serde_json::Value,
    store: &RuntimeSmartContextArtifactStore,
    plan: &runtime_proxy_crate::SmartContextRehydratePlan,
    stats: &mut RuntimeSmartContextTransformStats,
) {
    let aliases = runtime_smart_context_collect_artifact_aliases(value);
    let rehydrate_ids = plan
        .actions
        .iter()
        .filter_map(|action| match action {
            runtime_proxy_crate::SmartContextRehydrateAction::Rehydrate { id, .. } => {
                Some(id.clone())
            }
            runtime_proxy_crate::SmartContextRehydrateAction::Defer { .. } => None,
        })
        .collect::<BTreeSet<_>>();
    runtime_smart_context_rehydrate_value_for_ids(value, store, &rehydrate_ids, &aliases, stats);
}

pub(super) fn runtime_smart_context_rehydrate_value_for_ids(
    value: &mut serde_json::Value,
    store: &RuntimeSmartContextArtifactStore,
    rehydrate_ids: &BTreeSet<String>,
    aliases: &BTreeMap<String, String>,
    stats: &mut RuntimeSmartContextTransformStats,
) {
    if runtime_smart_context_value_is_static_context_item(value) {
        return;
    }
    match value {
        serde_json::Value::String(text) => {
            let mut next = text.clone();
            for reference in runtime_smart_context_artifact_ref_occurrences_from_text(text, aliases)
            {
                if rehydrate_ids.contains(&reference.id)
                    && let Some(artifact_text) = store.get_text(&reference.id)
                    && let Some(rehydrated_text) =
                        runtime_smart_context_rehydrated_artifact_reference_text(
                            &artifact_text,
                            &reference,
                        )
                {
                    let legacy_marker = format!("prodex-artifact:{}", reference.id);
                    let short_marker = runtime_smart_context_artifact_ref(&reference.id);
                    if reference.line_range.is_none()
                        && runtime_smart_context_text_is_artifact_marker_summary(
                            &next,
                            &reference.id,
                        )
                    {
                        next = rehydrated_text;
                        stats.rehydrated_refs += 1;
                        break;
                    } else if next.contains(&reference.marker) {
                        next = next.replace(&reference.marker, &rehydrated_text);
                        stats.rehydrated_refs += 1;
                    } else if next.contains(&legacy_marker) {
                        next = next.replace(&legacy_marker, &rehydrated_text);
                        stats.rehydrated_refs += 1;
                    } else if next.contains(&short_marker) {
                        next = next.replace(&short_marker, &rehydrated_text);
                        stats.rehydrated_refs += 1;
                    } else if next.trim() == reference.id {
                        next = rehydrated_text;
                        stats.rehydrated_refs += 1;
                    }
                }
            }
            *text = next;
        }
        serde_json::Value::Array(items) => {
            for item in items {
                runtime_smart_context_rehydrate_value_for_ids(
                    item,
                    store,
                    rehydrate_ids,
                    aliases,
                    stats,
                );
            }
        }
        serde_json::Value::Object(object) => {
            for (key, item) in object {
                if runtime_smart_context_static_prompt_field_key(key) {
                    continue;
                }
                runtime_smart_context_rehydrate_value_for_ids(
                    item,
                    store,
                    rehydrate_ids,
                    aliases,
                    stats,
                );
            }
        }
        _ => {}
    }
}

pub(super) fn runtime_smart_context_text_is_artifact_marker_summary(text: &str, id: &str) -> bool {
    let Some(first_line) = text.trim_start().lines().next() else {
        return false;
    };
    let legacy = (first_line.starts_with("prodex-sc artifact ")
        || first_line.starts_with("prodex-sc repeat "))
        && first_line.contains(&format!("prodex-artifact:{id}"));
    let short = (first_line.starts_with("psc art ")
        || first_line.starts_with("psc rep ")
        || first_line.starts_with("psc repeat ")
        || first_line.starts_with("psc co ")
        || first_line.starts_with("psc cmdout ")
        || first_line.starts_with("prodex-sc artifact ")
        || first_line.starts_with("prodex-sc repeat "))
        && first_line.contains(&runtime_smart_context_artifact_ref(id));
    legacy || short
}

pub(super) fn runtime_smart_context_rehydrated_artifact_text(
    artifact_text: &str,
    line_range: Option<RuntimeSmartContextLineRange>,
) -> Option<String> {
    let Some(line_range) = line_range else {
        return Some(artifact_text.to_string());
    };
    let lines = artifact_text.lines().collect::<Vec<_>>();
    if line_range.start > lines.len() {
        return None;
    }
    let end = line_range.end.min(lines.len());
    Some(lines[line_range.start - 1..end].join("\n"))
}

pub(super) fn runtime_smart_context_rehydrated_artifact_reference_text(
    artifact_text: &str,
    reference: &RuntimeSmartContextArtifactReference,
) -> Option<String> {
    if reference.line_ranges.is_empty() {
        return runtime_smart_context_rehydrated_artifact_text(artifact_text, reference.line_range);
    }
    let mut parts = Vec::new();
    for range in &reference.line_ranges {
        parts.push(runtime_smart_context_rehydrated_artifact_text(
            artifact_text,
            Some(*range),
        )?);
    }
    Some(parts.join("\n"))
}

#[cfg(test)]
pub(super) fn runtime_smart_context_selective_rehydrate_semantic_ranges(
    value: &mut serde_json::Value,
    store: &RuntimeSmartContextArtifactStore,
    exactness_guard: &runtime_proxy_crate::SmartContextExactnessGuard,
    terms: &RuntimeSmartContextSelectiveRehydrateTerms,
    stats: &mut RuntimeSmartContextTransformStats,
) -> usize {
    runtime_smart_context_selective_rehydrate_semantic_ranges_with_budget(
        value,
        store,
        exactness_guard,
        terms,
        usize::MAX,
        stats,
    )
}

pub(super) fn runtime_smart_context_selective_rehydrate_budget_aware_ranges(
    value: &mut serde_json::Value,
    store: &RuntimeSmartContextArtifactStore,
    exactness_guard: &runtime_proxy_crate::SmartContextExactnessGuard,
    terms: &RuntimeSmartContextSelectiveRehydrateTerms,
    plan: &runtime_proxy_crate::SmartContextRehydratePlan,
    token_budget: usize,
    stats: &mut RuntimeSmartContextTransformStats,
) -> usize {
    if exactness_guard.decision != runtime_proxy_crate::SmartContextExactnessDecision::Allow {
        return 0;
    }

    let mut remaining_tokens = token_budget;
    let strong_terms = runtime_smart_context_selective_rehydrate_terms_strong(terms);
    let mut count = if strong_terms {
        runtime_smart_context_selective_rehydrate_semantic_ranges_inner(
            value,
            store,
            terms,
            &mut remaining_tokens,
            stats,
        )
    } else {
        0
    };

    let deferred_ids = runtime_smart_context_token_budget_deferred_rehydrate_ids(plan);
    if strong_terms && !deferred_ids.is_empty() && remaining_tokens > 0 {
        count = count.saturating_add(
            runtime_smart_context_selective_rehydrate_deferred_read_plan(
                value,
                store,
                terms,
                &deferred_ids,
                &mut remaining_tokens,
                stats,
            ),
        );
    }
    count
}

#[cfg(test)]
pub(super) fn runtime_smart_context_selective_rehydrate_semantic_ranges_with_budget(
    value: &mut serde_json::Value,
    store: &RuntimeSmartContextArtifactStore,
    exactness_guard: &runtime_proxy_crate::SmartContextExactnessGuard,
    terms: &RuntimeSmartContextSelectiveRehydrateTerms,
    token_budget: usize,
    stats: &mut RuntimeSmartContextTransformStats,
) -> usize {
    if exactness_guard.decision != runtime_proxy_crate::SmartContextExactnessDecision::Allow
        || runtime_smart_context_selective_rehydrate_terms_empty(terms)
    {
        return 0;
    }
    let mut remaining_tokens = token_budget;
    runtime_smart_context_selective_rehydrate_semantic_ranges_inner(
        value,
        store,
        terms,
        &mut remaining_tokens,
        stats,
    )
}

pub(super) fn runtime_smart_context_selective_rehydrate_semantic_ranges_inner(
    value: &mut serde_json::Value,
    store: &RuntimeSmartContextArtifactStore,
    terms: &RuntimeSmartContextSelectiveRehydrateTerms,
    remaining_tokens: &mut usize,
    stats: &mut RuntimeSmartContextTransformStats,
) -> usize {
    if runtime_smart_context_value_is_static_context_item(value) {
        return 0;
    }
    match value {
        serde_json::Value::String(text) => {
            runtime_smart_context_selective_rehydrate_semantic_ranges_in_text(
                text,
                store,
                terms,
                remaining_tokens,
                stats,
            )
        }
        serde_json::Value::Array(items) => items
            .iter_mut()
            .map(|item| {
                runtime_smart_context_selective_rehydrate_semantic_ranges_inner(
                    item,
                    store,
                    terms,
                    remaining_tokens,
                    stats,
                )
            })
            .sum(),
        serde_json::Value::Object(object) => {
            let mut count = 0usize;
            for (key, item) in object {
                if runtime_smart_context_static_prompt_field_key(key) {
                    continue;
                }
                count = count.saturating_add(
                    runtime_smart_context_selective_rehydrate_semantic_ranges_inner(
                        item,
                        store,
                        terms,
                        remaining_tokens,
                        stats,
                    ),
                );
            }
            count
        }
        _ => 0,
    }
}

pub(super) fn runtime_smart_context_selective_rehydrate_semantic_ranges_in_text(
    text: &mut String,
    store: &RuntimeSmartContextArtifactStore,
    terms: &RuntimeSmartContextSelectiveRehydrateTerms,
    remaining_tokens: &mut usize,
    stats: &mut RuntimeSmartContextTransformStats,
) -> usize {
    if *remaining_tokens == 0 {
        return 0;
    }
    let ids = runtime_smart_context_collect_artifact_refs(&serde_json::Value::String(text.clone()))
        .into_iter()
        .map(|reference| reference.id)
        .collect::<BTreeSet<_>>();
    if ids.is_empty() {
        return 0;
    }

    let mut next = text.clone();
    let mut rehydrated_ranges = 0usize;
    for id in ids {
        let Some(line_index) = store.line_index(&id) else {
            continue;
        };
        let Some((appendix, range_count, token_cost)) =
            runtime_smart_context_matching_semantic_range_appendix_with_budget(
                &id,
                line_index,
                &next,
                terms,
                *remaining_tokens,
            )
        else {
            continue;
        };
        if !next.ends_with('\n') {
            next.push('\n');
        }
        next.push('\n');
        next.push_str(&appendix);
        rehydrated_ranges = rehydrated_ranges.saturating_add(range_count);
        runtime_smart_context_consume_rehydrate_budget(remaining_tokens, token_cost);
        if *remaining_tokens == 0 {
            break;
        }
    }

    if rehydrated_ranges > 0 {
        *text = next;
        stats.rehydrated_refs = stats.rehydrated_refs.saturating_add(rehydrated_ranges);
    }
    rehydrated_ranges
}

pub(super) fn runtime_smart_context_token_budget_deferred_rehydrate_ids(
    plan: &runtime_proxy_crate::SmartContextRehydratePlan,
) -> BTreeSet<String> {
    plan.actions
        .iter()
        .filter_map(|action| match action {
            runtime_proxy_crate::SmartContextRehydrateAction::Defer {
                id,
                reason:
                    runtime_proxy_crate::SmartContextRehydrateDeferReason::TokenBudgetExceeded
                    | runtime_proxy_crate::SmartContextRehydrateDeferReason::MinimalBudgetTier,
            } => Some(id.clone()),
            _ => None,
        })
        .collect()
}

pub(super) fn runtime_smart_context_selective_rehydrate_deferred_read_plan(
    value: &mut serde_json::Value,
    store: &RuntimeSmartContextArtifactStore,
    terms: &RuntimeSmartContextSelectiveRehydrateTerms,
    deferred_ids: &BTreeSet<String>,
    remaining_tokens: &mut usize,
    stats: &mut RuntimeSmartContextTransformStats,
) -> usize {
    if *remaining_tokens == 0 || deferred_ids.is_empty() {
        return 0;
    }
    if runtime_smart_context_value_is_static_context_item(value) {
        return 0;
    }
    match value {
        serde_json::Value::String(text) => {
            runtime_smart_context_selective_rehydrate_deferred_read_plan_in_text(
                text,
                store,
                terms,
                deferred_ids,
                remaining_tokens,
                stats,
            )
        }
        serde_json::Value::Array(items) => items
            .iter_mut()
            .map(|item| {
                runtime_smart_context_selective_rehydrate_deferred_read_plan(
                    item,
                    store,
                    terms,
                    deferred_ids,
                    remaining_tokens,
                    stats,
                )
            })
            .sum(),
        serde_json::Value::Object(object) => {
            let mut count = 0usize;
            for (key, item) in object {
                if runtime_smart_context_static_prompt_field_key(key) {
                    continue;
                }
                count = count.saturating_add(
                    runtime_smart_context_selective_rehydrate_deferred_read_plan(
                        item,
                        store,
                        terms,
                        deferred_ids,
                        remaining_tokens,
                        stats,
                    ),
                );
            }
            count
        }
        _ => 0,
    }
}

pub(super) fn runtime_smart_context_selective_rehydrate_deferred_read_plan_in_text(
    text: &mut String,
    store: &RuntimeSmartContextArtifactStore,
    terms: &RuntimeSmartContextSelectiveRehydrateTerms,
    deferred_ids: &BTreeSet<String>,
    remaining_tokens: &mut usize,
    stats: &mut RuntimeSmartContextTransformStats,
) -> usize {
    if *remaining_tokens == 0 {
        return 0;
    }
    let ids = runtime_smart_context_collect_artifact_refs(&serde_json::Value::String(text.clone()))
        .into_iter()
        .filter(|reference| reference.line_ranges.is_empty() && reference.line_range.is_none())
        .map(|reference| reference.id)
        .filter(|id| deferred_ids.contains(id))
        .collect::<BTreeSet<_>>();
    if ids.is_empty() {
        return 0;
    }

    let mut next = text.clone();
    let mut rehydrated_ranges = 0usize;
    for id in ids {
        let Some((appendix, range_count, token_cost)) =
            runtime_smart_context_deferred_read_plan_appendix(
                &id,
                store,
                terms,
                &next,
                *remaining_tokens,
            )
        else {
            continue;
        };
        if !next.ends_with('\n') {
            next.push('\n');
        }
        next.push('\n');
        next.push_str(&appendix);
        rehydrated_ranges = rehydrated_ranges.saturating_add(range_count);
        runtime_smart_context_consume_rehydrate_budget(remaining_tokens, token_cost);
        if *remaining_tokens == 0 {
            break;
        }
    }

    if rehydrated_ranges > 0 {
        *text = next;
        stats.rehydrated_refs = stats.rehydrated_refs.saturating_add(rehydrated_ranges);
    }
    rehydrated_ranges
}

pub(super) fn runtime_smart_context_deferred_read_plan_appendix(
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

pub(super) fn runtime_smart_context_push_scored_exact_range(
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

pub(super) fn runtime_smart_context_import_read_plan_ranges(
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

pub(super) fn runtime_smart_context_push_import_read_plan_range(
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

pub(super) fn runtime_smart_context_line_is_import(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("use ")
        || trimmed.starts_with("pub use ")
        || trimmed.starts_with("extern crate ")
        || trimmed.starts_with("import ")
        || trimmed.starts_with("from ") && trimmed.contains(" import ")
}

pub(super) fn runtime_smart_context_matching_semantic_range_appendix_with_budget(
    artifact_id: &str,
    line_index: &RuntimeSmartContextArtifactLineIndex,
    current_text: &str,
    terms: &RuntimeSmartContextSelectiveRehydrateTerms,
    token_budget: usize,
) -> Option<(String, usize, usize)> {
    let ranges = runtime_smart_context_matching_semantic_ranges(
        artifact_id,
        line_index,
        current_text,
        terms,
    );
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
        runtime_smart_context_semantic_rehydrate_range_cap(terms),
        token_budget,
        |range| {
            scored_ranges
                .iter()
                .find(|candidate| candidate.range.eq(range))
                .map_or(0, |candidate| candidate.score)
        },
    )
}

pub(super) fn runtime_smart_context_matching_semantic_ranges<'a>(
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
            std::cmp::Reverse(runtime_smart_context_semantic_range_score_with_command(
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

pub(super) fn runtime_smart_context_semantic_range_score_with_command(
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

pub(super) fn runtime_smart_context_symbol_matches_terms(
    symbol: &str,
    terms: &RuntimeSmartContextSelectiveRehydrateTerms,
) -> bool {
    terms
        .test_symbols
        .iter()
        .any(|term| runtime_smart_context_symbol_matches_term(symbol, term))
}

pub(super) fn runtime_smart_context_symbol_matches_term(symbol: &str, term: &str) -> bool {
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

pub(super) fn runtime_smart_context_path_matches_terms(
    path: &str,
    terms: &RuntimeSmartContextSelectiveRehydrateTerms,
) -> bool {
    terms
        .file_paths
        .iter()
        .any(|term| runtime_smart_context_path_matches_term(path, term))
}

pub(super) fn runtime_smart_context_path_matches_term(path: &str, term: &str) -> bool {
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

pub(super) fn runtime_smart_context_normalized_intent_path(value: &str) -> Option<String> {
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

pub(super) fn runtime_smart_context_error_code_matches_terms(
    code: &str,
    terms: &RuntimeSmartContextSelectiveRehydrateTerms,
) -> bool {
    terms
        .error_codes
        .iter()
        .any(|term| code.eq_ignore_ascii_case(term))
}

pub(super) fn runtime_smart_context_semantic_rehydrate_range_cap(
    terms: &RuntimeSmartContextSelectiveRehydrateTerms,
) -> usize {
    if runtime_smart_context_selective_rehydrate_terms_narrow(terms) {
        SMART_CONTEXT_SEMANTIC_REHYDRATE_NARROW_MAX_RANGES
    } else {
        SMART_CONTEXT_SEMANTIC_REHYDRATE_GLOBAL_MAX_RANGES
    }
}

pub(super) fn runtime_smart_context_selective_rehydrate_terms_narrow(
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

pub(super) fn runtime_smart_context_selective_rehydrate_terms_empty(
    terms: &RuntimeSmartContextSelectiveRehydrateTerms,
) -> bool {
    terms.file_paths.is_empty()
        && terms.error_codes.is_empty()
        && terms.test_symbols.is_empty()
        && terms.command_kinds.is_empty()
        && terms.diff_hunks.is_empty()
}

pub(super) fn runtime_smart_context_selective_rehydrate_terms_strong(
    terms: &RuntimeSmartContextSelectiveRehydrateTerms,
) -> bool {
    !terms.file_paths.is_empty()
        || !terms.error_codes.is_empty()
        || !terms.test_symbols.is_empty()
        || !terms.diff_hunks.is_empty()
}

pub(super) fn runtime_smart_context_semantic_range_matches_terms(
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

pub(super) fn runtime_smart_context_diff_hunk_range_matches_term(
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

pub(super) fn runtime_smart_context_artifact_semantic_range_valid(
    range: &RuntimeSmartContextArtifactSemanticLineRange,
) -> bool {
    range.start > 0
        && range.end >= range.start
        && range.byte_len == range.text.len()
        && range.content_hash == runtime_proxy_crate::smart_context_hash_text(&range.text)
}
