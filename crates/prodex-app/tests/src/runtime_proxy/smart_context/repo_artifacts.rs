use super::*;

#[test]
fn smart_context_repo_state_micro_cache_collapses_repeated_tool_output_facts() {
    let shared = smart_context_test_shared("repo-state-tool-repeat");
    register_runtime_smart_context_proxy_state(&shared.log_path, true, None, None);
    smart_context_observe_minimal_budget(&shared);
    let output = "\
Branch: feature/repo-cache
Dirty files:
- crates/prodex-app/src/runtime_proxy/smart_context.rs
Recent changed files:
- crates/prodex-app/tests/src/runtime_proxy/smart_context.rs
Package manager: cargo
Main test command: cargo test -q smart_context";
    let first = smart_context_test_request(serde_json::json!({
        "input": [{
            "type": "function_call_output",
            "call_id": "repo_state",
            "output": output
        }]
    }));
    let second = smart_context_test_request(serde_json::json!({
        "input": [{
            "type": "function_call_output",
            "call_id": "repo_state",
            "output": output
        }]
    }));

    let _ =
        prepare_runtime_smart_context_http_body(130, &first, &shared, RuntimeRouteKind::Responses);
    let prepared =
        prepare_runtime_smart_context_http_body(131, &second, &shared, RuntimeRouteKind::Responses);

    let Cow::Owned(body) = prepared else {
        panic!("expected repeated repo-state facts to rewrite");
    };
    let value = serde_json::from_slice::<serde_json::Value>(&body).unwrap();
    let rewritten = value["input"][0]["output"].as_str().unwrap();
    assert!(rewritten.starts_with("psc repo repeat"));
    assert!(rewritten.contains("branch=feature/repo-cache"));
    assert!(rewritten.contains("dirty=1"));
    assert!(rewritten.contains("recent=1"));
    assert!(rewritten.contains("pm=cargo"));
    assert!(rewritten.contains("cargo test -q smart_context"));
    assert!(!rewritten.contains("crates/prodex-app/src/runtime_proxy/smart_context.rs"));
    assert!(!rewritten.contains("crates/prodex-app/tests/src/runtime_proxy/smart_context.rs"));
}

#[test]
fn smart_context_repo_state_micro_cache_preserves_changed_tool_output_facts() {
    let shared = smart_context_test_shared("repo-state-tool-changed");
    register_runtime_smart_context_proxy_state(&shared.log_path, true, None, None);
    smart_context_observe_minimal_budget(&shared);
    let first_output = "\
Branch: feature/repo-cache
Dirty files:
- crates/prodex-app/src/runtime_proxy/smart_context.rs
Recent changed files:
- crates/prodex-app/tests/src/runtime_proxy/smart_context.rs
Package manager: cargo
Main test command: cargo test -q smart_context";
    let changed_output = "\
Branch: feature/repo-cache
Dirty files:
- crates/prodex-app/src/runtime_proxy/rotation.rs
Recent changed files:
- crates/prodex-app/tests/src/runtime_proxy/rotation.rs
Package manager: cargo
Main test command: cargo test -q smart_context";
    let first = smart_context_test_request(serde_json::json!({
        "input": [{
            "type": "function_call_output",
            "call_id": "repo_state",
            "output": first_output
        }]
    }));
    let changed = smart_context_test_request(serde_json::json!({
        "input": [{
            "type": "function_call_output",
            "call_id": "repo_state",
            "output": changed_output
        }]
    }));

    let _ =
        prepare_runtime_smart_context_http_body(132, &first, &shared, RuntimeRouteKind::Responses);
    let prepared = prepare_runtime_smart_context_http_body(
        133,
        &changed,
        &shared,
        RuntimeRouteKind::Responses,
    );

    let value = serde_json::from_slice::<serde_json::Value>(prepared.as_ref()).unwrap();
    let output = value["input"][0]["output"].as_str().unwrap();
    assert_eq!(output, changed_output);
    assert!(!output.contains("psc repo repeat"));
    assert!(output.contains("crates/prodex-app/src/runtime_proxy/rotation.rs"));
    assert!(output.contains("crates/prodex-app/tests/src/runtime_proxy/rotation.rs"));
}

#[test]
fn smart_context_repo_state_micro_cache_collapses_repeated_static_facts() {
    let shared = smart_context_test_shared("repo-state-static-repeat");
    register_runtime_smart_context_proxy_state(&shared.log_path, true, None, None);
    let exactness = runtime_proxy_crate::smart_context_exactness_guard(
        runtime_proxy_crate::SmartContextExactnessInput::default(),
    );
    let instructions = "\
Keep affinity exact.
Branch: feature/repo-cache
Dirty files:
- crates/prodex-app/src/runtime_proxy/smart_context.rs
Recent changed files:
- crates/prodex-app/tests/src/runtime_proxy/smart_context.rs
Package manager: cargo
Main test command: cargo test -q smart_context";

    with_runtime_smart_context_proxy_state(&shared, |state| {
        let mut first = serde_json::json!({ "instructions": instructions });
        let mut first_stats = RuntimeSmartContextTransformStats::default();
        assert!(!runtime_smart_context_apply_repo_state_micro_cache(
            &mut first,
            state,
            134,
            &exactness,
            true,
            &mut first_stats,
        ));
        assert_eq!(first["instructions"].as_str(), Some(instructions));
        assert_eq!(first_stats.repo_state_facts, 0);

        let mut second = serde_json::json!({ "instructions": instructions });
        let mut second_stats = RuntimeSmartContextTransformStats::default();
        assert!(runtime_smart_context_apply_repo_state_micro_cache(
            &mut second,
            state,
            135,
            &exactness,
            true,
            &mut second_stats,
        ));
        let rewritten = second["instructions"].as_str().unwrap();
        assert!(rewritten.contains("Keep affinity exact."));
        assert!(rewritten.contains("psc repo repeat"));
        assert!(rewritten.contains("branch=feature/repo-cache"));
        assert!(!rewritten.contains("Dirty files:"));
        assert!(!rewritten.contains("crates/prodex-app/src/runtime_proxy/smart_context.rs"));
        assert_eq!(second_stats.repo_state_facts, 1);
    })
    .unwrap();
}

#[test]
fn smart_context_reuses_existing_tool_output_artifact_with_short_ref() {
    let output = "repeat tool output ".repeat(200);
    let mut store = RuntimeSmartContextArtifactStore::default();
    let artifact = store.insert_text(1, &output).unwrap();
    let mut value = serde_json::json!({
        "input": [{
            "type": "function_call_output",
            "call_id": "call_repeat",
            "output": output
        }]
    });
    let mut stats = RuntimeSmartContextTransformStats::default();

    runtime_smart_context_condense_tool_outputs(
        &mut value,
        &mut store,
        2,
        runtime_proxy_crate::SmartContextTokenBudgetTier::Minimal,
        256,
        &RuntimeSmartContextIntentSignals::default(),
        &mut stats,
    );

    let rewritten = value["input"][0]["output"].as_str().unwrap();
    assert_eq!(rewritten, runtime_smart_context_artifact_ref(&artifact.id));
    assert!(!rewritten.contains("repeat tool output repeat tool output"));
    assert_eq!(stats.artifacts_stored, 0);
    assert_eq!(stats.repeat_tool_output_refs, 1);
    assert_eq!(stats.tool_outputs_condensed, 1);

    let mut rehydrate_stats = RuntimeSmartContextTransformStats::default();
    runtime_smart_context_rehydrate_value(&mut value, &store, &mut rehydrate_stats);
    assert_eq!(value["input"][0]["output"].as_str(), Some(output.as_str()));
    assert_eq!(rehydrate_stats.rehydrated_refs, 1);
}

#[test]
fn smart_context_reuses_existing_critical_tool_output_with_cache_summary() {
    let output = "error: repeated failure\nsrc/lib.rs:10:5\n".repeat(160);
    let mut store = RuntimeSmartContextArtifactStore::default();
    let artifact = store.insert_text(1, &output).unwrap();
    let mut value = serde_json::json!({
        "input": [{
            "type": "function_call_output",
            "call_id": "call_repeat",
            "output": output
        }]
    });
    let mut stats = RuntimeSmartContextTransformStats::default();

    runtime_smart_context_condense_tool_outputs(
        &mut value,
        &mut store,
        2,
        runtime_proxy_crate::SmartContextTokenBudgetTier::Minimal,
        256,
        &RuntimeSmartContextIntentSignals::default(),
        &mut stats,
    );

    let rewritten = value["input"][0]["output"].as_str().unwrap();
    assert!(rewritten.starts_with("psc co same"));
    assert!(rewritten.contains("id=call_repeat"));
    assert!(rewritten.contains(&runtime_smart_context_artifact_ref(&artifact.id)));
    assert!(rewritten.contains("sig: error: repeated failure"));
    assert!(!rewritten.contains("error: repeated failure\nsrc/lib.rs:10:5\nerror:"));
    assert_eq!(stats.artifacts_stored, 0);
    assert_eq!(stats.repeat_tool_output_refs, 1);
    assert_eq!(stats.tool_outputs_condensed, 1);

    let mut rehydrate_stats = RuntimeSmartContextTransformStats::default();
    runtime_smart_context_rehydrate_value(&mut value, &store, &mut rehydrate_stats);
    assert_eq!(value["input"][0]["output"].as_str(), Some(output.as_str()));
    assert_eq!(rehydrate_stats.rehydrated_refs, 1);
}

#[test]
fn smart_context_reuses_hash_guarded_persisted_artifact_after_restart() {
    let shared = smart_context_test_shared("artifact-persist-reuse");
    let artifact_path = shared
        .log_path
        .with_file_name("artifact-persist-reuse.json");
    let _ = std::fs::remove_file(&artifact_path);
    let output = "persisted exact tool output ".repeat(160);
    let mut persisted = RuntimeSmartContextArtifactStore::default();
    let artifact = persisted.insert_text(1, &output).unwrap();
    persisted.save_to_path(&artifact_path).unwrap();
    register_runtime_smart_context_proxy_state(
        &shared.log_path,
        true,
        None,
        Some(artifact_path.clone()),
    );
    let mut value = serde_json::json!({
        "input": [{
            "type": "function_call_output",
            "call_id": "call_repeat",
            "output": output
        }]
    });
    let mut stats = RuntimeSmartContextTransformStats::default();

    with_runtime_smart_context_artifacts(&shared, |store| {
        runtime_smart_context_condense_tool_outputs(
            &mut value,
            store,
            2,
            runtime_proxy_crate::SmartContextTokenBudgetTier::Minimal,
            256,
            &RuntimeSmartContextIntentSignals::default(),
            &mut stats,
        );
    })
    .unwrap();

    assert_eq!(
        value["input"][0]["output"].as_str(),
        Some(runtime_smart_context_artifact_ref(&artifact.id).as_str())
    );
    assert_eq!(stats.artifacts_stored, 0);
    assert_eq!(stats.repeat_tool_output_refs, 1);

    let _ = std::fs::remove_file(&artifact_path);
    let _ = std::fs::remove_file(crate::runtime_store::json_lock_file_path(&artifact_path));
}

#[test]
fn smart_context_persists_prewarmed_repo_and_symbol_maps_for_condensed_outputs() {
    let shared = smart_context_test_shared("artifact-map-prewarm");
    let artifact_path = shared.log_path.with_file_name("artifact-map-prewarm.json");
    let _ = std::fs::remove_file(&artifact_path);
    register_runtime_smart_context_proxy_state(
        &shared.log_path,
        true,
        None,
        Some(artifact_path.clone()),
    );
    let hidden_tail = "FULL_ARTIFACT_TAIL_SHOULD_NOT_BE_SENT";
    let output = std::iter::once("error[E0425]: cannot find value `missing`".to_string())
        .chain(std::iter::once(" --> src/runtime.rs:7:3".to_string()))
        .chain(std::iter::once("pub mod broker {".to_string()))
        .chain(std::iter::once("}".to_string()))
        .chain(std::iter::once("fn launch_super() {".to_string()))
        .chain(std::iter::once("}".to_string()))
        .chain((0..220).map(|index| format!("noise line {index}")))
        .chain(std::iter::once(hidden_tail.to_string()))
        .collect::<Vec<_>>()
        .join("\n");
    let mut value = serde_json::json!({
        "input": [{
            "type": "function_call_output",
            "call_id": "call_1",
            "output": output
        }]
    });
    let mut stats = RuntimeSmartContextTransformStats::default();

    with_runtime_smart_context_artifacts(&shared, |store| {
        runtime_smart_context_condense_tool_outputs(
            &mut value,
            store,
            122,
            runtime_proxy_crate::SmartContextTokenBudgetTier::Minimal,
            512,
            &RuntimeSmartContextIntentSignals::default(),
            &mut stats,
        );
    })
    .unwrap();
    persist_runtime_smart_context_artifacts(&shared);

    let body = value["input"][0]["output"].as_str().unwrap();
    assert!(body.contains("psc art psc:"));
    assert!(!body.contains(hidden_tail));
    assert_eq!(stats.artifacts_stored, 1);
    let raw = std::fs::read_to_string(&artifact_path).expect("artifact store should be persisted");
    assert!(raw.contains("repo_map_prewarm"));
    assert!(raw.contains("symbol_map_prewarm"));

    let loaded = RuntimeSmartContextArtifactStore::load_from_path(&artifact_path);
    let repo_map = loaded.repo_map_projection(64);
    let symbol_map = loaded.symbol_map_projection(64);
    assert!(repo_map.entries.iter().any(|entry| {
        entry.kind == RuntimeSmartContextArtifactRepoMapEntryKind::Path
            && entry.path.as_deref() == Some("src/runtime.rs")
    }));
    assert!(symbol_map.entries.iter().any(|entry| {
        entry.kind == RuntimeSmartContextArtifactRepoMapEntryKind::Module
            && entry.path.as_deref() == Some("src/runtime.rs")
            && entry.symbol.as_deref() == Some("broker")
    }));
    assert!(symbol_map.entries.iter().any(|entry| {
        entry.kind == RuntimeSmartContextArtifactRepoMapEntryKind::Symbol
            && entry.path.as_deref() == Some("src/runtime.rs")
            && entry.symbol.as_deref() == Some("launch_super")
    }));
    assert!(symbol_map.entries.iter().any(|entry| {
        entry.kind == RuntimeSmartContextArtifactRepoMapEntryKind::Error
            && entry.path.as_deref() == Some("src/runtime.rs")
            && entry.code.as_deref() == Some("E0425")
    }));

    let _ = std::fs::remove_file(&artifact_path);
    let _ = std::fs::remove_file(crate::runtime_store::json_lock_file_path(&artifact_path));
}
