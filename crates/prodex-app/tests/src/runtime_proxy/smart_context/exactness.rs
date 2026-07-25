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
fn smart_context_subthreshold_rewrite_discards_planned_state() {
    let shared = smart_context_test_shared("subthreshold-plan-discard");
    register_runtime_smart_context_proxy_state(&shared, true, None, None);
    smart_context_observe_minimal_budget(&shared);
    let repeated = "x".repeat(SMART_CONTEXT_DUPLICATE_TEXT_MIN_BYTES + 476);
    let request = smart_context_test_request(serde_json::json!({
        "input": [
            {"role": "user", "content": repeated},
            {"role": "user", "content": repeated}
        ]
    }));

    let before = smart_context_test_state_snapshot(&shared);
    let prepared =
        prepare_runtime_smart_context_http_body(81, &request, &shared, RuntimeRouteKind::Responses)
            .expect("smart context prepare");

    assert!(matches!(prepared, Cow::Borrowed(_)));
    assert_eq!(prepared.as_ref(), request.body.as_slice());
    assert_eq!(smart_context_test_state_snapshot(&shared), before);
    let log = fs::read_to_string(&shared.log_path).unwrap();
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
    let log = fs::read_to_string(&shared.log_path).expect("runtime log should be readable");
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
