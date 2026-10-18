use super::*;
use prodex_mojo_core::json::{
    SmartContextInlineDocument, SmartContextInlineOperation, SmartContextInlinePlan,
};

fn runtime_smart_context_inline_request(
    value: &serde_json::Value,
    candidate: Option<&serde_json::Value>,
    operation: SmartContextInlineOperation,
) -> SmartContextInlinePlan {
    let source = crate::json_boundary::app_json_tree(value)
        .expect("bounded Smart Context source JSON must fit its host ABI");
    let candidate = candidate
        .map(crate::json_boundary::app_json_tree)
        .transpose()
        .expect("bounded Smart Context candidate JSON must fit its host ABI");
    // The Rust-only SHA-256 dependency supplies facts; Mojo selects and verifies references.
    let hashes = if matches!(operation, SmartContextInlineOperation::Probe) {
        Vec::new()
    } else {
        source
            .nodes
            .iter()
            .map(|node| match node.kind {
                prodex_mojo_core::json::JsonKind::String => {
                    runtime_proxy_crate::smart_context_hash_text(node.text)
                }
                prodex_mojo_core::json::JsonKind::Null
                | prodex_mojo_core::json::JsonKind::False
                | prodex_mojo_core::json::JsonKind::True
                | prodex_mojo_core::json::JsonKind::Number
                | prodex_mojo_core::json::JsonKind::Array
                | prodex_mojo_core::json::JsonKind::Object => String::new(),
            })
            .collect::<Vec<_>>()
    };
    let hashes = hashes.iter().map(String::as_str).collect::<Vec<_>>();
    let source_raw = std::str::from_utf8(&source.raw).expect("Serde JSON remains UTF-8");
    let candidate = candidate.as_ref().map(|tree| SmartContextInlineDocument {
        nodes: &tree.nodes,
        raw: std::str::from_utf8(&tree.raw).expect("Serde JSON remains UTF-8"),
        hashes: &[],
    });
    prodex_mojo_core::json::smart_context_inline_plan(
        SmartContextInlineDocument {
            nodes: &source.nodes,
            raw: source_raw,
            hashes: &hashes,
        },
        candidate,
        operation,
    )
    .expect("compiled Mojo inline-reference projection returned invalid output")
}

pub(super) fn runtime_smart_context_dedupe_input_text_within_request(
    value: &mut serde_json::Value,
    stats: &mut RuntimeSmartContextTransformStats,
) {
    let plan =
        runtime_smart_context_inline_request(value, None, SmartContextInlineOperation::Rewrite);
    if let Some(body) = plan.body {
        *value = serde_json::from_slice(&body).expect("Mojo inline rewrite emits valid JSON");
        stats.duplicate_texts += plan.replacements;
    }
}

pub(super) fn runtime_smart_context_has_duplicate_input_text(value: &serde_json::Value) -> bool {
    let plan =
        runtime_smart_context_inline_request(value, None, SmartContextInlineOperation::Probe);
    plan.candidate_limit_exceeded || plan.replacements > 0
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
    let plan = runtime_smart_context_inline_request(
        original,
        Some(candidate),
        SmartContextInlineOperation::Expand,
    );
    if !plan.valid {
        return None;
    }
    let body = plan
        .body
        .expect("valid Mojo inline expansion must emit a JSON body");
    Some(serde_json::from_slice(&body).expect("Mojo inline expansion emits valid JSON"))
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
    let reason_bits = regression_check
        .reasons
        .iter()
        .fold(0_u64, |bits, reason| bits | (1_u64 << (*reason as u8)));
    prodex_mojo_core::runtime_decisions::smart_context_rewrite_validation_reason(
        critical_signal_check.has_loss(),
        regression_check.decision
            == runtime_proxy_crate::SmartContextRegressionSelfCheckDecision::FallbackExact,
        reason_bits,
        [
            stats.rehydrated_refs,
            stats.tool_outputs_condensed,
            stats.duplicate_texts,
            stats.cross_turn_duplicate_texts,
            stats.repeat_tool_output_refs,
            stats.static_context_deltas,
        ],
    )
    .expect("Mojo Smart Context rewrite validation reason returned invalid output")
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
    fn compiled_inline_projection_preserves_nested_escaped_text_roles_and_round_trip() {
        let text = "\0line \"quoted\" 🧭\n".repeat(96);
        let original = serde_json::json!({"input": [
            {"role": "system", "content": text},
            {"role": "developer", "content": text},
            {"role": "user", "payload": {"a": [text, text]}},
            {"output": {"escaped\"key": text}},
        ]});
        let mut candidate = original.clone();
        let mut stats = RuntimeSmartContextTransformStats::default();
        assert!(runtime_smart_context_has_duplicate_input_text(&candidate));
        runtime_smart_context_dedupe_input_text_within_request(&mut candidate, &mut stats);
        assert_eq!(stats.duplicate_texts, 1);
        for index in 0..3 {
            assert_eq!(candidate["input"][index], original["input"][index]);
        }
        let marker = candidate["input"][3]["output"]["escaped\"key"]
            .as_str()
            .unwrap();
        assert_eq!(
            marker,
            format!(
                "[prodex-context-ref v=1 source=original-input[2] digest={} bytes={}]",
                runtime_proxy_crate::smart_context_hash_text(&text),
                text.len(),
            )
        );
        runtime_smart_context_append_inline_reference_protocol(&mut candidate, &stats);
        let mut expanded =
            runtime_smart_context_expand_inline_references(&original, &candidate).unwrap();
        assert!(runtime_smart_context_inline_reference_round_trip_is_exact(
            &original,
            &mut expanded
        ));
        assert_eq!(expanded, original);
        candidate["input"][3]["output"]["escaped\"key"] = serde_json::json!(format!(
            "[prodex-context-ref v=1 source=original-input[18446744073709551616] digest={} bytes={}]",
            runtime_proxy_crate::smart_context_hash_text(&text),
            text.len(),
        ));
        assert!(runtime_smart_context_expand_inline_references(&original, &candidate).is_none());
    }

    #[test]
    fn compiled_inline_probe_limit_does_not_invent_rewrite_candidates() {
        let mut value = serde_json::json!({"input": (0..257).map(|index| {
            serde_json::json!({"output": format!("{index}: {}", "unique tool data ".repeat(80))})
        }).collect::<Vec<_>>()});
        let original = value.clone();
        let mut stats = RuntimeSmartContextTransformStats::default();
        assert!(runtime_smart_context_has_duplicate_input_text(&value));
        runtime_smart_context_dedupe_input_text_within_request(&mut value, &mut stats);
        assert_eq!(value, original);
        assert_eq!(stats.duplicate_texts, 0);
    }

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
