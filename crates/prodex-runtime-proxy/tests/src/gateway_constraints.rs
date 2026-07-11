use super::*;

fn alias(models: &[&str]) -> RuntimeGatewayRouteAlias {
    RuntimeGatewayRouteAlias {
        alias: "route".to_string(),
        models: models.iter().map(|model| (*model).to_string()).collect(),
        strategy: RuntimeGatewayRouteStrategy::Fallback,
        model_metrics: BTreeMap::new(),
    }
}

fn strict_policy() -> ProviderRequestConstraintPolicy {
    ProviderRequestConstraintPolicy {
        enabled: true,
        unknown_context: prodex_provider_core::ProviderUnknownContextPolicy::Reject,
        safe_window_tokens: 128_000,
        oversized_output: prodex_provider_core::ProviderOversizedOutputPolicy::Reject,
    }
}

fn large_body(model: &str, input_tokens: usize) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({
        "model": model,
        "input": "x".repeat(input_tokens.saturating_mul(4)),
    }))
    .unwrap()
}

#[test]
fn fallback_skips_small_actual_model_before_ranking() {
    let plan = runtime_gateway_plan_route_with_constraints(
        ProviderId::OpenAi,
        ProviderEndpoint::Responses,
        &large_body("route", 130_000),
        &[alias(&["gpt-5.3-codex-spark", "gpt-5.4"])],
        1,
        &BTreeMap::new(),
        strict_policy(),
        &[],
        None,
        false,
    )
    .unwrap();

    assert_eq!(plan.concrete_candidates.len(), 2);
    assert!(!plan.concrete_candidates[0].evaluation.eligible);
    assert_eq!(
        plan.concrete_candidates[0].evaluation.decision,
        ProviderRequestConstraintDecision::ContextWindowExceeded
    );
    assert!(plan.concrete_candidates[1].evaluation.eligible);
    assert_eq!(plan.selected_model.as_deref(), Some("gpt-5.4"));
}

#[test]
fn alias_target_is_resolved_to_actual_catalog_model() {
    let body = br#"{"model":"route","input":"hi"}"#;
    let plan = runtime_gateway_plan_route_with_constraints(
        ProviderId::OpenAi,
        ProviderEndpoint::Responses,
        body,
        &[alias(&["spark"])],
        1,
        &BTreeMap::new(),
        strict_policy(),
        &[],
        None,
        false,
    )
    .unwrap();

    assert_eq!(plan.concrete_candidates[0].model, "gpt-5.3-codex-spark");
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(
            &runtime_gateway_apply_constraint_plan_body(body, &plan).unwrap(),
        )
        .unwrap()["model"],
        "gpt-5.3-codex-spark"
    );
}

#[test]
fn all_incompatible_candidates_produce_no_route() {
    let plan = runtime_gateway_plan_route_with_constraints(
        ProviderId::OpenAi,
        ProviderEndpoint::Responses,
        &large_body("route", 410_000),
        &[alias(&["gpt-5.3-codex-spark", "gpt-5.4"])],
        1,
        &BTreeMap::new(),
        strict_policy(),
        &[],
        None,
        false,
    )
    .unwrap();

    assert_eq!(plan.selected_model, None);
    assert_eq!(
        plan.no_route_reason,
        Some(ProviderRequestConstraintDecision::ContextWindowExceeded)
    );
    assert_eq!(
        plan.trace.terminal_outcome,
        RuntimeRouteDecisionTerminalOutcome::NoCandidate
    );
}

#[test]
fn hard_affinity_owner_is_evaluated_alone_and_never_rotates_to_larger_model() {
    let plan = runtime_gateway_plan_route_with_constraints(
        ProviderId::OpenAi,
        ProviderEndpoint::Responses,
        &large_body("route", 130_000),
        &[alias(&["gpt-5.3-codex-spark", "gpt-5.4"])],
        1,
        &BTreeMap::new(),
        strict_policy(),
        &[],
        Some("gpt-5.3-codex-spark"),
        true,
    )
    .unwrap();

    assert_eq!(plan.concrete_candidates.len(), 1);
    assert_eq!(plan.selected_model, None);
    assert_eq!(
        plan.trace.terminal_outcome,
        RuntimeRouteDecisionTerminalOutcome::AffinityExhausted
    );
    assert!(plan.trace.affinity.hard);
}

#[test]
fn strict_direct_model_encodes_one_exact_upstream_attempt() {
    let body = br#"{"model":"gpt-5.4","input":"hi"}"#;
    let plan = runtime_gateway_plan_route_with_constraints(
        ProviderId::OpenAi,
        ProviderEndpoint::Responses,
        body,
        &[],
        1,
        &BTreeMap::new(),
        strict_policy(),
        &[],
        None,
        false,
    )
    .unwrap();
    assert_eq!(plan.selected_model.as_deref(), Some("gpt-5.4"));
    let rewritten: serde_json::Value =
        serde_json::from_slice(&runtime_gateway_apply_constraint_plan_body(body, &plan).unwrap())
            .unwrap();
    assert_eq!(rewritten["model"], "gpt-5.4");
    assert_eq!(
        prodex_provider_core::provider_model_fallback_chain(
            ProviderId::OpenAi,
            rewritten["model"].as_str().unwrap(),
        ),
        vec!["gpt-5.4".to_string()]
    );
}

#[test]
fn compatible_hard_owner_encodes_one_exact_upstream_attempt() {
    let body = br#"{"model":"route","input":"hi"}"#;
    let plan = runtime_gateway_plan_route_with_constraints(
        ProviderId::OpenAi,
        ProviderEndpoint::Responses,
        body,
        &[alias(&["gpt-5.4", "gpt-5.3-codex-spark"])],
        1,
        &BTreeMap::new(),
        strict_policy(),
        &[],
        Some("gpt-5.3-codex-spark"),
        true,
    )
    .unwrap();
    assert_eq!(plan.selected_model.as_deref(), Some("gpt-5.3-codex-spark"));
    let rewritten: serde_json::Value =
        serde_json::from_slice(&runtime_gateway_apply_constraint_plan_body(body, &plan).unwrap())
            .unwrap();
    assert_eq!(rewritten["model"], "gpt-5.3-codex-spark");
    assert_eq!(
        prodex_provider_core::provider_model_fallback_chain(
            ProviderId::OpenAi,
            rewritten["model"].as_str().unwrap(),
        ),
        vec!["gpt-5.3-codex-spark".to_string()]
    );
}

#[test]
fn hard_continuation_without_resolved_owner_never_becomes_fresh_fallback() {
    let plan = runtime_gateway_plan_route_with_constraints(
        ProviderId::OpenAi,
        ProviderEndpoint::Responses,
        br#"{"model":"route","previous_response_id":"response-opaque","input":"hi"}"#,
        &[alias(&["gpt-5.3-codex-spark", "gpt-5.4"])],
        1,
        &BTreeMap::new(),
        strict_policy(),
        &[],
        None,
        true,
    )
    .unwrap();

    assert!(plan.concrete_candidates.is_empty());
    assert_eq!(plan.selected_model, None);
    assert_eq!(
        plan.no_route_reason,
        Some(ProviderRequestConstraintDecision::AffinityOwnerUnavailable)
    );
    assert_eq!(
        plan.trace.terminal_outcome,
        RuntimeRouteDecisionTerminalOutcome::AffinityExhausted
    );
}

#[test]
fn disabled_policy_preserves_legacy_alias_and_malformed_limit_behavior() {
    let aliases = [alias(&["gpt-5.3-codex-spark", "gpt-5.4"])];
    let body = br#"{"model":"route","max_output_tokens":"invalid"}"#;
    let legacy =
        crate::runtime_gateway_rewrite_route_alias_with_state(body, &aliases, 1, &BTreeMap::new())
            .unwrap();
    let plan = runtime_gateway_plan_route_with_constraints(
        ProviderId::OpenAi,
        ProviderEndpoint::Responses,
        body,
        &aliases,
        1,
        &BTreeMap::new(),
        ProviderRequestConstraintPolicy::default(),
        &[],
        None,
        false,
    )
    .unwrap();

    assert_eq!(plan.selected_model.as_deref(), Some(legacy.model.as_str()));
    assert_eq!(
        runtime_gateway_apply_constraint_plan_body(body, &plan),
        Some(legacy.body)
    );
    assert_eq!(
        plan.trace
            .stages
            .iter()
            .find(|stage| stage.stage == RuntimeRouteDecisionStage::RequestConstraints)
            .unwrap()
            .outcome,
        RuntimeRouteDecisionStageOutcome::Skipped
    );
}

#[test]
fn disabled_direct_and_unmodeled_requests_remain_byte_compatible() {
    for body in [
        b"{ \"input\": \"hi\", \"model\": \"gpt-5.4\" }".as_slice(),
        br#"{"input":"hi"}"#.as_slice(),
        b"not-json".as_slice(),
    ] {
        let plan = runtime_gateway_plan_route_with_constraints(
            ProviderId::OpenAi,
            ProviderEndpoint::Responses,
            body,
            &[],
            1,
            &BTreeMap::new(),
            ProviderRequestConstraintPolicy::default(),
            &[],
            None,
            false,
        )
        .unwrap();
        assert_eq!(plan.no_route_reason, None);
        assert!(!plan.body_rewrite_required);
        assert_eq!(
            runtime_gateway_apply_constraint_plan_body(body, &plan).as_deref(),
            Some(body)
        );
    }
}

#[test]
fn late_apply_preserves_redaction_and_fails_closed_on_invalid_body() {
    let plan = runtime_gateway_plan_route_with_constraints(
        ProviderId::OpenAi,
        ProviderEndpoint::Responses,
        br#"{"model":"gpt-5.4","input":"person@example.com"}"#,
        &[],
        1,
        &BTreeMap::new(),
        strict_policy(),
        &[],
        None,
        false,
    )
    .unwrap();
    let applied = runtime_gateway_apply_constraint_plan_body(
        br#"{"model":"gpt-5.4","input":"[REDACTED]"}"#,
        &plan,
    )
    .unwrap();
    let value: serde_json::Value = serde_json::from_slice(&applied).unwrap();
    assert_eq!(value["input"], "[REDACTED]");
    assert_eq!(value["model"], "gpt-5.4");
    assert!(
        !String::from_utf8(applied)
            .unwrap()
            .contains("person@example.com")
    );
    assert_eq!(
        runtime_gateway_apply_constraint_plan_body(b"{", &plan),
        None
    );
}

#[test]
fn non_fallback_strategy_ranks_only_technically_eligible_models() {
    let route = RuntimeGatewayRouteAlias {
        alias: "route".to_string(),
        models: vec!["unknown-model".to_string(), "gpt-5.4".to_string()],
        strategy: RuntimeGatewayRouteStrategy::RoundRobin,
        model_metrics: BTreeMap::new(),
    };
    let plan = runtime_gateway_plan_route_with_constraints(
        ProviderId::OpenAi,
        ProviderEndpoint::Responses,
        br#"{"model":"route","input":"hi"}"#,
        &[route],
        1,
        &BTreeMap::new(),
        strict_policy(),
        &[],
        None,
        false,
    )
    .unwrap();
    assert_eq!(plan.selected_model.as_deref(), Some("gpt-5.4"));
    assert!(!plan.concrete_candidates[0].evaluation.eligible);
}

#[test]
fn embeddings_never_expand_generic_provider_fallback_chain() {
    let plan = runtime_gateway_plan_route_with_constraints(
        ProviderId::Copilot,
        ProviderEndpoint::Embeddings,
        br#"{"model":"codex","input":"hi"}"#,
        &[],
        1,
        &BTreeMap::new(),
        strict_policy(),
        &[],
        None,
        false,
    )
    .unwrap();

    assert_eq!(plan.concrete_candidates.len(), 1);
    assert_eq!(plan.concrete_candidates[0].model, "codex");
    assert_eq!(
        plan.concrete_candidates[0].evaluation.decision,
        ProviderRequestConstraintDecision::EndpointUnsupported
    );
}

#[test]
fn combo_clamp_uses_one_minimum_adjustment_for_every_retained_candidate() {
    let base_requirements = ProviderRequestRequirements {
        endpoint: ProviderEndpoint::Responses,
        requested_model: "route".to_string(),
        resolved_upstream_model: None,
        estimated_input_tokens: 10,
        explicit_output_tokens: Some(100),
        output_limit_field: Some(prodex_provider_core::ProviderOutputLimitField::MaxOutputTokens),
        default_output_reserve_tokens: None,
        reasoning_effort: None,
        reasoning_reserve_tokens: None,
        total_required_tokens: 110,
        required_features: Vec::new(),
    };
    let candidate = |model: &str, applied_tokens: u64| RuntimeGatewayConstraintCandidate {
        model: model.to_string(),
        original_order: 0,
        selected: false,
        evaluation: ProviderRequestConstraintEvaluation {
            decision: ProviderRequestConstraintDecision::OutputLimitClamped,
            eligible: true,
            requirements: base_requirements.clone(),
            missing_feature: None,
            available_context_tokens: Some(1_000),
            max_output_tokens: Some(applied_tokens),
            adjustment: Some(ProviderOutputAdjustment {
                field: prodex_provider_core::ProviderOutputLimitField::MaxOutputTokens,
                requested_tokens: 100,
                applied_tokens,
                reason: ProviderRequestConstraintDecision::OutputLimitClamped,
            }),
            warnings: Vec::new(),
        },
    };
    let mut candidates = vec![candidate("small", 40), candidate("large", 80)];

    normalize_combo_output_adjustment(&mut candidates);

    assert!(candidates.iter().all(|candidate| {
        candidate
            .evaluation
            .adjustment
            .as_ref()
            .is_some_and(|adjustment| adjustment.applied_tokens == 40)
            && candidate.evaluation.requirements.total_required_tokens == 50
    }));
}

#[test]
fn configured_reasoning_reserve_matches_the_actual_gemini_translation_budget() {
    let plan = runtime_gateway_plan_route_with_constraints_and_reasoning_reserve(
        ProviderId::Gemini,
        ProviderEndpoint::Responses,
        br#"{"model":"gemini-2.5-flash","input":"hi","reasoning":{"effort":"high"}}"#,
        &[],
        1,
        &BTreeMap::new(),
        strict_policy(),
        &[],
        Some(12_345),
        None,
        false,
    )
    .unwrap();

    assert_eq!(plan.requirements.reasoning_reserve_tokens, None);
    assert_eq!(
        plan.concrete_candidates[0]
            .evaluation
            .requirements
            .reasoning_reserve_tokens,
        Some(12_345)
    );

    let disabled = runtime_gateway_plan_route_with_constraints_and_reasoning_reserve(
        ProviderId::Gemini,
        ProviderEndpoint::Responses,
        br#"{"model":"gemini-2.5-flash","input":"hi","reasoning":{"effort":"none"}}"#,
        &[],
        1,
        &BTreeMap::new(),
        strict_policy(),
        &[],
        Some(12_345),
        None,
        false,
    )
    .unwrap();
    assert_eq!(disabled.requirements.reasoning_reserve_tokens, None);
    assert_eq!(
        disabled.concrete_candidates[0]
            .evaluation
            .requirements
            .reasoning_reserve_tokens,
        Some(0)
    );

    for model in ["gemini-3.1-pro-preview", "gemma-4-27b-it"] {
        let body = format!(r#"{{"model":"{model}","input":"hi","reasoning":{{"effort":"high"}}}}"#);
        let plan = runtime_gateway_plan_route_with_constraints_and_reasoning_reserve(
            ProviderId::Gemini,
            ProviderEndpoint::Responses,
            body.as_bytes(),
            &[],
            1,
            &BTreeMap::new(),
            strict_policy(),
            &[],
            Some(12_345),
            None,
            false,
        )
        .unwrap();
        assert_eq!(
            plan.concrete_candidates[0]
                .evaluation
                .requirements
                .reasoning_reserve_tokens,
            None,
            "{model} uses thinkingLevel, not a configured token budget"
        );
    }

    let native = runtime_gateway_plan_route_with_constraints_and_reasoning_reserve(
        ProviderId::Gemini,
        ProviderEndpoint::Responses,
        br#"{"model":"gemini-3.1-pro-preview","input":"hi","thinking":{"budget_tokens":12}}"#,
        &[],
        1,
        &BTreeMap::new(),
        strict_policy(),
        &[],
        Some(12_345),
        None,
        false,
    )
    .unwrap();
    assert_eq!(native.requirements.reasoning_reserve_tokens, Some(12));
    assert_eq!(
        native.concrete_candidates[0]
            .evaluation
            .requirements
            .reasoning_reserve_tokens,
        Some(12)
    );
}

#[test]
fn compatible_candidate_after_trace_limit_is_selected_and_retained() {
    let mut models = (0..crate::RUNTIME_ROUTE_DECISION_TRACE_MAX_CANDIDATES)
        .map(|index| format!("unknown-{index}"))
        .collect::<Vec<_>>();
    models.push("gpt-5.4".to_string());
    let route = RuntimeGatewayRouteAlias {
        alias: "route".to_string(),
        models,
        strategy: RuntimeGatewayRouteStrategy::First,
        model_metrics: BTreeMap::new(),
    };

    let plan = runtime_gateway_plan_route_with_constraints(
        ProviderId::OpenAi,
        ProviderEndpoint::Responses,
        br#"{"model":"route","input":"hi"}"#,
        &[route],
        1,
        &BTreeMap::new(),
        strict_policy(),
        &[],
        None,
        false,
    )
    .unwrap();

    assert_eq!(plan.selected_model.as_deref(), Some("gpt-5.4"));
    assert_eq!(
        plan.concrete_candidates.len(),
        crate::RUNTIME_ROUTE_DECISION_TRACE_MAX_CANDIDATES
    );
    assert!(
        plan.concrete_candidates
            .iter()
            .any(|candidate| candidate.model == "gpt-5.4" && candidate.selected)
    );
    assert_eq!(plan.omitted_candidates, 1);
    assert!(plan.truncated);
    assert_eq!(plan.trace.truncation.omitted_candidate_records, 1);
    assert!(plan.trace.selected_candidate.is_some());
    assert_eq!(
        plan.trace.terminal_outcome,
        RuntimeRouteDecisionTerminalOutcome::Selected
    );
}

#[test]
fn hostile_huge_alias_stops_at_the_bounded_selection_pool() {
    let route = RuntimeGatewayRouteAlias {
        alias: "route".to_string(),
        models: (0..100_000)
            .map(|index| format!("unknown-{index}"))
            .collect(),
        strategy: RuntimeGatewayRouteStrategy::Fallback,
        model_metrics: BTreeMap::new(),
    };

    let plan = runtime_gateway_plan_route_with_constraints(
        ProviderId::OpenAi,
        ProviderEndpoint::Responses,
        br#"{"model":"route","input":"hi"}"#,
        &[route],
        1,
        &BTreeMap::new(),
        strict_policy(),
        &[],
        None,
        false,
    )
    .unwrap();

    assert!(plan.selection_pool_truncated);
    assert!(plan.truncated);
    assert!(plan.omitted_candidates > 0);
    assert_eq!(
        plan.alias_chain.len(),
        crate::RUNTIME_ROUTE_DECISION_TRACE_MAX_CANDIDATES
    );
    assert_eq!(
        plan.trace.truncation.omitted_candidate_records,
        plan.omitted_candidates
    );
    assert!(
        plan.concrete_candidates
            .iter()
            .all(|candidate| candidate.original_order
                < RUNTIME_GATEWAY_CONSTRAINT_PLANNER_MAX_CANDIDATES)
    );
}
