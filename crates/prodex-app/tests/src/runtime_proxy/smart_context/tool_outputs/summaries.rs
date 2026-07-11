use super::super::*;

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
    assert!(output.contains("psc art psc:"));
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
    assert!(output.contains("psc art psc:"));
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
    assert!(output.contains("psc art psc:"));
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
