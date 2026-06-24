use super::super::*;

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
    assert!(log.contains("smart_context_disabled"));
    assert!(log.contains("decision=pass_through"));
    assert!(!log.contains("runtime_proxy_worker_panic"));

    let rewritten_after_panic = prepare_runtime_smart_context_websocket_text(
        45,
        &request_text,
        &handshake_request,
        &shared,
        "main",
    );
    assert!(matches!(rewritten_after_panic, Cow::Borrowed(_)));
    assert_eq!(rewritten_after_panic.as_ref(), request_text.as_str());
    let log = fs::read_to_string(&shared.log_path).expect("runtime log should be readable");
    assert!(log.contains("reason=panic_cooldown"));
}

#[test]
fn smart_context_websocket_unicode_static_context_does_not_enter_panic_cooldown() {
    let shared = smart_context_test_shared("websocket-unicode-static-context");
    register_runtime_smart_context_proxy_state(&shared.log_path, true, Some(32_000), None);
    smart_context_observe_minimal_budget(&shared);
    let request_text = serde_json::json!({
        "type": "response.create",
        "model": "gpt-5.5",
        "instructions": "You are a senior engineer’s request handler.",
        "previous_response_id": "resp_previous",
        "input": [
            {
                "type": "function_call_output",
                "call_id": "call_big",
                "output": "large websocket payload\n".repeat(256)
            }
        ],
        "tools": [
            {
                "type": "custom",
                "name": "apply_patch",
                "description": "Patch files"
            },
            {
                "type": "namespace",
                "name": "mcp__prodex_sqz",
                "tools": []
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

    let rewritten = prepare_runtime_smart_context_websocket_text(
        46,
        &request_text,
        &handshake_request,
        &shared,
        "main",
    );

    assert!(matches!(rewritten, Cow::Owned(_) | Cow::Borrowed(_)));
    let log = fs::read_to_string(&shared.log_path).expect("runtime log should be readable");
    assert!(log.contains("smart_context_autopilot"));
    assert!(!log.contains("smart_context_panic"));
    assert!(!log.contains("smart_context_disabled"));
    assert!(!log.contains("reason=panic_cooldown"));
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
