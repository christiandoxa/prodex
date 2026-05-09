use super::*;

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_exact_header(
    request: &RuntimeProxyRequest,
) -> bool {
    runtime_proxy_request_header_value(&request.headers, "x-prodex-smart-context")
        .is_some_and(|value| value.eq_ignore_ascii_case("exact"))
}

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_missing_artifact_refs(
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

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_rehydrate_ref_token_cost(
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

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_deferred_rehydrate_refs(
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
pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_rehydrate_value(
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

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_rehydrate_value_with_plan(
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

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_text_is_artifact_marker_summary(
    text: &str,
    id: &str,
) -> bool {
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

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_rehydrated_artifact_text(
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

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_rehydrated_artifact_reference_text(
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
