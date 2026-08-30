#[cfg(feature = "mojo")]
use super::*;

#[cfg(feature = "mojo")]
#[test]
fn mojo_trace_plan_matches_rust_trace_oracle() {
    let mut candidates = vec![
        trace_candidate(
            "endpoint",
            ProviderRequestConstraintDecision::EndpointUnsupported,
            false,
        ),
        trace_candidate(
            "constraints",
            ProviderRequestConstraintDecision::ContextWindowExceeded,
            false,
        ),
        trace_candidate(
            "selected",
            ProviderRequestConstraintDecision::Compatible,
            true,
        ),
    ];
    for (index, candidate) in candidates.iter_mut().enumerate() {
        candidate.original_order = index;
    }
    let model_state = BTreeMap::new();
    let actual = constraint_trace(ConstraintTraceInput {
        endpoint: ProviderEndpoint::Responses,
        provider: ProviderId::OpenAi,
        requested_model: "route",
        candidates: &candidates,
        selected_index: Some(2),
        selected_model: Some("selected"),
        no_route_reason: None,
        hard_affinity: false,
        model_state: &model_state,
    });
    let expected = constraint_trace_rust(ConstraintTraceInput {
        endpoint: ProviderEndpoint::Responses,
        provider: ProviderId::OpenAi,
        requested_model: "route",
        candidates: &candidates,
        selected_index: Some(2),
        selected_model: Some("selected"),
        no_route_reason: None,
        hard_affinity: false,
        model_state: &model_state,
    });
    assert_eq!(actual, expected);
}

#[cfg(feature = "mojo")]
fn trace_candidate(
    model: &str,
    decision: ProviderRequestConstraintDecision,
    eligible: bool,
) -> RuntimeGatewayConstraintCandidate {
    RuntimeGatewayConstraintCandidate {
        model: model.to_string(),
        original_order: 0,
        selected: eligible,
        evaluation: ProviderRequestConstraintEvaluation {
            decision,
            eligible,
            requirements: ProviderRequestRequirements {
                endpoint: ProviderEndpoint::Responses,
                requested_model: "route".to_string(),
                resolved_upstream_model: None,
                estimated_input_tokens: 1,
                explicit_output_tokens: None,
                output_limit_field: None,
                default_output_reserve_tokens: None,
                reasoning_effort: None,
                reasoning_reserve_tokens: None,
                total_required_tokens: 1,
                required_features: Vec::new(),
            },
            missing_feature: None,
            available_context_tokens: eligible.then_some(1_000),
            max_output_tokens: None,
            adjustment: None,
            warnings: Vec::new(),
        },
    }
}
