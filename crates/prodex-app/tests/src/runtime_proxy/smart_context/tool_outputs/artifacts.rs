use super::super::*;

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
    assert!(output.contains("psc art psc:"));
    assert!(output.contains("b="));
    assert!(output.contains(&format!("b={}", original_output.len())));
    assert!(!output.contains("prodex-artifact:"));
    assert!(!output.contains(" h="));
    assert!(output.contains(" lines=#Lx-Ly"));
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
    assert!(output.contains("psc art psc:"));
    assert!(output.contains(SMART_CONTEXT_LABEL_SUMMARY));
    assert!(output.contains(SMART_CONTEXT_LABEL_CRITICAL_EXACT));
    assert!(output.contains("psc:"));
    assert!(output.contains("#L"));
    assert!(output.contains("error[E0425]"));
    assert!(output.contains("runtime_proxy::progressive_output"));
    assert!(!output.contains(hidden_tail));
    assert!(!output.contains("noise line 419"));

    assert_eq!(
        value["input"].as_array().unwrap().len(),
        1,
        "visible artifact refs should not need an extra manifest"
    );
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
    assert!(deduped.contains(&format!(",L{}-L{}", CHUNK_LINES + 1, CHUNK_LINES * 2)));
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
