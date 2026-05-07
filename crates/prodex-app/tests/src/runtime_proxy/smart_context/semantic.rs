use super::*;
use std::collections::BTreeSet;

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
