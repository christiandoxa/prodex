use super::*;

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
fn smart_context_compact_session_body_does_not_panic() {
    let shared = smart_context_test_shared("compact-session-body");
    register_runtime_smart_context_proxy_state(&shared.log_path, true, Some(32_000), None);
    smart_context_observe_minimal_budget(&shared);
    let tool_output = (0..1600)
        .map(|index| {
            format!(
                "baris {index}: output panjang untuk remote compact; nilai=konfirmasi; path=crates/prodex-app/src/runtime_proxy/smart_context.rs"
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let body = serde_json::json!({
        "model": "gpt-5.5",
        "session_id": "sess-compact",
        "input": [
            {
                "type": "message",
                "role": "user",
                "content": [
                    {
                        "type": "input_text",
                        "text": "Investigasi kenapa remote compact gagal: café résumé jalur /responses/compact."
                    }
                ]
            },
            {
                "type": "function_call_output",
                "call_id": "call_big",
                "output": tool_output
            }
        ]
    })
    .to_string();
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/backend-api/prodex/responses/compact".to_string(),
        headers: vec![("Content-Type".to_string(), "application/json".to_string())],
        body: body.into_bytes(),
    };

    let rewritten = prepare_runtime_smart_context_http_body_for_profile(
        42,
        &request,
        &shared,
        RuntimeRouteKind::Compact,
        Some("main"),
    );

    assert!(matches!(rewritten, Cow::Owned(_) | Cow::Borrowed(_)));
    let log = fs::read_to_string(&shared.log_path).expect("runtime log should be readable");
    assert!(log.contains("smart_context_autopilot"));
}

#[test]
fn smart_context_compact_prepare_fault_falls_back_without_panic_recovery() {
    let shared = smart_context_test_shared("compact-explicit-fallback");
    register_runtime_smart_context_proxy_state(&shared.log_path, true, Some(32_000), None);
    smart_context_observe_minimal_budget(&shared);
    let body = serde_json::json!({
        "model": "gpt-5.5",
        "session_id": "sess-compact",
        "input": [
            {
                "type": "function_call_output",
                "call_id": "call_big",
                "output": "large compact payload\n".repeat(128)
            }
        ]
    })
    .to_string();
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/backend-api/prodex/responses/compact".to_string(),
        headers: vec![("Content-Type".to_string(), "application/json".to_string())],
        body: body.into_bytes(),
    };

    let rewritten = {
        let _fault = TestEnvVarGuard::set("PRODEX_RUNTIME_FAULT_SMART_CONTEXT_PANIC_ONCE", "1");
        prepare_runtime_smart_context_http_body_for_profile(
            43,
            &request,
            &shared,
            RuntimeRouteKind::Compact,
            Some("main"),
        )
    };
    assert!(!runtime_take_fault_injection(
        "PRODEX_RUNTIME_FAULT_SMART_CONTEXT_PANIC_ONCE"
    ));

    assert!(matches!(rewritten, Cow::Borrowed(_)));
    assert_eq!(rewritten.as_ref(), request.body.as_slice());
    let log = fs::read_to_string(&shared.log_path).expect("runtime log should be readable");
    assert!(log.contains("smart_context_prepare_fallback"));
    assert!(log.contains("route=compact"));
    assert!(log.contains("profile=main"));
    assert!(!log.contains("panic="));
    assert!(log.contains("reason=fault_injection"));
    assert!(log.contains("decision=pass_through"));
}

#[test]
fn smart_context_websocket_prepare_panic_falls_back_to_original_text() {
    let shared = smart_context_test_shared("websocket-panic-fallback");
    register_runtime_smart_context_proxy_state(&shared.log_path, true, Some(32_000), None);
    smart_context_observe_minimal_budget(&shared);
    let request_text = serde_json::json!({
        "type": "response.create",
        "model": "gpt-5.5",
        "session_id": "sess-websocket",
        "input": [
            {
                "type": "function_call_output",
                "call_id": "call_big",
                "output": "large websocket payload\n".repeat(128)
            }
        ]
    })
    .to_string();
    let handshake_request = RuntimeProxyRequest {
        method: "GET".to_string(),
        path_and_query: "/backend-api/prodex/responses".to_string(),
        headers: Vec::new(),
        body: Vec::new(),
    };

    let rewritten = {
        let _fault = TestEnvVarGuard::set("PRODEX_RUNTIME_FAULT_SMART_CONTEXT_UNWIND_ONCE", "1");
        prepare_runtime_smart_context_websocket_text(
            44,
            &request_text,
            &handshake_request,
            &shared,
            "main",
        )
    };
    assert!(!runtime_take_fault_injection(
        "PRODEX_RUNTIME_FAULT_SMART_CONTEXT_UNWIND_ONCE"
    ));

    assert!(matches!(rewritten, Cow::Borrowed(_)));
    assert_eq!(rewritten.as_ref(), request_text.as_str());
    let log = fs::read_to_string(&shared.log_path).expect("runtime log should be readable");
    assert!(log.contains("smart_context_panic"));
    assert!(log.contains("transport=websocket"));
    assert!(log.contains("route=websocket"));
    assert!(log.contains("profile=main"));
    assert!(log.contains("panic=non_string_panic"));
    assert!(log.contains("decision=pass_through"));
    assert!(!log.contains("runtime_proxy_worker_panic"));
}

#[test]
fn smart_context_panic_recovery_suppresses_only_smart_context_hook_output() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    let _guard = acquire_test_runtime_lock();

    struct PanicHookRestore {
        previous: Option<RuntimeSmartContextPanicHook>,
    }

    impl Drop for PanicHookRestore {
        fn drop(&mut self) {
            if let Some(previous) = self.previous.take() {
                std::panic::set_hook(previous);
            }
        }
    }

    let hook_calls = Arc::new(AtomicUsize::new(0));
    let restore = PanicHookRestore {
        previous: Some(std::panic::take_hook()),
    };
    let hook_calls_for_hook = Arc::clone(&hook_calls);
    std::panic::set_hook(Box::new(move |_| {
        hook_calls_for_hook.fetch_add(1, Ordering::SeqCst);
    }));

    let smart_context_panic = catch_runtime_smart_context_unwind_silently(|| {
        std::panic::panic_any(RuntimeSmartContextInjectedPanic);
    });
    assert!(smart_context_panic.is_err());
    assert_eq!(hook_calls.load(Ordering::SeqCst), 0);

    let normal_panic = std::panic::catch_unwind(|| {
        panic!("normal panic hook should still run");
    });
    assert!(normal_panic.is_err());
    assert_eq!(hook_calls.load(Ordering::SeqCst), 1);

    drop(restore);
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
fn smart_context_condenses_completed_tool_call_arguments() {
    let arguments = serde_json::json!({
        "command": "python3",
        "script": "print('large historical argument')\n".repeat(160),
    });
    let argument_text = serde_json::to_string(&arguments).unwrap();
    let mut value = serde_json::json!({
        "input": [
            {
                "type": "function_call",
                "call_id": "call_1",
                "arguments": arguments
            },
            {
                "type": "function_call_output",
                "call_id": "call_1",
                "output": "completed"
            }
        ]
    });
    let mut store = RuntimeSmartContextArtifactStore::default();
    let mut stats = RuntimeSmartContextTransformStats::default();

    runtime_smart_context_condense_historical_tool_call_arguments(
        &mut value,
        &mut store,
        9,
        runtime_proxy_crate::SmartContextTokenBudgetTier::Minimal,
        8 * 1024,
        &mut stats,
    );

    let replacement = value["input"][0]["arguments"].as_str().unwrap();
    assert!(replacement.starts_with("psc args psc:"));
    assert!(replacement.contains("b="));
    assert!(replacement.contains("p:"));
    assert!(replacement.len().saturating_mul(4) < argument_text.len());
    assert!(store.artifact_ref_for_exact_text(&argument_text).is_some());
    assert_eq!(stats.artifacts_stored, 1);
    assert_eq!(stats.tool_call_args_condensed, 1);
}

#[test]
fn smart_context_repeated_tool_call_arguments_use_short_repeat_ref() {
    let arguments = serde_json::json!({
        "cmd": "python3",
        "script": "print('same repeated historical argument')\n".repeat(180),
    });
    let argument_text = serde_json::to_string(&arguments).unwrap();
    let mut value = serde_json::json!({
        "input": [
            {
                "type": "function_call",
                "call_id": "call_1",
                "arguments": arguments
            },
            {
                "type": "function_call_output",
                "call_id": "call_1",
                "output": "completed"
            },
            {
                "type": "function_call",
                "call_id": "call_2",
                "arguments": serde_json::from_str::<serde_json::Value>(&argument_text).unwrap()
            },
            {
                "type": "function_call_output",
                "call_id": "call_2",
                "output": "completed"
            }
        ]
    });
    let mut store = RuntimeSmartContextArtifactStore::default();
    let mut stats = RuntimeSmartContextTransformStats::default();

    runtime_smart_context_condense_historical_tool_call_arguments(
        &mut value,
        &mut store,
        9,
        runtime_proxy_crate::SmartContextTokenBudgetTier::Minimal,
        8 * 1024,
        &mut stats,
    );

    let first = value["input"][0]["arguments"].as_str().unwrap();
    let second = value["input"][2]["arguments"].as_str().unwrap();
    assert!(first.starts_with("psc args psc:"));
    assert!(second.starts_with("psc args rep psc:"));
    assert!(!second.contains("same repeated historical argument"));
    assert!(second.len() < first.len());
    assert!(store.artifact_ref_for_exact_text(&argument_text).is_some());
    assert_eq!(stats.artifacts_stored, 1);
    assert_eq!(stats.tool_call_args_condensed, 2);
}

#[test]
fn smart_context_similar_tool_call_arguments_use_delta_ref() {
    let common_prefix = "let shared = 1;\n".repeat(160);
    let common_suffix = "println!(\"done\");\n".repeat(120);
    let first_script = format!("{common_prefix}println!(\"alpha\");\n{common_suffix}");
    let second_script = format!("{common_prefix}println!(\"beta\");\n{common_suffix}");
    let first_arguments = serde_json::json!({
        "cmd": "python3",
        "script": first_script,
    });
    let second_arguments = serde_json::json!({
        "cmd": "python3",
        "script": second_script,
    });
    let first_text = serde_json::to_string(&first_arguments).unwrap();
    let second_text = serde_json::to_string(&second_arguments).unwrap();
    let mut value = serde_json::json!({
        "input": [
            {
                "type": "function_call",
                "call_id": "call_1",
                "arguments": first_arguments
            },
            {
                "type": "function_call_output",
                "call_id": "call_1",
                "output": "completed"
            },
            {
                "type": "function_call",
                "call_id": "call_2",
                "arguments": second_arguments
            },
            {
                "type": "function_call_output",
                "call_id": "call_2",
                "output": "completed"
            }
        ]
    });
    let mut store = RuntimeSmartContextArtifactStore::default();
    let mut stats = RuntimeSmartContextTransformStats::default();

    runtime_smart_context_condense_historical_tool_call_arguments(
        &mut value,
        &mut store,
        10,
        runtime_proxy_crate::SmartContextTokenBudgetTier::Minimal,
        8 * 1024,
        &mut stats,
    );

    let first = value["input"][0]["arguments"].as_str().unwrap();
    let second = value["input"][2]["arguments"].as_str().unwrap();
    assert!(first.starts_with("psc args psc:"));
    assert!(second.starts_with("psc args d psc:"));
    assert!(second.contains(" base=psc:"));
    assert!(second.contains(" pre="));
    assert!(second.contains(" suf="));
    assert!(second.contains(" ih=sc:"));
    assert!(!second.contains(&common_prefix));
    assert!(!second.contains(&common_suffix));
    assert!(second.len().saturating_mul(4) < second_text.len());
    assert!(store.artifact_ref_for_exact_text(&first_text).is_some());
    assert!(store.artifact_ref_for_exact_text(&second_text).is_some());
    assert_eq!(stats.artifacts_stored, 2);
    assert_eq!(stats.tool_call_args_condensed, 2);
}

#[test]
fn smart_context_keeps_active_tool_call_arguments_exact() {
    let completed_arguments = serde_json::json!({
        "cmd": "python3",
        "script": "print('completed historical argument')\n".repeat(180),
    });
    let active_arguments = serde_json::json!({
        "cmd": "python3",
        "script": "print('active current argument must remain exact')\n".repeat(180),
    });
    let mut value = serde_json::json!({
        "input": [
            {
                "type": "function_call",
                "call_id": "call_1",
                "arguments": completed_arguments
            },
            {
                "type": "function_call_output",
                "call_id": "call_1",
                "output": "completed"
            },
            {
                "type": "function_call",
                "call_id": "call_active",
                "arguments": active_arguments
            }
        ]
    });
    let original_active = value["input"][2]["arguments"].clone();
    let mut store = RuntimeSmartContextArtifactStore::default();
    let mut stats = RuntimeSmartContextTransformStats::default();

    runtime_smart_context_condense_historical_tool_call_arguments(
        &mut value,
        &mut store,
        11,
        runtime_proxy_crate::SmartContextTokenBudgetTier::Minimal,
        8 * 1024,
        &mut stats,
    );

    assert!(value["input"][0]["arguments"].as_str().is_some());
    assert_eq!(value["input"][2]["arguments"], original_active);
    assert_eq!(stats.artifacts_stored, 1);
    assert_eq!(stats.tool_call_args_condensed, 1);
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
