use super::*;
use std::borrow::Cow;
use std::collections::BTreeSet;

#[test]
fn smart_context_condenses_tool_output_with_artifact_ref() {
    let original_output = (0..500)
        .map(|index| format!("line {index}: repeated command output"))
        .collect::<Vec<_>>()
        .join("\n");
    let mut value = serde_json::json!({
        "input": [{
            "type": "function_call_output",
            "call_id": "call_1",
            "output": original_output
        }]
    });
    let mut store = RuntimeSmartContextArtifactStore::default();
    let mut stats = RuntimeSmartContextTransformStats::default();

    runtime_smart_context_condense_tool_outputs(
        &mut value,
        &mut store,
        7,
        runtime_proxy_crate::SmartContextTokenBudgetTier::Minimal,
        4 * 1024,
        &RuntimeSmartContextIntentSignals::default(),
        &mut stats,
    );

    let output = value["input"][0]["output"].as_str().unwrap();
    assert!(output.contains("prodex-sc artifact prodex-artifact:sc:"));
    assert!(output.contains("b="));
    assert!(output.contains("h="));
    assert!(output.contains(&format!("b={}", original_output.len())));
    assert!(output.contains(&format!(
        "h={}",
        runtime_proxy_crate::smart_context_hash_text(&original_output)
    )));
    assert!(output.contains("rehydrate psc:"));
    assert!(output.contains("#Lstart-Lend"));
    assert!(!output.contains("artifact_id:"));
    assert_eq!(stats.artifacts_stored, 1);
    assert_eq!(stats.tool_outputs_condensed, 1);
}

#[test]
fn smart_context_large_failing_tool_output_uses_progressive_artifact_summary() {
    let hidden_tail = "FULL_TAIL_SHOULD_ONLY_EXIST_IN_ARTIFACT";
    let original_output = std::iter::once("running 1 test".to_string())
        .chain(std::iter::once(
            "---- runtime_proxy::progressive_output stdout ----".to_string(),
        ))
        .chain(std::iter::once(
            "thread 'runtime_proxy::progressive_output' panicked at crates/prodex-app/tests/src/runtime_proxy/smart_context.rs:12:5".to_string(),
        ))
        .chain(std::iter::once(
            "error[E0425]: cannot find value `missing` in this scope".to_string(),
        ))
        .chain(std::iter::once(" --> src/lib.rs:42:13".to_string()))
        .chain((0..420).map(|index| format!("noise line {index}: compile chatter")))
        .chain(std::iter::once(hidden_tail.to_string()))
        .collect::<Vec<_>>()
        .join("\n");
    let mut value = serde_json::json!({
        "input": [{
            "type": "function_call_output",
            "call_id": "call_1",
            "output": original_output
        }]
    });
    let mut store = RuntimeSmartContextArtifactStore::default();
    let mut stats = RuntimeSmartContextTransformStats::default();

    runtime_smart_context_condense_tool_outputs(
        &mut value,
        &mut store,
        7,
        runtime_proxy_crate::SmartContextTokenBudgetTier::Minimal,
        512,
        &RuntimeSmartContextIntentSignals::default(),
        &mut stats,
    );
    runtime_smart_context_append_artifact_manifest_if_useful(&mut value, &store, &stats);

    let output = value["input"][0]["output"].as_str().unwrap();
    assert!(output.contains("prodex-sc artifact prodex-artifact:sc:"));
    assert!(output.contains(SMART_CONTEXT_LABEL_SUMMARY));
    assert!(output.contains(SMART_CONTEXT_LABEL_CRITICAL_EXACT));
    assert!(output.contains("psc:"));
    assert!(output.contains("#L"));
    assert!(output.contains("error[E0425]"));
    assert!(output.contains("runtime_proxy::progressive_output"));
    assert!(!output.contains(hidden_tail));
    assert!(!output.contains("noise line 419"));

    let manifest = value["input"][1]["content"].as_str().unwrap();
    assert!(manifest.contains("psc manifest"));
    assert!(manifest.contains("content omitted"));
    assert!(!manifest.contains(hidden_tail));
    assert_eq!(stats.artifacts_stored, 1);
    assert_eq!(stats.tool_outputs_condensed, 1);
}

#[test]
fn smart_context_progressive_summary_replaces_exact_duplicate_chunks_with_refs() {
    const CHUNK_LINES: usize = 32;
    let chunk = (1..=CHUNK_LINES)
        .map(|line| format!("duplicate block line {line}: exact payload"))
        .collect::<Vec<_>>()
        .join("\n");
    let original_output = [chunk.as_str(), chunk.as_str(), chunk.as_str()].join("\n");
    let mut store = RuntimeSmartContextArtifactStore::default();
    let artifact = store.insert_text(7, &original_output).unwrap();
    let summary = [chunk.as_str(), chunk.as_str()].join("\n");

    let deduped = runtime_smart_context_dedupe_progressive_summary_chunks(
        &artifact.id,
        &original_output,
        &summary,
        store.chunk_index(&artifact.id),
    );

    assert!(deduped.contains(SMART_CONTEXT_LABEL_DUPLICATE_CHUNKS));
    assert!(deduped.contains(&runtime_smart_context_artifact_line_ref(
        &artifact.id,
        1,
        CHUNK_LINES
    )));
    assert!(deduped.contains(&runtime_smart_context_artifact_line_ref(
        &artifact.id,
        CHUNK_LINES + 1,
        CHUNK_LINES * 2
    )));
    assert_eq!(deduped.match_indices(&chunk).count(), 1);
    assert!(deduped.len() < summary.len());
}

#[test]
fn smart_context_progressive_artifact_ref_rehydrates_full_content_on_explicit_ref() {
    let original_output = std::iter::once("error: progressive failure".to_string())
        .chain((0..300).map(|index| format!("artifact body line {index}")))
        .collect::<Vec<_>>()
        .join("\n");
    let mut value = serde_json::json!({
        "input": [{
            "type": "function_call_output",
            "call_id": "call_1",
            "output": original_output
        }]
    });
    let mut store = RuntimeSmartContextArtifactStore::default();
    let mut stats = RuntimeSmartContextTransformStats::default();

    runtime_smart_context_condense_tool_outputs(
        &mut value,
        &mut store,
        7,
        runtime_proxy_crate::SmartContextTokenBudgetTier::Minimal,
        512,
        &RuntimeSmartContextIntentSignals::default(),
        &mut stats,
    );
    let marker = value["input"][0]["output"].as_str().unwrap().to_string();
    let artifact = runtime_smart_context_collect_artifact_refs(&serde_json::Value::String(marker))
        .into_iter()
        .next()
        .expect("condensed output should include artifact ref");
    let mut rehydrate_value = serde_json::json!({
        "input": [{
            "type": "message",
            "content": format!("please inspect prodex-artifact:{}", artifact.id)
        }]
    });
    let mut rehydrate_stats = RuntimeSmartContextTransformStats::default();

    runtime_smart_context_rehydrate_value(&mut rehydrate_value, &store, &mut rehydrate_stats);

    assert_eq!(
        rehydrate_value["input"][0]["content"],
        format!("please inspect {original_output}")
    );
    assert_eq!(rehydrate_stats.rehydrated_refs, 1);
}

#[test]
fn smart_context_uses_command_metadata_hint_for_tool_output_compaction() {
    let original_output = std::iter::once("src/lib.rs:42:needle once".to_string())
        .chain((0..120).map(|index| format!("filler line {index}: no match here")))
        .collect::<Vec<_>>()
        .join("\n");
    let mut value = serde_json::json!({
        "input": [
            {
                "type": "function_call",
                "call_id": "call_1",
                "name": "exec_command",
                "arguments": "{\"cmd\":\"rg needle src\"}"
            },
            {
                "type": "function_call_output",
                "call_id": "call_1",
                "output": original_output
            }
        ]
    });
    let mut store = RuntimeSmartContextArtifactStore::default();
    let mut stats = RuntimeSmartContextTransformStats::default();

    runtime_smart_context_condense_tool_outputs(
        &mut value,
        &mut store,
        7,
        runtime_proxy_crate::SmartContextTokenBudgetTier::Minimal,
        256,
        &RuntimeSmartContextIntentSignals::default(),
        &mut stats,
    );

    let output = value["input"][1]["output"].as_str().unwrap();
    assert!(output.contains("prodex-sc artifact prodex-artifact:sc:"));
    assert!(output.contains("sum: search matches=1, files=1"));
    assert!(output.contains("src/lib.rs (1 matches):"));
    assert_eq!(stats.tool_outputs_condensed, 1);
}

#[test]
fn smart_context_tool_metadata_diet_keeps_command_exit_kind_without_huge_hint() {
    let huge_metadata_path = "src/huge_metadata_should_not_leak.rs";
    let value = serde_json::json!({
        "input": [
            {
                "type": "function_call",
                "call_id": "call_1",
                "name": "exec_command",
                "arguments": serde_json::json!({
                    "cmd": "cargo test smart_context::metadata_hint",
                    "exit_code": 101,
                    "metadata": format!("{huge_metadata_path}\n{}", "noise ".repeat(4000))
                }).to_string()
            },
            {
                "type": "function_call",
                "call_id": "call_2",
                "name": "exec_command",
                "arguments": serde_json::json!({
                    "kind": "search",
                    "metadata": "ignored compact metadata"
                }).to_string()
            }
        ]
    });

    let first = value["input"][0].as_object().unwrap();
    let second = value["input"][1].as_object().unwrap();
    let first_metadata = runtime_smart_context_tool_item_metadata(first);
    let second_metadata = runtime_smart_context_tool_item_metadata(second);
    let signals = runtime_smart_context_collect_intent_signals(&value);

    assert_eq!(
        first_metadata.command.as_deref(),
        Some("cargo test smart_context::metadata_hint")
    );
    assert_eq!(first_metadata.exit_code, Some(101));
    assert_eq!(
        first_metadata.kind_hint,
        Some(prodex_context::CommandOutputKind::RustDiagnostics)
    );
    assert_eq!(
        second_metadata.kind_hint,
        Some(prodex_context::CommandOutputKind::Search)
    );
    assert!(
        signals
            .intent_terms
            .contains(&"smart_context::metadata_hint".to_string())
    );
    assert!(
        !signals
            .intent_terms
            .contains(&huge_metadata_path.to_string())
    );
    assert!(signals.command_kind_hints.contains("rust-diagnostics"));
    assert!(signals.command_kind_hints.contains("search"));
}

#[test]
fn smart_context_uses_success_summary_for_long_success_tool_output() {
    let mut original_output = String::new();
    original_output.push_str("added 82 packages, and audited 83 packages in 2s\n");
    original_output.push_str("found 0 vulnerabilities\n");
    original_output.push_str("vite v5.0.0 building for production...\n");
    for index in 0..60 {
        original_output.push_str(&format!("transforming src/module_{index}.ts\n"));
    }
    original_output.push_str("dist/index.html                  0.45 kB\n");
    original_output.push_str("dist/assets/app.js             24.12 kB\n");
    original_output.push_str("built in 1.42s\n");
    for index in 0..60 {
        original_output.push_str(&format!("src/generated/file_{index}.rs\n"));
    }
    let mut value = serde_json::json!({
        "input": [
            {
                "type": "function_call",
                "call_id": "call_1",
                "name": "exec_command",
                "arguments": "{\"cmd\":\"npm install && npm run build && find src -type f\"}"
            },
            {
                "type": "function_call_output",
                "call_id": "call_1",
                "output": original_output
            }
        ]
    });
    let mut store = RuntimeSmartContextArtifactStore::default();
    let mut stats = RuntimeSmartContextTransformStats::default();

    runtime_smart_context_condense_tool_outputs(
        &mut value,
        &mut store,
        7,
        runtime_proxy_crate::SmartContextTokenBudgetTier::Minimal,
        256,
        &RuntimeSmartContextIntentSignals::default(),
        &mut stats,
    );

    let output = value["input"][1]["output"].as_str().unwrap();
    assert!(output.contains("prodex-sc artifact prodex-artifact:sc:"));
    assert!(output.contains("pcs: success cmd"));
    assert!(output.contains("command: npm install && npm run build && find src -type f"));
    assert!(output.contains("exit: (unknown)"));
    assert!(output.contains("touched files ("));
    assert!(!output.contains("transforming src/module_59.ts"));
    assert!(
        store
            .artifact_ref_for_exact_text(&original_output)
            .is_some()
    );
    assert_eq!(stats.artifacts_stored, 1);
    assert_eq!(stats.tool_outputs_condensed, 1);
}

#[test]
fn smart_context_keeps_failure_output_on_critical_preserving_path() {
    let original_output =
        std::iter::once("added 82 packages, and audited 83 packages in 2s".to_string())
            .chain((0..80).map(|index| format!("transforming src/module_{index}.ts")))
            .chain([
                "error: build script failed".to_string(),
                "src/build.rs:42:9".to_string(),
                "process didn't exit successfully: `cargo build` (exit status: 101)".to_string(),
            ])
            .collect::<Vec<_>>()
            .join("\n");
    let mut value = serde_json::json!({
        "input": [
            {
                "type": "function_call",
                "call_id": "call_1",
                "name": "exec_command",
                "arguments": "{\"cmd\":\"npm install && cargo build\"}"
            },
            {
                "type": "function_call_output",
                "call_id": "call_1",
                "output": original_output
            }
        ]
    });
    let mut store = RuntimeSmartContextArtifactStore::default();
    let mut stats = RuntimeSmartContextTransformStats::default();

    runtime_smart_context_condense_tool_outputs(
        &mut value,
        &mut store,
        7,
        runtime_proxy_crate::SmartContextTokenBudgetTier::Minimal,
        256,
        &RuntimeSmartContextIntentSignals::default(),
        &mut stats,
    );

    let output = value["input"][1]["output"].as_str().unwrap();
    assert!(output.contains("prodex-sc artifact prodex-artifact:sc:"));
    assert!(!output.contains("successful command output"));
    assert!(output.contains(SMART_CONTEXT_LABEL_CRITICAL_EXACT));
    assert!(output.contains("error: build script failed"));
    assert!(output.contains("src/build.rs:42:9"));
    assert!(output.contains("exit status: 101"));
    assert!(
        store
            .artifact_ref_for_exact_text(&original_output)
            .is_some()
    );
    assert_eq!(stats.tool_outputs_condensed, 1);
}

#[test]
fn smart_context_intent_signals_collect_request_terms_metadata_and_refs() {
    let value = serde_json::json!({
        "input": [
            {
                "type": "message",
                "role": "user",
                "content": "Focus src/runtime_proxy/smart_context.rs error[E0425] smart_context::intent_signal_bus MissingIntentSymbol prodex-artifact:sc:abc123"
            },
            {
                "type": "function_call",
                "call_id": "call_1",
                "name": "exec_command",
                "arguments": "{\"cmd\":\"cargo test smart_context::metadata_hint\"}"
            }
        ]
    });

    let signals = runtime_smart_context_collect_intent_signals(&value);

    assert!(
        signals
            .intent_terms
            .contains(&"src/runtime_proxy/smart_context.rs".to_string())
    );
    assert!(signals.intent_terms.contains(&"E0425".to_string()));
    assert!(
        signals
            .intent_terms
            .contains(&"smart_context::intent_signal_bus".to_string())
    );
    assert!(
        signals
            .intent_terms
            .contains(&"MissingIntentSymbol".to_string())
    );
    assert!(
        signals
            .intent_terms
            .contains(&"smart_context::metadata_hint".to_string())
    );
    assert!(
        signals
            .semantic_terms
            .file_paths
            .contains("src/runtime_proxy/smart_context.rs")
    );
    assert!(signals.semantic_terms.error_codes.contains("E0425"));
    assert!(
        signals
            .semantic_terms
            .test_symbols
            .contains("smart_context::intent_signal_bus")
    );
    assert_eq!(signals.artifact_refs.len(), 1);
    assert_eq!(signals.artifact_refs[0].id, "sc:abc123");
    assert!(signals.command_kind_hints.contains("rust-diagnostics"));
}

#[test]
fn smart_context_request_intent_terms_prioritize_tool_output_compaction() {
    let original_output = std::iter::once("build started".to_string())
        .chain((0..70).map(|index| format!("early filler line {index}: no signal")))
        .chain([
            "src/runtime_proxy/smart_context.rs:314:5: intent path hit".to_string(),
            "error[E0425]: cannot find value `intent_missing` in this scope".to_string(),
            "---- smart_context::intent_signal_bus stdout ----".to_string(),
            "thread 'smart_context::intent_signal_bus' panicked at MissingIntentSymbol".to_string(),
        ])
        .chain((0..70).map(|index| format!("late filler line {index}: no signal")))
        .collect::<Vec<_>>()
        .join("\n");
    let mut value = serde_json::json!({
        "input": [
            {
                "type": "message",
                "role": "user",
                "content": "Investigate src/runtime_proxy/smart_context.rs E0425 smart_context::intent_signal_bus MissingIntentSymbol"
            },
            {
                "type": "function_call_output",
                "call_id": "call_1",
                "output": original_output
            }
        ]
    });
    let intent_signals = runtime_smart_context_collect_intent_signals(&value);
    let mut store = RuntimeSmartContextArtifactStore::default();
    let mut stats = RuntimeSmartContextTransformStats::default();

    runtime_smart_context_condense_tool_outputs(
        &mut value,
        &mut store,
        7,
        runtime_proxy_crate::SmartContextTokenBudgetTier::Minimal,
        256,
        &intent_signals,
        &mut stats,
    );

    let output = value["input"][1]["output"].as_str().unwrap();
    assert!(output.contains("int:"));
    assert!(output.contains("src/runtime_proxy/smart_context.rs"));
    assert!(output.contains("error[E0425]"));
    assert!(output.contains("smart_context::intent_signal_bus"));
    assert!(output.contains("MissingIntentSymbol"));
    assert_eq!(stats.artifacts_stored, 1);
    assert_eq!(stats.tool_outputs_condensed, 1);
}

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

    assert!(manifest.contains("psc manifest"));
    assert!(manifest.contains(&runtime_smart_context_artifact_ref(&artifact.id)));
    assert!(manifest.contains(&format!("b={}", artifact_text.len())));
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
            diff_hunks: Vec::new(),
        },
        &mut stats,
    );

    let content = value["input"][0]["content"].as_str().unwrap();
    assert_eq!(count, stats.rehydrated_refs);
    assert!(count >= 3);
    assert!(content.contains(SMART_CONTEXT_LABEL_SEMANTIC_EXACT));
    assert!(content.contains(&runtime_smart_context_artifact_line_ref(&artifact.id, 8, 8)));
    assert!(content.contains("error[E0425]: cannot find value `missing` in this scope"));
    assert!(content.contains(&runtime_smart_context_artifact_line_ref(&artifact.id, 9, 9)));
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

    assert!(
        value["input"][1]["content"]
            .as_str()
            .unwrap()
            .contains("psc dup")
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
fn smart_context_regression_fallback_exact_on_quality_risk() {
    let stats = RuntimeSmartContextTransformStats {
        artifacts_stored: 1,
        tool_outputs_condensed: 1,
        duplicate_texts: 0,
        cross_turn_duplicate_texts: 0,
        repeat_tool_output_refs: 0,
        blob_outputs_condensed: 0,
        rehydrated_refs: 0,
        static_context_deltas: 0,
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
    let budget = runtime_smart_context_budget(
        &shared,
        b"small current request body payload",
        runtime_proxy_crate::smart_context_exactness_guard(
            runtime_proxy_crate::SmartContextExactnessInput::default(),
        ),
        Vec::new(),
        changed_observation.changed,
    );

    assert!(!first_observation.changed);
    assert_eq!(first_observation.item_count, 1);
    assert!(!volatile_observation.changed);
    assert_eq!(volatile_observation.delta_count, 1);
    assert!(changed_observation.changed);
    assert_eq!(
        budget.policy.mode,
        runtime_proxy_crate::SmartContextBudgetMode::ExactPassThrough
    );
    assert!(
        budget
            .policy
            .reasons
            .contains(&runtime_proxy_crate::SmartContextBudgetPolicyReason::StaticContextChanged)
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
            .contains("prodex static context unchanged")
    );

    let second_prepared =
        prepare_runtime_smart_context_http_body(91, &second, &shared, RuntimeRouteKind::Responses);

    let Cow::Owned(body) = second_prepared else {
        panic!("expected static context delta body");
    };
    let text = String::from_utf8(body.clone()).unwrap();
    assert!(text.contains("prodex static context unchanged scpc:"));
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
        !String::from_utf8_lossy(prepared.as_ref()).contains("prodex static context unchanged")
    );
}

#[test]
fn smart_context_delta_preserves_changed_static_context() {
    let shared = smart_context_test_shared("static-context-delta-changed");
    register_runtime_smart_context_proxy_state(&shared.log_path, true, None, None);
    let first = smart_context_test_request(serde_json::json!({
        "instructions": "Use repo rules.\nKeep account affinity.",
        "input": [{"role": "user", "content": "first fresh request"}]
    }));
    let changed = smart_context_test_request(serde_json::json!({
        "instructions": "Use repo rules.\nAllow account rotation.",
        "input": [{"role": "user", "content": "changed fresh request"}]
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
    assert!(
        !String::from_utf8_lossy(prepared.as_ref()).contains("prodex static context unchanged")
    );
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
    assert!(!fresh_text.contains("prodex static context unchanged"));

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
    assert!(
        String::from_utf8_lossy(second_prepared.as_ref())
            .contains("prodex static context unchanged scpc:")
    );

    let _ = std::fs::remove_file(&artifact_path);
    let _ = std::fs::remove_file(crate::runtime_store::json_lock_file_path(&artifact_path));
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
fn smart_context_exact_range_label_parser_accepts_legacy_critical_label() {
    let legacy = "old summary\n\ncritical exact ranges:\nL1-L1:\nerror: legacy";

    let body = runtime_smart_context_labeled_section_body(
        legacy,
        &[
            SMART_CONTEXT_LABEL_CRITICAL_EXACT,
            SMART_CONTEXT_LABEL_CRITICAL_EXACT_LEGACY,
        ],
    );

    assert_eq!(body, Some("L1-L1:\nerror: legacy"));
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
        duplicate_texts: 0,
        cross_turn_duplicate_texts: 0,
        repeat_tool_output_refs: 0,
        blob_outputs_condensed: 0,
        rehydrated_refs: 0,
        static_context_deltas: 0,
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
fn smart_context_self_check_passes_through_growth_without_rehydrate() {
    let stats = RuntimeSmartContextTransformStats {
        artifacts_stored: 1,
        tool_outputs_condensed: 1,
        duplicate_texts: 0,
        cross_turn_duplicate_texts: 0,
        repeat_tool_output_refs: 0,
        blob_outputs_condensed: 0,
        rehydrated_refs: 0,
        static_context_deltas: 0,
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
