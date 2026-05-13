use super::super::{
    RuntimeSmartContextArtifactStore, RuntimeSmartContextSelectiveRehydrateTerms,
    RuntimeSmartContextTransformStats, runtime_smart_context_collect_artifact_refs,
    runtime_smart_context_consume_rehydrate_budget, runtime_smart_context_static_prompt_field_key,
    runtime_smart_context_value_is_static_context_item,
};
#[cfg(test)]
use super::runtime_smart_context_selective_rehydrate_terms_empty;
use super::{
    runtime_smart_context_deferred_read_plan_appendix,
    runtime_smart_context_matching_semantic_range_appendix_with_budget,
    runtime_smart_context_selective_rehydrate_terms_strong,
};
use std::collections::BTreeSet;

#[cfg(test)]
pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_selective_rehydrate_semantic_ranges(
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

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_selective_rehydrate_budget_aware_ranges(
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
pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_selective_rehydrate_semantic_ranges_with_budget(
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

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_selective_rehydrate_semantic_ranges_inner(
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

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_selective_rehydrate_semantic_ranges_in_text(
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

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_token_budget_deferred_rehydrate_ids(
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

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_selective_rehydrate_deferred_read_plan(
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

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_selective_rehydrate_deferred_read_plan_in_text(
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
