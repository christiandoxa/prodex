use super::*;

#[test]
fn smart_context_artifact_manifest_lists_refs_without_full_content() {
    let secret_line = "SECRET_FULL_ARTIFACT_BODY_SHOULD_NOT_APPEAR";
    let artifact_text = format!(
        "running 1 test\n---- tests::hidden_case stdout ----\nthread 'tests::hidden_case' panicked at src/lib.rs:7:3:\nerror[E0425]: cannot find value\n --> src/lib.rs:7:3\n{secret_line}\ntest result: FAILED. 0 passed; 1 failed"
    );
    let mut store = RuntimeSmartContextArtifactStore::default();
    let artifact = store.insert_text(7, &artifact_text).unwrap();

    let manifest =
        runtime_smart_context_artifact_manifest(&store).expect("artifact manifest should render");

    assert!(manifest.contains("psc m"));
    assert!(!manifest.contains(" set="));
    assert!(manifest.contains(&runtime_smart_context_artifact_ref(&artifact.id)));
    assert!(manifest.contains(&format!("b={}", artifact_text.len())));
    assert!(!manifest.contains(" h="));
    assert!(manifest.contains("cr="));
    assert!(manifest.contains("sr="));
    assert!(manifest.contains("k=cargo-test"));
    assert!(!manifest.contains(secret_line));
    assert!(!manifest.contains("cannot find value"));
}

#[test]
fn smart_context_appends_artifact_manifest_only_when_rewrite_useful() {
    let mut store = RuntimeSmartContextArtifactStore::default();
    store
        .insert_text(1, "error: compacted output\nsrc/lib.rs:1:1")
        .unwrap();
    let mut value = serde_json::json!({
        "input": [{
            "type": "message",
            "role": "user",
            "content": "existing prompt"
        }]
    });

    assert!(!runtime_smart_context_append_artifact_manifest_if_useful(
        &mut value,
        &store,
        &RuntimeSmartContextTransformStats::default(),
    ));
    assert_eq!(value["input"].as_array().unwrap().len(), 1);

    let useful_stats = RuntimeSmartContextTransformStats {
        tool_outputs_condensed: 1,
        ..RuntimeSmartContextTransformStats::default()
    };
    assert!(runtime_smart_context_append_artifact_manifest_if_useful(
        &mut value,
        &store,
        &useful_stats,
    ));
    let input = value["input"].as_array().unwrap();
    assert_eq!(input.len(), 2);
    let manifest = input[1]["content"].as_str().unwrap();
    assert!(manifest.contains("psc:"));
    assert!(!manifest.contains("error: compacted output"));
}

#[test]
fn smart_context_manifest_skips_refs_already_visible_in_payload() {
    let mut store = RuntimeSmartContextArtifactStore::default();
    let artifact = store
        .insert_text(1, "error: compacted output\nsrc/lib.rs:1:1")
        .unwrap();
    let mut value = serde_json::json!({
        "input": [{
            "type": "message",
            "role": "user",
            "content": format!("visible {}", runtime_smart_context_artifact_ref(&artifact.id))
        }]
    });
    let useful_stats = RuntimeSmartContextTransformStats {
        tool_outputs_condensed: 1,
        ..RuntimeSmartContextTransformStats::default()
    };

    assert!(!runtime_smart_context_append_artifact_manifest_if_useful(
        &mut value,
        &store,
        &useful_stats,
    ));
    assert_eq!(value["input"].as_array().unwrap().len(), 1);
}

#[test]
fn smart_context_manifest_default_omits_detail_fields_until_requested() {
    let artifact_text = "error[E0425]: cannot find value\nsrc/lib.rs:7:3";
    let mut store = RuntimeSmartContextArtifactStore::default();
    store.insert_text(1, artifact_text).unwrap();
    let mut value = serde_json::json!({
        "input": [{"type": "message", "role": "user", "content": "existing prompt"}]
    });
    let stats = RuntimeSmartContextTransformStats {
        tool_outputs_condensed: 1,
        ..RuntimeSmartContextTransformStats::default()
    };

    assert!(runtime_smart_context_append_artifact_manifest_if_useful(
        &mut value, &store, &stats,
    ));

    let manifest = value["input"][1]["content"].as_str().unwrap();
    assert!(manifest.starts_with("psc m refs"));
    assert!(!manifest.contains(" set="));
    assert!(manifest.contains("b="));
    assert!(!manifest.contains("cr="));
    assert!(!manifest.contains("sr="));
    assert!(!manifest.contains("k="));
}

#[test]
fn smart_context_manifest_delta_appends_only_when_manifest_set_changes() {
    let shared = smart_context_test_shared("manifest-delta");
    register_runtime_smart_context_proxy_state(&shared.log_path, true, None, None);
    let useful_stats = RuntimeSmartContextTransformStats {
        tool_outputs_condensed: 1,
        ..RuntimeSmartContextTransformStats::default()
    };
    let relevant_intent = RuntimeSmartContextIntentSignals {
        command_kind_hints: BTreeSet::from(["test".to_string()]),
        ..RuntimeSmartContextIntentSignals::default()
    };

    with_runtime_smart_context_proxy_state(&shared, |state| {
        state
            .artifacts
            .insert_text(1, "first artifact\nerror: one")
            .unwrap();
        let mut first = serde_json::json!({
            "input": [{"type": "message", "role": "user", "content": "first"}]
        });
        assert!(
            runtime_smart_context_append_artifact_manifest_delta_if_useful(
                &mut first,
                state,
                &useful_stats,
                &relevant_intent,
            )
        );
        assert_eq!(first["input"].as_array().unwrap().len(), 2);

        let mut unchanged = serde_json::json!({
            "input": [{"type": "message", "role": "user", "content": "second"}]
        });
        assert!(
            !runtime_smart_context_append_artifact_manifest_delta_if_useful(
                &mut unchanged,
                state,
                &useful_stats,
                &relevant_intent,
            )
        );
        assert_eq!(unchanged["input"].as_array().unwrap().len(), 1);

        state.last_artifact_manifest_emitted_at = Some(
            Instant::now() - Duration::from_millis(SMART_CONTEXT_ARTIFACT_MANIFEST_COOLDOWN_MS + 1),
        );
        state
            .artifacts
            .insert_text(2, "second artifact\nerror: two")
            .unwrap();
        let mut changed = serde_json::json!({
            "input": [{"type": "message", "role": "user", "content": "third"}]
        });
        assert!(
            runtime_smart_context_append_artifact_manifest_delta_if_useful(
                &mut changed,
                state,
                &useful_stats,
                &relevant_intent,
            )
        );
        assert_eq!(changed["input"].as_array().unwrap().len(), 2);
        let changed_manifest = changed["input"][1]["content"].as_str().unwrap();
        assert!(!changed_manifest.contains(" set="));
        assert!(!changed_manifest.contains("first artifact"));
        assert!(changed_manifest.contains("same=1"));
        assert_eq!(changed_manifest.matches("psc:").count(), 1);
    })
    .unwrap();
}

#[test]
fn smart_context_explicit_manifest_request_keeps_full_manifest() {
    let shared = smart_context_test_shared("manifest-delta-explicit-full");
    register_runtime_smart_context_proxy_state(&shared.log_path, true, None, None);
    let useful_stats = RuntimeSmartContextTransformStats {
        tool_outputs_condensed: 1,
        ..RuntimeSmartContextTransformStats::default()
    };
    let manifest_intent = RuntimeSmartContextIntentSignals {
        intent_terms: vec!["manifest".to_string()],
        ..RuntimeSmartContextIntentSignals::default()
    };

    with_runtime_smart_context_proxy_state(&shared, |state| {
        state.artifacts.insert_text(1, "first artifact").unwrap();
        let mut first = serde_json::json!({
            "input": [{"type": "message", "role": "user", "content": "manifest"}]
        });
        assert!(
            runtime_smart_context_append_artifact_manifest_delta_if_useful(
                &mut first,
                state,
                &useful_stats,
                &manifest_intent,
            )
        );

        state.artifacts.insert_text(2, "second artifact").unwrap();
        let mut second = serde_json::json!({
            "input": [{"type": "message", "role": "user", "content": "manifest"}]
        });
        assert!(
            runtime_smart_context_append_artifact_manifest_delta_if_useful(
                &mut second,
                state,
                &useful_stats,
                &manifest_intent,
            )
        );
        let manifest = second["input"][1]["content"].as_str().unwrap();
        assert!(manifest.contains("same=1"));
        assert_eq!(manifest.matches("psc:").count(), 2);
    })
    .unwrap();
}

#[test]
fn smart_context_manifest_delta_suppressed_for_resolved_explicit_ref() {
    let shared = smart_context_test_shared("manifest-delta-explicit-ref");
    register_runtime_smart_context_proxy_state(&shared.log_path, true, None, None);
    let useful_stats = RuntimeSmartContextTransformStats {
        tool_outputs_condensed: 1,
        ..RuntimeSmartContextTransformStats::default()
    };

    with_runtime_smart_context_proxy_state(&shared, |state| {
        let artifact = state.artifacts.insert_text(1, "first artifact").unwrap();
        let reference = runtime_smart_context_artifact_ref(&artifact.id);
        let intent = RuntimeSmartContextIntentSignals {
            artifact_refs: vec![RuntimeSmartContextArtifactReference {
                id: artifact.id.clone(),
                marker: reference.clone(),
                line_range: None,
                line_ranges: Vec::new(),
            }],
            intent_terms: vec![artifact.id.clone()],
            command_kind_hints: BTreeSet::from(["test".to_string()]),
            ..RuntimeSmartContextIntentSignals::default()
        };
        let mut value = serde_json::json!({
            "input": [{"type": "message", "role": "user", "content": format!("inspect {reference}")}]
        });
        assert!(!runtime_smart_context_append_artifact_manifest_delta_if_useful(
            &mut value,
            state,
            &useful_stats,
            &intent,
        ));
        assert_eq!(value["input"].as_array().unwrap().len(), 1);
    })
    .unwrap();
}

#[test]
fn smart_context_manifest_delta_kept_for_missing_explicit_ref() {
    let shared = smart_context_test_shared("manifest-delta-missing-ref");
    register_runtime_smart_context_proxy_state(&shared.log_path, true, None, None);
    let useful_stats = RuntimeSmartContextTransformStats {
        tool_outputs_condensed: 1,
        ..RuntimeSmartContextTransformStats::default()
    };

    with_runtime_smart_context_proxy_state(&shared, |state| {
        state.artifacts.insert_text(1, "first artifact").unwrap();
        let intent = RuntimeSmartContextIntentSignals {
            artifact_refs: vec![RuntimeSmartContextArtifactReference {
                id: "sc:missing".to_string(),
                marker: "psc:missing".to_string(),
                line_range: None,
                line_ranges: Vec::new(),
            }],
            command_kind_hints: BTreeSet::from(["test".to_string()]),
            ..RuntimeSmartContextIntentSignals::default()
        };
        let mut value = serde_json::json!({
            "input": [{"type": "message", "role": "user", "content": "inspect psc:missing"}]
        });
        assert!(
            runtime_smart_context_append_artifact_manifest_delta_if_useful(
                &mut value,
                state,
                &useful_stats,
                &intent,
            )
        );
        assert_eq!(value["input"].as_array().unwrap().len(), 2);
    })
    .unwrap();
}

#[test]
fn smart_context_manifest_delta_requires_manifest_or_relevant_intent() {
    let shared = smart_context_test_shared("manifest-delta-gated");
    register_runtime_smart_context_proxy_state(&shared.log_path, true, None, None);
    let useful_stats = RuntimeSmartContextTransformStats {
        tool_outputs_condensed: 1,
        ..RuntimeSmartContextTransformStats::default()
    };

    with_runtime_smart_context_proxy_state(&shared, |state| {
        state
            .artifacts
            .insert_text(1, "first artifact\nerror: one")
            .unwrap();
        let mut unrequested = serde_json::json!({
            "input": [{"type": "message", "role": "user", "content": "continue"}]
        });
        assert!(
            !runtime_smart_context_append_artifact_manifest_delta_if_useful(
                &mut unrequested,
                state,
                &useful_stats,
                &RuntimeSmartContextIntentSignals::default(),
            )
        );
        assert_eq!(unrequested["input"].as_array().unwrap().len(), 1);

        let relevant_intent = RuntimeSmartContextIntentSignals {
            command_kind_hints: BTreeSet::from(["test".to_string()]),
            ..RuntimeSmartContextIntentSignals::default()
        };
        let mut relevant = serde_json::json!({
            "input": [{"type": "message", "role": "user", "content": "continue"}]
        });
        assert!(
            runtime_smart_context_append_artifact_manifest_delta_if_useful(
                &mut relevant,
                state,
                &useful_stats,
                &relevant_intent,
            )
        );
        assert_eq!(relevant["input"].as_array().unwrap().len(), 2);

        state
            .artifacts
            .insert_text(2, "second artifact\nerror: two")
            .unwrap();
        let mut cooled_down = serde_json::json!({
            "input": [{"type": "message", "role": "user", "content": "continue"}]
        });
        assert!(
            !runtime_smart_context_append_artifact_manifest_delta_if_useful(
                &mut cooled_down,
                state,
                &useful_stats,
                &relevant_intent,
            )
        );
        assert_eq!(cooled_down["input"].as_array().unwrap().len(), 1);
    })
    .unwrap();
}
