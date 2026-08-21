use std::collections::BTreeMap;

use super::*;
use crate::{ProviderCatalogEntry, ProviderCatalogFeatureFlags};

fn requirements(body: &[u8]) -> ProviderRequestRequirements {
    provider_request_requirements(body, ProviderEndpoint::Responses, "test-model", &[]).unwrap()
}

fn entry(
    context_window_tokens: Option<u64>,
    max_output_tokens: Option<u64>,
) -> ProviderCatalogEntry {
    ProviderCatalogEntry {
        provider: ProviderId::OpenAi,
        owned_by: "example".to_string(),
        id: "test-model".to_string(),
        display_name: "Test Model".to_string(),
        description: "Synthetic test model.".to_string(),
        context_window_tokens,
        max_output_tokens,
        default_output_reserve_tokens: None,
        supported_reasoning_efforts: None,
        default_reasoning_effort: None,
        reasoning_reserve_tokens: None,
        embedding_compatible: None,
        input_cost_per_million_microusd: None,
        output_cost_per_million_microusd: None,
        supported_endpoints: vec![ProviderEndpoint::Responses],
        aliases: Vec::new(),
        feature_flags: ProviderCatalogFeatureFlags {
            tools: true,
            json_schema: true,
            vision: true,
            audio: true,
            web_search: true,
            reasoning: true,
        },
        pricing_known: false,
    }
}

fn strict_policy() -> ProviderRequestConstraintPolicy {
    ProviderRequestConstraintPolicy {
        enabled: true,
        unknown_context: ProviderUnknownContextPolicy::Reject,
        safe_window_tokens: 128_000,
        oversized_output: ProviderOversizedOutputPolicy::Reject,
    }
}

fn evaluate(
    requirements: &ProviderRequestRequirements,
    policy: ProviderRequestConstraintPolicy,
    entry: &ProviderCatalogEntry,
) -> ProviderRequestConstraintEvaluation {
    evaluate_provider_request_constraints_with_catalog_entry(
        ProviderId::OpenAi,
        "test-model",
        requirements,
        policy,
        Some(entry),
    )
}

#[test]
fn output_limit_parser_accepts_one_positive_alias_and_rejects_malformed_or_conflicting_values() {
    for (field, expected) in [
        (
            "max_output_tokens",
            ProviderOutputLimitField::MaxOutputTokens,
        ),
        (
            "max_completion_tokens",
            ProviderOutputLimitField::MaxCompletionTokens,
        ),
        ("max_tokens", ProviderOutputLimitField::MaxTokens),
    ] {
        let body = format!(r#"{{"model":"test-model","{field}":42}}"#);
        let parsed = requirements(body.as_bytes());
        assert_eq!(parsed.output_limit_field, Some(expected));
        assert_eq!(parsed.explicit_output_tokens, Some(42));
    }
    for body in [
        br#"{"model":"test-model","max_output_tokens":0}"#.as_slice(),
        br#"{"model":"test-model","max_output_tokens":-1}"#.as_slice(),
        br#"{"model":"test-model","max_output_tokens":"42"}"#.as_slice(),
        br#"{"model":"test-model","max_output_tokens":18446744073709551616}"#.as_slice(),
        br#"{"model":"test-model","max_output_tokens":1,"max_tokens":1}"#.as_slice(),
    ] {
        assert!(
            provider_request_requirements(body, ProviderEndpoint::Responses, "test-model", &[],)
                .is_err()
        );
    }
}

#[test]
fn compatibility_output_parser_preserves_legacy_field_precedence() {
    let value = serde_json::json!({
        "max_tokens": 7,
        "max_completion_tokens": 11
    });
    assert_eq!(provider_requested_output_tokens_compat(&value), Some(7));
    assert!(provider_requested_output_tokens(&value).is_err());
}

#[test]
fn production_catalog_compact_and_embedding_constraints_follow_runtime_capabilities() {
    let gemini_compact = provider_request_requirements(
        br#"{"model":"gemini-2.5-flash","input":"hi"}"#,
        ProviderEndpoint::ResponsesCompact,
        "gemini-2.5-flash",
        &[],
    )
    .unwrap();
    let compact = evaluate_provider_request_constraints(
        ProviderId::Gemini,
        "gemini-2.5-flash",
        &gemini_compact,
        strict_policy(),
    );
    assert!(compact.eligible);

    let openai_compact = provider_request_requirements(
        br#"{"model":"gpt-5.4","input":"hi"}"#,
        ProviderEndpoint::ResponsesCompact,
        "gpt-5.4",
        &[],
    )
    .unwrap();
    let compact = evaluate_provider_request_constraints(
        ProviderId::OpenAi,
        "gpt-5.4",
        &openai_compact,
        strict_policy(),
    );
    assert!(compact.eligible);

    for (provider, model) in [
        (ProviderId::OpenAi, "gpt-5.4"),
        (ProviderId::Gemini, "gemini-2.5-flash"),
    ] {
        let request = provider_request_requirements(
            format!(r#"{{"model":"{model}","input":"hi"}}"#).as_bytes(),
            ProviderEndpoint::Embeddings,
            model,
            &[],
        )
        .unwrap();
        let result =
            evaluate_provider_request_constraints(provider, model, &request, strict_policy());
        assert!(result.eligible, "{provider:?}:{model}");
    }

    let unknown = provider_request_requirements(
        br#"{"model":"unknown-embedding-model","input":"hi"}"#,
        ProviderEndpoint::Embeddings,
        "unknown-embedding-model",
        &[],
    )
    .unwrap();
    let result = evaluate_provider_request_constraints(
        ProviderId::OpenAi,
        "unknown-embedding-model",
        &unknown,
        strict_policy(),
    );
    assert!(!result.eligible);
    assert_eq!(
        result.decision,
        ProviderRequestConstraintDecision::EndpointUnsupported
    );
}

#[test]
fn output_limit_boundary_and_clamp_are_explicit() {
    let model = entry(Some(100), Some(20));
    let at_limit = evaluate(
        &requirements(br#"{"model":"test-model","input":"hi","max_output_tokens":20}"#),
        strict_policy(),
        &model,
    );
    assert!(at_limit.eligible);
    assert_eq!(
        at_limit.decision,
        ProviderRequestConstraintDecision::Compatible
    );

    let above = requirements(br#"{"model":"test-model","input":"hi","max_output_tokens":21}"#);
    let rejected = evaluate(&above, strict_policy(), &model);
    assert!(!rejected.eligible);
    assert_eq!(
        rejected.decision,
        ProviderRequestConstraintDecision::RequestedOutputExceedsModelLimit
    );
    let clamped = evaluate(
        &above,
        ProviderRequestConstraintPolicy {
            oversized_output: ProviderOversizedOutputPolicy::ClampWithNotice,
            ..strict_policy()
        },
        &model,
    );
    assert!(clamped.eligible);
    assert_eq!(
        clamped.adjustment,
        Some(ProviderOutputAdjustment {
            field: ProviderOutputLimitField::MaxOutputTokens,
            requested_tokens: 21,
            applied_tokens: 20,
            reason: ProviderRequestConstraintDecision::OutputLimitClamped,
        })
    );
}

#[test]
fn unknown_context_policies_and_safe_window_boundary_are_exact() {
    let model = entry(None, None);
    let request = requirements(br#"{"model":"test-model","input":"12345678","max_tokens":3}"#);
    let total = request.total_required_tokens;
    for (unknown_context, safe_window_tokens, eligible, decision) in [
        (
            ProviderUnknownContextPolicy::Allow,
            total.saturating_sub(1),
            true,
            ProviderRequestConstraintDecision::ContextWindowUnknown,
        ),
        (
            ProviderUnknownContextPolicy::Reject,
            total,
            false,
            ProviderRequestConstraintDecision::ContextWindowUnknown,
        ),
        (
            ProviderUnknownContextPolicy::SafeWindow,
            total,
            true,
            ProviderRequestConstraintDecision::ContextWindowUnknown,
        ),
        (
            ProviderUnknownContextPolicy::SafeWindow,
            total.saturating_sub(1),
            false,
            ProviderRequestConstraintDecision::ContextWindowExceeded,
        ),
    ] {
        let result = evaluate(
            &request,
            ProviderRequestConstraintPolicy {
                enabled: true,
                unknown_context,
                safe_window_tokens,
                oversized_output: ProviderOversizedOutputPolicy::Passthrough,
            },
            &model,
        );
        assert_eq!(result.eligible, eligible);
        assert_eq!(result.decision, decision);
    }
}

#[test]
fn no_explicit_output_uses_known_default_reserve_and_saturates_totals() {
    let mut model = entry(Some(u64::MAX), None);
    model.default_output_reserve_tokens = Some(9);
    let request = requirements(br#"{"model":"test-model","input":"hi"}"#);
    let result = evaluate(&request, strict_policy(), &model);
    assert_eq!(result.requirements.default_output_reserve_tokens, Some(9));
    assert_eq!(result.requirements.total_required_tokens, 10);

    let mut overflow = request;
    overflow.estimated_input_tokens = u64::MAX;
    let saturated = evaluate(&overflow, strict_policy(), &model);
    assert_eq!(saturated.requirements.total_required_tokens, u64::MAX);
}

#[test]
fn endpoint_and_required_feature_failures_are_typed() {
    let mut model = entry(Some(100), None);
    model.supported_endpoints = vec![ProviderEndpoint::Models];
    let endpoint = evaluate(
        &requirements(br#"{"model":"test-model","input":"hi"}"#),
        strict_policy(),
        &model,
    );
    assert_eq!(
        endpoint.decision,
        ProviderRequestConstraintDecision::EndpointUnsupported
    );

    model.supported_endpoints = vec![ProviderEndpoint::Responses];
    model.feature_flags = ProviderCatalogFeatureFlags {
        tools: false,
        json_schema: false,
        vision: false,
        audio: false,
        web_search: false,
        reasoning: false,
    };
    for feature in [
        ProviderRequestFeature::Tools,
        ProviderRequestFeature::JsonSchema,
        ProviderRequestFeature::Vision,
        ProviderRequestFeature::Audio,
        ProviderRequestFeature::WebSearch,
        ProviderRequestFeature::Websocket,
    ] {
        let request = provider_request_requirements(
            br#"{"model":"test-model","input":"hi"}"#,
            ProviderEndpoint::Responses,
            "test-model",
            &[feature],
        )
        .unwrap();
        let result = evaluate(&request, strict_policy(), &model);
        assert!(!result.eligible, "{feature:?}");
        assert_eq!(result.missing_feature, Some(feature));
    }
}

#[test]
fn categorical_endpoint_rejection_precedes_unknown_model_policy() {
    let request = provider_request_requirements(
        br#"{"model":"unknown-model","input":"hi"}"#,
        ProviderEndpoint::Embeddings,
        "unknown-model",
        &[],
    )
    .unwrap();
    let result = evaluate_provider_request_constraints_with_catalog_entry(
        ProviderId::Kiro,
        "unknown-model",
        &request,
        ProviderRequestConstraintPolicy {
            unknown_context: ProviderUnknownContextPolicy::Allow,
            ..strict_policy()
        },
        None,
    );
    assert!(!result.eligible);
    assert_eq!(
        result.decision,
        ProviderRequestConstraintDecision::EndpointUnsupported
    );
}

#[test]
fn kiro_messages_catalog_matches_the_runtime_endpoint() {
    let request = provider_request_requirements(
        br#"{"model":"auto","messages":[{"role":"user","content":"hi"}],"max_tokens":64}"#,
        ProviderEndpoint::Messages,
        "auto",
        &[],
    )
    .unwrap();
    let result =
        evaluate_provider_request_constraints(ProviderId::Kiro, "auto", &request, strict_policy());
    assert!(result.eligible);
    assert_ne!(
        result.decision,
        ProviderRequestConstraintDecision::EndpointUnsupported
    );
}

#[test]
fn reasoning_reserve_is_counted_and_can_be_the_excess() {
    let mut model = entry(Some(8), None);
    model.supported_reasoning_efforts = Some(vec![ProviderReasoningEffort::High]);
    model.default_reasoning_effort = Some(ProviderReasoningEffort::High);
    model.reasoning_reserve_tokens = Some(BTreeMap::from([(ProviderReasoningEffort::High, 8)]));
    let result = evaluate(
        &requirements(br#"{"model":"test-model","input":"hi"}"#),
        strict_policy(),
        &model,
    );
    assert!(!result.eligible);
    assert_eq!(
        result.decision,
        ProviderRequestConstraintDecision::ReasoningReserveExcessive
    );
    assert_eq!(result.requirements.reasoning_reserve_tokens, Some(8));
}

#[test]
fn explicit_thinking_budget_is_counted_without_inventing_native_level_limits() {
    let request = requirements(
        br#"{"model":"test-model","input":"hi","thinking":{"type":"enabled","budget_tokens":12}}"#,
    );
    assert_eq!(request.reasoning_reserve_tokens, Some(12));
    assert!(
        request
            .required_features
            .contains(&ProviderRequestFeature::Reasoning)
    );
    assert_eq!(request.total_required_tokens, 13);

    let result = evaluate(&request, strict_policy(), &entry(Some(13), None));
    assert!(result.eligible);
    assert_eq!(result.requirements.reasoning_reserve_tokens, Some(12));
    for budget in [serde_json::json!(0), serde_json::json!("12")] {
        let value = serde_json::json!({
            "model": "test-model",
            "thinking": {"budget_tokens": budget}
        });
        assert!(
            provider_request_requirements_from_value(
                &value,
                ProviderEndpoint::Responses,
                "test-model",
                &[],
            )
            .is_err()
        );
    }
}

#[test]
fn streaming_compact_and_websocket_requirements_are_explicit() {
    let streaming = provider_request_requirements(
        br#"{"model":"test-model","stream":true}"#,
        ProviderEndpoint::Responses,
        "test-model",
        &[],
    )
    .unwrap();
    assert!(
        streaming
            .required_features
            .contains(&ProviderRequestFeature::Streaming)
    );
    let compact = provider_request_requirements(
        br#"{"model":"test-model"}"#,
        ProviderEndpoint::ResponsesCompact,
        "test-model",
        &[],
    )
    .unwrap();
    assert!(
        compact
            .required_features
            .contains(&ProviderRequestFeature::Compact)
    );
    let websocket = provider_request_requirements(
        br#"{"model":"test-model"}"#,
        ProviderEndpoint::Responses,
        "test-model",
        &[ProviderRequestFeature::Websocket],
    )
    .unwrap();
    let result = evaluate(&websocket, strict_policy(), &entry(Some(100), None));
    assert!(!result.eligible);
    assert_eq!(
        result.missing_feature,
        Some(ProviderRequestFeature::Websocket)
    );
}

#[test]
fn json_capability_inference_only_uses_top_level_response_controls() {
    let text = provider_request_requirements(
        br#"{"model":"test-model","response_format":{"type":"text"},"tools":[{"type":"function","function":{"parameters":{"format":"json_schema"}}}]}"#,
        ProviderEndpoint::Responses,
        "test-model",
        &[],
    )
    .unwrap();
    assert!(
        !text
            .required_features
            .contains(&ProviderRequestFeature::JsonSchema)
    );

    let json = provider_request_requirements(
        br#"{"model":"test-model","text":{"format":{"type":"json_schema"}}}"#,
        ProviderEndpoint::Responses,
        "test-model",
        &[],
    )
    .unwrap();
    assert!(
        json.required_features
            .contains(&ProviderRequestFeature::JsonSchema)
    );
}

#[test]
fn catalog_optional_constraint_fields_remain_backward_compatible() {
    let value = serde_json::json!({
        "provider": "openai",
        "owned_by": "example",
        "id": "legacy",
        "display_name": "Legacy",
        "description": "Legacy catalog row.",
        "context_window_tokens": 100,
        "input_cost_per_million_microusd": null,
        "output_cost_per_million_microusd": null,
        "supported_endpoints": ["responses"],
        "aliases": [],
        "feature_flags": {
            "tools": false,
            "json_schema": false,
            "vision": false,
            "audio": false,
            "web_search": false,
            "reasoning": false
        },
        "pricing_known": false
    });
    let entry: ProviderCatalogEntry = serde_json::from_value(value).unwrap();
    assert_eq!(entry.max_output_tokens, None);
    assert_eq!(entry.reasoning_reserve_tokens, None);
    assert_eq!(entry.embedding_compatible, None);
}

#[cfg(feature = "mojo")]
#[test]
fn provider_constraints_match_rust_oracle_for_generated_normalized_cases() {
    let mut state = 0x70726f7669646572_u64;
    for case in 0..2_000 {
        let next = |state: &mut u64| {
            *state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            *state
        };
        let endpoint = if next(&mut state) & 1 == 0 {
            ProviderEndpoint::Responses
        } else {
            ProviderEndpoint::Embeddings
        };
        let mut model = entry(
            (next(&mut state) & 1 == 0).then(|| (next(&mut state) % 500).max(1)),
            (next(&mut state) & 1 == 0).then(|| (next(&mut state) % 200).max(1)),
        );
        model.supported_endpoints =
            if endpoint == ProviderEndpoint::Embeddings && next(&mut state) & 1 != 0 {
                vec![ProviderEndpoint::Embeddings]
            } else {
                vec![ProviderEndpoint::Responses]
            };
        model.feature_flags = ProviderCatalogFeatureFlags {
            tools: next(&mut state) & 1 != 0,
            json_schema: next(&mut state) & 1 != 0,
            vision: next(&mut state) & 1 != 0,
            audio: next(&mut state) & 1 != 0,
            web_search: next(&mut state) & 1 != 0,
            reasoning: next(&mut state) & 1 != 0,
        };
        model.supported_reasoning_efforts = Some(vec![ProviderReasoningEffort::High]);
        let required_features = (0..3)
            .filter_map(|index| {
                (next(&mut state) & 1 != 0).then_some(match index {
                    0 => ProviderRequestFeature::Tools,
                    1 => ProviderRequestFeature::Reasoning,
                    _ => ProviderRequestFeature::Vision,
                })
            })
            .collect::<Vec<_>>();
        let explicit_output_tokens = (next(&mut state) & 1 != 0).then(|| next(&mut state) % 300);
        let requirements = ProviderRequestRequirements {
            endpoint,
            requested_model: "synthetic-model".to_string(),
            resolved_upstream_model: None,
            estimated_input_tokens: next(&mut state) % 700,
            explicit_output_tokens,
            output_limit_field: explicit_output_tokens.map(|_| match next(&mut state) % 3 {
                0 => ProviderOutputLimitField::MaxOutputTokens,
                1 => ProviderOutputLimitField::MaxCompletionTokens,
                _ => ProviderOutputLimitField::MaxTokens,
            }),
            default_output_reserve_tokens: (next(&mut state) & 1 != 0)
                .then(|| next(&mut state) % 100),
            reasoning_effort: (next(&mut state) & 1 != 0).then(|| {
                if next(&mut state) & 1 == 0 {
                    ProviderReasoningEffort::High
                } else {
                    ProviderReasoningEffort::Unknown
                }
            }),
            reasoning_reserve_tokens: (next(&mut state) & 1 != 0).then(|| next(&mut state) % 200),
            total_required_tokens: 0,
            required_features,
        };
        let policy = ProviderRequestConstraintPolicy {
            enabled: next(&mut state) & 1 != 0,
            unknown_context: match next(&mut state) % 3 {
                0 => ProviderUnknownContextPolicy::Allow,
                1 => ProviderUnknownContextPolicy::SafeWindow,
                _ => ProviderUnknownContextPolicy::Reject,
            },
            safe_window_tokens: next(&mut state) % 500,
            oversized_output: match next(&mut state) % 3 {
                0 => ProviderOversizedOutputPolicy::Passthrough,
                1 => ProviderOversizedOutputPolicy::Reject,
                _ => ProviderOversizedOutputPolicy::ClampWithNotice,
            },
        };
        let entry = (next(&mut state) & 1 != 0).then_some(model);
        let entry_ref = entry.as_ref();
        let mut resolved =
            resolve_provider_request_requirements(&requirements, "synthetic-model", entry_ref);
        recalculate_total(&mut resolved);
        let expected = evaluate_provider_request_constraints_resolved_rust(
            ProviderId::OpenAi,
            &requirements,
            policy,
            resolved,
            entry_ref,
        );
        let actual = evaluate_provider_request_constraints_with_catalog_entry(
            ProviderId::OpenAi,
            "synthetic-model",
            &requirements,
            policy,
            entry_ref,
        );
        assert_eq!(actual, expected, "provider constraint case {case}");
    }
}
