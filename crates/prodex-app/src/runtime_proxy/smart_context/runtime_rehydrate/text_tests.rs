use super::*;

#[test]
fn compiled_artifact_text_rehydrates_multiple_ranges_with_original_line_endings() {
    let mut store = RuntimeSmartContextArtifactStore::default();
    let artifact = store.insert_text("first\r\n東京🧭\r\nlast\r").unwrap();
    let marker = super::super::super::runtime_smart_context_artifact_ref(&artifact.id);
    let mut value = serde_json::json!({"input": [{"output": format!("need {marker}#L2-L99,1")}]});
    let references = runtime_smart_context_collect_rehydratable_artifact_refs(&value);
    assert_eq!(
        runtime_smart_context_rehydrate_ref_token_cost(&references[0], &store),
        Some(
            runtime_proxy_crate::smart_context_estimate_tokens_from_body_bytes(
                "東京🧭\nlast\r\nfirst".len()
            ) as usize
        )
    );
    let mut stats = RuntimeSmartContextTransformStats::default();
    runtime_smart_context_rehydrate_value(&mut value, &store, &mut stats);
    assert_eq!(value["input"][0]["output"], "need 東京🧭\nlast\r\nfirst");
    assert_eq!(stats.rehydration_performed, 1);

    let mut missing = serde_json::json!({"output": format!("{marker}#L100-L102")});
    let references = runtime_smart_context_collect_rehydratable_artifact_refs(&missing);
    assert_eq!(
        runtime_smart_context_rehydrate_ref_token_cost(&references[0], &store),
        None
    );
    let original = missing.clone();
    let mut stats = RuntimeSmartContextTransformStats::default();
    runtime_smart_context_rehydrate_value(&mut missing, &store, &mut stats);
    assert_eq!(missing, original);
    assert_eq!(stats.rehydration_performed, 0);
}

#[test]
fn compiled_artifact_text_preserves_ordered_cascading_replacements() {
    let mut store = RuntimeSmartContextArtifactStore::default();
    let second = store.insert_text("second body").unwrap();
    let second_marker = super::super::super::runtime_smart_context_artifact_ref(&second.id);
    let first = store
        .insert_text(&format!("first: {second_marker}"))
        .unwrap();
    let first_marker = super::super::super::runtime_smart_context_artifact_ref(&first.id);
    let mut value = serde_json::json!({"output": format!("{first_marker} {second_marker}")});
    let mut stats = RuntimeSmartContextTransformStats::default();
    runtime_smart_context_rehydrate_value(&mut value, &store, &mut stats);
    assert_eq!(value["output"], "first: second body second body");
    assert_eq!(stats.rehydration_planned, 2);
    assert_eq!(stats.rehydration_performed, 2);
}
