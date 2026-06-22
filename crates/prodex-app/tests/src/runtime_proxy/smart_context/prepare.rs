use super::*;

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
fn smart_context_large_websocket_payload_minifies_without_rewrite_panic() {
    let shared = smart_context_test_shared("large-websocket-minify");
    register_runtime_smart_context_proxy_state(&shared.log_path, true, Some(32_000), None);
    smart_context_observe_minimal_budget(&shared);
    let output = (0..4200)
        .map(|index| format!("line {index}: noisy resumed goal output with src/main.rs:{index}:1"))
        .collect::<Vec<_>>()
        .join("\n");
    let body = serde_json::to_string_pretty(&serde_json::json!({
        "type": "response.create",
        "model": "gpt-5.5",
        "previous_response_id": "resp_large_ws",
        "session_id": "sess-large-ws",
        "input": [{
            "type": "function_call_output",
            "call_id": "call_large_ws",
            "output": output
        }]
    }))
    .unwrap();
    assert!(body.len() > SMART_CONTEXT_WEBSOCKET_REWRITE_MAX_BYTES);
    let handshake_request = RuntimeProxyRequest {
        method: "GET".to_string(),
        path_and_query: "/backend-api/codex/v1/responses".to_string(),
        headers: Vec::new(),
        body: Vec::new(),
    };

    let prepared = prepare_runtime_smart_context_websocket_text(
        146,
        &body,
        &handshake_request,
        &shared,
        "main",
    );

    let prepared_text = prepared.as_ref();
    let value = serde_json::from_str::<serde_json::Value>(prepared_text).unwrap();
    assert_eq!(
        value["previous_response_id"].as_str(),
        Some("resp_large_ws")
    );
    assert_eq!(value["session_id"].as_str(), Some("sess-large-ws"));
    assert_eq!(
        value["input"][0]["output"].as_str().unwrap().len(),
        output.len()
    );
    let log = fs::read_to_string(&shared.log_path).expect("runtime log should be readable");
    assert!(log.contains("reason=websocket_large_payload"));
    assert!(!log.contains("smart_context_panic"));
    assert!(!log.contains("panic_cooldown"));
}

#[test]
fn smart_context_websocket_generate_false_prewarm_skips_rewrite() {
    let shared = smart_context_test_shared("websocket-generate-false");
    register_runtime_smart_context_proxy_state(&shared.log_path, true, Some(32_000), None);
    smart_context_observe_minimal_budget(&shared);
    let tool_description = "large prewarm tool schema ".repeat(2200);
    let body = serde_json::to_string_pretty(&serde_json::json!({
        "type": "response.create",
        "model": "gpt-5.5",
        "instructions": "prewarm instructions ".repeat(900),
        "generate": false,
        "input": [],
        "tools": [{
            "type": "function",
            "name": "large_schema",
            "description": tool_description,
            "parameters": {
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Search query"
                    }
                }
            }
        }]
    }))
    .unwrap();
    assert!(body.len() < SMART_CONTEXT_WEBSOCKET_REWRITE_MAX_BYTES);
    let handshake_request = RuntimeProxyRequest {
        method: "GET".to_string(),
        path_and_query: "/backend-api/codex/v1/responses".to_string(),
        headers: Vec::new(),
        body: Vec::new(),
    };

    let prepared = prepare_runtime_smart_context_websocket_text(
        147,
        &body,
        &handshake_request,
        &shared,
        "main",
    );

    let value = serde_json::from_str::<serde_json::Value>(prepared.as_ref()).unwrap();
    assert_eq!(value["generate"].as_bool(), Some(false));
    assert_eq!(value["type"].as_str(), Some("response.create"));
    assert_eq!(
        value["tools"][0]["description"].as_str().unwrap().len(),
        tool_description.len()
    );
    let log = fs::read_to_string(&shared.log_path).expect("runtime log should be readable");
    assert!(log.contains("reason=websocket_generate_false"));
    assert!(!log.contains("smart_context_panic"));
    assert!(!log.contains("decision=rewritten"));
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
