use super::*;

#[test]
fn smart_context_budget_uses_runtime_token_usage_observation() {
    let shared = smart_context_test_shared("budget");
    register_runtime_smart_context_proxy_state(&shared.log_path, true, None, None);
    observe_runtime_smart_context_token_usage(
        &shared,
        RuntimeTokenUsage {
            input_tokens: 24_000,
            cached_input_tokens: 0,
            output_tokens: 7_000,
            reasoning_tokens: 1_000,
        },
    );

    let budget = runtime_smart_context_budget(RuntimeSmartContextBudgetInput {
        shared: &shared,
        body: b"small current request body payload",
        route_kind: RuntimeRouteKind::Responses,
        transport: RuntimeSmartContextTransport::Http,
        profile_name: None,
        exactness_guard: runtime_proxy_crate::smart_context_exactness_guard(
            runtime_proxy_crate::SmartContextExactnessInput::default(),
        ),
        missing_rehydrate_refs: Vec::new(),
        static_context_changed: false,
    });

    assert_eq!(
        budget.tier,
        runtime_proxy_crate::SmartContextTokenBudgetTier::Minimal
    );
    assert_eq!(budget.observed_context_tokens, Some(32_000));
    assert_eq!(budget.token_usage_source, "runtime_usage");
    assert_eq!(budget.model_context_window_tokens, 32_000);
    assert_eq!(budget.model_context_window_source, "fallback");
    assert_eq!(
        budget.pressure.pressure_band,
        runtime_proxy_crate::SmartContextPressureBand::Exhausted
    );
    assert_eq!(
        budget.pressure.effective_usable_context_tokens,
        Some(27_904)
    );
    assert_eq!(budget.pressure.pressure_basis_points, Some(11_467));
}

#[test]
fn smart_context_budget_uses_configured_model_context_window() {
    let shared = smart_context_test_shared("budget-custom-window");
    register_runtime_smart_context_proxy_state(&shared.log_path, true, Some(64_000), None);
    observe_runtime_smart_context_token_usage(
        &shared,
        RuntimeTokenUsage {
            input_tokens: 24_000,
            cached_input_tokens: 0,
            output_tokens: 7_000,
            reasoning_tokens: 1_000,
        },
    );

    let budget = runtime_smart_context_budget(RuntimeSmartContextBudgetInput {
        shared: &shared,
        body: b"small current request body payload",
        route_kind: RuntimeRouteKind::Responses,
        transport: RuntimeSmartContextTransport::Http,
        profile_name: None,
        exactness_guard: runtime_proxy_crate::smart_context_exactness_guard(
            runtime_proxy_crate::SmartContextExactnessInput::default(),
        ),
        missing_rehydrate_refs: Vec::new(),
        static_context_changed: false,
    });

    assert_eq!(
        budget.tier,
        runtime_proxy_crate::SmartContextTokenBudgetTier::Exact
    );
    assert_eq!(budget.model_context_window_tokens, 64_000);
    assert_eq!(budget.model_context_window_source, "launch_config");
    assert_eq!(budget.observed_context_tokens, Some(32_000));
}

#[test]
fn smart_context_budget_uses_model_registry_window_when_unconfigured() {
    let shared = smart_context_test_shared("budget-model-registry-window");
    register_runtime_smart_context_proxy_state(&shared.log_path, true, None, None);

    let budget = runtime_smart_context_budget(RuntimeSmartContextBudgetInput {
        shared: &shared,
        body: br#"{"model":"gpt-5.1-codex","input":"small current request body payload"}"#,
        route_kind: RuntimeRouteKind::Responses,
        transport: RuntimeSmartContextTransport::Http,
        profile_name: None,
        exactness_guard: runtime_proxy_crate::smart_context_exactness_guard(
            runtime_proxy_crate::SmartContextExactnessInput::default(),
        ),
        missing_rehydrate_refs: Vec::new(),
        static_context_changed: false,
    });

    assert_eq!(budget.model_context_window_tokens, 200_000);
    assert_eq!(budget.model_context_window_source, "model_registry");
    assert_eq!(
        budget.pressure.pressure_band,
        runtime_proxy_crate::SmartContextPressureBand::Low
    );
}

#[test]
fn smart_context_budget_prefers_configured_window_over_model_registry() {
    let shared = smart_context_test_shared("budget-config-over-registry");
    register_runtime_smart_context_proxy_state(&shared.log_path, true, Some(64_000), None);

    let budget = runtime_smart_context_budget(RuntimeSmartContextBudgetInput {
        shared: &shared,
        body: br#"{"model":"gpt-5.1-codex","input":"small current request body payload"}"#,
        route_kind: RuntimeRouteKind::Responses,
        transport: RuntimeSmartContextTransport::Http,
        profile_name: None,
        exactness_guard: runtime_proxy_crate::smart_context_exactness_guard(
            runtime_proxy_crate::SmartContextExactnessInput::default(),
        ),
        missing_rehydrate_refs: Vec::new(),
        static_context_changed: false,
    });

    assert_eq!(budget.model_context_window_tokens, 64_000);
    assert_eq!(budget.model_context_window_source, "launch_config");
}

#[test]
fn smart_context_budget_uses_matching_token_calibration_bucket() {
    let shared = smart_context_test_shared("budget-bucket");
    register_runtime_smart_context_proxy_state(&shared.log_path, true, Some(64_000), None);
    observe_runtime_smart_context_token_usage_for_bucket(
        &shared,
        RuntimeTokenUsage {
            input_tokens: 48_000,
            ..RuntimeTokenUsage::default()
        },
        Some(runtime_smart_context_token_calibration_bucket_key(
            RuntimeRouteKind::Responses,
            RuntimeSmartContextTransport::Http,
            Some("alpha"),
        )),
    );
    observe_runtime_smart_context_token_usage_for_bucket(
        &shared,
        RuntimeTokenUsage {
            input_tokens: 56_000,
            ..RuntimeTokenUsage::default()
        },
        Some(runtime_smart_context_token_calibration_bucket_key(
            RuntimeRouteKind::Websocket,
            RuntimeSmartContextTransport::Websocket,
            Some("beta"),
        )),
    );

    let alpha = runtime_smart_context_budget(RuntimeSmartContextBudgetInput {
        shared: &shared,
        body: b"small current request body payload",
        route_kind: RuntimeRouteKind::Responses,
        transport: RuntimeSmartContextTransport::Http,
        profile_name: Some("alpha"),
        exactness_guard: runtime_proxy_crate::smart_context_exactness_guard(
            runtime_proxy_crate::SmartContextExactnessInput::default(),
        ),
        missing_rehydrate_refs: Vec::new(),
        static_context_changed: false,
    });
    let beta = runtime_smart_context_budget(RuntimeSmartContextBudgetInput {
        shared: &shared,
        body: b"small current request body payload",
        route_kind: RuntimeRouteKind::Websocket,
        transport: RuntimeSmartContextTransport::Websocket,
        profile_name: Some("beta"),
        exactness_guard: runtime_proxy_crate::smart_context_exactness_guard(
            runtime_proxy_crate::SmartContextExactnessInput::default(),
        ),
        missing_rehydrate_refs: Vec::new(),
        static_context_changed: false,
    });

    assert_eq!(
        alpha.tier,
        runtime_proxy_crate::SmartContextTokenBudgetTier::Large
    );
    assert_eq!(alpha.observed_context_tokens, Some(48_000));
    assert_eq!(
        beta.tier,
        runtime_proxy_crate::SmartContextTokenBudgetTier::Condensed
    );
    assert_eq!(beta.observed_context_tokens, Some(56_000));
}

#[test]
fn smart_context_budget_uses_model_specific_token_calibration_bucket() {
    let shared = smart_context_test_shared("budget-model-bucket");
    register_runtime_smart_context_proxy_state(&shared.log_path, true, Some(64_000), None);
    observe_runtime_smart_context_token_usage_for_bucket(
        &shared,
        RuntimeTokenUsage {
            input_tokens: 44_000,
            ..RuntimeTokenUsage::default()
        },
        Some(
            runtime_smart_context_token_calibration_bucket_key_with_model(
                RuntimeRouteKind::Responses,
                RuntimeSmartContextTransport::Http,
                Some("alpha"),
                Some("gpt-5"),
            ),
        ),
    );
    observe_runtime_smart_context_token_usage_for_bucket(
        &shared,
        RuntimeTokenUsage {
            input_tokens: 56_000,
            ..RuntimeTokenUsage::default()
        },
        Some(
            runtime_smart_context_token_calibration_bucket_key_with_model(
                RuntimeRouteKind::Responses,
                RuntimeSmartContextTransport::Http,
                Some("alpha"),
                Some("gpt-5.2"),
            ),
        ),
    );

    let gpt5 = runtime_smart_context_budget(RuntimeSmartContextBudgetInput {
        shared: &shared,
        body: br#"{"model":"gpt-5","input":"small current request body payload"}"#,
        route_kind: RuntimeRouteKind::Responses,
        transport: RuntimeSmartContextTransport::Http,
        profile_name: Some("alpha"),
        exactness_guard: runtime_proxy_crate::smart_context_exactness_guard(
            runtime_proxy_crate::SmartContextExactnessInput::default(),
        ),
        missing_rehydrate_refs: Vec::new(),
        static_context_changed: false,
    });
    let gpt52 = runtime_smart_context_budget(RuntimeSmartContextBudgetInput {
        shared: &shared,
        body: br#"{"model":"gpt-5.2","input":"small current request body payload"}"#,
        route_kind: RuntimeRouteKind::Responses,
        transport: RuntimeSmartContextTransport::Http,
        profile_name: Some("alpha"),
        exactness_guard: runtime_proxy_crate::smart_context_exactness_guard(
            runtime_proxy_crate::SmartContextExactnessInput::default(),
        ),
        missing_rehydrate_refs: Vec::new(),
        static_context_changed: false,
    });

    assert_eq!(gpt5.observed_context_tokens, Some(44_000));
    assert_eq!(gpt52.observed_context_tokens, Some(56_000));
    assert_eq!(
        runtime_smart_context_model_name_from_body(
            br#"{"model":"gpt-5.2","input":"small current request body payload"}"#
        )
        .as_deref(),
        Some("gpt-5.2")
    );
}

#[test]
fn smart_context_budget_expands_large_preview_after_recent_safe_rewrite() {
    let shared = smart_context_test_shared("budget-recent-safe");
    register_runtime_smart_context_proxy_state(&shared.log_path, true, Some(64_000), None);
    observe_runtime_smart_context_token_usage(
        &shared,
        RuntimeTokenUsage {
            input_tokens: 48_000,
            cached_input_tokens: 0,
            output_tokens: 0,
            reasoning_tokens: 0,
        },
    );

    let before = runtime_smart_context_budget(RuntimeSmartContextBudgetInput {
        shared: &shared,
        body: b"small current request body payload",
        route_kind: RuntimeRouteKind::Responses,
        transport: RuntimeSmartContextTransport::Http,
        profile_name: None,
        exactness_guard: runtime_proxy_crate::smart_context_exactness_guard(
            runtime_proxy_crate::SmartContextExactnessInput::default(),
        ),
        missing_rehydrate_refs: Vec::new(),
        static_context_changed: false,
    });
    observe_runtime_smart_context_rewrite_safety(
        &shared,
        RuntimeSmartContextRewriteSafetyObservation {
            safe: true,
            saved_tokens: runtime_proxy_crate::SMART_CONTEXT_RECENT_SAFE_REWRITE_MIN_SAVED_TOKENS,
        },
    );
    let after = runtime_smart_context_budget(RuntimeSmartContextBudgetInput {
        shared: &shared,
        body: b"small current request body payload",
        route_kind: RuntimeRouteKind::Responses,
        transport: RuntimeSmartContextTransport::Http,
        profile_name: None,
        exactness_guard: runtime_proxy_crate::smart_context_exactness_guard(
            runtime_proxy_crate::SmartContextExactnessInput::default(),
        ),
        missing_rehydrate_refs: Vec::new(),
        static_context_changed: false,
    });

    assert_eq!(
        before.tier,
        runtime_proxy_crate::SmartContextTokenBudgetTier::Large
    );
    assert_eq!(before.policy.max_inline_tool_output_bytes, 32 * 1024);
    assert_eq!(after.policy.max_inline_tool_output_bytes, 64 * 1024);
    assert!(
        after.policy.reasons.contains(
            &runtime_proxy_crate::SmartContextBudgetPolicyReason::RecentRewriteSavingsSafe
        )
    );
}

#[test]
fn smart_context_budget_loads_persisted_recent_safe_rewrite() {
    let first_shared = smart_context_test_shared("budget-persisted-recent-safe");
    let artifact_path = first_shared
        .log_path
        .with_file_name("budget-persisted-recent-safe-artifacts.json");
    let calibration_path = runtime_smart_context_token_calibration_path(&artifact_path);
    let _ = std::fs::remove_file(&artifact_path);
    let _ = std::fs::remove_file(&calibration_path);
    register_runtime_smart_context_proxy_state(
        &first_shared.log_path,
        true,
        Some(64_000),
        Some(artifact_path.clone()),
    );
    observe_runtime_smart_context_token_usage(
        &first_shared,
        RuntimeTokenUsage {
            input_tokens: 48_000,
            cached_input_tokens: 0,
            output_tokens: 0,
            reasoning_tokens: 0,
        },
    );
    observe_runtime_smart_context_rewrite_safety(
        &first_shared,
        RuntimeSmartContextRewriteSafetyObservation {
            safe: true,
            saved_tokens: runtime_proxy_crate::SMART_CONTEXT_RECENT_SAFE_REWRITE_MIN_SAVED_TOKENS,
        },
    );

    let fresh_shared = smart_context_test_shared("budget-persisted-recent-safe-fresh");
    register_runtime_smart_context_proxy_state(
        &fresh_shared.log_path,
        true,
        Some(64_000),
        Some(artifact_path.clone()),
    );
    let budget = runtime_smart_context_budget(RuntimeSmartContextBudgetInput {
        shared: &fresh_shared,
        body: b"small current request body payload",
        route_kind: RuntimeRouteKind::Responses,
        transport: RuntimeSmartContextTransport::Http,
        profile_name: None,
        exactness_guard: runtime_proxy_crate::smart_context_exactness_guard(
            runtime_proxy_crate::SmartContextExactnessInput::default(),
        ),
        missing_rehydrate_refs: Vec::new(),
        static_context_changed: false,
    });

    assert_eq!(
        budget.tier,
        runtime_proxy_crate::SmartContextTokenBudgetTier::Large
    );
    assert_eq!(budget.policy.max_inline_tool_output_bytes, 64 * 1024);
    assert!(
        budget.policy.reasons.contains(
            &runtime_proxy_crate::SmartContextBudgetPolicyReason::RecentRewriteSavingsSafe
        )
    );

    let _ = std::fs::remove_file(&artifact_path);
    let _ = std::fs::remove_file(&calibration_path);
    let _ = std::fs::remove_file(crate::runtime_store::json_lock_file_path(&artifact_path));
}

#[test]
fn smart_context_budget_relaxes_from_safe_saving_telemetry_ring() {
    let shared = smart_context_test_shared("budget-telemetry-relax");
    register_runtime_smart_context_proxy_state(&shared.log_path, true, Some(64_000), None);
    observe_runtime_smart_context_token_usage(
        &shared,
        RuntimeTokenUsage {
            input_tokens: 48_000,
            ..RuntimeTokenUsage::default()
        },
    );
    with_runtime_smart_context_proxy_state(&shared, |state| {
        state
            .rewrite_telemetry_history
            .push(RuntimeSmartContextRewriteTelemetryRecord {
                body_bytes_before: 8_000,
                body_bytes_after: 3_000,
                estimated_tokens_before: 2_000,
                estimated_tokens_after: 750,
                rewrite_kind: "rewritten".to_string(),
                status: "ok_saved".to_string(),
                fallback_reason: None,
                ..RuntimeSmartContextRewriteTelemetryRecord::default()
            });
        state
            .rewrite_telemetry_history
            .push(RuntimeSmartContextRewriteTelemetryRecord {
                body_bytes_before: 7_000,
                body_bytes_after: 2_800,
                estimated_tokens_before: 1_750,
                estimated_tokens_after: 700,
                rewrite_kind: "rewritten".to_string(),
                status: "ok_saved".to_string(),
                fallback_reason: None,
                ..RuntimeSmartContextRewriteTelemetryRecord::default()
            });
        state
            .rewrite_telemetry_history
            .push(RuntimeSmartContextRewriteTelemetryRecord {
                body_bytes_before: 100,
                body_bytes_after: 100,
                estimated_tokens_before: 25,
                estimated_tokens_after: 25,
                rewrite_kind: "pass_through".to_string(),
                status: "noop".to_string(),
                fallback_reason: None,
                ..RuntimeSmartContextRewriteTelemetryRecord::default()
            });
    })
    .unwrap();

    let budget = runtime_smart_context_budget(RuntimeSmartContextBudgetInput {
        shared: &shared,
        body: b"small current request body payload",
        route_kind: RuntimeRouteKind::Responses,
        transport: RuntimeSmartContextTransport::Http,
        profile_name: None,
        exactness_guard: runtime_proxy_crate::smart_context_exactness_guard(
            runtime_proxy_crate::SmartContextExactnessInput::default(),
        ),
        missing_rehydrate_refs: Vec::new(),
        static_context_changed: false,
    });

    assert_eq!(
        budget.tier,
        runtime_proxy_crate::SmartContextTokenBudgetTier::Large
    );
    assert_eq!(budget.policy.max_inline_tool_output_bytes, 64 * 1024);
    assert_eq!(budget.policy.max_rehydrate_tokens, 11_904);
}

#[test]
fn smart_context_budget_tightens_for_marginal_or_fallback_telemetry() {
    let shared = smart_context_test_shared("budget-telemetry-tighten");
    register_runtime_smart_context_proxy_state(&shared.log_path, true, Some(64_000), None);
    observe_runtime_smart_context_token_usage(
        &shared,
        RuntimeTokenUsage {
            input_tokens: 48_000,
            ..RuntimeTokenUsage::default()
        },
    );
    with_runtime_smart_context_proxy_state(&shared, |state| {
        state
            .rewrite_telemetry_history
            .push(RuntimeSmartContextRewriteTelemetryRecord {
                body_bytes_before: 10_000,
                body_bytes_after: 9_000,
                estimated_tokens_before: 2_500,
                estimated_tokens_after: 2_250,
                rewrite_kind: "rewritten".to_string(),
                status: "ok_saved".to_string(),
                fallback_reason: None,
                ..RuntimeSmartContextRewriteTelemetryRecord::default()
            });
        state
            .rewrite_telemetry_history
            .push(RuntimeSmartContextRewriteTelemetryRecord {
                body_bytes_before: 8_000,
                body_bytes_after: 7_200,
                estimated_tokens_before: 2_000,
                estimated_tokens_after: 1_800,
                rewrite_kind: "rewritten".to_string(),
                status: "ok_saved".to_string(),
                fallback_reason: None,
                ..RuntimeSmartContextRewriteTelemetryRecord::default()
            });
    })
    .unwrap();

    let budget = runtime_smart_context_budget(RuntimeSmartContextBudgetInput {
        shared: &shared,
        body: b"small current request body payload",
        route_kind: RuntimeRouteKind::Responses,
        transport: RuntimeSmartContextTransport::Http,
        profile_name: None,
        exactness_guard: runtime_proxy_crate::smart_context_exactness_guard(
            runtime_proxy_crate::SmartContextExactnessInput::default(),
        ),
        missing_rehydrate_refs: Vec::new(),
        static_context_changed: false,
    });
    assert_eq!(
        budget.policy.max_inline_tool_output_bytes,
        (32 * 1024) * 9 / 10
    );
    assert_eq!(budget.policy.max_rehydrate_tokens, 10_713);

    with_runtime_smart_context_proxy_state(&shared, |state| {
        state
            .rewrite_telemetry_history
            .push(RuntimeSmartContextRewriteTelemetryRecord {
                body_bytes_before: 8_000,
                body_bytes_after: 3_000,
                estimated_tokens_before: 2_000,
                estimated_tokens_after: 750,
                rewrite_kind: "self_check_passthrough".to_string(),
                status: "critical_signal_loss".to_string(),
                fallback_reason: Some("critical_signal_loss".to_string()),
                ..RuntimeSmartContextRewriteTelemetryRecord::default()
            });
    })
    .unwrap();
    let fallback_budget = runtime_smart_context_budget(RuntimeSmartContextBudgetInput {
        shared: &shared,
        body: b"small current request body payload",
        route_kind: RuntimeRouteKind::Responses,
        transport: RuntimeSmartContextTransport::Http,
        profile_name: None,
        exactness_guard: runtime_proxy_crate::smart_context_exactness_guard(
            runtime_proxy_crate::SmartContextExactnessInput::default(),
        ),
        missing_rehydrate_refs: Vec::new(),
        static_context_changed: false,
    });
    assert_eq!(
        fallback_budget.policy.max_inline_tool_output_bytes,
        (32 * 1024) * 9 / 10
    );
    assert_eq!(fallback_budget.policy.max_rehydrate_tokens, 10_713);
}
