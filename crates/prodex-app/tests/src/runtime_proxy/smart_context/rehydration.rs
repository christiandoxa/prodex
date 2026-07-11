use super::*;
use std::collections::BTreeSet;

#[test]
fn smart_context_rehydrates_known_artifact_refs() {
    let mut store = RuntimeSmartContextArtifactStore::default();
    let artifact = store.insert_text(1, "exact artifact text").unwrap();
    let mut value = serde_json::json!({
        "input": [{
            "type": "message",
            "content": format!("need prodex-artifact:{}", artifact.id)
        }]
    });
    let mut stats = RuntimeSmartContextTransformStats::default();

    runtime_smart_context_rehydrate_value(&mut value, &store, &mut stats);

    assert_eq!(value["input"][0]["content"], "need exact artifact text");
    assert_eq!(stats.rehydrated_refs, 1);
}

#[test]
fn smart_context_rehydrate_preserves_static_prompt_prefix() {
    let mut store = RuntimeSmartContextArtifactStore::default();
    let artifact = store.insert_text(1, "exact artifact text").unwrap();
    let static_ref = format!("keep prodex-artifact:{}", artifact.id);
    let mut value = serde_json::json!({
        "instructions": static_ref,
        "system": format!("system prodex-artifact:{}", artifact.id),
        "developer": format!("developer prodex-artifact:{}", artifact.id),
        "input": [
            {
                "role": "system",
                "content": format!("input system prodex-artifact:{}", artifact.id),
            },
            {
                "role": "developer",
                "content": format!("input developer prodex-artifact:{}", artifact.id),
            },
            {
                "type": "message",
                "content": format!("need prodex-artifact:{}", artifact.id),
            }
        ]
    });
    let mut stats = RuntimeSmartContextTransformStats::default();

    runtime_smart_context_rehydrate_value(&mut value, &store, &mut stats);

    assert_eq!(value["instructions"].as_str(), Some(static_ref.as_str()));
    assert_eq!(
        value["system"].as_str(),
        Some(format!("system prodex-artifact:{}", artifact.id).as_str())
    );
    assert_eq!(
        value["developer"].as_str(),
        Some(format!("developer prodex-artifact:{}", artifact.id).as_str())
    );
    assert_eq!(
        value["input"][0]["content"].as_str(),
        Some(format!("input system prodex-artifact:{}", artifact.id).as_str())
    );
    assert_eq!(
        value["input"][1]["content"].as_str(),
        Some(format!("input developer prodex-artifact:{}", artifact.id).as_str())
    );
    assert_eq!(
        value["input"][2]["content"].as_str(),
        Some("need exact artifact text")
    );
    assert_eq!(stats.rehydrated_refs, 1);
}

#[test]
fn smart_context_rehydrates_artifact_line_ranges() {
    let mut store = RuntimeSmartContextArtifactStore::default();
    let artifact = store
        .insert_text(1, "line one\nline two\nline three\nline four")
        .unwrap();
    let mut value = serde_json::json!({
        "input": [{
            "type": "message",
            "content": format!("need prodex-artifact:{}#L2-L3", artifact.id)
        }]
    });
    let mut stats = RuntimeSmartContextTransformStats::default();

    runtime_smart_context_rehydrate_value(&mut value, &store, &mut stats);

    assert_eq!(value["input"][0]["content"], "need line two\nline three");
    assert_eq!(stats.rehydrated_refs, 1);
}

#[test]
fn smart_context_rehydrates_compact_multi_line_ranges() {
    let mut store = RuntimeSmartContextArtifactStore::default();
    let artifact = store
        .insert_text(1, "line one\nline two\nline three\nline four\nline five")
        .unwrap();
    let mut value = serde_json::json!({
        "input": [{
            "type": "message",
            "content": format!("need {}#L1-L2,L4-L5", runtime_smart_context_artifact_ref(&artifact.id))
        }]
    });
    let mut stats = RuntimeSmartContextTransformStats::default();

    runtime_smart_context_rehydrate_value(&mut value, &store, &mut stats);

    assert_eq!(
        value["input"][0]["content"],
        "need line one\nline two\nline four\nline five"
    );
    assert_eq!(stats.rehydrated_refs, 1);
}

#[test]
fn smart_context_budget_limited_rehydrate_prefers_symbol_critical_and_import_ranges() {
    let unrelated = (0..80)
        .map(|index| format!("fn unrelated_{index}() -> usize {{ {index} }}"))
        .collect::<Vec<_>>()
        .join("\n");
    let artifact_text = format!(
        "\
use crate::runtime::Thing;
use std::sync::Arc;

{unrelated}

fn target_symbol() -> usize {{
    let _thing = Thing::default();
    Arc::strong_count(&Arc::new(1))
}}

error[E0425]: cannot find value `missing` in this scope
src/lib.rs:77:9
diagnostic context line
FULL_ARTIFACT_TAIL_SHOULD_NOT_REHYDRATE"
    );
    let mut store = RuntimeSmartContextArtifactStore::default();
    let artifact = store.insert_text(1, &artifact_text).unwrap();
    let mut value = serde_json::json!({
        "input": [{
            "type": "message",
            "content": format!("inspect {}", runtime_smart_context_artifact_ref(&artifact.id))
        }]
    });
    let plan = runtime_smart_context_auto_rehydrate_plan(
        &value,
        &store,
        96,
        runtime_proxy_crate::SmartContextTokenBudgetTier::Condensed,
    );
    assert!(plan.actions.iter().any(|action| matches!(
        action,
        runtime_proxy_crate::SmartContextRehydrateAction::Defer {
            id,
            reason: runtime_proxy_crate::SmartContextRehydrateDeferReason::TokenBudgetExceeded
        } if id == &artifact.id
    )));

    let mut stats = RuntimeSmartContextTransformStats::default();
    runtime_smart_context_rehydrate_value_with_plan(&mut value, &store, &plan, &mut stats);
    let count = runtime_smart_context_selective_rehydrate_budget_aware_ranges(
        &mut value,
        &store,
        &runtime_proxy_crate::smart_context_exactness_guard(
            runtime_proxy_crate::SmartContextExactnessInput::default(),
        ),
        &RuntimeSmartContextSelectiveRehydrateTerms {
            error_codes: BTreeSet::from(["E0425".to_string()]),
            test_symbols: BTreeSet::from(["crate::module::target_symbol".to_string()]),
            ..RuntimeSmartContextSelectiveRehydrateTerms::default()
        },
        &plan,
        180,
        &mut stats,
    );

    let content = value["input"][0]["content"].as_str().unwrap();
    assert!(count >= 3);
    assert_eq!(stats.rehydrated_refs, count);
    assert!(content.contains(SMART_CONTEXT_LABEL_SEMANTIC_EXACT));
    assert!(content.contains(SMART_CONTEXT_LABEL_REHYDRATE_PLAN_EXACT));
    assert!(content.contains("use crate::runtime::Thing;"));
    assert!(content.contains("fn target_symbol() -> usize"));
    assert!(content.contains("error[E0425]: cannot find value `missing` in this scope"));
    assert!(content.contains("src/lib.rs:77:9"));
    assert!(!content.contains("FULL_ARTIFACT_TAIL_SHOULD_NOT_REHYDRATE"));
}

#[test]
fn smart_context_budget_available_rehydrates_full_artifact_without_read_plan() {
    let artifact_text = "fn target_symbol() -> usize { 1 }\nFULL_ARTIFACT_TAIL_REHYDRATED";
    let mut store = RuntimeSmartContextArtifactStore::default();
    let artifact = store.insert_text(1, artifact_text).unwrap();
    let mut value = serde_json::json!({
        "input": [{
            "type": "message",
            "content": format!("inspect {}", runtime_smart_context_artifact_ref(&artifact.id))
        }]
    });
    let plan = runtime_smart_context_auto_rehydrate_plan(
        &value,
        &store,
        usize::MAX,
        runtime_proxy_crate::SmartContextTokenBudgetTier::Exact,
    );
    let mut stats = RuntimeSmartContextTransformStats::default();

    runtime_smart_context_rehydrate_value_with_plan(&mut value, &store, &plan, &mut stats);
    let count = runtime_smart_context_selective_rehydrate_budget_aware_ranges(
        &mut value,
        &store,
        &runtime_proxy_crate::smart_context_exactness_guard(
            runtime_proxy_crate::SmartContextExactnessInput::default(),
        ),
        &RuntimeSmartContextSelectiveRehydrateTerms {
            test_symbols: BTreeSet::from(["target_symbol".to_string()]),
            ..RuntimeSmartContextSelectiveRehydrateTerms::default()
        },
        &plan,
        usize::MAX,
        &mut stats,
    );

    let content = value["input"][0]["content"].as_str().unwrap();
    assert_eq!(count, 0);
    assert_eq!(stats.rehydrated_refs, 1);
    assert!(content.contains("FULL_ARTIFACT_TAIL_REHYDRATED"));
    assert!(!content.contains(SMART_CONTEXT_LABEL_REHYDRATE_PLAN_EXACT));
}

#[test]
fn smart_context_parser_accepts_short_and_legacy_artifact_refs() {
    let refs = runtime_smart_context_collect_artifact_refs(&serde_json::Value::String(
        "new psc:abc123#L2-L4 full psc:sc:def456 old prodex-artifact:sc:789abc?lines=L1-L1"
            .to_string(),
    ));

    assert!(refs.iter().any(|reference| {
        reference.id == "sc:abc123"
            && reference.line_range == Some(RuntimeSmartContextLineRange { start: 2, end: 4 })
    }));
    assert!(
        refs.iter()
            .any(|reference| reference.id == "sc:def456" && reference.line_range.is_none())
    );
    assert!(refs.iter().any(|reference| {
        reference.id == "sc:789abc"
            && reference.line_range == Some(RuntimeSmartContextLineRange { start: 1, end: 1 })
    }));
    let multi = runtime_smart_context_collect_artifact_refs(&serde_json::Value::String(
        "multi psc:abc123#L1-L2,L4-L5".to_string(),
    ));
    let multi = multi
        .iter()
        .find(|reference| reference.id == "sc:abc123")
        .unwrap();
    assert_eq!(
        multi.line_ranges,
        vec![
            RuntimeSmartContextLineRange { start: 1, end: 2 },
            RuntimeSmartContextLineRange { start: 4, end: 5 },
        ]
    );
}

#[test]
fn smart_context_alias_parser_rehydrates_alias_refs_when_legend_present() {
    let mut store = RuntimeSmartContextArtifactStore::default();
    let artifact = store
        .insert_text(1, "line one\nline two\nline three")
        .unwrap();
    let mut value = serde_json::json!({
        "input": [{
            "type": "message",
            "content": format!(
                "psc aliases @0={}\nneed @0#L2-L3",
                runtime_smart_context_artifact_ref(&artifact.id)
            )
        }]
    });
    let mut stats = RuntimeSmartContextTransformStats::default();

    runtime_smart_context_rehydrate_value(&mut value, &store, &mut stats);

    let expected = format!(
        "psc aliases @0={}\nneed line two\nline three",
        runtime_smart_context_artifact_ref(&artifact.id)
    );
    assert_eq!(
        value["input"][0]["content"].as_str(),
        Some(expected.as_str())
    );
    assert_eq!(stats.rehydrated_refs, 1);
}

#[test]
fn smart_context_alias_parser_ignores_alias_refs_without_legend() {
    let mut store = RuntimeSmartContextArtifactStore::default();
    store
        .insert_text(1, "line one\nline two\nline three")
        .unwrap();
    let mut value = serde_json::json!({
        "input": [{"type": "message", "content": "need @0#L2-L3"}]
    });
    let mut stats = RuntimeSmartContextTransformStats::default();

    runtime_smart_context_rehydrate_value(&mut value, &store, &mut stats);

    assert_eq!(value["input"][0]["content"].as_str(), Some("need @0#L2-L3"));
    assert_eq!(stats.rehydrated_refs, 0);
}
