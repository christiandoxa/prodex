use super::*;

pub(super) fn runtime_smart_context_dedupe_input_text_within_request(
    value: &mut serde_json::Value,
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
}

pub(super) fn runtime_smart_context_has_duplicate_input_text(value: &serde_json::Value) -> bool {
    let Some(input) = value.get("input").and_then(serde_json::Value::as_array) else {
        return false;
    };
    let mut seen = BTreeMap::<String, Vec<(usize, &str)>>::new();
    let mut candidate_count = 0usize;
    for (index, item) in input.iter().enumerate() {
        if runtime_smart_context_value_is_static_context_item(item) {
            continue;
        }
        if runtime_smart_context_value_has_duplicate_text(
            item,
            index,
            &mut seen,
            &mut candidate_count,
        ) {
            return true;
        }
    }
    false
}

fn runtime_smart_context_value_has_duplicate_text<'a>(
    value: &'a serde_json::Value,
    item_index: usize,
    seen: &mut BTreeMap<String, Vec<(usize, &'a str)>>,
    candidate_count: &mut usize,
) -> bool {
    match value {
        serde_json::Value::String(text) if text.len() >= SMART_CONTEXT_DUPLICATE_TEXT_MIN_BYTES => {
            *candidate_count = candidate_count.saturating_add(1);
            if *candidate_count > 256 {
                return true;
            }
            let hash = runtime_proxy_crate::smart_context_hash_text(text);
            let entries = seen.entry(hash).or_default();
            if entries
                .iter()
                .any(|(first_index, first)| *first_index != item_index && *first == text)
            {
                return true;
            }
            entries.push((item_index, text));
            false
        }
        serde_json::Value::Array(items) => items.iter().any(|item| {
            runtime_smart_context_value_has_duplicate_text(item, item_index, seen, candidate_count)
        }),
        serde_json::Value::Object(object) => object.values().any(|item| {
            runtime_smart_context_value_has_duplicate_text(item, item_index, seen, candidate_count)
        }),
        _ => false,
    }
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
            if let Some(first_index) = seen.get(&hash).filter(|first| **first != item_index) {
                *text = format!(
                    "[prodex-context-ref v=1 source=original-input[{first_index}] digest={hash} bytes={}]",
                    text.len()
                );
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

pub(super) const SMART_CONTEXT_INLINE_REFERENCE_PROTOCOL: &str = "Prodex context reference protocol v1: a `prodex-context-ref` value is byte-for-byte identical to the referenced `original-input[N]` value in this request. Resolve it only from that earlier input item and verify its SHA-256 `sc2:` digest and byte length. No external retrieval is available.";

pub(super) fn runtime_smart_context_append_inline_reference_protocol(
    value: &mut serde_json::Value,
    stats: &RuntimeSmartContextTransformStats,
) -> bool {
    if stats.duplicate_texts == 0 {
        return false;
    }
    let Some(input) = value
        .get_mut("input")
        .and_then(serde_json::Value::as_array_mut)
    else {
        return false;
    };
    input.push(serde_json::json!({
        "type": "message",
        "role": "developer",
        "content": SMART_CONTEXT_INLINE_REFERENCE_PROTOCOL,
    }));
    true
}

pub(super) fn runtime_smart_context_expand_inline_references(
    original: &serde_json::Value,
    candidate: &serde_json::Value,
) -> Option<serde_json::Value> {
    let original_input = original.get("input")?.as_array()?;
    let mut expanded = candidate.clone();
    runtime_smart_context_expand_inline_references_in_value(&mut expanded, original_input)?;
    Some(expanded)
}

pub(super) fn runtime_smart_context_inline_reference_round_trip_is_exact(
    original: &serde_json::Value,
    expanded: &mut serde_json::Value,
) -> bool {
    let Some(input) = expanded
        .get_mut("input")
        .and_then(serde_json::Value::as_array_mut)
    else {
        return false;
    };
    let protocol = serde_json::json!({
        "type": "message",
        "role": "developer",
        "content": SMART_CONTEXT_INLINE_REFERENCE_PROTOCOL,
    });
    if input.last() != Some(&protocol) {
        return false;
    }
    input.pop();
    expanded == original
}

fn runtime_smart_context_expand_inline_references_in_value(
    value: &mut serde_json::Value,
    original_input: &[serde_json::Value],
) -> Option<()> {
    match value {
        serde_json::Value::String(text) => {
            if !text.starts_with("[prodex-context-ref ") {
                return Some(());
            }
            let (index, digest, byte_len) = runtime_smart_context_parse_inline_reference(text)?;
            let source = original_input.get(index)?;
            *text = runtime_smart_context_find_inline_reference_text(source, &digest, byte_len)?;
            Some(())
        }
        serde_json::Value::Array(items) => {
            for item in items {
                runtime_smart_context_expand_inline_references_in_value(item, original_input)?;
            }
            Some(())
        }
        serde_json::Value::Object(object) => {
            for item in object.values_mut() {
                runtime_smart_context_expand_inline_references_in_value(item, original_input)?;
            }
            Some(())
        }
        _ => Some(()),
    }
}

fn runtime_smart_context_parse_inline_reference(text: &str) -> Option<(usize, String, usize)> {
    let body = text
        .strip_prefix("[prodex-context-ref v=1 source=original-input[")?
        .strip_suffix(']')?;
    let (index, body) = body.split_once("] digest=")?;
    let (digest, byte_len) = body.split_once(" bytes=")?;
    let index = index.parse().ok()?;
    let byte_len = byte_len.parse().ok()?;
    runtime_smart_context_artifact_id_valid(digest).then(|| (index, digest.to_string(), byte_len))
}

fn runtime_smart_context_find_inline_reference_text(
    value: &serde_json::Value,
    digest: &str,
    byte_len: usize,
) -> Option<String> {
    match value {
        serde_json::Value::String(text)
            if text.len() == byte_len
                && runtime_proxy_crate::smart_context_hash_matches_text(digest, text) =>
        {
            Some(text.clone())
        }
        serde_json::Value::Array(items) => items.iter().find_map(|item| {
            runtime_smart_context_find_inline_reference_text(item, digest, byte_len)
        }),
        serde_json::Value::Object(object) => object.values().find_map(|item| {
            runtime_smart_context_find_inline_reference_text(item, digest, byte_len)
        }),
        _ => None,
    }
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
    before_count: &runtime_proxy_crate::SmartContextTokenCount,
    after_count: &runtime_proxy_crate::SmartContextTokenCount,
    critical_signal_check: prodex_context::CriticalSignalSelfCheck,
    exactness_guard: runtime_proxy_crate::SmartContextExactnessGuard,
    missing_rehydrate_refs: Vec<String>,
) -> runtime_proxy_crate::SmartContextRegressionSelfCheck {
    let before_text = String::from_utf8_lossy(before);
    let after_text = String::from_utf8_lossy(after);
    let token_count_source = if before_count.is_proven() && after_count.is_proven() {
        runtime_proxy_crate::SmartContextTokenCountSource::TokenizerCounted
    } else {
        runtime_proxy_crate::SmartContextTokenCountSource::Estimated
    };
    runtime_proxy_crate::smart_context_regression_self_check(
        runtime_proxy_crate::SmartContextRegressionSelfCheckInput {
            exactness_guard,
            before_hash: runtime_proxy_crate::smart_context_hash_text(&before_text),
            after_hash: runtime_proxy_crate::smart_context_hash_text(&after_text),
            before_tokens: before_count.tokens,
            after_tokens: after_count.tokens,
            token_count_source,
            future_retrieval_overhead_tokens: 0,
            injected_protocol_overhead_tokens: 0,
            expected_recovery_overhead_tokens: 0,
            before_critical_signal_count: critical_signal_check.before.total(),
            after_critical_signal_count: critical_signal_check.after.total(),
            missing_rehydrate_refs,
            unresolved_rehydrate_refs_are_segment_local: false,
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
    } else if reasons.iter().any(|reason| {
        matches!(
            reason,
            runtime_proxy_crate::SmartContextRegressionSelfCheckReason::TokenSavingsBelowSafetyMargin
        )
    }) {
        "token_savings_below_safety_margin"
    } else if reasons.iter().any(|reason| {
        matches!(
            reason,
            runtime_proxy_crate::SmartContextRegressionSelfCheckReason::TokenizerEstimateNotEligible
        )
    }) {
        "unsupported_tokenizer"
    } else {
        "token_budget_did_not_improve"
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duplicate_admission_requires_exact_text_in_distinct_input_items() {
        let text = "same exact tool output".repeat(64);
        let duplicate = serde_json::json!({"input": [
            {"output": text.clone()},
            {"output": text.clone()},
        ]});
        assert!(runtime_smart_context_has_duplicate_input_text(&duplicate));

        let same_item =
            serde_json::json!({"input": [{"first": text.clone(), "second": text.clone()}]});
        assert!(!runtime_smart_context_has_duplicate_input_text(&same_item));

        let distinct = serde_json::json!({"input": [
            {"output": text},
            {"output": "different tool output".repeat(64)},
        ]});
        assert!(!runtime_smart_context_has_duplicate_input_text(&distinct));
    }

    #[test]
    fn inline_reference_round_trip_requires_exact_restoration() {
        let text = "same exact tool output".repeat(64);
        let original = serde_json::json!({
            "model": "gpt-5.4",
            "input": [{"output": text.clone()}, {"output": text}],
        });
        let mut candidate = original.clone();
        let mut stats = RuntimeSmartContextTransformStats::default();
        runtime_smart_context_dedupe_input_text_within_request(&mut candidate, &mut stats);
        assert!(runtime_smart_context_append_inline_reference_protocol(
            &mut candidate,
            &stats,
        ));

        let mut expanded =
            runtime_smart_context_expand_inline_references(&original, &candidate).unwrap();
        assert!(runtime_smart_context_inline_reference_round_trip_is_exact(
            &original,
            &mut expanded,
        ));
        assert_eq!(expanded, original);

        let mut tampered =
            runtime_smart_context_expand_inline_references(&original, &candidate).unwrap();
        tampered["model"] = serde_json::json!("other-model");
        assert!(!runtime_smart_context_inline_reference_round_trip_is_exact(
            &original,
            &mut tampered,
        ));
    }
}
