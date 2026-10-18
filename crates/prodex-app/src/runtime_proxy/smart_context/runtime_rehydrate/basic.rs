use super::super::{
    RuntimeProxyRequest, RuntimeSmartContextArtifactReference, RuntimeSmartContextArtifactStore,
    RuntimeSmartContextLineRange, RuntimeSmartContextTransformStats,
    runtime_smart_context_artifact_ref_occurrences_from_text,
    runtime_smart_context_collect_artifact_aliases,
    runtime_smart_context_collect_rehydratable_artifact_refs,
    runtime_smart_context_static_prompt_field_key,
    runtime_smart_context_value_is_static_context_item,
};
use prodex_mojo_core::rich::ascii_casefold_equal_exact;
use runtime_proxy_crate::runtime_proxy_request_header_value;
use std::collections::{BTreeMap, BTreeSet};

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_exact_header(
    request: &RuntimeProxyRequest,
) -> bool {
    runtime_proxy_request_header_value(&request.headers, "x-prodex-smart-context").is_some_and(
        |value| {
            ascii_casefold_equal_exact(value, "exact")
                .expect("Mojo smart-context mode comparison failed")
        },
    )
}

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_missing_artifact_refs_in_store(
    ref_ids: impl IntoIterator<Item = String>,
    store: &RuntimeSmartContextArtifactStore,
) -> Vec<String> {
    ref_ids
        .into_iter()
        .filter(|id| !store.contains(id))
        .collect()
}

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_auto_rehydrate_plan(
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
        let token_cost = runtime_smart_context_rehydrate_ref_token_cost(&reference, store)
            .unwrap_or(0)
            .max(1);
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

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_rehydrate_ref_token_cost(
    reference: &RuntimeSmartContextArtifactReference,
    store: &RuntimeSmartContextArtifactStore,
) -> Option<usize> {
    let artifact_text = store.get_text(&reference.id)?;
    let (ranges, legacy) = artifact_line_ranges(reference);
    let bytes = prodex_mojo_core::smart_context_artifact_ref::artifact_line_byte_len(
        &artifact_text,
        &ranges,
        legacy,
    )
    .expect("compiled artifact line measurement returned invalid output")?;
    runtime_proxy_crate::smart_context_estimate_tokens_from_body_bytes(bytes)
        .try_into()
        .ok()
}

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_rehydrate_value_with_budget(
    value: &mut serde_json::Value,
    store: &RuntimeSmartContextArtifactStore,
    available_tokens: usize,
    tier: runtime_proxy_crate::SmartContextTokenBudgetTier,
    stats: &mut RuntimeSmartContextTransformStats,
) {
    let plan = runtime_smart_context_auto_rehydrate_plan(value, store, available_tokens, tier);
    runtime_smart_context_rehydrate_value_with_plan(value, store, &plan, stats);
}

#[cfg(test)]
pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_rehydrate_value(
    value: &mut serde_json::Value,
    store: &RuntimeSmartContextArtifactStore,
    stats: &mut RuntimeSmartContextTransformStats,
) {
    let available_tokens = 1_000_000;
    runtime_smart_context_rehydrate_value_with_budget(
        value,
        store,
        available_tokens,
        runtime_proxy_crate::smart_context_token_budget_tier(available_tokens),
        stats,
    );
}

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_rehydrate_value_with_plan(
    value: &mut serde_json::Value,
    store: &RuntimeSmartContextArtifactStore,
    plan: &runtime_proxy_crate::SmartContextRehydratePlan,
    stats: &mut RuntimeSmartContextTransformStats,
) {
    let planned = plan
        .actions
        .iter()
        .filter(|action| {
            matches!(
                action,
                runtime_proxy_crate::SmartContextRehydrateAction::Rehydrate { .. }
            )
        })
        .count();
    let deferred = plan.actions.len().saturating_sub(planned);
    stats.rehydration_planned = stats.rehydration_planned.saturating_add(planned);
    stats.rehydration_deferred = stats.rehydration_deferred.saturating_add(deferred);
    stats.rehydration_token_cost = stats
        .rehydration_token_cost
        .saturating_add(plan.used_tokens);
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

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_rehydrate_value_for_ids(
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
            runtime_smart_context_rehydrate_text(text, store, rehydrate_ids, aliases, stats);
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

fn runtime_smart_context_rehydrate_text(
    text: &mut String,
    store: &RuntimeSmartContextArtifactStore,
    rehydrate_ids: &BTreeSet<String>,
    aliases: &BTreeMap<String, String>,
    stats: &mut RuntimeSmartContextTransformStats,
) {
    let mut next = text.clone();
    for reference in runtime_smart_context_artifact_ref_occurrences_from_text(text, aliases) {
        if !rehydrate_ids.contains(&reference.id) {
            continue;
        }
        let Some(artifact_text) = store.get_text(&reference.id) else {
            continue;
        };
        let Some(rehydrated_text) =
            runtime_smart_context_rehydrated_artifact_reference_text(&artifact_text, &reference)
        else {
            continue;
        };
        use prodex_mojo_core::smart_context_artifact_ref::{
            ArtifactReplacement, ArtifactReplacementInput, replace_artifact_reference,
        };
        let Some(replacement) = replace_artifact_reference(ArtifactReplacementInput {
            text: &next,
            id: &reference.id,
            marker: &reference.marker,
            replacement: &rehydrated_text,
            has_line_range: reference.line_range.is_some(),
        })
        .expect("compiled artifact replacement returned invalid output") else {
            continue;
        };
        let (replacement, stop) = match replacement {
            ArtifactReplacement::Continue(text) => (text, false),
            ArtifactReplacement::Complete(text) => (text, true),
        };
        next = replacement;
        stats.rehydrated_refs = stats.rehydrated_refs.saturating_add(1);
        stats.rehydration_performed = stats.rehydration_performed.saturating_add(1);
        if stop {
            break;
        }
    }
    *text = next;
}

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_rehydrated_artifact_reference_text<
    'a,
>(
    artifact_text: &'a str,
    reference: &RuntimeSmartContextArtifactReference,
) -> Option<std::borrow::Cow<'a, str>> {
    let (ranges, legacy) = artifact_line_ranges(reference);
    prodex_mojo_core::smart_context_artifact_ref::rehydrate_artifact_lines(
        artifact_text,
        &ranges,
        legacy,
    )
    .expect("compiled artifact line selection returned invalid output")
}

fn artifact_line_ranges(
    reference: &RuntimeSmartContextArtifactReference,
) -> (
    Vec<prodex_mojo_core::smart_context_artifact_ref::ArtifactLineRange>,
    Option<prodex_mojo_core::smart_context_artifact_ref::ArtifactLineRange>,
) {
    let range = |range: RuntimeSmartContextLineRange| {
        prodex_mojo_core::smart_context_artifact_ref::ArtifactLineRange {
            start: range.start,
            end: range.end,
        }
    };
    let ranges = reference
        .line_ranges
        .iter()
        .copied()
        .map(range)
        .collect::<Vec<_>>();
    (ranges, reference.line_range.map(range))
}

#[cfg(test)]
#[path = "text_tests.rs"]
mod text_tests;
