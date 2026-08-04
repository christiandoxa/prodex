#[test]
fn smart_context_rewrite_telemetry_ring_records_bytes_tokens_and_fallback() {
    let shared = smart_context_test_shared("rewrite-telemetry");
    register_runtime_smart_context_proxy_state(&shared, true, None, None);
    let budget = runtime_smart_context_budget(RuntimeSmartContextBudgetInput {
        shared: &shared,
        body: br#"{"input":"test"}"#,
        route_kind: RuntimeRouteKind::Responses,
        transport: RuntimeSmartContextTransport::Http,
        profile_name: Some("main"),
        exactness_guard: runtime_proxy_crate::SmartContextExactnessGuard {
            decision: runtime_proxy_crate::SmartContextExactnessDecision::Allow,
            reasons: Vec::new(),
        },
        missing_rehydrate_refs: Vec::new(),
        static_context_changed: false,
    });
    let scope = runtime_smart_context_scope_id(&shared, Some("main")).unwrap();
    let rollout = runtime_proxy_crate::smart_context_rollout_decision(
        runtime_proxy_crate::SmartContextRolloutDecisionInput {
            enabled: true,
            explicit_exact_mode: false,
            shadow_mode: false,
            canary_percent: 100,
            stable_key: scope.to_string(),
        },
    );

    for index in 0..(SMART_CONTEXT_REWRITE_TELEMETRY_HISTORY_LIMIT + 2) {
        runtime_smart_context_log(RuntimeSmartContextLogInput {
            request_id: index as u64,
            shared: &shared,
            scope: &scope,
            rollout: &rollout,
            route_kind: RuntimeRouteKind::Responses,
            transport: RuntimeSmartContextTransport::Http,
            tier: "minimal",
            decision: "rewritten",
            reasons: "-",
            body_bytes_before: 400 + index,
            body_bytes_after: 300,
            stats: RuntimeSmartContextTransformStats::default(),
            budget: &budget,
            token_count_after: &budget.request_token_count,
            self_check: "ok_saved",
        });
    }
    runtime_smart_context_log(RuntimeSmartContextLogInput {
        request_id: 99,
        shared: &shared,
        scope: &scope,
        rollout: &rollout,
        route_kind: RuntimeRouteKind::Responses,
        transport: RuntimeSmartContextTransport::Http,
        tier: "minimal",
        decision: "self_check_passthrough",
        reasons: "-",
        body_bytes_before: 999,
        body_bytes_after: 999,
        stats: RuntimeSmartContextTransformStats::default(),
        budget: &budget,
        token_count_after: &budget.request_token_count,
        self_check: "critical_signal_loss",
    });

    with_runtime_smart_context_proxy_state(&shared, |state| {
        assert_eq!(
            state.rewrite_telemetry_history.len(),
            SMART_CONTEXT_REWRITE_TELEMETRY_HISTORY_LIMIT
        );
        let first = state.rewrite_telemetry_history.first().unwrap();
        assert_eq!(first.body_bytes_before, 402);
        let last = state.rewrite_telemetry_history.last().unwrap();
        assert_eq!(last.tokens_before, budget.request_token_count.tokens);
        assert_eq!(
            last.token_count_source,
            runtime_proxy_crate::SmartContextTokenCountSource::Estimated
        );
        assert_eq!(last.rewrite_kind, "rewritten");
        assert_eq!(last.status, "ok_saved");
        assert_eq!(last.fallback_reason, None);
        assert_eq!(last.pressure_band, "low");
        assert_eq!(last.estimator_confidence, "medium");
        assert_eq!(last.effective_usable_context_tokens, Some(27_904));
        assert_eq!(last.absolute_safety_floor_tokens, 1_395);
        assert!(last.pressure_basis_points.is_some());
    })
    .unwrap();

    let log_text = crate::read_runtime_proxy_test_log(&shared.log_path);
    assert!(log_text.contains("tokens_before="));
    assert!(log_text.contains("token_count_source=estimated"));
    assert!(log_text.contains("rewrite_kind=rewritten"));
    assert!(log_text.contains("rewrite_kind=self_check_passthrough"));
    assert!(log_text.contains("fallback_reason=critical_signal_loss"));
    assert!(log_text.contains("task_quality_model_reread_requests=0"));
    assert!(log_text.contains("task_quality_task_completed=unknown"));
    assert!(log_text.contains("pressure_basis_points="));
    assert!(log_text.contains("pressure_band=low"));
    assert!(log_text.contains("estimator_confidence=medium"));
    assert!(log_text.contains("effective_usable_context_tokens=27904"));
    assert!(log_text.contains("absolute_safety_floor_tokens=1395"));
    assert!(log_text.contains("candidate_count=0"));
    assert!(log_text.contains("selected_candidate_count=0"));
    assert!(log_text.contains("rejected_candidate_count=0"));
    assert!(log_text.contains("selected_candidate_utility_points=0"));
    assert!(log_text.contains("transformed_segment_categories=-"));
    assert!(log_text.contains("segment_rollback_count=0"));
    assert!(log_text.contains("full_request_fallback_count=0"));
    assert!(log_text.contains("rehydration_planned=0"));
    assert!(log_text.contains("rehydration_performed=0"));
    assert!(log_text.contains("rehydration_deferred=0"));
    assert!(log_text.contains("rehydration_token_cost=0"));
    assert!(log_text.contains("artifact_hash_failures=0"));
}
#[test]
fn smart_context_rewrite_telemetry_samples_preserve_quality_outcomes() {
    let history = vec![RuntimeSmartContextRewriteTelemetryRecord {
        body_bytes_before: 8_000,
        body_bytes_after: 3_000,
        tokens_before: 2_000,
        tokens_after: 750,
        token_count_source: runtime_proxy_crate::SmartContextTokenCountSource::TokenizerCounted,
        rewrite_kind: "rewritten".to_string(),
        status: "ok_saved".to_string(),
        fallback_reason: None,
        model_reread_requests: 1,
        task_completed: Some(false),
        final_total_input_tokens: Some(12_345),
        pressure_band: "high".to_string(),
        estimator_confidence: "low".to_string(),
        ..RuntimeSmartContextRewriteTelemetryRecord::default()
    }];

    let samples = runtime_smart_context_rewrite_telemetry_samples(&history);

    assert_eq!(samples.len(), 1);
    assert_eq!(samples[0].model_reread_requests, 1);
    assert_eq!(samples[0].task_completed, Some(false));
    assert_eq!(samples[0].final_total_input_tokens, Some(12_345));
    assert!(!samples[0].safe);
}

#[test]
fn smart_context_regression_fallback_exact_on_quality_risk() {
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
    let before = br#"{"input":[{"content":"error: failed\nsrc/main.rs:10:5"}]}"#;
    let after = br#"{"input":[{"content":"summary"}]}"#;
    let before_count =
        runtime_proxy_crate::smart_context_count_serialized_request(before, Some("gpt-5.4"));
    let after_count =
        runtime_proxy_crate::smart_context_count_serialized_request(after, Some("gpt-5.4"));
    let regression = runtime_smart_context_regression_self_check(
        before,
        after,
        &before_count,
        &after_count,
        runtime_smart_context_critical_signal_self_check(before, after),
        runtime_proxy_crate::smart_context_exactness_guard(
            runtime_proxy_crate::SmartContextExactnessInput::default(),
        ),
        Vec::new(),
    );
    let critical = runtime_smart_context_critical_signal_self_check(before, after);

    assert_eq!(
        runtime_smart_context_fallback_exact_reason(&regression, critical, &stats),
        Some("critical_signal_loss")
    );
}

#[test]
fn smart_context_auto_rehydrate_plan_defers_over_budget_refs() {
    let mut store = RuntimeSmartContextArtifactStore::default();
    let artifact = store.insert_text(&"large artifact ".repeat(400)).unwrap();
    let mut value = serde_json::json!({
        "input": [{"type": "message", "content": format!("need prodex-artifact:{}", artifact.id)}]
    });
    let plan = runtime_smart_context_auto_rehydrate_plan(
        &value,
        &store,
        1,
        runtime_proxy_crate::SmartContextTokenBudgetTier::Minimal,
    );
    let mut stats = RuntimeSmartContextTransformStats::default();

    runtime_smart_context_rehydrate_value_with_plan(&mut value, &store, &plan, &mut stats);

    assert!(matches!(
        plan.actions.first(),
        Some(runtime_proxy_crate::SmartContextRehydrateAction::Defer { .. })
    ));
    assert_eq!(stats.rehydrated_refs, 0);
    assert_eq!(stats.rehydration_planned, 0);
    assert_eq!(stats.rehydration_performed, 0);
    assert_eq!(stats.rehydration_deferred, 1);
    assert_eq!(stats.rehydration_token_cost, 0);
    assert!(
        value["input"][0]["content"]
            .as_str()
            .unwrap()
            .contains("prodex-artifact:")
    );
}
