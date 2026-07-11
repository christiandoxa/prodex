use super::*;

#[test]
fn smart_context_rehydrates_short_artifact_refs_and_line_ranges() {
    let mut store = RuntimeSmartContextArtifactStore::default();
    let artifact = store
        .insert_text(1, "line one\nline two\nline three")
        .unwrap();
    let mut value = serde_json::json!({
        "input": [{
            "type": "message",
            "content": format!(
                "need {}",
                runtime_smart_context_artifact_line_ref(&artifact.id, 2, 3)
            )
        }]
    });
    let mut stats = RuntimeSmartContextTransformStats::default();

    runtime_smart_context_rehydrate_value(&mut value, &store, &mut stats);

    assert_eq!(value["input"][0]["content"], "need line two\nline three");
    assert_eq!(stats.rehydrated_refs, 1);
}

#[test]
fn smart_context_rehydrates_legacy_verbose_artifact_marker_summary() {
    let artifact_text = "legacy exact artifact body\nwith second line";
    let mut store = RuntimeSmartContextArtifactStore::default();
    let artifact = store.insert_text(1, artifact_text).unwrap();
    let mut value = serde_json::json!({
        "input": [{
            "type": "message",
            "content": format!(
                "prodex-sc artifact prodex-artifact:{} bytes={} hash={}; rehydrate: use prodex-artifact:{} or prodex-artifact:{}#Lstart-Lend\nlegacy summary",
                artifact.id,
                artifact.byte_len,
                artifact.content_hash,
                artifact.id,
                artifact.id
            )
        }]
    });
    let mut stats = RuntimeSmartContextTransformStats::default();

    runtime_smart_context_rehydrate_value(&mut value, &store, &mut stats);

    assert_eq!(value["input"][0]["content"], artifact_text);
    assert_eq!(stats.rehydrated_refs, 1);
}

#[test]
fn smart_context_dedupes_repeated_input_text() {
    let repeated = "same ".repeat(300);
    let mut value = serde_json::json!({
        "input": [
            {"type": "message", "content": repeated},
            {"type": "message", "content": repeated}
        ]
    });
    let mut stats = RuntimeSmartContextTransformStats::default();

    let store = RuntimeSmartContextArtifactStore::default();

    runtime_smart_context_dedupe_input_text(
        &mut value,
        &store,
        &runtime_proxy_crate::smart_context_exactness_guard(
            runtime_proxy_crate::SmartContextExactnessInput::default(),
        ),
        &mut stats,
    );

    assert_eq!(
        value["input"][1]["content"].as_str(),
        Some("[psc dup input[0]]")
    );
    assert_eq!(stats.duplicate_texts, 1);
}

#[test]
fn smart_context_dedupe_preserves_static_prompt_prefix() {
    let repeated = "static prompt prefix ".repeat(120);
    let mut store = RuntimeSmartContextArtifactStore::default();
    let artifact = store.insert_text(1, &repeated).unwrap();
    let mut value = serde_json::json!({
        "input": [
            {"role": "system", "content": repeated},
            {"role": "developer", "content": repeated},
            {"type": "message", "content": repeated}
        ]
    });
    let mut stats = RuntimeSmartContextTransformStats::default();

    runtime_smart_context_dedupe_input_text(
        &mut value,
        &store,
        &runtime_proxy_crate::smart_context_exactness_guard(
            runtime_proxy_crate::SmartContextExactnessInput::default(),
        ),
        &mut stats,
    );

    assert_eq!(
        value["input"][0]["content"].as_str(),
        Some(repeated.as_str())
    );
    assert_eq!(
        value["input"][1]["content"].as_str(),
        Some(repeated.as_str())
    );
    assert!(
        value["input"][2]["content"]
            .as_str()
            .unwrap()
            .contains(&runtime_smart_context_artifact_ref(&artifact.id))
    );
    assert_eq!(stats.duplicate_texts, 0);
    assert_eq!(stats.cross_turn_duplicate_texts, 1);
}

#[test]
fn smart_context_dedupes_large_exact_text_outside_top_level_input() {
    let repeated = "external exact metadata ".repeat(140);
    let mut store = RuntimeSmartContextArtifactStore::default();
    let artifact = store.insert_text(1, &repeated).unwrap();
    let mut value = serde_json::json!({
        "instructions": repeated.as_str(),
        "metadata": {
            "transcript": repeated.as_str()
        },
        "input": [
            {"type": "message", "content": "use the exact external transcript if needed"}
        ]
    });
    let mut stats = RuntimeSmartContextTransformStats::default();

    runtime_smart_context_dedupe_input_text(
        &mut value,
        &store,
        &runtime_proxy_crate::smart_context_exactness_guard(
            runtime_proxy_crate::SmartContextExactnessInput::default(),
        ),
        &mut stats,
    );

    assert_eq!(
        value["instructions"].as_str(),
        Some(repeated.as_str()),
        "static prompt fields must not be rewritten from persisted fingerprints"
    );
    assert!(
        value["metadata"]["transcript"]
            .as_str()
            .unwrap()
            .contains(&runtime_smart_context_artifact_ref(&artifact.id))
    );
    assert_eq!(stats.cross_turn_duplicate_texts, 1);
}
