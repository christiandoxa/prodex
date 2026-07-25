use super::*;

#[test]
fn smart_context_rehydrates_short_artifact_refs_and_line_ranges() {
    let mut store = RuntimeSmartContextArtifactStore::default();
    let artifact = store.insert_text("line one\nline two\nline three").unwrap();
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
    let artifact = store.insert_text(artifact_text).unwrap();
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
    let original = value.clone();
    let mut stats = RuntimeSmartContextTransformStats::default();

    runtime_smart_context_dedupe_input_text_within_request(&mut value, &mut stats);
    assert!(runtime_smart_context_append_inline_reference_protocol(
        &mut value, &stats
    ));

    let reference = value["input"][1]["content"].as_str().unwrap();
    assert!(reference.starts_with("[prodex-context-ref v=1 source=original-input[0]"));
    assert!(!reference.contains("psc"));
    assert_eq!(value["input"][2]["role"].as_str(), Some("developer"));
    assert_eq!(
        runtime_smart_context_expand_inline_references(&original, &value).unwrap()["input"][1]
            ["content"]
            .as_str(),
        Some(repeated.as_str())
    );
    assert_eq!(stats.duplicate_texts, 1);
}

#[test]
fn smart_context_inline_reference_rejects_tampered_digest() {
    let repeated = "same ".repeat(300);
    let original = serde_json::json!({
        "input": [
            {"type": "message", "content": repeated},
            {"type": "message", "content": repeated}
        ]
    });
    let mut candidate = original.clone();
    let mut stats = RuntimeSmartContextTransformStats::default();
    runtime_smart_context_dedupe_input_text_within_request(&mut candidate, &mut stats);
    let reference = candidate["input"][1]["content"].as_str().unwrap();
    candidate["input"][1]["content"] =
        serde_json::Value::String(reference.replacen("sc2:", "sc2:0", 1));

    assert!(runtime_smart_context_expand_inline_references(&original, &candidate).is_none());
}

#[test]
fn smart_context_dedupe_preserves_static_prompt_prefix() {
    let repeated = "static prompt prefix ".repeat(120);
    let mut value = serde_json::json!({
        "input": [
            {"role": "system", "content": repeated},
            {"role": "developer", "content": repeated},
            {"type": "message", "content": repeated},
            {"type": "message", "content": repeated}
        ]
    });
    let mut stats = RuntimeSmartContextTransformStats::default();

    runtime_smart_context_dedupe_input_text_within_request(&mut value, &mut stats);

    assert_eq!(
        value["input"][0]["content"].as_str(),
        Some(repeated.as_str())
    );
    assert_eq!(
        value["input"][1]["content"].as_str(),
        Some(repeated.as_str())
    );
    assert!(
        value["input"][3]["content"]
            .as_str()
            .unwrap()
            .contains("prodex-context-ref v=1")
    );
    assert_eq!(stats.duplicate_texts, 1);
}
