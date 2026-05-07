use super::*;
use std::borrow::Cow;
use std::collections::BTreeSet;

#[path = "smart_context/tool_outputs.rs"]
mod tool_outputs;

#[path = "smart_context/manifest.rs"]
mod manifest;

#[path = "smart_context/rehydration.rs"]
mod rehydration;

#[test]
fn smart_context_generated_summary_uses_aliases_only_when_shorter() {
    let mut store = RuntimeSmartContextArtifactStore::default();
    let artifact = store
        .insert_text(1, "line one\nline two\nline three\nline four")
        .unwrap();
    let refs = (1usize..=10)
        .map(|line| runtime_smart_context_artifact_line_ref(&artifact.id, line.min(4), line.min(4)))
        .collect::<Vec<_>>()
        .join("\n");
    let marker = runtime_smart_context_artifact_marker_line("artifact", &artifact);
    let mut value = serde_json::json!({
        "input": [{
            "type": "function_call_output",
            "call_id": "call_1",
            "output": format!("{marker}\n{SMART_CONTEXT_LABEL_CRITICAL_EXACT}\n{refs}")
        }]
    });
    let before = value["input"][0]["output"].as_str().unwrap().len();

    assert!(runtime_smart_context_apply_artifact_aliases_to_generated_texts(&mut value));

    let output = value["input"][0]["output"].as_str().unwrap();
    assert!(output.contains("psc a @0=psc:"));
    assert!(output.contains("@0#L1-L1"));
    assert!(output.len() < before);
}

#[test]
fn smart_context_generated_summary_uses_path_aliases_only_when_shorter() {
    let repo = "/workspace/prodex";
    let mut value = serde_json::json!({
        "input": [{
            "type": "function_call_output",
            "call_id": "call_1",
            "output": format!(
                "psc manifest 4 artifacts\n{repo}/crates/prodex-app/src/runtime_proxy/smart_context.rs\n{repo}/crates/prodex-app/tests/src/runtime_proxy/smart_context.rs\n{repo}/crates/prodex-runtime-proxy/src/smart_context.rs\n{repo}/crates/prodex-runtime-mem/src/lib.rs"
            )
        }]
    });
    let before = value["input"][0]["output"].as_str().unwrap().len();

    assert!(runtime_smart_context_apply_path_aliases_to_generated_texts(
        &mut value
    ));

    let output = value["input"][0]["output"].as_str().unwrap();
    assert!(output.contains("psc p $R=/workspace/prodex"));
    assert!(output.contains("$R/crates/prodex-app/src/runtime_proxy/smart_context.rs"));
    assert!(output.len() < before);
}

#[test]
fn smart_context_prepare_aliases_existing_generated_paths_without_new_transform() {
    let shared = smart_context_test_shared("existing-generated-path-aliases");
    register_runtime_smart_context_proxy_state(&shared.log_path, true, None, None);
    let repo = "/workspace/prodex";
    let request = smart_context_test_request(serde_json::json!({
        "input": [{
            "type": "function_call_output",
            "call_id": "call_1",
            "output": format!(
                "psc m refs-only\n{repo}/crates/prodex-app/src/runtime_proxy/smart_context.rs\n{repo}/crates/prodex-app/tests/src/runtime_proxy/smart_context.rs\n{repo}/crates/prodex-runtime-proxy/src/smart_context.rs\n{repo}/crates/prodex-runtime-mem/src/lib.rs"
            )
        }]
    }));
    let before_len = request.body.len();

    let prepared = prepare_runtime_smart_context_http_body(
        135,
        &request,
        &shared,
        RuntimeRouteKind::Responses,
    );

    let Cow::Owned(body) = prepared else {
        panic!("expected generated paths to be aliased");
    };
    let output = serde_json::from_slice::<serde_json::Value>(&body).unwrap()["input"][0]["output"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(body.len() < before_len);
    assert!(output.contains("psc p $R=/workspace/prodex"));
    assert!(output.contains("$R/crates/prodex-app/src/runtime_proxy/smart_context.rs"));
    assert!(!output.contains("/workspace/prodex/crates/prodex-app/src/runtime_proxy"));
}

#[test]
fn smart_context_generated_summary_dedupes_existing_alias_legend() {
    let mut store = RuntimeSmartContextArtifactStore::default();
    let artifact = store
        .insert_text(1, "line one\nline two\nline three\nline four")
        .unwrap();
    let ref_line = runtime_smart_context_artifact_line_ref(&artifact.id, 1, 1);
    let mut value = serde_json::json!({
        "input": [{
            "type": "function_call_output",
            "call_id": "call_1",
            "output": format!(
                "psc art {}\npsc a @0={}\n{ref_line}\n{ref_line}\n{ref_line}",
                runtime_smart_context_artifact_ref(&artifact.id),
                runtime_smart_context_artifact_ref(&artifact.id),
            )
        }]
    });

    assert!(runtime_smart_context_apply_artifact_aliases_to_generated_texts(&mut value));

    let output = value["input"][0]["output"].as_str().unwrap();
    assert_eq!(output.matches("psc a ").count(), 1);
    assert!(output.contains("@0#L1-L1"));
}

#[test]
fn smart_context_generated_summary_keeps_stateful_artifact_alias_stable() {
    let shared = smart_context_test_shared("stable-artifact-alias");
    register_runtime_smart_context_proxy_state(&shared.log_path, true, None, None);
    let mut store = RuntimeSmartContextArtifactStore::default();
    let first_artifact = store
        .insert_text(1, "line one\nline two\nline three\nline four")
        .unwrap();
    let second_artifact = store.insert_text(2, "alpha\nbeta\ngamma\ndelta").unwrap();
    let first_refs = (1usize..=8)
        .map(|line| {
            runtime_smart_context_artifact_line_ref(&first_artifact.id, line.min(4), line.min(4))
        })
        .collect::<Vec<_>>()
        .join("\n");
    let second_refs = (1usize..=8)
        .map(|line| {
            runtime_smart_context_artifact_line_ref(&second_artifact.id, line.min(4), line.min(4))
        })
        .collect::<Vec<_>>()
        .join("\n");

    with_runtime_smart_context_proxy_state(&shared, |state| {
        let mut first = serde_json::json!({
            "input": [{"type": "function_call_output", "output": format!("psc m refs-only\n{first_refs}")}]
        });
        assert!(runtime_smart_context_apply_artifact_aliases_to_generated_texts_with_state(
            &mut first,
            state,
        ));
        assert!(
            first["input"][0]["output"]
                .as_str()
                .unwrap()
                .contains("psc a @0=psc:")
        );

        let mut second = serde_json::json!({
            "input": [{"type": "function_call_output", "output": format!("psc m refs-only\n{second_refs}\n{first_refs}")}]
        });
        assert!(runtime_smart_context_apply_artifact_aliases_to_generated_texts_with_state(
            &mut second,
            state,
        ));
        let output = second["input"][0]["output"].as_str().unwrap();
        assert!(output.contains(&format!(
            "@0={}",
            runtime_smart_context_artifact_ref(&first_artifact.id)
        )));
        assert!(output.contains("@0#L1-L1"));
    })
    .unwrap();
}

#[test]
fn smart_context_persists_stateful_artifact_aliases_across_start() {
    let first_shared = smart_context_test_shared("stable-artifact-alias-persist-first");
    let artifact_path = first_shared
        .log_path
        .with_file_name("stable-artifact-alias-persist-artifacts.json");
    let calibration_path = runtime_smart_context_token_calibration_path(&artifact_path);
    let _ = std::fs::remove_file(&artifact_path);
    let _ = std::fs::remove_file(&calibration_path);
    register_runtime_smart_context_proxy_state(
        &first_shared.log_path,
        true,
        None,
        Some(artifact_path.clone()),
    );
    let mut store = RuntimeSmartContextArtifactStore::default();
    let artifact = store
        .insert_text(1, "line one\nline two\nline three\nline four")
        .unwrap();
    let refs = (1usize..=8)
        .map(|line| runtime_smart_context_artifact_line_ref(&artifact.id, line.min(4), line.min(4)))
        .collect::<Vec<_>>()
        .join("\n");
    with_runtime_smart_context_proxy_state(&first_shared, |state| {
        let mut value = serde_json::json!({
            "input": [{"type": "function_call_output", "output": format!("psc m refs-only\n{refs}")}]
        });
        assert!(runtime_smart_context_apply_artifact_aliases_to_generated_texts_with_state(
            &mut value,
            state,
        ));
    })
    .unwrap();
    persist_runtime_smart_context_token_calibration_metadata(
        &first_shared,
        "smart_context_artifact_aliases",
    );

    let fresh_shared = smart_context_test_shared("stable-artifact-alias-persist-fresh");
    register_runtime_smart_context_proxy_state(
        &fresh_shared.log_path,
        true,
        None,
        Some(artifact_path.clone()),
    );
    with_runtime_smart_context_proxy_state(&fresh_shared, |state| {
        let mut value = serde_json::json!({
            "input": [{"type": "function_call_output", "output": format!("psc m refs-only\n{refs}")}]
        });
        assert!(runtime_smart_context_apply_artifact_aliases_to_generated_texts_with_state(
            &mut value,
            state,
        ));
        let output = value["input"][0]["output"].as_str().unwrap();
        assert!(output.contains(&format!(
            "psc a @0={}",
            runtime_smart_context_artifact_ref(&artifact.id)
        )));
        assert!(output.contains("@0#L1-L1"));
    })
    .unwrap();

    let raw = std::fs::read_to_string(&calibration_path).unwrap();
    assert!(raw.contains("\"artifact_aliases\""));
    assert!(!raw.contains("line one"));
    let _ = std::fs::remove_file(&artifact_path);
    let _ = std::fs::remove_file(&calibration_path);
    let _ = std::fs::remove_file(crate::runtime_store::json_lock_file_path(&artifact_path));
    let _ = std::fs::remove_file(crate::runtime_store::json_lock_file_path(&calibration_path));
}

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
fn smart_context_selective_rehydrate_semantic_terms_append_exact_ranges_only() {
    let artifact_text = "\
diff --git a/src/diff.rs b/src/diff.rs
--- a/src/diff.rs
+++ b/src/diff.rs
@@ -10,2 +10,3 @@ fn changed()
-old diff line
+new diff line
noise before diagnostic
error[E0425]: cannot find value `missing` in this scope
src/lib.rs:42:13
---- runtime_proxy::semantic_rehydrate stdout ----
thread 'runtime_proxy::semantic_rehydrate' panicked at 'boom', crates/prodex-app/tests/src/runtime_proxy/smart_context.rs:12:5
test result: FAILED. 0 passed; 1 failed
unrelated full artifact tail";
    let mut store = RuntimeSmartContextArtifactStore::default();
    let artifact = store.insert_text(1, artifact_text).unwrap();
    let mut value = serde_json::json!({
        "input": [{
            "type": "message",
            "content": format!("summary prodex-artifact:{}", artifact.id)
        }]
    });
    let mut stats = RuntimeSmartContextTransformStats::default();

    let count = runtime_smart_context_selective_rehydrate_semantic_ranges(
        &mut value,
        &store,
        &runtime_proxy_crate::smart_context_exactness_guard(
            runtime_proxy_crate::SmartContextExactnessInput::default(),
        ),
        &RuntimeSmartContextSelectiveRehydrateTerms {
            file_paths: BTreeSet::from(["src/lib.rs".to_string()]),
            error_codes: BTreeSet::from(["E0425".to_string()]),
            test_symbols: BTreeSet::from(["runtime_proxy::semantic_rehydrate".to_string()]),
            command_kinds: BTreeSet::new(),
            diff_hunks: Vec::new(),
        },
        &mut stats,
    );

    let content = value["input"][0]["content"].as_str().unwrap();
    assert_eq!(count, stats.rehydrated_refs);
    assert!(count >= 3);
    assert!(content.contains(SMART_CONTEXT_LABEL_SEMANTIC_EXACT));
    assert!(content.contains(&format!(
        "{}#L8-L",
        runtime_smart_context_artifact_ref(&artifact.id)
    )));
    assert!(content.contains("error[E0425]: cannot find value `missing` in this scope"));
    assert!(content.contains("src/lib.rs:42:13"));
    assert!(content.contains("runtime_proxy::semantic_rehydrate"));
    assert!(!content.contains("diff --git a/src/diff.rs b/src/diff.rs"));
    assert!(!content.contains("unrelated full artifact tail"));
}

#[test]
fn smart_context_selective_rehydrate_semantic_diff_hunk_term() {
    let artifact_text = "\
diff --git a/src/diff.rs b/src/diff.rs
--- a/src/diff.rs
+++ b/src/diff.rs
@@ -10,2 +10,3 @@ fn changed()
-old diff line
+new diff line
unrelated tail";
    let mut store = RuntimeSmartContextArtifactStore::default();
    let artifact = store.insert_text(1, artifact_text).unwrap();
    let mut value = serde_json::json!({
        "input": [{
            "type": "message",
            "content": format!("summary prodex-artifact:{}", artifact.id)
        }]
    });
    let mut stats = RuntimeSmartContextTransformStats::default();

    let count = runtime_smart_context_selective_rehydrate_semantic_ranges(
        &mut value,
        &store,
        &runtime_proxy_crate::smart_context_exactness_guard(
            runtime_proxy_crate::SmartContextExactnessInput::default(),
        ),
        &RuntimeSmartContextSelectiveRehydrateTerms {
            diff_hunks: vec![RuntimeSmartContextSelectiveDiffHunkTerm {
                path: Some("src/diff.rs".to_string()),
                old_start: Some(10),
                new_start: Some(10),
            }],
            ..RuntimeSmartContextSelectiveRehydrateTerms::default()
        },
        &mut stats,
    );

    let content = value["input"][0]["content"].as_str().unwrap();
    assert_eq!(count, 1);
    assert_eq!(stats.rehydrated_refs, 1);
    assert!(content.contains(&runtime_smart_context_artifact_line_ref(&artifact.id, 4, 6)));
    assert!(content.contains("@@ -10,2 +10,3 @@ fn changed()"));
    assert!(content.contains("-old diff line"));
    assert!(content.contains("+new diff line"));
    assert!(!content.contains("unrelated tail"));
}

#[test]
fn smart_context_selective_rehydrate_matches_intent_suffixes_and_status_codes() {
    let artifact_text = "\
running 1 test
---- runtime_proxy::metadata_hint stdout ----
thread 'runtime_proxy::metadata_hint' panicked at crates/prodex-app/src/runtime_proxy/smart_context.rs:99:1
process exited with exit code 101
crates/prodex-app/src/runtime_proxy/smart_context.rs:99:1
unrelated tail";
    let mut store = RuntimeSmartContextArtifactStore::default();
    let artifact = store.insert_text(1, artifact_text).unwrap();
    let mut value = serde_json::json!({
        "input": [
            {
                "type": "message",
                "role": "user",
                "content": "cargo test metadata_hint smart_context.rs exit code 101"
            },
            {
                "type": "message",
                "content": format!("summary {}", runtime_smart_context_artifact_ref(&artifact.id))
            }
        ]
    });
    let signals = runtime_smart_context_collect_intent_signals(&value);
    let mut stats = RuntimeSmartContextTransformStats::default();

    let count = runtime_smart_context_selective_rehydrate_semantic_ranges(
        &mut value,
        &store,
        &runtime_proxy_crate::smart_context_exactness_guard(
            runtime_proxy_crate::SmartContextExactnessInput::default(),
        ),
        &signals.semantic_terms,
        &mut stats,
    );

    let content = value["input"][1]["content"].as_str().unwrap();
    assert!(count >= 3);
    assert_eq!(count, stats.rehydrated_refs);
    assert!(content.contains("runtime_proxy::metadata_hint"));
    assert!(content.contains("process exited with exit code 101"));
    assert!(content.contains("crates/prodex-app/src/runtime_proxy/smart_context.rs:99:1"));
    assert!(!content.contains("unrelated tail"));
}

#[test]
fn smart_context_selective_rehydrate_skips_command_only_weak_signal() {
    let artifact_text = "\
running 1 test
---- runtime_proxy::metadata_hint stdout ----
thread 'runtime_proxy::metadata_hint' panicked at src/lib.rs:12:1
test result: FAILED. 0 passed; 1 failed";
    let mut store = RuntimeSmartContextArtifactStore::default();
    let artifact = store.insert_text(1, artifact_text).unwrap();
    let original = format!(
        "summary {}",
        runtime_smart_context_artifact_ref(&artifact.id)
    );
    let mut value = serde_json::json!({
        "input": [{
            "type": "message",
            "content": original
        }]
    });
    let plan = runtime_smart_context_auto_rehydrate_plan(
        &value,
        &store,
        1,
        runtime_proxy_crate::SmartContextTokenBudgetTier::Minimal,
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
            command_kinds: BTreeSet::from(["cargo-test".to_string()]),
            ..RuntimeSmartContextSelectiveRehydrateTerms::default()
        },
        &plan,
        1_000,
        &mut stats,
    );

    assert_eq!(count, 0);
    assert_eq!(stats.rehydrated_refs, 0);
    assert_eq!(
        value["input"][0]["content"].as_str(),
        Some(original.as_str())
    );
}

#[test]
fn smart_context_selective_rehydrate_semantic_terms_cap_narrow_matches() {
    let artifact_text = (0..16)
        .map(|index| format!("error[E0001]: repeated failure {index}"))
        .collect::<Vec<_>>()
        .join("\n");
    let mut store = RuntimeSmartContextArtifactStore::default();
    let artifact = store.insert_text(1, &artifact_text).unwrap();
    let mut value = serde_json::json!({
        "input": [{
            "type": "message",
            "content": format!("summary prodex-artifact:{}", artifact.id)
        }]
    });
    let mut stats = RuntimeSmartContextTransformStats::default();

    let count = runtime_smart_context_selective_rehydrate_semantic_ranges(
        &mut value,
        &store,
        &runtime_proxy_crate::smart_context_exactness_guard(
            runtime_proxy_crate::SmartContextExactnessInput::default(),
        ),
        &RuntimeSmartContextSelectiveRehydrateTerms {
            error_codes: BTreeSet::from(["E0001".to_string()]),
            ..RuntimeSmartContextSelectiveRehydrateTerms::default()
        },
        &mut stats,
    );

    let content = value["input"][0]["content"].as_str().unwrap();
    assert_eq!(count, SMART_CONTEXT_SEMANTIC_REHYDRATE_NARROW_MAX_RANGES);
    assert_eq!(
        stats.rehydrated_refs,
        SMART_CONTEXT_SEMANTIC_REHYDRATE_NARROW_MAX_RANGES
    );
    assert!(content.contains("repeated failure 0"));
    assert!(content.contains("repeated failure 3"));
    assert!(!content.contains("repeated failure 4"));
}

#[test]
fn smart_context_selective_rehydrate_semantic_terms_cap_broad_matches() {
    let artifact_text = (0..20)
        .map(|index| {
            let code = if index % 2 == 0 { "E0001" } else { "E0002" };
            format!("error[{code}]: repeated failure {index}")
        })
        .collect::<Vec<_>>()
        .join("\n");
    let mut store = RuntimeSmartContextArtifactStore::default();
    let artifact = store.insert_text(1, &artifact_text).unwrap();
    let mut value = serde_json::json!({
        "input": [{
            "type": "message",
            "content": format!("summary prodex-artifact:{}", artifact.id)
        }]
    });
    let mut stats = RuntimeSmartContextTransformStats::default();

    let count = runtime_smart_context_selective_rehydrate_semantic_ranges(
        &mut value,
        &store,
        &runtime_proxy_crate::smart_context_exactness_guard(
            runtime_proxy_crate::SmartContextExactnessInput::default(),
        ),
        &RuntimeSmartContextSelectiveRehydrateTerms {
            error_codes: BTreeSet::from(["E0001".to_string(), "E0002".to_string()]),
            ..RuntimeSmartContextSelectiveRehydrateTerms::default()
        },
        &mut stats,
    );

    let content = value["input"][0]["content"].as_str().unwrap();
    assert_eq!(count, SMART_CONTEXT_SEMANTIC_REHYDRATE_GLOBAL_MAX_RANGES);
    assert_eq!(
        stats.rehydrated_refs,
        SMART_CONTEXT_SEMANTIC_REHYDRATE_GLOBAL_MAX_RANGES
    );
    assert!(content.contains("repeated failure 11"));
    assert!(!content.contains("repeated failure 12"));
}

#[test]
fn smart_context_selective_rehydrate_semantic_terms_respect_exactness_guard() {
    let artifact_text = "error[E0425]: hidden\nsrc/lib.rs:42:13";
    let mut store = RuntimeSmartContextArtifactStore::default();
    let artifact = store.insert_text(1, artifact_text).unwrap();
    let original = format!("summary prodex-artifact:{}", artifact.id);
    let mut value = serde_json::json!({
        "input": [{
            "type": "message",
            "content": original
        }]
    });
    let mut stats = RuntimeSmartContextTransformStats::default();

    let count = runtime_smart_context_selective_rehydrate_semantic_ranges(
        &mut value,
        &store,
        &runtime_proxy_crate::smart_context_exactness_guard(
            runtime_proxy_crate::SmartContextExactnessInput {
                previous_response_id: Some("resp_1".to_string()),
                ..runtime_proxy_crate::SmartContextExactnessInput::default()
            },
        ),
        &RuntimeSmartContextSelectiveRehydrateTerms {
            error_codes: BTreeSet::from(["E0425".to_string()]),
            ..RuntimeSmartContextSelectiveRehydrateTerms::default()
        },
        &mut stats,
    );

    assert_eq!(count, 0);
    assert_eq!(stats.rehydrated_refs, 0);
    assert_eq!(
        value["input"][0]["content"].as_str(),
        Some(original.as_str())
    );
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

#[test]
fn smart_context_budget_uses_runtime_token_usage_observation() {
    let shared = smart_context_test_shared("budget");
    register_runtime_smart_context_proxy_state(&shared.log_path, true, None, None);
    observe_runtime_smart_context_token_usage(
        &shared,
        RuntimeTokenUsage {
            input_tokens: 24_000,
            cached_input_tokens: 0,
            output_tokens: 7_000,
            reasoning_tokens: 1_000,
        },
    );

    let budget = runtime_smart_context_budget(
        &shared,
        b"small current request body payload",
        RuntimeRouteKind::Responses,
        RuntimeSmartContextTransport::Http,
        None,
        runtime_proxy_crate::smart_context_exactness_guard(
            runtime_proxy_crate::SmartContextExactnessInput::default(),
        ),
        Vec::new(),
        false,
    );

    assert_eq!(
        budget.tier,
        runtime_proxy_crate::SmartContextTokenBudgetTier::Minimal
    );
    assert_eq!(budget.observed_context_tokens, Some(32_000));
    assert_eq!(budget.token_usage_source, "runtime_usage");
    assert_eq!(budget.model_context_window_tokens, 32_000);
    assert_eq!(budget.model_context_window_source, "fallback");
}

#[test]
fn smart_context_budget_uses_configured_model_context_window() {
    let shared = smart_context_test_shared("budget-custom-window");
    register_runtime_smart_context_proxy_state(&shared.log_path, true, Some(64_000), None);
    observe_runtime_smart_context_token_usage(
        &shared,
        RuntimeTokenUsage {
            input_tokens: 24_000,
            cached_input_tokens: 0,
            output_tokens: 7_000,
            reasoning_tokens: 1_000,
        },
    );

    let budget = runtime_smart_context_budget(
        &shared,
        b"small current request body payload",
        RuntimeRouteKind::Responses,
        RuntimeSmartContextTransport::Http,
        None,
        runtime_proxy_crate::smart_context_exactness_guard(
            runtime_proxy_crate::SmartContextExactnessInput::default(),
        ),
        Vec::new(),
        false,
    );

    assert_eq!(
        budget.tier,
        runtime_proxy_crate::SmartContextTokenBudgetTier::Exact
    );
    assert_eq!(budget.model_context_window_tokens, 64_000);
    assert_eq!(budget.model_context_window_source, "launch_config");
    assert_eq!(budget.observed_context_tokens, Some(32_000));
}

#[test]
fn smart_context_budget_uses_matching_token_calibration_bucket() {
    let shared = smart_context_test_shared("budget-bucket");
    register_runtime_smart_context_proxy_state(&shared.log_path, true, Some(64_000), None);
    observe_runtime_smart_context_token_usage_for_bucket(
        &shared,
        RuntimeTokenUsage {
            input_tokens: 48_000,
            ..RuntimeTokenUsage::default()
        },
        Some(runtime_smart_context_token_calibration_bucket_key(
            RuntimeRouteKind::Responses,
            RuntimeSmartContextTransport::Http,
            Some("alpha"),
        )),
    );
    observe_runtime_smart_context_token_usage_for_bucket(
        &shared,
        RuntimeTokenUsage {
            input_tokens: 56_000,
            ..RuntimeTokenUsage::default()
        },
        Some(runtime_smart_context_token_calibration_bucket_key(
            RuntimeRouteKind::Websocket,
            RuntimeSmartContextTransport::Websocket,
            Some("beta"),
        )),
    );

    let alpha = runtime_smart_context_budget(
        &shared,
        b"small current request body payload",
        RuntimeRouteKind::Responses,
        RuntimeSmartContextTransport::Http,
        Some("alpha"),
        runtime_proxy_crate::smart_context_exactness_guard(
            runtime_proxy_crate::SmartContextExactnessInput::default(),
        ),
        Vec::new(),
        false,
    );
    let beta = runtime_smart_context_budget(
        &shared,
        b"small current request body payload",
        RuntimeRouteKind::Websocket,
        RuntimeSmartContextTransport::Websocket,
        Some("beta"),
        runtime_proxy_crate::smart_context_exactness_guard(
            runtime_proxy_crate::SmartContextExactnessInput::default(),
        ),
        Vec::new(),
        false,
    );

    assert_eq!(
        alpha.tier,
        runtime_proxy_crate::SmartContextTokenBudgetTier::Large
    );
    assert_eq!(alpha.observed_context_tokens, Some(48_000));
    assert_eq!(
        beta.tier,
        runtime_proxy_crate::SmartContextTokenBudgetTier::Condensed
    );
    assert_eq!(beta.observed_context_tokens, Some(56_000));
}

#[test]
fn smart_context_budget_uses_model_specific_token_calibration_bucket() {
    let shared = smart_context_test_shared("budget-model-bucket");
    register_runtime_smart_context_proxy_state(&shared.log_path, true, Some(64_000), None);
    observe_runtime_smart_context_token_usage_for_bucket(
        &shared,
        RuntimeTokenUsage {
            input_tokens: 44_000,
            ..RuntimeTokenUsage::default()
        },
        Some(
            runtime_smart_context_token_calibration_bucket_key_with_model(
                RuntimeRouteKind::Responses,
                RuntimeSmartContextTransport::Http,
                Some("alpha"),
                Some("gpt-5"),
            ),
        ),
    );
    observe_runtime_smart_context_token_usage_for_bucket(
        &shared,
        RuntimeTokenUsage {
            input_tokens: 56_000,
            ..RuntimeTokenUsage::default()
        },
        Some(
            runtime_smart_context_token_calibration_bucket_key_with_model(
                RuntimeRouteKind::Responses,
                RuntimeSmartContextTransport::Http,
                Some("alpha"),
                Some("gpt-5.2"),
            ),
        ),
    );

    let gpt5 = runtime_smart_context_budget(
        &shared,
        br#"{"model":"gpt-5","input":"small current request body payload"}"#,
        RuntimeRouteKind::Responses,
        RuntimeSmartContextTransport::Http,
        Some("alpha"),
        runtime_proxy_crate::smart_context_exactness_guard(
            runtime_proxy_crate::SmartContextExactnessInput::default(),
        ),
        Vec::new(),
        false,
    );
    let gpt52 = runtime_smart_context_budget(
        &shared,
        br#"{"model":"gpt-5.2","input":"small current request body payload"}"#,
        RuntimeRouteKind::Responses,
        RuntimeSmartContextTransport::Http,
        Some("alpha"),
        runtime_proxy_crate::smart_context_exactness_guard(
            runtime_proxy_crate::SmartContextExactnessInput::default(),
        ),
        Vec::new(),
        false,
    );

    assert_eq!(gpt5.observed_context_tokens, Some(44_000));
    assert_eq!(gpt52.observed_context_tokens, Some(56_000));
    assert_eq!(
        runtime_smart_context_model_name_from_body(
            br#"{"model":"gpt-5.2","input":"small current request body payload"}"#
        )
        .as_deref(),
        Some("gpt-5.2")
    );
}

#[test]
fn smart_context_budget_expands_large_preview_after_recent_safe_rewrite() {
    let shared = smart_context_test_shared("budget-recent-safe");
    register_runtime_smart_context_proxy_state(&shared.log_path, true, Some(64_000), None);
    observe_runtime_smart_context_token_usage(
        &shared,
        RuntimeTokenUsage {
            input_tokens: 48_000,
            cached_input_tokens: 0,
            output_tokens: 0,
            reasoning_tokens: 0,
        },
    );

    let before = runtime_smart_context_budget(
        &shared,
        b"small current request body payload",
        RuntimeRouteKind::Responses,
        RuntimeSmartContextTransport::Http,
        None,
        runtime_proxy_crate::smart_context_exactness_guard(
            runtime_proxy_crate::SmartContextExactnessInput::default(),
        ),
        Vec::new(),
        false,
    );
    observe_runtime_smart_context_rewrite_safety(
        &shared,
        RuntimeSmartContextRewriteSafetyObservation {
            safe: true,
            saved_tokens: runtime_proxy_crate::SMART_CONTEXT_RECENT_SAFE_REWRITE_MIN_SAVED_TOKENS,
        },
    );
    let after = runtime_smart_context_budget(
        &shared,
        b"small current request body payload",
        RuntimeRouteKind::Responses,
        RuntimeSmartContextTransport::Http,
        None,
        runtime_proxy_crate::smart_context_exactness_guard(
            runtime_proxy_crate::SmartContextExactnessInput::default(),
        ),
        Vec::new(),
        false,
    );

    assert_eq!(
        before.tier,
        runtime_proxy_crate::SmartContextTokenBudgetTier::Large
    );
    assert_eq!(before.policy.max_inline_tool_output_bytes, 32 * 1024);
    assert_eq!(after.policy.max_inline_tool_output_bytes, 64 * 1024);
    assert!(
        after.policy.reasons.contains(
            &runtime_proxy_crate::SmartContextBudgetPolicyReason::RecentRewriteSavingsSafe
        )
    );
}

#[test]
fn smart_context_budget_loads_persisted_recent_safe_rewrite() {
    let first_shared = smart_context_test_shared("budget-persisted-recent-safe");
    let artifact_path = first_shared
        .log_path
        .with_file_name("budget-persisted-recent-safe-artifacts.json");
    let calibration_path = runtime_smart_context_token_calibration_path(&artifact_path);
    let _ = std::fs::remove_file(&artifact_path);
    let _ = std::fs::remove_file(&calibration_path);
    register_runtime_smart_context_proxy_state(
        &first_shared.log_path,
        true,
        Some(64_000),
        Some(artifact_path.clone()),
    );
    observe_runtime_smart_context_token_usage(
        &first_shared,
        RuntimeTokenUsage {
            input_tokens: 48_000,
            cached_input_tokens: 0,
            output_tokens: 0,
            reasoning_tokens: 0,
        },
    );
    observe_runtime_smart_context_rewrite_safety(
        &first_shared,
        RuntimeSmartContextRewriteSafetyObservation {
            safe: true,
            saved_tokens: runtime_proxy_crate::SMART_CONTEXT_RECENT_SAFE_REWRITE_MIN_SAVED_TOKENS,
        },
    );

    let fresh_shared = smart_context_test_shared("budget-persisted-recent-safe-fresh");
    register_runtime_smart_context_proxy_state(
        &fresh_shared.log_path,
        true,
        Some(64_000),
        Some(artifact_path.clone()),
    );
    let budget = runtime_smart_context_budget(
        &fresh_shared,
        b"small current request body payload",
        RuntimeRouteKind::Responses,
        RuntimeSmartContextTransport::Http,
        None,
        runtime_proxy_crate::smart_context_exactness_guard(
            runtime_proxy_crate::SmartContextExactnessInput::default(),
        ),
        Vec::new(),
        false,
    );

    assert_eq!(
        budget.tier,
        runtime_proxy_crate::SmartContextTokenBudgetTier::Large
    );
    assert_eq!(budget.policy.max_inline_tool_output_bytes, 64 * 1024);
    assert!(
        budget.policy.reasons.contains(
            &runtime_proxy_crate::SmartContextBudgetPolicyReason::RecentRewriteSavingsSafe
        )
    );

    let _ = std::fs::remove_file(&artifact_path);
    let _ = std::fs::remove_file(&calibration_path);
    let _ = std::fs::remove_file(crate::runtime_store::json_lock_file_path(&artifact_path));
}

#[test]
fn smart_context_budget_relaxes_from_safe_saving_telemetry_ring() {
    let shared = smart_context_test_shared("budget-telemetry-relax");
    register_runtime_smart_context_proxy_state(&shared.log_path, true, Some(64_000), None);
    observe_runtime_smart_context_token_usage(
        &shared,
        RuntimeTokenUsage {
            input_tokens: 48_000,
            ..RuntimeTokenUsage::default()
        },
    );
    with_runtime_smart_context_proxy_state(&shared, |state| {
        state
            .rewrite_telemetry_history
            .push(RuntimeSmartContextRewriteTelemetryRecord {
                body_bytes_before: 8_000,
                body_bytes_after: 3_000,
                estimated_tokens_before: 2_000,
                estimated_tokens_after: 750,
                rewrite_kind: "rewritten".to_string(),
                status: "ok_saved".to_string(),
                fallback_reason: None,
            });
        state
            .rewrite_telemetry_history
            .push(RuntimeSmartContextRewriteTelemetryRecord {
                body_bytes_before: 7_000,
                body_bytes_after: 2_800,
                estimated_tokens_before: 1_750,
                estimated_tokens_after: 700,
                rewrite_kind: "rewritten".to_string(),
                status: "ok_saved".to_string(),
                fallback_reason: None,
            });
        state
            .rewrite_telemetry_history
            .push(RuntimeSmartContextRewriteTelemetryRecord {
                body_bytes_before: 100,
                body_bytes_after: 100,
                estimated_tokens_before: 25,
                estimated_tokens_after: 25,
                rewrite_kind: "pass_through".to_string(),
                status: "noop".to_string(),
                fallback_reason: None,
            });
    })
    .unwrap();

    let budget = runtime_smart_context_budget(
        &shared,
        b"small current request body payload",
        RuntimeRouteKind::Responses,
        RuntimeSmartContextTransport::Http,
        None,
        runtime_proxy_crate::smart_context_exactness_guard(
            runtime_proxy_crate::SmartContextExactnessInput::default(),
        ),
        Vec::new(),
        false,
    );

    assert_eq!(
        budget.tier,
        runtime_proxy_crate::SmartContextTokenBudgetTier::Large
    );
    assert_eq!(budget.policy.max_inline_tool_output_bytes, 64 * 1024);
    assert_eq!(budget.policy.max_rehydrate_tokens, 11_904);
}

#[test]
fn smart_context_budget_tightens_for_marginal_or_fallback_telemetry() {
    let shared = smart_context_test_shared("budget-telemetry-tighten");
    register_runtime_smart_context_proxy_state(&shared.log_path, true, Some(64_000), None);
    observe_runtime_smart_context_token_usage(
        &shared,
        RuntimeTokenUsage {
            input_tokens: 48_000,
            ..RuntimeTokenUsage::default()
        },
    );
    with_runtime_smart_context_proxy_state(&shared, |state| {
        state
            .rewrite_telemetry_history
            .push(RuntimeSmartContextRewriteTelemetryRecord {
                body_bytes_before: 10_000,
                body_bytes_after: 9_000,
                estimated_tokens_before: 2_500,
                estimated_tokens_after: 2_250,
                rewrite_kind: "rewritten".to_string(),
                status: "ok_saved".to_string(),
                fallback_reason: None,
            });
        state
            .rewrite_telemetry_history
            .push(RuntimeSmartContextRewriteTelemetryRecord {
                body_bytes_before: 8_000,
                body_bytes_after: 7_200,
                estimated_tokens_before: 2_000,
                estimated_tokens_after: 1_800,
                rewrite_kind: "rewritten".to_string(),
                status: "ok_saved".to_string(),
                fallback_reason: None,
            });
    })
    .unwrap();

    let budget = runtime_smart_context_budget(
        &shared,
        b"small current request body payload",
        RuntimeRouteKind::Responses,
        RuntimeSmartContextTransport::Http,
        None,
        runtime_proxy_crate::smart_context_exactness_guard(
            runtime_proxy_crate::SmartContextExactnessInput::default(),
        ),
        Vec::new(),
        false,
    );
    assert_eq!(
        budget.policy.max_inline_tool_output_bytes,
        (32 * 1024) * 9 / 10
    );
    assert_eq!(budget.policy.max_rehydrate_tokens, 10_713);

    with_runtime_smart_context_proxy_state(&shared, |state| {
        state
            .rewrite_telemetry_history
            .push(RuntimeSmartContextRewriteTelemetryRecord {
                body_bytes_before: 8_000,
                body_bytes_after: 3_000,
                estimated_tokens_before: 2_000,
                estimated_tokens_after: 750,
                rewrite_kind: "self_check_passthrough".to_string(),
                status: "critical_signal_loss".to_string(),
                fallback_reason: Some("critical_signal_loss".to_string()),
            });
    })
    .unwrap();
    let fallback_budget = runtime_smart_context_budget(
        &shared,
        b"small current request body payload",
        RuntimeRouteKind::Responses,
        RuntimeSmartContextTransport::Http,
        None,
        runtime_proxy_crate::smart_context_exactness_guard(
            runtime_proxy_crate::SmartContextExactnessInput::default(),
        ),
        Vec::new(),
        false,
    );
    assert_eq!(
        fallback_budget.policy.max_inline_tool_output_bytes,
        (32 * 1024) * 9 / 10
    );
    assert_eq!(fallback_budget.policy.max_rehydrate_tokens, 10_713);
}

#[test]
fn smart_context_tool_preview_lines_follow_budget_tier_and_limit() {
    assert_eq!(
        runtime_smart_context_tool_preview_max_lines(
            runtime_proxy_crate::SmartContextTokenBudgetTier::Minimal,
            1024,
        ),
        Some(8)
    );
    assert_eq!(
        runtime_smart_context_tool_preview_max_lines(
            runtime_proxy_crate::SmartContextTokenBudgetTier::Condensed,
            8 * 1024,
        ),
        Some(32)
    );
    assert_eq!(
        runtime_smart_context_tool_preview_max_lines(
            runtime_proxy_crate::SmartContextTokenBudgetTier::Large,
            64 * 1024,
        ),
        Some(240)
    );
    assert_eq!(
        runtime_smart_context_tool_preview_max_lines(
            runtime_proxy_crate::SmartContextTokenBudgetTier::Exact,
            usize::MAX,
        ),
        None
    );
}

#[test]
fn smart_context_prepare_rewrites_when_savings_and_critical_signals_preserved() {
    let shared = smart_context_test_shared("rewrite-savings");
    register_runtime_smart_context_proxy_state(&shared.log_path, true, None, None);
    smart_context_observe_minimal_budget(&shared);
    let output = std::iter::once("error: failed at src/main.rs:10:5".to_string())
        .chain((0..500).map(|index| format!("line {index}: noisy build output")))
        .collect::<Vec<_>>()
        .join("\n");
    let request = smart_context_test_request(serde_json::json!({
        "input": [{
            "type": "function_call_output",
            "call_id": "call_1",
            "output": output
        }]
    }));
    let before_len = request.body.len();

    let rewritten =
        prepare_runtime_smart_context_http_body(42, &request, &shared, RuntimeRouteKind::Responses);

    let Cow::Owned(body) = rewritten else {
        panic!("expected rewritten body");
    };
    assert!(body.len() < before_len);
    let text = String::from_utf8(body).unwrap();
    assert!(text.contains("psc:"));
    assert!(text.contains("error: failed at src/main.rs:10:5"));
}

#[test]
fn smart_context_http_prepare_rewritten_body_remains_valid_json() {
    let shared = smart_context_test_shared("rewrite-valid-json");
    register_runtime_smart_context_proxy_state(&shared.log_path, true, None, None);
    smart_context_observe_minimal_budget(&shared);
    let output = std::iter::once("error[E0425]: missing symbol at src/lib.rs:42:13".to_string())
        .chain((0..650).map(|index| format!("line {index}: noisy build output")))
        .collect::<Vec<_>>()
        .join("\n");
    let request = smart_context_test_request(serde_json::json!({
        "model": "gpt-5.5",
        "input": [{
            "type": "function_call_output",
            "call_id": "call_json_valid",
            "output": output
        }]
    }));

    let rewritten = prepare_runtime_smart_context_http_body(
        142,
        &request,
        &shared,
        RuntimeRouteKind::Responses,
    );

    let Cow::Owned(body) = rewritten else {
        panic!("expected smart-context rewrite");
    };
    let value = serde_json::from_slice::<serde_json::Value>(&body)
        .expect("rewritten prepare body must remain valid JSON");
    assert_eq!(value["model"].as_str(), Some("gpt-5.5"));
    let output = value["input"][0]["output"].as_str().unwrap();
    assert!(output.contains("psc:"));
    assert!(output.contains("error[E0425]: missing symbol at src/lib.rs:42:13"));
}

#[test]
fn smart_context_prepare_explicit_line_ref_rehydrates_exact_critical_content() {
    let shared = smart_context_test_shared("prepare-explicit-ref-exact");
    register_runtime_smart_context_proxy_state(&shared.log_path, true, Some(32_000), None);
    let artifact_text = "\
setup line
panic: exact hidden failure
src/runtime.rs:88:13
tail line";
    let artifact = with_runtime_smart_context_artifacts(&shared, |store| {
        store.insert_text(1, artifact_text).unwrap()
    })
    .unwrap();
    let request = smart_context_test_request(serde_json::json!({
        "input": [{
            "type": "message",
            "content": format!("inspect {}", runtime_smart_context_artifact_line_ref(&artifact.id, 2, 3))
        }]
    }));

    let prepared = prepare_runtime_smart_context_http_body(
        143,
        &request,
        &shared,
        RuntimeRouteKind::Responses,
    );

    let value = serde_json::from_slice::<serde_json::Value>(prepared.as_ref()).unwrap();
    assert_eq!(
        value["input"][0]["content"].as_str(),
        Some("inspect panic: exact hidden failure\nsrc/runtime.rs:88:13")
    );
}

#[test]
fn smart_context_prepare_affinity_exactness_minifies_without_unsafe_rewrite() {
    for (name, request) in [
        (
            "previous",
            RuntimeProxyRequest {
                method: "POST".to_string(),
                path_and_query: "/backend-api/codex/v1/responses".to_string(),
                headers: Vec::new(),
                body: serde_json::to_vec_pretty(&serde_json::json!({
                    "previous_response_id": "resp_owned",
                    "input": [{
                        "type": "function_call_output",
                        "call_id": "call_previous",
                        "output": "exact previous-response output\n".repeat(160)
                    }]
                }))
                .unwrap(),
            },
        ),
        (
            "turn-state",
            RuntimeProxyRequest {
                method: "POST".to_string(),
                path_and_query: "/backend-api/codex/v1/responses".to_string(),
                headers: vec![(
                    "x-codex-turn-state".to_string(),
                    "turn_state_owned".to_string(),
                )],
                body: serde_json::to_vec_pretty(&serde_json::json!({
                    "input": [{
                        "type": "function_call_output",
                        "call_id": "call_turn",
                        "output": "exact turn-state output\n".repeat(160)
                    }]
                }))
                .unwrap(),
            },
        ),
        (
            "session",
            RuntimeProxyRequest {
                method: "POST".to_string(),
                path_and_query: "/backend-api/codex/v1/responses".to_string(),
                headers: Vec::new(),
                body: serde_json::to_vec_pretty(&serde_json::json!({
                    "session_id": "sess_owned",
                    "input": [{
                        "type": "function_call_output",
                        "call_id": "call_session",
                        "output": "exact session output\n".repeat(160)
                    }]
                }))
                .unwrap(),
            },
        ),
    ] {
        let shared = smart_context_test_shared(&format!("affinity-exact-{name}"));
        register_runtime_smart_context_proxy_state(&shared.log_path, true, Some(32_000), None);
        let original = serde_json::from_slice::<serde_json::Value>(&request.body).unwrap();

        let prepared = prepare_runtime_smart_context_http_body(
            144,
            &request,
            &shared,
            RuntimeRouteKind::Responses,
        );

        let value = serde_json::from_slice::<serde_json::Value>(prepared.as_ref()).unwrap();
        assert_eq!(
            value, original,
            "{name} affinity payload changed semantically"
        );
        let text = String::from_utf8_lossy(prepared.as_ref());
        assert!(
            !text.contains("psc:") && !text.contains("prodex-artifact:"),
            "{name} affinity payload should not be condensed: {text}"
        );
        let log = fs::read_to_string(&shared.log_path).expect("runtime log should be readable");
        assert!(
            log.contains("decision=require_exact"),
            "{name} affinity exactness should be logged: {log}"
        );
        assert!(
            !log.contains("decision=rewritten"),
            "{name} affinity exactness should not rewrite: {log}"
        );
    }
}

#[test]
fn smart_context_http_and_websocket_prepare_match_for_same_payload_class() {
    let body = serde_json::json!({
        "type": "response.create",
        "model": "gpt-5.5",
        "input": [{
            "type": "function_call_output",
            "call_id": "call_parity",
            "output": std::iter::once("error: parity failure at src/lib.rs:12:5".to_string())
                .chain((0..620).map(|index| format!("line {index}: shared noisy output")))
                .collect::<Vec<_>>()
                .join("\n")
        }]
    })
    .to_string();
    let http_shared = smart_context_test_shared("prepare-http-parity");
    let ws_shared = smart_context_test_shared("prepare-ws-parity");
    register_runtime_smart_context_proxy_state(&http_shared.log_path, true, None, None);
    register_runtime_smart_context_proxy_state(&ws_shared.log_path, true, None, None);
    smart_context_observe_minimal_budget(&http_shared);
    smart_context_observe_minimal_budget(&ws_shared);
    let http_request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/backend-api/codex/v1/responses".to_string(),
        headers: Vec::new(),
        body: body.as_bytes().to_vec(),
    };
    let handshake_request = RuntimeProxyRequest {
        method: "GET".to_string(),
        path_and_query: "/backend-api/codex/v1/responses".to_string(),
        headers: Vec::new(),
        body: Vec::new(),
    };

    let http = prepare_runtime_smart_context_http_body(
        145,
        &http_request,
        &http_shared,
        RuntimeRouteKind::Responses,
    );
    let websocket = prepare_runtime_smart_context_websocket_text(
        145,
        &body,
        &handshake_request,
        &ws_shared,
        "main",
    );

    let Cow::Owned(http_body) = http else {
        panic!("expected HTTP prepare rewrite");
    };
    let Cow::Owned(websocket_text) = websocket else {
        panic!("expected websocket prepare rewrite");
    };
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&http_body).unwrap(),
        serde_json::from_str::<serde_json::Value>(&websocket_text).unwrap()
    );
}

#[test]
fn smart_context_prepare_rewrites_affinity_continuation_under_critical_pressure() {
    let shared = smart_context_test_shared("rewrite-affinity-pressure");
    register_runtime_smart_context_proxy_state(&shared.log_path, true, None, None);
    smart_context_observe_minimal_budget(&shared);
    let output = std::iter::once("error: failed at src/main.rs:10:5".to_string())
        .chain((0..600).map(|index| format!("line {index}: noisy continuation output")))
        .collect::<Vec<_>>()
        .join("\n");
    let mut request = smart_context_test_request(serde_json::json!({
        "previous_response_id": "resp_owned",
        "session_id": "sess_owned",
        "input": [{
            "type": "function_call_output",
            "call_id": "call_1",
            "output": output
        }]
    }));
    request.headers.push((
        "x-codex-turn-state".to_string(),
        "turn_state_owned".to_string(),
    ));
    let before_len = request.body.len();

    let rewritten =
        prepare_runtime_smart_context_http_body(43, &request, &shared, RuntimeRouteKind::Responses);

    let Cow::Owned(body) = rewritten else {
        panic!("expected critical continuation to rewrite");
    };
    assert!(body.len() < before_len);
    let value = serde_json::from_slice::<serde_json::Value>(&body).unwrap();
    assert_eq!(value["previous_response_id"].as_str(), Some("resp_owned"));
    assert_eq!(value["session_id"].as_str(), Some("sess_owned"));
    let rewritten_output = value["input"][0]["output"].as_str().unwrap();
    assert!(rewritten_output.contains("psc:"));
    assert!(rewritten_output.contains("error: failed at src/main.rs:10:5"));
    assert!(
        prodex_context::critical_signal_self_check(
            &String::from_utf8_lossy(&request.body),
            &String::from_utf8_lossy(&body),
        )
        .passed()
    );
    let log = fs::read_to_string(&shared.log_path).expect("runtime log should be readable");
    assert!(log.contains("decision=rewritten"));
    assert!(log.contains("reasons=affinity_pressure"));
    assert!(log.contains("policy_reasons=critical_budget"));
    assert!(log.contains("self_check=ok_saved"));
}

#[test]
fn smart_context_prepare_turn_state_only_affinity_rewrites_under_critical_pressure() {
    let shared = smart_context_test_shared("rewrite-turn-state-affinity-pressure");
    register_runtime_smart_context_proxy_state(&shared.log_path, true, None, None);
    smart_context_observe_minimal_budget(&shared);
    let output = std::iter::once("error: turn state owner failed at src/lib.rs:44:9".to_string())
        .chain((0..600).map(|index| format!("line {index}: noisy turn state continuation output")))
        .collect::<Vec<_>>()
        .join("\n");
    let mut request = smart_context_test_request(serde_json::json!({
        "input": [{
            "type": "function_call_output",
            "call_id": "call_1",
            "output": output
        }]
    }));
    request.headers.push((
        "x-codex-turn-state".to_string(),
        "turn_state_only_owner".to_string(),
    ));

    let rewritten =
        prepare_runtime_smart_context_http_body(44, &request, &shared, RuntimeRouteKind::Responses);

    let Cow::Owned(body) = rewritten else {
        panic!("expected turn-state affinity continuation to rewrite");
    };
    assert!(body.len() < request.body.len());
    let value = serde_json::from_slice::<serde_json::Value>(&body).unwrap();
    assert!(value.get("previous_response_id").is_none());
    assert!(value.get("session_id").is_none());
    let rewritten_output = value["input"][0]["output"].as_str().unwrap();
    assert!(rewritten_output.contains("psc:"));
    assert!(rewritten_output.contains("error: turn state owner failed at src/lib.rs:44:9"));
    assert!(
        prodex_context::critical_signal_self_check(
            &String::from_utf8_lossy(&request.body),
            &String::from_utf8_lossy(&body),
        )
        .passed()
    );
    let log = fs::read_to_string(&shared.log_path).expect("runtime log should be readable");
    assert!(log.contains("decision=rewritten"));
    assert!(log.contains("reasons=affinity_pressure"));
    assert!(log.contains("policy_reasons=critical_budget"));
}

#[test]
fn smart_context_prepare_missing_rehydrate_ref_blocks_affinity_pressure_rewrite() {
    let shared = smart_context_test_shared("rewrite-affinity-missing-rehydrate");
    register_runtime_smart_context_proxy_state(&shared.log_path, true, None, None);
    smart_context_observe_minimal_budget(&shared);
    let missing_ref = "prodex-artifact:sc:feedface";
    let request = smart_context_test_request(serde_json::json!({
        "previous_response_id": "resp_owned",
        "input": [{
            "role": "user",
            "content": format!("Continue from {missing_ref}")
        }]
    }));

    let prepared =
        prepare_runtime_smart_context_http_body(45, &request, &shared, RuntimeRouteKind::Responses);

    let value = serde_json::from_slice::<serde_json::Value>(prepared.as_ref()).unwrap();
    assert_eq!(value["previous_response_id"].as_str(), Some("resp_owned"));
    assert!(
        value["input"][0]["content"]
            .as_str()
            .unwrap()
            .contains(missing_ref)
    );
    let log = fs::read_to_string(&shared.log_path).expect("runtime log should be readable");
    assert!(log.contains("decision=require_exact"));
    assert!(log.contains("reasons=previous_response,rehydrate"));
    assert!(log.contains("policy_reasons=exactness_required,missing_rehydrate_refs"));
    assert!(!log.contains("reasons=affinity_pressure"));
}

#[test]
fn smart_context_prepare_changed_static_context_blocks_affinity_pressure_rewrite() {
    let shared = smart_context_test_shared("rewrite-affinity-static-changed");
    register_runtime_smart_context_proxy_state(&shared.log_path, true, None, None);
    smart_context_observe_minimal_budget(&shared);
    let first = smart_context_test_request(serde_json::json!({
        "instructions": "Use repo rules.\nKeep account affinity.",
        "input": [{"role": "user", "content": "first request"}]
    }));
    let changed = smart_context_test_request(serde_json::json!({
        "previous_response_id": "resp_owned",
        "instructions": "Use repo rules.\nAllow account rotation.",
        "input": [{
            "type": "function_call_output",
            "call_id": "call_1",
            "output": "error: static changed path src/lib.rs:9:1\n".repeat(600)
        }]
    }));

    let _ =
        prepare_runtime_smart_context_http_body(46, &first, &shared, RuntimeRouteKind::Responses);
    let prepared =
        prepare_runtime_smart_context_http_body(47, &changed, &shared, RuntimeRouteKind::Responses);

    let value = serde_json::from_slice::<serde_json::Value>(prepared.as_ref()).unwrap();
    assert_eq!(value["previous_response_id"].as_str(), Some("resp_owned"));
    assert_eq!(
        value["instructions"].as_str(),
        Some("Use repo rules.\nAllow account rotation.")
    );
    assert!(
        value["input"][0]["output"]
            .as_str()
            .unwrap()
            .contains("error: static changed path src/lib.rs:9:1")
    );
    let log = fs::read_to_string(&shared.log_path).expect("runtime log should be readable");
    assert!(log.contains("decision=require_exact"));
    assert!(log.contains("reasons=previous_response"));
    assert!(log.contains("policy_reasons=exactness_required,static_context_changed"));
    assert!(!log.contains("reasons=affinity_pressure"));
}

#[test]
fn smart_context_prepare_rewrite_preserves_static_prompt_prefix_text() {
    let shared = smart_context_test_shared("rewrite-static-prefix");
    register_runtime_smart_context_proxy_state(&shared.log_path, true, None, None);
    smart_context_observe_minimal_budget(&shared);
    let instructions = "Generated at: 2026-05-04T01:02:03Z\nKeep exact static prefix.  ";
    let system = "System prefix line one.\n\nSystem prefix line two.  ";
    let developer = "Developer prefix stays exact.\nUse repo rules.  ";
    let input_system = "Input system prefix\nwith blank lines.\n\nDo not rewrite.  ";
    let output = std::iter::once("error: failed at src/main.rs:10:5".to_string())
        .chain((0..500).map(|index| format!("line {index}: noisy build output")))
        .collect::<Vec<_>>()
        .join("\n");
    let request = smart_context_test_request(serde_json::json!({
        "instructions": instructions,
        "system": system,
        "developer": developer,
        "input": [
            {
                "role": "system",
                "content": input_system,
            },
            {
                "type": "function_call_output",
                "call_id": "call_1",
                "output": output,
            }
        ]
    }));

    let rewritten =
        prepare_runtime_smart_context_http_body(42, &request, &shared, RuntimeRouteKind::Responses);

    let Cow::Owned(body) = rewritten else {
        panic!("expected rewritten body");
    };
    let value = serde_json::from_slice::<serde_json::Value>(&body).unwrap();
    assert_eq!(value["instructions"].as_str(), Some(instructions));
    assert_eq!(value["system"].as_str(), Some(system));
    assert_eq!(value["developer"].as_str(), Some(developer));
    assert_eq!(value["input"][0]["content"].as_str(), Some(input_system));
    assert!(
        value["input"][1]["output"]
            .as_str()
            .unwrap()
            .contains("psc:")
    );
}

#[test]
fn smart_context_static_context_cross_field_dedupe_keeps_one_exact_copy() {
    let repeated = "Use repo rules exactly.\n".repeat(80);
    let mut value = serde_json::json!({
        "instructions": repeated.as_str(),
        "system": repeated.as_str(),
        "input": [
            {"role": "user", "content": "do work"}
        ]
    });
    let mut stats = RuntimeSmartContextTransformStats::default();

    runtime_smart_context_apply_static_context_cross_field_dedupe(
        &mut value,
        &runtime_proxy_crate::smart_context_exactness_guard(
            runtime_proxy_crate::SmartContextExactnessInput::default(),
        ),
        &mut stats,
    );

    assert_eq!(value["instructions"].as_str(), Some(repeated.as_str()));
    assert_eq!(
        value["system"].as_str(),
        Some("psc static dup instructions")
    );
    assert_eq!(value["input"][0]["content"].as_str(), Some("do work"));
    assert_eq!(stats.static_context_deltas, 1);
}

#[test]
fn smart_context_static_context_chunk_dedupe_replaces_repeated_chunk_only() {
    let shared_chunk = (0..80)
        .map(|index| format!("Shared policy sentence number {index} stays semantically identical."))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(shared_chunk.len() >= SMART_CONTEXT_STATIC_CONTEXT_CHUNK_MIN_BYTES);
    let instructions = format!("Primary intro.\n\n{shared_chunk}\n\nPrimary tail.");
    let system = format!("System intro.\n\n{shared_chunk}\n\nSystem tail.");
    let mut value = serde_json::json!({
        "instructions": instructions.as_str(),
        "system": system.as_str(),
        "input": [
            {"role": "user", "content": "do work"}
        ]
    });
    let mut stats = RuntimeSmartContextTransformStats::default();

    runtime_smart_context_apply_static_context_chunk_dedupe(
        &mut value,
        &runtime_proxy_crate::smart_context_exactness_guard(
            runtime_proxy_crate::SmartContextExactnessInput::default(),
        ),
        &mut stats,
    );

    assert_eq!(value["instructions"].as_str(), Some(instructions.as_str()));
    let system_after = value["system"].as_str().unwrap();
    assert!(system_after.contains("System intro."));
    assert!(system_after.contains("System tail."));
    assert!(system_after.contains(SMART_CONTEXT_STATIC_CONTEXT_CHUNK_DUP_MARKER_PREFIX));
    assert!(!system_after.contains(&shared_chunk));
    assert_eq!(stats.static_context_deltas, 1);
}

#[test]
fn smart_context_static_context_section_dedupe_replaces_later_identical_heading_section() {
    let body = (0..80)
        .map(|index| format!("Shared section rule {index} remains exact."))
        .collect::<Vec<_>>()
        .join("\n");
    let repeated_section = format!("## Runtime Proxy\n{body}\n");
    assert!(repeated_section.len() >= SMART_CONTEXT_STATIC_CONTEXT_CHUNK_MIN_BYTES);
    let instructions = format!(
        "# One\nunique intro\n\n{repeated_section}\n## Other\nunique tail\n\n{repeated_section}"
    );
    let sections = runtime_smart_context_static_context_heading_sections(&instructions);
    assert_eq!(sections.len(), 2);
    assert_eq!(
        instructions[sections[0].start..sections[0].end].trim(),
        instructions[sections[1].start..sections[1].end].trim()
    );
    let mut value = serde_json::json!({
        "instructions": instructions.as_str(),
        "input": [
            {"role": "user", "content": "do work"}
        ]
    });
    let mut stats = RuntimeSmartContextTransformStats::default();

    runtime_smart_context_apply_static_context_section_dedupe(
        &mut value,
        &runtime_proxy_crate::smart_context_exactness_guard(
            runtime_proxy_crate::SmartContextExactnessInput::default(),
        ),
        &mut stats,
    );

    let after = value["instructions"].as_str().unwrap();
    assert!(after.contains(SMART_CONTEXT_STATIC_CONTEXT_SECTION_DUP_MARKER_PREFIX));
    assert_eq!(after.matches("## Runtime Proxy").count(), 2);
    assert_eq!(
        after
            .matches("Shared section rule 79 remains exact.")
            .count(),
        1
    );
    assert!(after.contains("## Other"));
    assert_eq!(stats.static_context_deltas, 1);
}

#[test]
fn smart_context_static_context_section_dedupe_respects_require_exact() {
    let body = (0..80)
        .map(|index| format!("Shared exact section rule {index}."))
        .collect::<Vec<_>>()
        .join("\n");
    let instructions = format!("## Same\n{body}\n## Same\n{body}");
    let mut value = serde_json::json!({
        "instructions": instructions.as_str(),
        "input": [
            {"role": "user", "content": "do work"}
        ]
    });
    let mut stats = RuntimeSmartContextTransformStats::default();

    runtime_smart_context_apply_static_context_section_dedupe(
        &mut value,
        &runtime_proxy_crate::SmartContextExactnessGuard {
            decision: runtime_proxy_crate::SmartContextExactnessDecision::RequireExact,
            reasons: Vec::new(),
        },
        &mut stats,
    );

    assert_eq!(value["instructions"].as_str(), Some(instructions.as_str()));
    assert_eq!(stats.static_context_deltas, 0);
}

#[test]
fn smart_context_persisted_static_section_fingerprints_dedupe_fresh_start() {
    let first_shared = smart_context_test_shared("static-section-persist-first");
    let artifact_path = first_shared
        .log_path
        .with_file_name("static-section-persist-artifacts.json");
    let calibration_path = runtime_smart_context_token_calibration_path(&artifact_path);
    let _ = std::fs::remove_file(&artifact_path);
    let _ = std::fs::remove_file(&calibration_path);
    let body = (0..90)
        .map(|index| format!("Stable section guidance {index}."))
        .collect::<Vec<_>>()
        .join("\n");
    let instructions = format!("# Stable Section\n{body}\n");
    register_runtime_smart_context_proxy_state(
        &first_shared.log_path,
        true,
        None,
        Some(artifact_path.clone()),
    );
    let first = smart_context_test_request(serde_json::json!({
        "instructions": instructions.as_str(),
        "input": [{"role": "user", "content": "first request"}]
    }));

    let first_prepared = prepare_runtime_smart_context_http_body(
        120,
        &first,
        &first_shared,
        RuntimeRouteKind::Responses,
    );
    let first_value = serde_json::from_slice::<serde_json::Value>(first_prepared.as_ref()).unwrap();
    assert_eq!(
        first_value["instructions"].as_str(),
        Some(instructions.as_str())
    );
    assert!(
        std::fs::read_to_string(&calibration_path)
            .unwrap()
            .contains("static_section_fingerprints")
    );

    let fresh_shared = smart_context_test_shared("static-section-persist-fresh");
    register_runtime_smart_context_proxy_state(
        &fresh_shared.log_path,
        true,
        None,
        Some(artifact_path.clone()),
    );
    let fresh = smart_context_test_request(serde_json::json!({
        "instructions": instructions.as_str(),
        "input": [{"role": "user", "content": "fresh request"}]
    }));

    let fresh_prepared = prepare_runtime_smart_context_http_body(
        121,
        &fresh,
        &fresh_shared,
        RuntimeRouteKind::Responses,
    );
    let fresh_text = String::from_utf8_lossy(fresh_prepared.as_ref());
    assert!(fresh_text.contains(SMART_CONTEXT_STATIC_CONTEXT_SECTION_DUP_MARKER_PREFIX));
    assert!(!fresh_text.contains("Stable section guidance 89."));
    assert!(prodex_context::critical_signal_self_check(&instructions, &fresh_text).passed());

    let _ = std::fs::remove_file(&artifact_path);
    let _ = std::fs::remove_file(&calibration_path);
    let _ = std::fs::remove_file(crate::runtime_store::json_lock_file_path(&artifact_path));
    let _ = std::fs::remove_file(crate::runtime_store::json_lock_file_path(&calibration_path));
}

#[test]
fn smart_context_rewrite_telemetry_ring_records_bytes_tokens_and_fallback() {
    let shared = smart_context_test_shared("rewrite-telemetry");
    register_runtime_smart_context_proxy_state(&shared.log_path, true, None, None);
    let budget = runtime_smart_context_budget(
        &shared,
        br#"{"input":"test"}"#,
        RuntimeRouteKind::Responses,
        RuntimeSmartContextTransport::Http,
        Some("main"),
        runtime_proxy_crate::SmartContextExactnessGuard {
            decision: runtime_proxy_crate::SmartContextExactnessDecision::Allow,
            reasons: Vec::new(),
        },
        Vec::new(),
        false,
    );

    for index in 0..(SMART_CONTEXT_REWRITE_TELEMETRY_HISTORY_LIMIT + 2) {
        runtime_smart_context_log(
            index as u64,
            &shared,
            RuntimeRouteKind::Responses,
            RuntimeSmartContextTransport::Http,
            "minimal",
            "self_check_passthrough",
            "-",
            400 + index,
            300,
            RuntimeSmartContextTransformStats::default(),
            &budget,
            "critical_signal_loss",
        );
    }

    with_runtime_smart_context_proxy_state(&shared, |state| {
        assert_eq!(
            state.rewrite_telemetry_history.len(),
            SMART_CONTEXT_REWRITE_TELEMETRY_HISTORY_LIMIT
        );
        let first = state.rewrite_telemetry_history.first().unwrap();
        assert_eq!(first.body_bytes_before, 402);
        let last = state.rewrite_telemetry_history.last().unwrap();
        assert_eq!(
            last.estimated_tokens_before,
            runtime_proxy_crate::smart_context_estimate_tokens_from_body_bytes(
                last.body_bytes_before
            )
        );
        assert_eq!(last.rewrite_kind, "self_check_passthrough");
        assert_eq!(last.status, "critical_signal_loss");
        assert_eq!(
            last.fallback_reason.as_deref(),
            Some("critical_signal_loss")
        );
    })
    .unwrap();

    let log_text = std::fs::read_to_string(&shared.log_path).unwrap();
    assert!(log_text.contains("estimated_tokens_before="));
    assert!(log_text.contains("rewrite_kind=self_check_passthrough"));
    assert!(log_text.contains("fallback_reason=critical_signal_loss"));
}

#[test]
fn smart_context_regression_fallback_exact_on_quality_risk() {
    let stats = RuntimeSmartContextTransformStats {
        artifacts_stored: 1,
        tool_outputs_condensed: 1,
        tool_call_args_condensed: 0,
        duplicate_texts: 0,
        cross_turn_duplicate_texts: 0,
        repeat_tool_output_refs: 0,
        blob_outputs_condensed: 0,
        rehydrated_refs: 0,
        static_context_deltas: 0,
        repo_state_facts: 0,
    };
    let before = br#"{"input":[{"content":"error: failed\nsrc/main.rs:10:5"}]}"#;
    let after = br#"{"input":[{"content":"summary"}]}"#;
    let regression = runtime_smart_context_regression_self_check(
        before,
        after,
        runtime_proxy_crate::smart_context_exactness_guard(
            runtime_proxy_crate::SmartContextExactnessInput::default(),
        ),
        Vec::new(),
    );
    let critical = runtime_smart_context_critical_signal_self_check(before, after);

    assert_eq!(
        runtime_smart_context_fallback_exact_reason(&regression, critical, &stats),
        Some("critical_signal_loss")
    );
}

#[test]
fn smart_context_cross_turn_duplicate_uses_artifact_plan_and_exact_guard() {
    let repeated = "cross turn blob ".repeat(120);
    let mut store = RuntimeSmartContextArtifactStore::default();
    let artifact = store.insert_text(1, &repeated).unwrap();
    let mut value = serde_json::json!({
        "input": [{"type": "message", "content": repeated}]
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

    let content = value["input"][0]["content"].as_str().unwrap();
    assert!(content.contains(&runtime_smart_context_artifact_ref(&artifact.id)));
    assert_eq!(
        content,
        format!(
            "[psc rep {} b={}]",
            runtime_smart_context_artifact_ref(&artifact.id),
            repeated.len()
        )
    );
    assert!(!content.contains(" h="));
    assert_eq!(stats.cross_turn_duplicate_texts, 1);

    let mut exact_value = serde_json::json!({
        "input": [{"type": "message", "content": "cross turn blob ".repeat(120)}]
    });
    let mut exact_stats = RuntimeSmartContextTransformStats::default();
    runtime_smart_context_dedupe_input_text(
        &mut exact_value,
        &store,
        &runtime_proxy_crate::smart_context_exactness_guard(
            runtime_proxy_crate::SmartContextExactnessInput {
                exact_mode: true,
                ..runtime_proxy_crate::SmartContextExactnessInput::default()
            },
        ),
        &mut exact_stats,
    );

    assert_eq!(
        exact_value["input"][0]["content"],
        "cross turn blob ".repeat(120)
    );
    assert_eq!(exact_stats.cross_turn_duplicate_texts, 0);
}

#[test]
fn smart_context_auto_rehydrate_plan_defers_over_budget_refs() {
    let mut store = RuntimeSmartContextArtifactStore::default();
    let artifact = store
        .insert_text(1, &"large artifact ".repeat(400))
        .unwrap();
    let mut value = serde_json::json!({
        "input": [{"type": "message", "content": format!("need prodex-artifact:{}", artifact.id)}]
    });
    let plan = runtime_smart_context_auto_rehydrate_plan(
        &value,
        &store,
        1,
        runtime_proxy_crate::SmartContextTokenBudgetTier::Minimal,
    );
    let mut stats = RuntimeSmartContextTransformStats::default();

    runtime_smart_context_rehydrate_value_with_plan(&mut value, &store, &plan, &mut stats);

    assert!(matches!(
        plan.actions.first(),
        Some(runtime_proxy_crate::SmartContextRehydrateAction::Defer { .. })
    ));
    assert_eq!(stats.rehydrated_refs, 0);
    assert!(
        value["input"][0]["content"]
            .as_str()
            .unwrap()
            .contains("prodex-artifact:")
    );
}

#[test]
fn smart_context_static_context_fingerprint_drives_exact_policy_on_real_change() {
    let shared = smart_context_test_shared("static-context");
    register_runtime_smart_context_proxy_state(&shared.log_path, true, None, None);
    let first = serde_json::json!({
        "instructions": "Generated at: 2026-05-04T01:02:03Z\nKeep affinity\n"
    });
    let volatile_only = serde_json::json!({
        "instructions": "Generated at: 2027-01-02T03:04:05Z\nKeep affinity\n"
    });
    let changed = serde_json::json!({
        "instructions": "Generated at: 2027-01-02T03:04:05Z\nAllow rotation\n"
    });

    let first_observation = runtime_smart_context_observe_static_context(&shared, &first);
    let volatile_observation =
        runtime_smart_context_observe_static_context(&shared, &volatile_only);
    let changed_observation = runtime_smart_context_observe_static_context(&shared, &changed);
    assert!(!first_observation.changed);
    assert_eq!(first_observation.item_count, 1);
    assert!(!volatile_observation.changed);
    assert_eq!(volatile_observation.delta_count, 1);
    assert!(changed_observation.changed);
    assert_eq!(
        changed_observation.changed_item_ids,
        BTreeSet::from(["instructions".to_string()])
    );
}

#[test]
fn smart_context_delta_replaces_unchanged_fresh_static_context_with_marker() {
    let shared = smart_context_test_shared("static-context-delta");
    register_runtime_smart_context_proxy_state(&shared.log_path, true, None, None);
    let instructions = format!(
        "Use repo rules.\nKeep account affinity.\n{}",
        "Static instruction line. ".repeat(80)
    );
    let input_system = format!(
        "Input system prefix stays stable.\n{}",
        "Static system line. ".repeat(80)
    );
    let first = smart_context_test_request(serde_json::json!({
        "instructions": instructions.as_str(),
        "input": [
            {"role": "system", "content": input_system.as_str()},
            {"role": "user", "content": "first fresh request"}
        ]
    }));
    let second = smart_context_test_request(serde_json::json!({
        "instructions": instructions.as_str(),
        "input": [
            {"role": "system", "content": input_system.as_str()},
            {"role": "user", "content": "second fresh request"}
        ]
    }));

    let first_prepared =
        prepare_runtime_smart_context_http_body(90, &first, &shared, RuntimeRouteKind::Responses);
    assert!(
        !String::from_utf8_lossy(first_prepared.as_ref())
            .contains(SMART_CONTEXT_STATIC_CONTEXT_DELTA_MARKER_PREFIX)
    );

    let second_prepared =
        prepare_runtime_smart_context_http_body(91, &second, &shared, RuntimeRouteKind::Responses);

    let Cow::Owned(body) = second_prepared else {
        panic!("expected static context delta body");
    };
    let text = String::from_utf8(body.clone()).unwrap();
    assert!(text.contains("psc static scpc:"));
    assert!(!text.contains("prodex static context unchanged scpc:"));
    assert!(!text.contains(&instructions));
    assert!(!text.contains(&input_system));
    let value = serde_json::from_slice::<serde_json::Value>(&body).unwrap();
    assert_eq!(
        value["input"][1]["content"].as_str(),
        Some("second fresh request")
    );
}

#[test]
fn smart_context_delta_preserves_exact_static_context() {
    let shared = smart_context_test_shared("static-context-delta-exact");
    register_runtime_smart_context_proxy_state(&shared.log_path, true, None, None);
    let instructions = "Use repo rules.\nKeep exact static content.";
    let first = smart_context_test_request(serde_json::json!({
        "instructions": instructions,
        "input": [{"role": "user", "content": "first fresh request"}]
    }));
    let exact = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/backend-api/codex/v1/responses".to_string(),
        headers: vec![("x-prodex-smart-context".to_string(), "exact".to_string())],
        body: serde_json::to_vec(&serde_json::json!({
            "instructions": instructions,
            "input": [{"role": "user", "content": "exact request"}]
        }))
        .unwrap(),
    };

    let _ =
        prepare_runtime_smart_context_http_body(92, &first, &shared, RuntimeRouteKind::Responses);
    let prepared =
        prepare_runtime_smart_context_http_body(93, &exact, &shared, RuntimeRouteKind::Responses);

    let value = serde_json::from_slice::<serde_json::Value>(prepared.as_ref()).unwrap();
    assert_eq!(value["instructions"].as_str(), Some(instructions));
    assert!(
        !String::from_utf8_lossy(prepared.as_ref())
            .contains(SMART_CONTEXT_STATIC_CONTEXT_DELTA_MARKER_PREFIX)
    );
}

#[test]
fn smart_context_delta_preserves_changed_static_context() {
    let shared = smart_context_test_shared("static-context-delta-changed");
    register_runtime_smart_context_proxy_state(&shared.log_path, true, None, None);
    let stable_system = format!("Stable system prefix\n{}", "stable ".repeat(80));
    let first = smart_context_test_request(serde_json::json!({
        "instructions": "Use repo rules.\nKeep account affinity.",
        "input": [
            {"role": "system", "content": stable_system.as_str()},
            {"role": "user", "content": "first fresh request"}
        ]
    }));
    let changed = smart_context_test_request(serde_json::json!({
        "instructions": "Use repo rules.\nAllow account rotation.",
        "input": [
            {"role": "system", "content": stable_system.as_str()},
            {"role": "user", "content": "changed fresh request"}
        ]
    }));

    let _ =
        prepare_runtime_smart_context_http_body(94, &first, &shared, RuntimeRouteKind::Responses);
    let prepared =
        prepare_runtime_smart_context_http_body(95, &changed, &shared, RuntimeRouteKind::Responses);

    let value = serde_json::from_slice::<serde_json::Value>(prepared.as_ref()).unwrap();
    assert_eq!(
        value["instructions"].as_str(),
        Some("Use repo rules.\nAllow account rotation.")
    );
    let text = String::from_utf8_lossy(prepared.as_ref());
    assert!(text.contains("psc static scpc:"));
    assert!(!text.contains(stable_system.as_str()));
}

#[test]
fn smart_context_persists_static_fingerprints_but_does_not_delta_on_fresh_start() {
    let first_shared = smart_context_test_shared("static-persist-first");
    let artifact_path = first_shared
        .log_path
        .with_file_name("static-persist-artifacts.json");
    let _ = std::fs::remove_file(&artifact_path);
    register_runtime_smart_context_proxy_state(
        &first_shared.log_path,
        true,
        None,
        Some(artifact_path.clone()),
    );
    let instructions = format!("Persistent static context\n{}", "stable rule ".repeat(120));
    let first = smart_context_test_request(serde_json::json!({
        "instructions": instructions.as_str(),
        "input": [{"role": "user", "content": "first request"}]
    }));

    let _ = prepare_runtime_smart_context_http_body(
        96,
        &first,
        &first_shared,
        RuntimeRouteKind::Responses,
    );
    let loaded = RuntimeSmartContextArtifactStore::load_from_path(&artifact_path);
    assert!(
        loaded
            .static_context_prompt_cache_hash()
            .is_some_and(|hash| hash.starts_with("scpc:"))
    );
    assert_eq!(loaded.static_context_fingerprints().len(), 1);

    let fresh_shared = smart_context_test_shared("static-persist-fresh");
    register_runtime_smart_context_proxy_state(
        &fresh_shared.log_path,
        true,
        None,
        Some(artifact_path.clone()),
    );
    let fresh_first = smart_context_test_request(serde_json::json!({
        "instructions": instructions.as_str(),
        "input": [{"role": "user", "content": "fresh request"}]
    }));
    let fresh_prepared = prepare_runtime_smart_context_http_body(
        97,
        &fresh_first,
        &fresh_shared,
        RuntimeRouteKind::Responses,
    );
    let fresh_text = String::from_utf8_lossy(fresh_prepared.as_ref());
    assert!(fresh_text.contains("Persistent static context"));
    assert!(!fresh_text.contains(SMART_CONTEXT_STATIC_CONTEXT_DELTA_MARKER_PREFIX));

    let fresh_second = smart_context_test_request(serde_json::json!({
        "instructions": instructions.as_str(),
        "input": [{"role": "user", "content": "fresh second request"}]
    }));
    let second_prepared = prepare_runtime_smart_context_http_body(
        98,
        &fresh_second,
        &fresh_shared,
        RuntimeRouteKind::Responses,
    );
    assert!(String::from_utf8_lossy(second_prepared.as_ref()).contains("psc static scpc:"));

    let _ = std::fs::remove_file(&artifact_path);
    let _ = std::fs::remove_file(crate::runtime_store::json_lock_file_path(&artifact_path));
}

#[test]
fn smart_context_static_delta_prompt_cache_key_accepts_short_and_legacy_markers() {
    let shared = smart_context_test_shared("static-marker-compat");
    register_runtime_smart_context_proxy_state(&shared.log_path, true, None, None);
    let short = smart_context_test_request(serde_json::json!({
        "instructions": "psc static scpc:short123"
    }));
    let legacy = smart_context_test_request(serde_json::json!({
        "instructions": "prodex static context unchanged scpc:legacy123"
    }));

    assert_eq!(
        runtime_smart_context_effective_prompt_cache_key(&short, &shared, true).as_deref(),
        Some("scpc:short123")
    );
    assert_eq!(
        runtime_smart_context_effective_prompt_cache_key(&legacy, &shared, true).as_deref(),
        Some("scpc:legacy123")
    );
}

#[test]
fn smart_context_prompt_cache_key_prefers_explicit_request_key() {
    let shared = smart_context_test_shared("prompt-cache-explicit");
    register_runtime_smart_context_proxy_state(&shared.log_path, true, None, None);
    let request = smart_context_test_request(serde_json::json!({
        "prompt_cache_key": " explicit-cache-key ",
        "instructions": "Static instructions"
    }));

    let key = runtime_smart_context_effective_prompt_cache_key(&request, &shared, true);

    assert_eq!(key.as_deref(), Some("explicit-cache-key"));
}

#[test]
fn smart_context_prompt_cache_key_is_stable_for_same_static_context() {
    let shared = smart_context_test_shared("prompt-cache-stable");
    register_runtime_smart_context_proxy_state(&shared.log_path, true, None, None);
    let first = smart_context_test_request(serde_json::json!({
        "instructions": "Generated at: 2026-05-04T01:02:03Z\nUse repo rules",
        "input": [{"type": "message", "content": "first user message"}]
    }));
    let second = smart_context_test_request(serde_json::json!({
        "instructions": "Generated at: 2027-06-07T08:09:10Z\nUse repo rules",
        "input": [{"type": "message", "content": "different user message"}]
    }));

    let first_key = runtime_smart_context_effective_prompt_cache_key(&first, &shared, true);
    let second_key = runtime_smart_context_effective_prompt_cache_key(&second, &shared, true);

    assert!(
        first_key
            .as_deref()
            .is_some_and(|key| key.starts_with("scpc:"))
    );
    assert_eq!(first_key, second_key);
}

#[test]
fn smart_context_prompt_cache_key_absent_when_disabled_or_no_static_context() {
    let disabled = smart_context_test_shared("prompt-cache-disabled");
    register_runtime_smart_context_proxy_state(&disabled.log_path, false, None, None);
    let static_request = smart_context_test_request(serde_json::json!({
        "instructions": "Static instructions"
    }));
    assert_eq!(
        runtime_smart_context_effective_prompt_cache_key(&static_request, &disabled, true),
        None
    );

    let enabled = smart_context_test_shared("prompt-cache-no-static");
    register_runtime_smart_context_proxy_state(&enabled.log_path, true, None, None);
    let dynamic_only_request = smart_context_test_request(serde_json::json!({
        "input": [{"type": "message", "content": "user-only context"}]
    }));
    assert_eq!(
        runtime_smart_context_effective_prompt_cache_key(&dynamic_only_request, &enabled, true),
        None
    );
}

#[test]
fn smart_context_prompt_cache_key_derivation_does_not_mutate_upstream_payload() {
    let shared = smart_context_test_shared("prompt-cache-payload");
    register_runtime_smart_context_proxy_state(&shared.log_path, true, None, None);
    let request = smart_context_test_request(serde_json::json!({
        "instructions": "Static instructions",
        "input": [{"type": "message", "content": "hello"}]
    }));
    let before = request.body.clone();

    let key = runtime_smart_context_effective_prompt_cache_key(&request, &shared, true);
    let prepared =
        prepare_runtime_smart_context_http_body(88, &request, &shared, RuntimeRouteKind::Responses);

    assert!(key.as_deref().is_some_and(|key| key.starts_with("scpc:")));
    assert_eq!(request.body, before);
    assert_eq!(prepared.as_ref(), before.as_slice());
    let upstream = serde_json::from_slice::<serde_json::Value>(prepared.as_ref()).unwrap();
    assert!(upstream.get("prompt_cache_key").is_none());
}

#[test]
fn smart_context_static_context_items_have_stable_id_order() {
    let value = serde_json::json!({
        "developer": "dev rules",
        "instructions": "root instructions",
        "input": [
            {"type": "message", "role": "developer", "content": "input dev"},
            {"type": "message", "role": "system", "content": "input system"}
        ],
        "system": "system prompt"
    });

    let ids = runtime_smart_context_static_context_items(&value)
        .into_iter()
        .map(|item| item.id)
        .collect::<Vec<_>>();

    assert_eq!(
        ids,
        vec![
            "developer",
            "input[0].developer",
            "input[1].system",
            "instructions",
            "system"
        ]
    );
}

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

#[test]
fn smart_context_compaction_appends_missing_critical_exact_ranges() {
    let original = "\
line 1 noisy
error: hidden failure
src/main.rs:22:5
line 4 noisy
";
    let compacted = "summary without failure".to_string();

    let repaired = runtime_smart_context_append_missing_critical_ranges(original, compacted, 8);

    assert!(repaired.contains(SMART_CONTEXT_LABEL_CRITICAL_EXACT));
    assert!(repaired.contains("L1-L4:"));
    assert!(repaired.contains("error: hidden failure"));
    assert!(repaired.contains("src/main.rs:22:5"));
    assert!(prodex_context::critical_signal_self_check(original, &repaired).passed());
}

#[test]
fn smart_context_exact_appendices_dedupe_duplicate_range_bodies_when_shorter() {
    let duplicate = "error: repeated failure\nsrc/lib.rs:10:5\n".repeat(12);
    let ranges = vec![
        RuntimeSmartContextExactAppendixRange {
            reference: "psc:abc#L1-L24".to_string(),
            body: duplicate.clone(),
        },
        RuntimeSmartContextExactAppendixRange {
            reference: "psc:abc#L49-L72".to_string(),
            body: duplicate.clone(),
        },
    ];
    let naive = format!(
        "{SMART_CONTEXT_LABEL_CRITICAL_EXACT}\npsc:abc#L1-L24\n{duplicate}\npsc:abc#L49-L72\n{duplicate}"
    );

    let (crit, count) =
        runtime_smart_context_render_exact_appendix(SMART_CONTEXT_LABEL_CRITICAL_EXACT, ranges)
            .unwrap();
    let (sem, sem_count) = runtime_smart_context_render_exact_appendix(
        SMART_CONTEXT_LABEL_SEMANTIC_EXACT,
        vec![
            RuntimeSmartContextExactAppendixRange {
                reference: "psc:abc#L1-L24".to_string(),
                body: duplicate.clone(),
            },
            RuntimeSmartContextExactAppendixRange {
                reference: "psc:abc#L49-L72".to_string(),
                body: duplicate.clone(),
            },
        ],
    )
    .unwrap();

    assert_eq!(count, 2);
    assert_eq!(sem_count, 2);
    assert!(crit.len() < naive.len());
    assert_eq!(crit.match_indices(&duplicate).count(), 1);
    assert!(crit.contains("[psc exdup h="));
    assert!(crit.contains(&format!("b={}", duplicate.len())));
    assert!(crit.contains("refs=psc:abc#L1-L24"));
    assert!(sem.contains(SMART_CONTEXT_LABEL_SEMANTIC_EXACT));
    assert_eq!(sem.match_indices(&duplicate).count(), 1);
}

#[test]
fn smart_context_exact_appendices_merge_adjacent_line_ranges() {
    let ranges = vec![
        RuntimeSmartContextExactAppendixRange {
            reference: "psc:abc#L10-L12".to_string(),
            body: "error: first\nsrc/lib.rs:10:5\ncontext".to_string(),
        },
        RuntimeSmartContextExactAppendixRange {
            reference: "psc:abc#L13-L14".to_string(),
            body: "panic: second\nsrc/lib.rs:14:5".to_string(),
        },
    ];

    let (appendix, count) =
        runtime_smart_context_render_exact_appendix(SMART_CONTEXT_LABEL_CRITICAL_EXACT, ranges)
            .unwrap();

    assert_eq!(count, 2);
    assert!(appendix.contains("psc:abc#L10-L14"));
    assert!(appendix.contains("error: first"));
    assert!(appendix.contains("panic: second"));
    assert_eq!(appendix.matches("psc:abc#L").count(), 1);
}

#[test]
fn smart_context_exact_ref_lists_emit_compact_multi_ranges_when_shorter() {
    let refs = vec![
        "psc:abc#L1-L4".to_string(),
        "psc:abc#L9-L12".to_string(),
        "psc:abc#L20-L24".to_string(),
    ];

    let compact = runtime_smart_context_compact_line_refs_if_shorter(&refs);
    let parsed = runtime_smart_context_parse_non_alias_artifact_reference(&compact).unwrap();

    assert_eq!(compact, "psc:abc#L1-L4,L9-L12,L20-L24");
    assert_eq!(parsed.line_ranges.len(), 3);
    assert!(compact.len() < refs.join(",").len());
}

#[test]
fn smart_context_scored_exact_appendix_keeps_high_signal_and_refs_overflow() {
    let ranges = (1usize..=14)
        .map(|index| RuntimeSmartContextExactAppendixRange {
            reference: runtime_smart_context_artifact_line_ref("sc:abc", index, index),
            body: if index == 13 {
                "error[E0425]: cannot find value\nsrc/lib.rs:13:5".to_string()
            } else {
                format!("context line {index}")
            },
        })
        .collect::<Vec<_>>();

    let (appendix, count) = runtime_smart_context_render_scored_exact_appendix(
        SMART_CONTEXT_LABEL_SEMANTIC_EXACT,
        ranges,
        4,
        runtime_smart_context_critical_exact_appendix_score,
    )
    .unwrap();

    assert_eq!(count, 4);
    assert!(appendix.contains("error[E0425]"));
    assert!(appendix.contains("refs: psc:abc#"));
    assert!(appendix.contains(",L"));
    assert!(appendix.matches("context line ").count() <= 4);
}

#[test]
fn smart_context_exact_range_label_parser_accepts_legacy_critical_label() {
    let legacy = "old summary\n\ncritical exact ranges:\nL1-L1:\nerror: legacy";
    let v1 = "old summary\n\ncrit exact:\nL2-L2:\nerror: v1";

    let body = runtime_smart_context_labeled_section_body(
        legacy,
        &[
            SMART_CONTEXT_LABEL_CRITICAL_EXACT,
            SMART_CONTEXT_LABEL_CRITICAL_EXACT_V1,
            SMART_CONTEXT_LABEL_CRITICAL_EXACT_LEGACY,
        ],
    );
    let v1_body = runtime_smart_context_labeled_section_body(
        v1,
        &[
            SMART_CONTEXT_LABEL_CRITICAL_EXACT,
            SMART_CONTEXT_LABEL_CRITICAL_EXACT_V1,
            SMART_CONTEXT_LABEL_CRITICAL_EXACT_LEGACY,
        ],
    );

    assert_eq!(body, Some("L1-L1:\nerror: legacy"));
    assert_eq!(v1_body, Some("L2-L2:\nerror: v1"));
}

#[test]
fn smart_context_surgical_rehydrate_adds_lost_critical_ranges() {
    let artifact_text = std::iter::once("setup".to_string())
        .chain(std::iter::once("error: hidden failure".to_string()))
        .chain(std::iter::once("src/main.rs:22:5".to_string()))
        .chain((0..200).map(|index| format!("noise line {index}")))
        .collect::<Vec<_>>()
        .join("\n");
    let shared = smart_context_test_shared("surgical-critical");
    register_runtime_smart_context_proxy_state(&shared.log_path, true, None, None);
    let artifact = with_runtime_smart_context_artifacts(&shared, |store| {
        store.insert_text(1, &artifact_text).unwrap()
    })
    .unwrap();
    let original = serde_json::to_vec(&serde_json::json!({
        "input": [{
            "type": "function_call_output",
            "call_id": "call_1",
            "output": artifact_text
        }]
    }))
    .unwrap();
    let value = serde_json::json!({
        "input": [{
            "type": "function_call_output",
            "call_id": "call_1",
            "output": format!("prodex-artifact:{}\nsummary without failure", artifact.id)
        }]
    });
    let stats = RuntimeSmartContextTransformStats {
        artifacts_stored: 1,
        tool_outputs_condensed: 1,
        tool_call_args_condensed: 0,
        duplicate_texts: 0,
        cross_turn_duplicate_texts: 0,
        repeat_tool_output_refs: 0,
        blob_outputs_condensed: 0,
        rehydrated_refs: 0,
        static_context_deltas: 0,
        repo_state_facts: 0,
    };

    let (body, repaired_stats) = runtime_smart_context_try_surgical_rehydrate_critical_ranges(
        &value,
        &shared,
        &original,
        &runtime_proxy_crate::smart_context_exactness_guard(
            runtime_proxy_crate::SmartContextExactnessInput::default(),
        ),
        &[],
        &stats,
    )
    .expect("lost critical lines should be surgically rehydrated");

    let text = String::from_utf8(body).unwrap();
    assert!(text.contains(SMART_CONTEXT_LABEL_CRITICAL_EXACT));
    assert!(text.contains(&runtime_smart_context_artifact_line_ref(&artifact.id, 1, 4)));
    assert!(text.contains("error: hidden failure"));
    assert!(text.contains("src/main.rs:22:5"));
    assert!(repaired_stats.rehydrated_refs > stats.rehydrated_refs);
    assert!(
        prodex_context::critical_signal_self_check(&String::from_utf8_lossy(&original), &text)
            .passed()
    );
}

#[test]
fn smart_context_surgical_rehydrate_prefers_artifact_line_index() {
    let artifact_text = "\
setup
error: hidden indexed failure
src/main.rs:22:5
noise";
    let mut store = RuntimeSmartContextArtifactStore::default();
    let artifact = store.insert_text(1, artifact_text).unwrap();
    let line_index = store
        .line_index(&artifact.id)
        .expect("inserted artifact should have line index");

    let (appendix, range_count) = runtime_smart_context_missing_critical_range_appendix(
        &artifact.id,
        "fallback text without indexed critical signals",
        Some(line_index),
        &format!("prodex-artifact:{}\nsummary without failure", artifact.id),
    )
    .expect("indexed critical range should be rehydrated");

    assert_eq!(range_count, 1);
    assert!(appendix.contains(SMART_CONTEXT_LABEL_CRITICAL_EXACT));
    assert!(appendix.contains(&runtime_smart_context_artifact_line_ref(&artifact.id, 1, 4)));
    assert!(appendix.contains("error: hidden indexed failure"));
    assert!(appendix.contains("src/main.rs:22:5"));
}

#[test]
fn smart_context_surgical_rehydrate_falls_back_for_legacy_unindexed_artifact() {
    let artifact_text = "\
setup
error: legacy failure
src/main.rs:22:5
noise";
    let artifact_id = runtime_proxy_crate::smart_context_hash_text(artifact_text);

    let (appendix, range_count) = runtime_smart_context_missing_critical_range_appendix(
        &artifact_id,
        artifact_text,
        None,
        &format!("prodex-artifact:{artifact_id}\nsummary without failure"),
    )
    .expect("legacy artifact should still rehydrate by rescanning");

    assert_eq!(range_count, 1);
    assert!(appendix.contains(&runtime_smart_context_artifact_line_ref(&artifact_id, 1, 4)));
    assert!(appendix.contains("error: legacy failure"));
    assert!(appendix.contains("src/main.rs:22:5"));
}

#[test]
fn smart_context_minifies_structural_json_without_touching_strings() {
    let body = br#"{
      "input": [
        {
          "type": "message",
          "content": "keep  spaces\ninside string"
        }
      ]
    }"#;
    let value = serde_json::from_slice::<serde_json::Value>(body).unwrap();

    let minified = runtime_smart_context_minified_json_body(&value, body).unwrap();
    let text = String::from_utf8(minified).unwrap();

    assert!(text.len() < body.len());
    assert!(text.contains("keep  spaces\\ninside string"));
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&text).unwrap(),
        value
    );
}

#[test]
fn smart_context_prepare_minifies_exact_json_without_changing_payload() {
    let shared = smart_context_test_shared("prepare-minify-exact");
    register_runtime_smart_context_proxy_state(&shared.log_path, true, None, None);
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/backend-api/codex/v1/responses".to_string(),
        headers: vec![("x-prodex-smart-context".to_string(), "exact".to_string())],
        body: br#"{
          "input": [
            {
              "type": "message",
              "content": "keep  spaces\ninside string"
            }
          ]
        }"#
        .to_vec(),
    };
    let before = serde_json::from_slice::<serde_json::Value>(&request.body).unwrap();

    let prepared =
        prepare_runtime_smart_context_http_body(77, &request, &shared, RuntimeRouteKind::Responses);

    let Cow::Owned(body) = prepared else {
        panic!("expected minified body");
    };
    let after = serde_json::from_slice::<serde_json::Value>(&body).unwrap();
    assert!(body.len() < request.body.len());
    assert_eq!(after, before);
    assert_eq!(
        after["input"][0]["content"].as_str(),
        Some("keep  spaces\ninside string")
    );
}

#[test]
fn smart_context_prepare_passes_invalid_json_unchanged() {
    let shared = smart_context_test_shared("prepare-invalid-json");
    register_runtime_smart_context_proxy_state(&shared.log_path, true, None, None);
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/backend-api/codex/v1/responses".to_string(),
        headers: Vec::new(),
        body: b"{ invalid\n".to_vec(),
    };

    let prepared =
        prepare_runtime_smart_context_http_body(78, &request, &shared, RuntimeRouteKind::Responses);

    assert!(matches!(&prepared, Cow::Borrowed(_)));
    assert_eq!(prepared.as_ref(), request.body.as_slice());
}

#[test]
fn smart_context_prepare_passes_too_deep_json_unchanged_without_panic_fallback() {
    let shared = smart_context_test_shared("prepare-too-deep-json");
    register_runtime_smart_context_proxy_state(&shared.log_path, true, Some(32_000), None);
    let mut nested = serde_json::Value::String("leaf".to_string());
    for _ in 0..RUNTIME_SMART_CONTEXT_MAX_JSON_DEPTH {
        nested = serde_json::json!({ "nested": nested });
    }
    let body = serde_json::json!({
        "model": "gpt-5.5",
        "input": [{
            "type": "message",
            "role": "user",
            "content": "keep exact"
        }],
        "metadata": nested
    })
    .to_string();
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/backend-api/codex/v1/responses".to_string(),
        headers: Vec::new(),
        body: body.into_bytes(),
    };

    let prepared =
        prepare_runtime_smart_context_http_body(79, &request, &shared, RuntimeRouteKind::Responses);

    assert!(matches!(&prepared, Cow::Borrowed(_)));
    assert_eq!(prepared.as_ref(), request.body.as_slice());
    let log = fs::read_to_string(&shared.log_path).expect("runtime log should be readable");
    assert!(log.contains("decision=unsupported_json_shape"));
    assert!(log.contains("reasons=json_depth_limit"));
    assert!(!log.contains("smart_context_panic"));
}

#[test]
fn smart_context_json_shape_guard_rejects_excessive_node_count_iteratively() {
    let value = serde_json::Value::Array(vec![
        serde_json::Value::Null;
        RUNTIME_SMART_CONTEXT_MAX_JSON_NODES + 1
    ]);

    assert_eq!(
        runtime_smart_context_unsupported_json_shape_reason(&value),
        Some("json_node_limit")
    );
}

#[test]
fn smart_context_static_section_body_rejects_non_char_boundary_offsets_without_panic() {
    let text = "## Résumé café\n".repeat(90);
    let bad_start = text.find('é').unwrap() + 1;
    let section = RuntimeSmartContextStaticHeadingSection {
        heading: "## Résumé café".to_string(),
        start: bad_start,
        end: text.len(),
        ordinal: 0,
    };

    assert_eq!(
        runtime_smart_context_static_heading_section_body(&text, &section),
        None
    );
}

#[test]
fn smart_context_self_check_passes_through_growth_without_rehydrate() {
    let stats = RuntimeSmartContextTransformStats {
        artifacts_stored: 1,
        tool_outputs_condensed: 1,
        tool_call_args_condensed: 0,
        duplicate_texts: 0,
        cross_turn_duplicate_texts: 0,
        repeat_tool_output_refs: 0,
        blob_outputs_condensed: 0,
        rehydrated_refs: 0,
        static_context_deltas: 0,
        repo_state_facts: 0,
    };

    assert_eq!(
        runtime_smart_context_rewrite_self_check(100, 101, &stats),
        "growth"
    );
    assert!(runtime_smart_context_should_pass_through_after_self_check(
        100, 101, &stats
    ));
}

fn smart_context_test_request(body: serde_json::Value) -> RuntimeProxyRequest {
    RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/backend-api/codex/v1/responses".to_string(),
        headers: Vec::new(),
        body: serde_json::to_vec(&body).unwrap(),
    }
}

fn smart_context_observe_minimal_budget(shared: &RuntimeRotationProxyShared) {
    observe_runtime_smart_context_token_usage(
        shared,
        RuntimeTokenUsage {
            input_tokens: 24_000,
            cached_input_tokens: 0,
            output_tokens: 7_000,
            reasoning_tokens: 1_000,
        },
    );
}

fn smart_context_test_shared(name: &str) -> RuntimeRotationProxyShared {
    static NEXT_LOG_ID: AtomicU64 = AtomicU64::new(1);
    let unique = NEXT_LOG_ID.fetch_add(1, Ordering::Relaxed);
    let root = env::temp_dir().join(format!(
        "prodex-smart-context-{name}-{}-{unique}",
        std::process::id()
    ));
    let paths = AppPaths {
        state_file: root.join("state.json"),
        managed_profiles_root: root.join("profiles"),
        shared_codex_root: root.join("shared-codex"),
        legacy_shared_codex_root: root.join("shared"),
        root,
    };

    RuntimeRotationProxyShared {
        upstream_no_proxy: false,
        async_client: reqwest::Client::new(),
        async_runtime: Arc::new(
            TokioRuntimeBuilder::new_current_thread()
                .enable_all()
                .build()
                .expect("tokio runtime"),
        ),
        runtime: Arc::new(Mutex::new(RuntimeRotationState {
            paths,
            state: AppState::default(),
            upstream_base_url: "http://127.0.0.1".to_string(),
            include_code_review: false,
            current_profile: "main".to_string(),
            profile_usage_auth: BTreeMap::new(),
            turn_state_bindings: BTreeMap::new(),
            session_id_bindings: BTreeMap::new(),
            continuation_statuses: RuntimeContinuationStatuses::default(),
            profile_probe_cache: BTreeMap::new(),
            profile_usage_snapshots: BTreeMap::new(),
            profile_retry_backoff_until: BTreeMap::new(),
            profile_transport_backoff_until: BTreeMap::new(),
            profile_route_circuit_open_until: BTreeMap::new(),
            profile_inflight: BTreeMap::new(),
            profile_health: BTreeMap::new(),
        })),
        log_path: env::temp_dir().join(format!(
            "prodex-smart-context-{name}-{}-{unique}.log",
            std::process::id()
        )),
        request_sequence: Arc::new(AtomicU64::new(1)),
        state_save_revision: Arc::new(AtomicU64::new(0)),
        local_overload_backoff_until: Arc::new(AtomicU64::new(0)),
        active_request_count: Arc::new(AtomicUsize::new(0)),
        active_request_limit: 8,
        runtime_state_lock_wait_counters:
            RuntimeRotationProxyShared::new_runtime_state_lock_wait_counters(),
        lane_admission: RuntimeProxyLaneAdmission::new(RuntimeProxyLaneLimits {
            responses: 8,
            compact: 8,
            websocket: 8,
            standard: 8,
        }),
    }
}
