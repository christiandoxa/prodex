use super::*;

#[test]
fn compiled_grouped_rehydration_applies_escaped_paths_and_preserves_static_fields() {
    let mut store = RuntimeSmartContextArtifactStore::default();
    let artifact = store.insert_text("first\r\nline two\0🧭").unwrap();
    let marker = super::super::super::runtime_smart_context_artifact_ref(&artifact.id);
    let mut value = serde_json::json!({
        "": marker,
        "a/b~c": [null, {"雪\n\"key": format!("{marker}#L2-L2")}],
        "plain": artifact.id,
        "input": [
            {"role": "system", "content": format!("@9={marker} @9")},
            {"role": "user", "content": "@9"},
        ],
        "instructions": marker,
    });
    let mut expected = value.clone();
    expected[""] = serde_json::json!("first\r\nline two\0🧭");
    expected["a/b~c"][1]["雪\n\"key"] = serde_json::json!("line two\0🧭");
    expected["plain"] = serde_json::json!("first\r\nline two\0🧭");
    expected["input"][1]["content"] = serde_json::json!("first\r\nline two\0🧭");
    let mut stats = RuntimeSmartContextTransformStats::default();
    runtime_smart_context_rehydrate_value(&mut value, &store, &mut stats);
    assert_eq!(value, expected);
    assert_eq!(stats.rehydration_performed, 4);

    let mut root = serde_json::json!(marker);
    runtime_smart_context_rehydrate_value(&mut root, &store, &mut stats);
    assert_eq!(root, "first\r\nline two\0🧭");
    assert_eq!(stats.rehydration_performed, 5);
}
