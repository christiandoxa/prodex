use super::*;
use std::borrow::Cow;

pub(super) fn runtime_smart_context_dedupe_input_text(
    value: &mut serde_json::Value,
    store: &RuntimeSmartContextArtifactStore,
    exactness_guard: &runtime_proxy_crate::SmartContextExactnessGuard,
    stats: &mut RuntimeSmartContextTransformStats,
) {
    let Some(input) = value
        .get_mut("input")
        .and_then(serde_json::Value::as_array_mut)
    else {
        return;
    };
    let mut seen = BTreeMap::<String, usize>::new();
    for (index, item) in input.iter_mut().enumerate() {
        if runtime_smart_context_value_is_static_context_item(item) {
            continue;
        }
        runtime_smart_context_dedupe_value_text(item, index, &mut seen, stats);
    }
    runtime_smart_context_replace_cross_turn_duplicate_refs(value, store, exactness_guard, stats);
}

pub(super) fn runtime_smart_context_dedupe_value_text(
    value: &mut serde_json::Value,
    item_index: usize,
    seen: &mut BTreeMap<String, usize>,
    stats: &mut RuntimeSmartContextTransformStats,
) {
    match value {
        serde_json::Value::String(text) => {
            if text.len() < SMART_CONTEXT_DUPLICATE_TEXT_MIN_BYTES {
                return;
            }
            let hash = runtime_proxy_crate::smart_context_hash_text(text);
            if let Some(first_index) = seen.get(&hash) {
                *text = format!("[psc dup input[{first_index}]]");
                stats.duplicate_texts += 1;
            } else {
                seen.insert(hash.clone(), item_index);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                runtime_smart_context_dedupe_value_text(item, item_index, seen, stats);
            }
        }
        serde_json::Value::Object(object) => {
            for item in object.values_mut() {
                runtime_smart_context_dedupe_value_text(item, item_index, seen, stats);
            }
        }
        _ => {}
    }
}

pub(super) fn runtime_smart_context_replace_cross_turn_duplicate_refs(
    value: &mut serde_json::Value,
    store: &RuntimeSmartContextArtifactStore,
    exactness_guard: &runtime_proxy_crate::SmartContextExactnessGuard,
    stats: &mut RuntimeSmartContextTransformStats,
) {
    let items = runtime_smart_context_collect_large_rehydratable_text_items(value);
    if items.is_empty() {
        return;
    }
    let artifacts = runtime_smart_context_available_artifacts_for_text_items(items.iter(), store);
    let plan = runtime_proxy_crate::smart_context_cross_turn_duplicate_ref_plan(
        items,
        artifacts,
        SMART_CONTEXT_DUPLICATE_TEXT_MIN_BYTES,
        exactness_guard,
    );
    let replacements = plan
        .actions
        .into_iter()
        .filter_map(|action| {
            let runtime_proxy_crate::SmartContextCrossTurnDuplicateRefAction::ReplaceWithArtifactRef {
                id,
                artifact,
                content_hash: _,
                byte_len,
            } = action
            else {
                return None;
            };
            Some((
                id,
                format!(
                    "[psc rep {} b={}]",
                    runtime_smart_context_artifact_ref(&artifact.id),
                    byte_len
                ),
            ))
        })
        .collect::<BTreeMap<_, _>>();
    if replacements.is_empty() {
        return;
    }
    stats.cross_turn_duplicate_texts +=
        runtime_smart_context_apply_text_replacements(value, &replacements);
}

pub(super) fn runtime_smart_context_available_artifacts_for_text_items<'a>(
    items: impl IntoIterator<Item = &'a runtime_proxy_crate::SmartContextConversationItem>,
    store: &RuntimeSmartContextArtifactStore,
) -> Vec<runtime_proxy_crate::SmartContextArtifactRef> {
    items
        .into_iter()
        .filter_map(|item| {
            let content_hash = runtime_proxy_crate::smart_context_hash_text(&item.text);
            let artifact_text = store.get_text(&content_hash)?;
            (artifact_text == item.text).then_some(runtime_proxy_crate::SmartContextArtifactRef {
                id: content_hash.clone(),
                byte_len: item.text.len(),
                content_hash,
            })
        })
        .collect()
}

pub(super) fn runtime_smart_context_collect_large_rehydratable_text_items(
    value: &serde_json::Value,
) -> Vec<runtime_proxy_crate::SmartContextConversationItem> {
    let mut items = Vec::new();
    let Some(object) = value.as_object() else {
        return items;
    };
    let mut keys = object.keys().cloned().collect::<Vec<_>>();
    keys.sort();
    for key in keys {
        if runtime_smart_context_static_prompt_field_key(&key) {
            continue;
        }
        if let Some(item) = object.get(&key) {
            runtime_smart_context_collect_large_text_items_from_value(key, item, &mut items);
        }
    }
    items
}

pub(super) fn runtime_smart_context_collect_large_text_items_from_value(
    id: String,
    value: &serde_json::Value,
    items: &mut Vec<runtime_proxy_crate::SmartContextConversationItem>,
) {
    if runtime_smart_context_value_is_static_context_item(value) {
        return;
    }
    match value {
        serde_json::Value::String(text)
            if text.len() >= SMART_CONTEXT_DUPLICATE_TEXT_MIN_BYTES
                && !text.contains("prodex-artifact:")
                && !text.contains(SMART_CONTEXT_SHORT_ARTIFACT_REF_PREFIX)
                && !text.contains("prodex smart context artifact")
                && !text.contains("prodex-sc ") =>
        {
            items.push(runtime_proxy_crate::SmartContextConversationItem {
                id,
                text: text.clone(),
            });
        }
        serde_json::Value::Array(values) => {
            for (index, item) in values.iter().enumerate() {
                runtime_smart_context_collect_large_text_items_from_value(
                    format!("{id}[{index}]"),
                    item,
                    items,
                );
            }
        }
        serde_json::Value::Object(object) => {
            let mut keys = object.keys().cloned().collect::<Vec<_>>();
            keys.sort();
            for key in keys {
                if let Some(item) = object.get(&key) {
                    if runtime_smart_context_static_prompt_field_key(&key) {
                        continue;
                    }
                    runtime_smart_context_collect_large_text_items_from_value(
                        format!("{id}.{key}"),
                        item,
                        items,
                    );
                }
            }
        }
        _ => {}
    }
}

pub(super) fn runtime_smart_context_apply_text_replacements(
    value: &mut serde_json::Value,
    replacements: &BTreeMap<String, String>,
) -> usize {
    let Some(object) = value.as_object_mut() else {
        return 0;
    };
    let mut replaced = 0usize;
    let mut keys = object.keys().cloned().collect::<Vec<_>>();
    keys.sort();
    for key in keys {
        if runtime_smart_context_static_prompt_field_key(&key) {
            continue;
        }
        if let Some(item) = object.get_mut(&key) {
            replaced +=
                runtime_smart_context_apply_text_replacements_to_value(item, key, replacements);
        }
    }
    replaced
}

pub(super) fn runtime_smart_context_apply_text_replacements_to_value(
    value: &mut serde_json::Value,
    id: String,
    replacements: &BTreeMap<String, String>,
) -> usize {
    if runtime_smart_context_value_is_static_context_item(value) {
        return 0;
    }
    match value {
        serde_json::Value::String(text) => {
            if let Some(replacement) = replacements.get(&id) {
                *text = replacement.clone();
                1
            } else {
                0
            }
        }
        serde_json::Value::Array(values) => values
            .iter_mut()
            .enumerate()
            .map(|(index, item)| {
                runtime_smart_context_apply_text_replacements_to_value(
                    item,
                    format!("{id}[{index}]"),
                    replacements,
                )
            })
            .sum(),
        serde_json::Value::Object(object) => {
            let mut keys = object.keys().cloned().collect::<Vec<_>>();
            keys.sort();
            keys.into_iter()
                .filter_map(|key| {
                    if runtime_smart_context_static_prompt_field_key(&key) {
                        return None;
                    }
                    object.get_mut(&key).map(|item| {
                        runtime_smart_context_apply_text_replacements_to_value(
                            item,
                            format!("{id}.{key}"),
                            replacements,
                        )
                    })
                })
                .sum()
        }
        _ => 0,
    }
}

pub(super) fn runtime_smart_context_likely_blob_or_noise(text: &str) -> bool {
    prodex_context::is_context_blob_noise(text)
}

pub(super) fn runtime_smart_context_critical_signal_self_check(
    before: &[u8],
    after: &[u8],
) -> prodex_context::CriticalSignalSelfCheck {
    let before = String::from_utf8_lossy(before);
    let after = String::from_utf8_lossy(after);
    prodex_context::critical_signal_self_check(&before, &after)
}

pub(super) fn runtime_smart_context_regression_self_check(
    before: &[u8],
    after: &[u8],
    exactness_guard: runtime_proxy_crate::SmartContextExactnessGuard,
    missing_rehydrate_refs: Vec<String>,
) -> runtime_proxy_crate::SmartContextRegressionSelfCheck {
    let before_text = String::from_utf8_lossy(before);
    let after_text = String::from_utf8_lossy(after);
    runtime_proxy_crate::smart_context_regression_self_check(
        runtime_proxy_crate::SmartContextRegressionSelfCheckInput {
            exactness_guard,
            before_hash: runtime_proxy_crate::smart_context_hash_text(&before_text),
            after_hash: runtime_proxy_crate::smart_context_hash_text(&after_text),
            before_estimated_tokens:
                runtime_proxy_crate::smart_context_estimate_tokens_from_body_bytes(before.len()),
            after_estimated_tokens:
                runtime_proxy_crate::smart_context_estimate_tokens_from_body_bytes(after.len()),
            before_critical_signal_count: prodex_context::count_critical_signals(&before_text)
                .total(),
            after_critical_signal_count: prodex_context::count_critical_signals(&after_text)
                .total(),
            missing_rehydrate_refs,
            unresolved_rehydrate_refs_are_segment_local: true,
        },
    )
}

pub(super) fn runtime_smart_context_fallback_exact_reason(
    regression_check: &runtime_proxy_crate::SmartContextRegressionSelfCheck,
    critical_signal_check: prodex_context::CriticalSignalSelfCheck,
    stats: &RuntimeSmartContextTransformStats,
) -> Option<&'static str> {
    if critical_signal_check.has_loss() {
        return Some("critical_signal_loss");
    }
    if runtime_smart_context_rewrite_is_rehydrate_only(stats) {
        return None;
    }
    if regression_check.decision
        == runtime_proxy_crate::SmartContextRegressionSelfCheckDecision::FallbackExact
    {
        return Some(runtime_smart_context_regression_reason_label(
            &regression_check.reasons,
        ));
    }
    None
}

pub(super) fn runtime_smart_context_rewrite_is_rehydrate_only(
    stats: &RuntimeSmartContextTransformStats,
) -> bool {
    stats.rehydrated_refs > 0
        && stats.tool_outputs_condensed == 0
        && stats.duplicate_texts == 0
        && stats.cross_turn_duplicate_texts == 0
        && stats.repeat_tool_output_refs == 0
        && stats.static_context_deltas == 0
}

pub(super) fn runtime_smart_context_regression_reason_label(
    reasons: &[runtime_proxy_crate::SmartContextRegressionSelfCheckReason],
) -> &'static str {
    if reasons
        .iter()
        .any(|reason| matches!(reason, runtime_proxy_crate::SmartContextRegressionSelfCheckReason::CriticalSignalDropped))
    {
        "critical_signal_loss"
    } else if reasons.iter().any(|reason| {
        matches!(
            reason,
            runtime_proxy_crate::SmartContextRegressionSelfCheckReason::MissingRehydrateRefs
        )
    }) {
        "missing_rehydrate_refs"
    } else if reasons.iter().any(|reason| {
        matches!(
            reason,
            runtime_proxy_crate::SmartContextRegressionSelfCheckReason::ExactnessRequiredButPayloadChanged
        )
    }) {
        "exactness_required"
    } else if reasons.iter().any(|reason| {
        matches!(
            reason,
            runtime_proxy_crate::SmartContextRegressionSelfCheckReason::EmptyAfterPayload
        )
    }) {
        "empty_after_payload"
    } else {
        "token_budget_did_not_improve"
    }
}

pub(super) fn runtime_smart_context_minified_json_body(
    value: &serde_json::Value,
    original_body: &[u8],
) -> Option<Vec<u8>> {
    let Cow::Owned(body) =
        runtime_proxy_crate::smart_context_structural_minify_json_value_body(original_body, value)
    else {
        return None;
    };
    runtime_smart_context_validated_minified_json_body(body, original_body)
}

pub(super) fn runtime_smart_context_minified_json_body_from_original(
    original_body: &[u8],
) -> Option<Vec<u8>> {
    let Cow::Owned(body) =
        runtime_proxy_crate::smart_context_structural_minify_json_body(original_body)
    else {
        return None;
    };
    runtime_smart_context_validated_minified_json_body(body, original_body)
}

pub(super) fn runtime_smart_context_validated_minified_json_body(
    body: Vec<u8>,
    original_body: &[u8],
) -> Option<Vec<u8>> {
    if body.len() >= original_body.len() {
        return None;
    }
    let before = String::from_utf8_lossy(original_body);
    let after = String::from_utf8_lossy(&body);
    prodex_context::critical_signal_self_check(&before, &after)
        .passed()
        .then_some(body)
}

#[cfg(test)]
pub(super) fn runtime_smart_context_should_pass_through_after_self_check(
    body_bytes_before: usize,
    body_bytes_after: usize,
    stats: &RuntimeSmartContextTransformStats,
) -> bool {
    stats.rehydrated_refs == 0
        && stats.static_context_deltas == 0
        && body_bytes_after >= body_bytes_before
}

pub(super) fn runtime_smart_context_tier_label(
    tier: runtime_proxy_crate::SmartContextTokenBudgetTier,
) -> &'static str {
    match tier {
        runtime_proxy_crate::SmartContextTokenBudgetTier::Exact => "exact",
        runtime_proxy_crate::SmartContextTokenBudgetTier::Large => "large",
        runtime_proxy_crate::SmartContextTokenBudgetTier::Condensed => "condensed",
        runtime_proxy_crate::SmartContextTokenBudgetTier::Minimal => "minimal",
    }
}
