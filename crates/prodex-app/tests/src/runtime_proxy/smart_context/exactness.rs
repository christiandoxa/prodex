use super::*;

#[test]
fn smart_context_state_snapshots_share_artifact_content() {
    let shared = smart_context_test_shared("snapshot-artifact-arc");
    register_runtime_smart_context_proxy_state(&shared, true, None, None);

    let (_, first) = runtime_smart_context_proxy_state_snapshot(&shared).unwrap();
    let (_, second) = runtime_smart_context_proxy_state_snapshot(&shared).unwrap();

    assert!(Arc::ptr_eq(&first.artifacts, &second.artifacts));
}

#[test]
fn smart_context_prepare_exact_returns_original_bytes_without_state_mutation() {
    let shared = smart_context_test_shared("prepare-minify-exact");
    register_runtime_smart_context_proxy_state(&shared, true, None, None);
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/backend-api/codex/v1/responses".to_string(),
        headers: vec![("x-prodex-smart-context".to_string(), "exact".to_string())],
        body: br#"{
          "input": [
            {
              "type": "message",
              "role": "developer",
              "content": "keep  spaces\ninside string"
            }
          ]
        }"#
        .to_vec(),
    };
    let before_state = smart_context_test_state_snapshot(&shared);

    let prepared =
        prepare_runtime_smart_context_http_body(77, &request, &shared, RuntimeRouteKind::Responses)
            .expect("smart context prepare");

    assert!(matches!(prepared, Cow::Borrowed(_)));
    assert_eq!(prepared.as_ref(), request.body.as_slice());
    assert_eq!(smart_context_test_state_snapshot(&shared), before_state);
}

#[test]
fn smart_context_prepare_noop_returns_original_bytes() {
    let shared = smart_context_test_shared("prepare-noop-exact-bytes");
    register_runtime_smart_context_proxy_state(&shared, true, None, None);
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/backend-api/codex/v1/responses".to_string(),
        headers: Vec::new(),
        body: br#"{
          "model": "gpt-5.5",
          "input": [{"type":"message","role":"user","content":"short request"}]
        }"#
        .to_vec(),
    };
    let before_state = smart_context_test_state_snapshot(&shared);

    let prepared = prepare_runtime_smart_context_http_body(
        770,
        &request,
        &shared,
        RuntimeRouteKind::Responses,
    )
    .expect("smart context prepare");

    assert!(matches!(prepared, Cow::Borrowed(_)));
    assert_eq!(prepared.as_ref(), request.body.as_slice());
    assert_eq!(smart_context_test_state_snapshot(&shared), before_state);
}

#[test]
#[cfg(feature = "mojo-quota")]
fn smart_context_subthreshold_rewrite_discards_planned_state() {
    let shared = smart_context_test_shared("subthreshold-plan-discard");
    register_runtime_smart_context_proxy_state(&shared, true, None, None);
    smart_context_observe_minimal_budget(&shared);
    let repeated = "x".repeat(SMART_CONTEXT_DUPLICATE_TEXT_MIN_BYTES + 476);
    let request = smart_context_test_request(serde_json::json!({
        "instructions": "Keep this static instruction.",
        "input": [
            {"type":"function_call_output", "call_id":"audit", "output": repeated},
            {"type":"function_call_output", "call_id":"audit", "output": repeated}
        ]
    }));

    let before = smart_context_test_state_snapshot(&shared);
    let prepared =
        prepare_runtime_smart_context_http_body(81, &request, &shared, RuntimeRouteKind::Responses)
            .expect("smart context prepare");

    assert!(matches!(prepared, Cow::Borrowed(_)));
    assert_eq!(prepared.as_ref(), request.body.as_slice());
    assert_eq!(smart_context_test_state_snapshot(&shared), before);
    let log = read_runtime_proxy_test_log(&shared.log_path);
    assert!(log.contains("self_check=token_savings_below_safety_margin"));
}

#[test]
fn smart_context_prepare_passes_invalid_json_unchanged() {
    let shared = smart_context_test_shared("prepare-invalid-json");
    register_runtime_smart_context_proxy_state(&shared, true, None, None);
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/backend-api/codex/v1/responses".to_string(),
        headers: Vec::new(),
        body: b"{ invalid\n".to_vec(),
    };

    let prepared =
        prepare_runtime_smart_context_http_body(78, &request, &shared, RuntimeRouteKind::Responses)
            .expect("smart context prepare");

    assert!(matches!(&prepared, Cow::Borrowed(_)));
    assert_eq!(prepared.as_ref(), request.body.as_slice());
}

#[test]
#[cfg(feature = "mojo-quota")]
fn smart_context_prepare_passes_too_deep_json_unchanged_without_panic_fallback() {
    let shared = smart_context_test_shared("prepare-too-deep-json");
    register_runtime_smart_context_proxy_state(&shared, true, Some(32_000), None);
    let mut nested = serde_json::Value::String("leaf".to_string());
    for _ in 0..RUNTIME_SMART_CONTEXT_MAX_JSON_DEPTH {
        nested = serde_json::json!({ "nested": nested });
    }
    let body = serde_json::json!({
        "model": "gpt-5.5",
        "input": [{
            "type": "message",
            "role": "user",
            "content": "keep exact"
        }],
        "metadata": nested
    })
    .to_string();
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/backend-api/codex/v1/responses".to_string(),
        headers: Vec::new(),
        body: body.into_bytes(),
    };

    let prepared =
        prepare_runtime_smart_context_http_body(79, &request, &shared, RuntimeRouteKind::Responses)
            .expect("smart context prepare");

    assert!(matches!(&prepared, Cow::Borrowed(_)));
    assert_eq!(prepared.as_ref(), request.body.as_slice());
    let log = read_runtime_proxy_test_log(&shared.log_path);
    assert!(log.contains("smart_context_prepare_fallback"));
    assert!(log.contains("reason=json_depth_limit"));
    assert!(log.contains("decision=pass_through"));
    assert!(!log.contains("smart_context_panic"));
}

#[test]
fn smart_context_json_shape_guard_rejects_excessive_node_count_iteratively() {
    let value = serde_json::Value::Array(vec![
        serde_json::Value::Null;
        RUNTIME_SMART_CONTEXT_MAX_JSON_NODES + 1
    ]);

    assert_eq!(
        runtime_smart_context_unsupported_json_shape_reason(&value),
        Some("json_node_limit")
    );
}

#[test]
fn smart_context_self_check_passes_through_growth_without_rehydrate() {
    let stats = RuntimeSmartContextTransformStats {
        artifacts_stored: 1,
        tool_outputs_condensed: 1,
        tool_call_args_condensed: 0,
        duplicate_texts: 0,
        cross_turn_duplicate_texts: 0,
        repeat_tool_output_refs: 0,
        blob_outputs_condensed: 0,
        rehydrated_refs: 0,
        static_context_deltas: 0,
        repo_state_facts: 0,
        ..RuntimeSmartContextTransformStats::default()
    };

    assert_eq!(
        runtime_smart_context_rewrite_self_check(100, 101, &stats),
        "growth"
    );
}

#[test]
fn smart_context_prepare_preserves_deferred_tool_catalog_during_rewrite() {
    let shared = smart_context_test_shared("rewrite-valid-json");
    register_runtime_smart_context_proxy_state(&shared, true, None, None);
    smart_context_observe_minimal_budget(&shared);
    let output = std::iter::once("error[E0425]: missing symbol at src/lib.rs:42:13".to_string())
        .chain((0..650).map(|index| format!("line {index}: noisy build output")))
        .collect::<Vec<_>>()
        .join("\n");
    let original = serde_json::json!({
        "model": "gpt-4o",
        "tools": [{"type": "tool_search", "execution": "client",
            "parameters": {"type": "object", "properties": {"query": {"type": "string"}}}}],
        "input": [
            {"type": "message", "role": "developer", "content": [
                {"type": "input_text", "text": "Use tools within their documented scope."},
                {"type": "input_text", "text": "<tools>\nDeferred tool namespaces:\n- mcp__notes: Tools for working with notes.\n</tools>"}
            ]},
            {"type": "function_call_output", "call_id": "call_json_valid_1", "output": output},
            {"type": "function_call_output", "call_id": "call_json_valid_2", "output": output}
        ]
    });
    let request = smart_context_test_request(original.clone());

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
    assert_eq!(value["tools"], original["tools"]);
    assert_eq!(value["input"][0], original["input"][0]);
    assert_eq!(value["input"][3]["role"].as_str(), Some("developer"));
    assert!(
        value["input"][1]["output"]
            .as_str()
            .unwrap()
            .contains("error[E0425]: missing symbol at src/lib.rs:42:13")
    );
    assert!(
        value["input"][2]["output"]
            .as_str()
            .unwrap()
            .contains("prodex-context-ref v=1")
    );
    let mut expanded = runtime_smart_context_expand_inline_references(&original, &value).unwrap();
    assert!(runtime_smart_context_inline_reference_round_trip_is_exact(
        &original,
        &mut expanded
    ));
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
fn smart_context_codex_01671_preserves_incremental_tools_and_hosted_search() {
    for compaction in [false, true] {
        let shared = smart_context_test_shared("codex-incremental-tools");
        register_runtime_smart_context_proxy_state(&shared, true, None, None);
        smart_context_observe_minimal_budget(&shared);
        let repeated = (0..650)
            .map(|index| format!("line {index}: repeated build diagnostics"))
            .collect::<Vec<_>>()
            .join("\n");
        let mut original = serde_json::json!({
            "model": "gpt-4o",
            "instructions": "",
            "stream": true,
            "tools": [{"type": "web_search", "search_context_size": "medium"}],
            "input": [
                {"type": "additional_tools", "role": "developer", "tools": [
                    {"type": "namespace", "name": "mcp__fixture", "description": "Fixture tools",
                        "tools": [{"type": "function", "name": "read", "parameters": {
                            "type": "object", "properties": {"path": {"type": "string"}}
                        }}]}
                ]},
                {"type": "message", "role": "developer", "content": [{
                    "type": "input_text", "text": "The following tools are no longer available. Do not call them:\n- mcp__fixture.read"
                }]},
                {"type": "additional_tools", "role": "developer", "tools": [
                    {"type": "namespace", "name": "mcp__fixture", "description": "Updated fixture tools",
                        "tools": [{"type": "function", "name": "read", "parameters": {
                            "type": "object", "properties": {"path": {"type": "string"}, "limit": {"type": "integer"}}
                        }}]}
                ]},
                {"type": "function_call_output", "call_id": "call_first", "output": repeated},
                {"type": "function_call_output", "call_id": "call_second", "output": repeated}
            ]
        });
        if compaction {
            original["input"]
                .as_array_mut()
                .unwrap()
                .push(serde_json::json!({"type": "compaction_trigger"}));
        }
        let request = smart_context_test_request(original.clone());
        let prepared = prepare_runtime_smart_context_http_body(
            1671,
            &request,
            &shared,
            RuntimeRouteKind::Responses,
        )
        .unwrap();
        let Cow::Owned(body) = prepared else {
            panic!("expected a real duplicate-output rewrite, not a no-op");
        };
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(value["tools"], original["tools"]);
        assert_eq!(value["instructions"], original["instructions"]);
        for index in 0..3 {
            assert_eq!(value["input"][index], original["input"][index]);
        }
        if compaction {
            assert_eq!(value["input"][5], original["input"][5]);
        }
        assert!(
            value["input"][4]["output"]
                .as_str()
                .unwrap()
                .contains("prodex-context-ref v=1")
        );
        let mut expanded =
            runtime_smart_context_expand_inline_references(&original, &value).unwrap();
        assert!(runtime_smart_context_inline_reference_round_trip_is_exact(
            &original,
            &mut expanded
        ));
    }
}
