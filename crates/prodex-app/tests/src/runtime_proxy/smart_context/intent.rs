use super::*;

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
    assert!(signals.semantic_terms.command_kinds.contains("cargo-test"));
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
fn smart_context_intent_signals_collect_exit_status_and_command_intent() {
    let value = serde_json::json!({
        "input": [{
            "type": "message",
            "role": "user",
            "content": "Run cargo test metadata_hint for smart_context.rs; previous run exited with exit code 101 and status code 500"
        }]
    });

    let signals = runtime_smart_context_collect_intent_signals(&value);

    assert!(
        signals
            .semantic_terms
            .test_symbols
            .contains("metadata_hint")
    );
    assert!(
        signals
            .semantic_terms
            .file_paths
            .contains("smart_context.rs")
    );
    assert!(signals.semantic_terms.error_codes.contains("exit_code_101"));
    assert!(
        signals
            .semantic_terms
            .error_codes
            .contains("status_code_500")
    );
    assert!(signals.semantic_terms.command_kinds.contains("cargo-test"));
}
