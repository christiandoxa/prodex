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
