use super::*;

#[test]
fn smart_context_prepare_rewrites_when_savings_and_critical_signals_preserved() {
    let shared = smart_context_test_shared("rewrite-savings");
    register_runtime_smart_context_proxy_state(&shared, true, None, None);
    smart_context_observe_minimal_budget(&shared);
    let output = std::iter::once("error: failed at src/main.rs:10:5".to_string())
        .chain((0..500).map(|index| format!("line {index}: noisy build output")))
        .collect::<Vec<_>>()
        .join("\n");
    let request = smart_context_test_request(serde_json::json!({
        "input": [
            {"type": "function_call_output", "call_id": "call_1", "output": output},
            {"type": "function_call_output", "call_id": "call_2", "output": output}
        ]
    }));
    let before_len = request.body.len();

    let rewritten =
        prepare_runtime_smart_context_http_body(42, &request, &shared, RuntimeRouteKind::Responses)
            .expect("smart context prepare");

    let Cow::Owned(body) = rewritten else {
        panic!("expected rewritten body");
    };
    assert!(body.len() < before_len);
    let text = String::from_utf8(body).unwrap();
    assert!(text.contains("prodex-context-ref v=1"));
    assert!(text.contains("\"role\":\"developer\""));
    assert!(!text.contains("psc2:"));
    assert!(text.contains("error: failed at src/main.rs:10:5"));
}

#[test]
fn smart_context_single_occurrence_stays_inline_without_persisting() {
    let shared = smart_context_test_shared("durable-before-reference");
    let artifact_path = register_persistent_runtime_smart_context_test_state(&shared, None);
    smart_context_observe_minimal_budget(&shared);
    let output = std::iter::once("error: durable reference at src/main.rs:10:5".to_string())
        .chain((0..500).map(|index| format!("line {index}: durable build output")))
        .collect::<Vec<_>>()
        .join("\n");
    let request = smart_context_test_request(serde_json::json!({
        "input": [{
            "type": "function_call_output",
            "call_id": "call_durable",
            "output": output
        }]
    }));

    let before = smart_context_test_state_snapshot(&shared);
    let first =
        prepare_runtime_smart_context_http_body(40, &request, &shared, RuntimeRouteKind::Responses)
            .expect("smart context prepare");
    assert!(matches!(first, Cow::Borrowed(_)));
    assert_eq!(first.as_ref(), request.body.as_slice());
    assert_eq!(smart_context_test_state_snapshot(&shared), before);
    assert!(!artifact_path.exists());

    let second =
        prepare_runtime_smart_context_http_body(41, &request, &shared, RuntimeRouteKind::Responses)
            .expect("smart context prepare");
    assert!(matches!(second, Cow::Borrowed(_)));
    assert_eq!(second.as_ref(), request.body.as_slice());
    assert_eq!(smart_context_test_state_snapshot(&shared), before);
}

#[test]
fn smart_context_http_prepare_rewritten_body_remains_valid_json() {
    let shared = smart_context_test_shared("rewrite-valid-json");
    register_runtime_smart_context_proxy_state(&shared, true, None, None);
    smart_context_observe_minimal_budget(&shared);
    let output = std::iter::once("error[E0425]: missing symbol at src/lib.rs:42:13".to_string())
        .chain((0..650).map(|index| format!("line {index}: noisy build output")))
        .collect::<Vec<_>>()
        .join("\n");
    let request = smart_context_test_request(serde_json::json!({
        "model": "gpt-4o",
        "input": [
            {"type": "function_call_output", "call_id": "call_json_valid_1", "output": output},
            {"type": "function_call_output", "call_id": "call_json_valid_2", "output": output}
        ]
    }));

    let rewritten = prepare_runtime_smart_context_http_body(
        142,
        &request,
        &shared,
        RuntimeRouteKind::Responses,
    )
    .expect("smart context prepare");

    let Cow::Owned(body) = rewritten else {
        panic!("expected smart-context rewrite");
    };
    let value = serde_json::from_slice::<serde_json::Value>(&body)
        .expect("rewritten prepare body must remain valid JSON");
    assert_eq!(value["model"].as_str(), Some("gpt-4o"));
    assert_eq!(value["input"][2]["role"].as_str(), Some("developer"));
    assert!(
        value["input"][0]["output"]
            .as_str()
            .unwrap()
            .contains("error[E0425]: missing symbol at src/lib.rs:42:13")
    );
    assert!(
        value["input"][1]["output"]
            .as_str()
            .unwrap()
            .contains("prodex-context-ref v=1")
    );
}

#[test]
fn smart_context_prepare_explicit_line_ref_rehydrates_exact_critical_content() {
    let shared = smart_context_test_shared("prepare-explicit-ref-exact");
    register_runtime_smart_context_proxy_state(&shared, true, Some(32_000), None);
    let artifact_text = "\
setup line
panic: exact hidden failure
src/runtime.rs:88:13
tail line";
    let artifact = with_runtime_smart_context_artifacts(&shared, |store| {
        store.insert_text(artifact_text).unwrap()
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
    )
    .expect("smart context prepare");

    let value = serde_json::from_slice::<serde_json::Value>(prepared.as_ref()).unwrap();
    assert_eq!(
        value["input"][0]["content"].as_str(),
        Some("inspect panic: exact hidden failure\nsrc/runtime.rs:88:13")
    );
}

#[test]
fn smart_context_prepare_affinity_without_candidate_is_exact_noop() {
    for (name, request) in [
        (
            "previous",
            RuntimeProxyRequest {
                method: "POST".to_string(),
                path_and_query: "/backend-api/codex/v1/responses".to_string(),
                headers: Vec::new(),
                body: serde_json::to_vec_pretty(&serde_json::json!({
                    "model": "gpt-4o",
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
                    "model": "gpt-4o",
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
                    "model": "gpt-4o",
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
        register_runtime_smart_context_proxy_state(&shared, true, Some(32_000), None);
        let original = serde_json::from_slice::<serde_json::Value>(&request.body).unwrap();

        let prepared = prepare_runtime_smart_context_http_body(
            144,
            &request,
            &shared,
            RuntimeRouteKind::Responses,
        )
        .expect("smart context prepare");

        let value = serde_json::from_slice::<serde_json::Value>(prepared.as_ref()).unwrap();
        assert_eq!(
            value, original,
            "{name} affinity payload changed semantically"
        );
        let log = read_runtime_proxy_test_log(&shared.log_path);
        assert!(
            log.contains("reason=no_duplicate_candidate"),
            "{name} affinity payload should decline without a rewrite candidate: {log}"
        );
        assert!(
            !log.contains("decision=require_exact"),
            "{name} affinity should not force full exact passthrough: {log}"
        );
    }
}

#[test]
fn smart_context_http_and_websocket_prepare_match_for_same_payload_class() {
    let output = std::iter::once("error: parity failure at src/lib.rs:12:5".to_string())
        .chain((0..620).map(|index| format!("line {index}: shared noisy output")))
        .collect::<Vec<_>>()
        .join("\n");
    let body = serde_json::json!({
        "type": "response.create",
        "model": "gpt-5.5",
        "input": [
            {"type": "function_call_output", "call_id": "call_parity_1", "output": output},
            {"type": "function_call_output", "call_id": "call_parity_2", "output": output}
        ]
    })
    .to_string();
    let http_shared = smart_context_test_shared("prepare-http-parity");
    let ws_shared = smart_context_test_shared("prepare-ws-parity");
    register_runtime_smart_context_proxy_state(&http_shared, true, Some(32_000), None);
    register_runtime_smart_context_proxy_state(&ws_shared, true, Some(32_000), None);
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
    )
    .expect("smart context prepare");
    let websocket = prepare_runtime_smart_context_websocket_text(
        145,
        &body,
        &handshake_request,
        &ws_shared,
        "main",
    )
    .expect("smart context prepare");

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
fn smart_context_large_websocket_payload_returns_original_bytes() {
    let shared = smart_context_test_shared("large-websocket-minify");
    register_runtime_smart_context_proxy_state(&shared, true, Some(32_000), None);
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
    )
    .expect("smart context prepare");

    assert!(matches!(prepared, Cow::Borrowed(_)));
    assert_eq!(prepared.as_ref(), body);
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
    let log = read_runtime_proxy_test_log(&shared.log_path);
    assert!(log.contains("reason=websocket_large_payload"));
    assert!(!log.contains("smart_context_panic"));
    assert!(!log.contains("panic_cooldown"));
}

#[test]
fn smart_context_websocket_generate_false_prewarm_skips_rewrite() {
    let shared = smart_context_test_shared("websocket-generate-false");
    register_runtime_smart_context_proxy_state(&shared, true, Some(32_000), None);
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
    )
    .expect("smart context prepare");

    assert!(matches!(prepared, Cow::Borrowed(_)));
    assert_eq!(prepared.as_ref(), body);
    let value = serde_json::from_str::<serde_json::Value>(prepared.as_ref()).unwrap();
    assert_eq!(value["generate"].as_bool(), Some(false));
    assert_eq!(value["type"].as_str(), Some("response.create"));
    assert_eq!(
        value["tools"][0]["description"].as_str().unwrap().len(),
        tool_description.len()
    );
    let log = read_runtime_proxy_test_log(&shared.log_path);
    assert!(log.contains("reason=websocket_generate_false"));
    assert!(!log.contains("smart_context_panic"));
    assert!(!log.contains("decision=rewritten"));
}

#[test]
fn smart_context_prepare_rewrites_affinity_continuation_under_critical_pressure() {
    let shared = smart_context_test_shared("rewrite-affinity-pressure");
    register_runtime_smart_context_proxy_state(&shared, true, None, None);
    smart_context_observe_minimal_budget(&shared);
    let output = std::iter::once("error: failed at src/main.rs:10:5".to_string())
        .chain((0..600).map(|index| format!("line {index}: noisy continuation output")))
        .collect::<Vec<_>>()
        .join("\n");
    let mut request = smart_context_test_request(serde_json::json!({
        "previous_response_id": "resp_owned",
        "session_id": "sess_owned",
        "input": [
            {"type": "function_call_output", "call_id": "call_1", "output": output},
            {"type": "function_call_output", "call_id": "call_2", "output": output}
        ]
    }));
    request.headers.push((
        "x-codex-turn-state".to_string(),
        "turn_state_owned".to_string(),
    ));
    let before_len = request.body.len();

    let rewritten =
        prepare_runtime_smart_context_http_body(43, &request, &shared, RuntimeRouteKind::Responses)
            .expect("smart context prepare");

    let Cow::Owned(body) = rewritten else {
        panic!("expected critical continuation to rewrite");
    };
    assert!(body.len() < before_len);
    let value = serde_json::from_slice::<serde_json::Value>(&body).unwrap();
    assert_eq!(value["previous_response_id"].as_str(), Some("resp_owned"));
    assert_eq!(value["session_id"].as_str(), Some("sess_owned"));
    assert_eq!(value["input"][2]["role"].as_str(), Some("developer"));
    assert!(
        value["input"][0]["output"]
            .as_str()
            .unwrap()
            .contains("error: failed at src/main.rs:10:5")
    );
    assert!(
        value["input"][1]["output"]
            .as_str()
            .unwrap()
            .contains("prodex-context-ref v=1")
    );
    assert!(
        prodex_context::critical_signal_self_check(
            &String::from_utf8_lossy(&request.body),
            &String::from_utf8_lossy(&body),
        )
        .passed()
    );
    let log = read_runtime_proxy_test_log(&shared.log_path);
    assert!(log.contains("decision=rewritten"));
    assert!(log.contains("reasons=affinity_pressure"));
    assert!(log.contains("policy_reasons=critical_budget"));
    assert!(log.contains("self_check=ok_saved"));
}

#[test]
fn smart_context_prepare_turn_state_only_affinity_rewrites_under_critical_pressure() {
    let shared = smart_context_test_shared("rewrite-turn-state-affinity-pressure");
    register_runtime_smart_context_proxy_state(&shared, true, None, None);
    smart_context_observe_minimal_budget(&shared);
    let output = std::iter::once("error: turn state owner failed at src/lib.rs:44:9".to_string())
        .chain((0..600).map(|index| format!("line {index}: noisy turn state continuation output")))
        .collect::<Vec<_>>()
        .join("\n");
    let mut request = smart_context_test_request(serde_json::json!({
        "input": [
            {"type": "function_call_output", "call_id": "call_1", "output": output},
            {"type": "function_call_output", "call_id": "call_2", "output": output}
        ]
    }));
    request.headers.push((
        "x-codex-turn-state".to_string(),
        "turn_state_only_owner".to_string(),
    ));

    let rewritten =
        prepare_runtime_smart_context_http_body(44, &request, &shared, RuntimeRouteKind::Responses)
            .expect("smart context prepare");

    let Cow::Owned(body) = rewritten else {
        panic!("expected turn-state affinity continuation to rewrite");
    };
    assert!(body.len() < request.body.len());
    let value = serde_json::from_slice::<serde_json::Value>(&body).unwrap();
    assert!(value.get("previous_response_id").is_none());
    assert!(value.get("session_id").is_none());
    assert_eq!(value["input"][2]["role"].as_str(), Some("developer"));
    assert!(
        value["input"][0]["output"]
            .as_str()
            .unwrap()
            .contains("error: turn state owner failed at src/lib.rs:44:9")
    );
    assert!(
        value["input"][1]["output"]
            .as_str()
            .unwrap()
            .contains("prodex-context-ref v=1")
    );
    assert!(
        prodex_context::critical_signal_self_check(
            &String::from_utf8_lossy(&request.body),
            &String::from_utf8_lossy(&body),
        )
        .passed()
    );
    let log = read_runtime_proxy_test_log(&shared.log_path);
    assert!(log.contains("decision=rewritten"));
    assert!(log.contains("reasons=affinity_pressure"));
    assert!(log.contains("policy_reasons=critical_budget"));
}

#[test]
fn smart_context_prepare_missing_rehydrate_ref_fails_before_upstream() {
    let shared = smart_context_test_shared("rewrite-affinity-missing-rehydrate");
    register_runtime_smart_context_proxy_state(&shared, true, None, None);
    smart_context_observe_minimal_budget(&shared);
    let missing_ref = "prodex-artifact:sc:0123456789abcdef";
    let request = smart_context_test_request(serde_json::json!({
        "previous_response_id": "resp_owned",
        "input": [{
            "role": "user",
            "content": format!("Continue from {missing_ref}")
        }]
    }));

    let error =
        prepare_runtime_smart_context_http_body(45, &request, &shared, RuntimeRouteKind::Responses)
            .expect_err("an unresolved mandatory reference must fail closed");

    assert_eq!(error.missing_artifact_count, 1);
    let log = read_runtime_proxy_test_log(&shared.log_path);
    assert!(log.contains("smart_context_prepare_error"));
    assert!(log.contains("reason=missing_artifact_refs"));
    assert!(log.contains("missing_artifact_count=1"));
}

#[test]
fn smart_context_prepare_changed_static_context_stays_exact_without_learning() {
    let shared = smart_context_test_shared("rewrite-affinity-static-changed");
    register_runtime_smart_context_proxy_state(&shared, true, None, None);
    smart_context_observe_minimal_budget(&shared);
    let stable_rules = "Stable workspace rule. ".repeat(80);
    let first_instructions = format!("Use repo rules.\nKeep account affinity.\n{stable_rules}");
    let changed_instructions = format!("Use repo rules.\nAllow account rotation.\n{stable_rules}");
    let first = smart_context_test_request(serde_json::json!({
        "instructions": first_instructions,
        "input": [{"role": "user", "content": "first request"}]
    }));
    let changed = smart_context_test_request(serde_json::json!({
        "previous_response_id": "resp_owned",
        "instructions": changed_instructions,
        "input": [{
            "type": "function_call_output",
            "call_id": "call_1",
            "output": "error: static changed path src/lib.rs:9:1\n".repeat(600)
        }]
    }));

    let before = smart_context_test_state_snapshot(&shared);
    let first_prepared =
        prepare_runtime_smart_context_http_body(46, &first, &shared, RuntimeRouteKind::Responses)
            .expect("smart context prepare");
    let prepared =
        prepare_runtime_smart_context_http_body(47, &changed, &shared, RuntimeRouteKind::Responses)
            .expect("smart context prepare");

    assert_eq!(first_prepared.as_ref(), first.body.as_slice());
    assert_eq!(prepared.as_ref(), changed.body.as_slice());
    assert_eq!(smart_context_test_state_snapshot(&shared), before);
    let value = serde_json::from_slice::<serde_json::Value>(prepared.as_ref()).unwrap();
    assert_eq!(value["previous_response_id"].as_str(), Some("resp_owned"));
    assert_eq!(
        value["instructions"].as_str(),
        Some(changed_instructions.as_str())
    );
    assert!(
        value["input"][0]["output"]
            .as_str()
            .unwrap()
            .contains("error: static changed path src/lib.rs:9:1")
    );
    let log = read_runtime_proxy_test_log(&shared.log_path);
    assert!(log.contains("decision=pass_through"));
    assert!(log.contains("reason=no_duplicate_candidate"));
    assert!(!log.contains("static_context_changed"));
}

#[test]
fn smart_context_prepare_rewrite_preserves_static_prompt_prefix_text() {
    let shared = smart_context_test_shared("rewrite-static-prefix");
    register_runtime_smart_context_proxy_state(&shared, true, None, None);
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
            },
            {
                "type": "function_call_output",
                "call_id": "call_2",
                "output": output,
            }
        ]
    }));

    let rewritten =
        prepare_runtime_smart_context_http_body(42, &request, &shared, RuntimeRouteKind::Responses)
            .expect("smart context prepare");

    let Cow::Owned(body) = rewritten else {
        panic!("expected rewritten body");
    };
    let value = serde_json::from_slice::<serde_json::Value>(&body).unwrap();
    assert_eq!(value["instructions"].as_str(), Some(instructions));
    assert_eq!(value["system"].as_str(), Some(system));
    assert_eq!(value["developer"].as_str(), Some(developer));
    assert_eq!(value["input"][3]["role"].as_str(), Some("developer"));
    assert_eq!(value["input"][0]["content"].as_str(), Some(input_system));
    assert!(
        value["input"][1]["output"]
            .as_str()
            .unwrap()
            .contains("error: failed")
    );
    assert!(
        value["input"][2]["output"]
            .as_str()
            .unwrap()
            .contains("prodex-context-ref v=1")
    );
}

#[test]
fn smart_context_prepare_canary_out_returns_original_body() {
    let _canary = TestEnvVarGuard::set("PRODEX_SMART_CONTEXT_CANARY_PERCENT", "0");
    let _shadow = TestEnvVarGuard::unset("PRODEX_SMART_CONTEXT_SHADOW");
    let shared = smart_context_test_shared("rewrite-rollout-canary-out");
    register_runtime_smart_context_proxy_state(&shared, true, None, None);
    smart_context_observe_minimal_budget(&shared);
    let request = smart_context_test_request(serde_json::json!({
        "input": [{
            "type": "function_call_output",
            "call_id": "call_1",
            "output": "error: canary path src/lib.rs:1:1\n".repeat(500)
        }]
    }));

    let prepared =
        prepare_runtime_smart_context_http_body(88, &request, &shared, RuntimeRouteKind::Responses)
            .expect("smart context prepare");

    assert!(matches!(prepared, Cow::Borrowed(_)));
    assert_eq!(prepared.as_ref(), request.body.as_slice());
    let log = read_runtime_proxy_test_log(&shared.log_path);
    assert!(log.contains("reason=rollout_canary_out"));
}

#[test]
fn smart_context_prepare_shadow_returns_original_without_live_state_mutation() {
    let _shadow = TestEnvVarGuard::set("PRODEX_SMART_CONTEXT_SHADOW", "1");
    let _canary = TestEnvVarGuard::set("PRODEX_SMART_CONTEXT_CANARY_PERCENT", "100");
    let shared = smart_context_test_shared("rewrite-rollout-shadow");
    register_runtime_smart_context_proxy_state(&shared, true, None, None);
    smart_context_observe_minimal_budget(&shared);
    let output = "error: shadow path src/lib.rs:1:1\n".repeat(500);
    let mut request = smart_context_test_request(serde_json::json!({
        "input": [
            {"type": "function_call_output", "call_id": "call_1", "output": output},
            {"type": "function_call_output", "call_id": "call_2", "output": output}
        ]
    }));
    let session_id = (0..10_000)
        .map(|index| format!("shadow-session-{index}"))
        .find(|session_id| {
            request.headers = vec![("session_id".to_string(), session_id.clone())];
            runtime_smart_context_rollout_decision(
                89,
                &request,
                &shared,
                RuntimeRouteKind::Responses,
                RuntimeSmartContextTransport::Http,
                None,
            )
            .canary_bucket
                < SMART_CONTEXT_SHADOW_SAMPLE_BASIS_POINTS
        })
        .expect("a deterministic shadow sample");
    request.headers = vec![("session_id".to_string(), session_id)];
    let before_state = smart_context_test_state_snapshot(&shared);

    let prepared =
        prepare_runtime_smart_context_http_body(89, &request, &shared, RuntimeRouteKind::Responses)
            .expect("smart context prepare");

    assert!(matches!(prepared, Cow::Borrowed(_)));
    assert_eq!(prepared.as_ref(), request.body.as_slice());
    assert_eq!(smart_context_test_state_snapshot(&shared), before_state);
    let log = read_runtime_proxy_test_log(&shared.log_path);
    assert!(log.contains("decision=shadow_rewrite"));
    assert!(log.contains("rollout_mode=shadow"));
}

#[test]
fn smart_context_prepare_shadow_sampled_out_skips_state() {
    let _shadow = TestEnvVarGuard::set("PRODEX_SMART_CONTEXT_SHADOW", "1");
    let shared = smart_context_test_shared("rewrite-rollout-shadow-out");
    register_runtime_smart_context_proxy_state(&shared, true, None, None);
    let request = smart_context_test_request(serde_json::json!({
        "input": [{"role": "user", "content": "large body ".repeat(500)}]
    }));
    let before_state = smart_context_test_state_snapshot(&shared);

    let prepared =
        prepare_runtime_smart_context_http_body(90, &request, &shared, RuntimeRouteKind::Responses)
            .expect("smart context prepare");

    assert!(matches!(prepared, Cow::Borrowed(_)));
    assert_eq!(prepared.as_ref(), request.body.as_slice());
    assert_eq!(smart_context_test_state_snapshot(&shared), before_state);
    let log = read_runtime_proxy_test_log(&shared.log_path);
    assert!(
        log.contains("reason=rollout_shadow_sampled_out") || log.contains("decision=pass_through")
    );
}
