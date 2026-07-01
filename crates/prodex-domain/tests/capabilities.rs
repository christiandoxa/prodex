use prodex_domain::{
    CapabilityDecision, CapabilityErrorStatus, CapabilityRequest, CapabilitySet, ModelCapability,
    ModelRouteCandidate, negotiate_capability, plan_capability_decision_error_response,
};

fn candidate(
    provider: &str,
    model: &str,
    capabilities: Vec<ModelCapability>,
) -> ModelRouteCandidate {
    ModelRouteCandidate::new(provider, model, CapabilitySet::new(capabilities))
}

#[test]
fn capability_set_sorts_and_deduplicates() {
    let set = CapabilitySet::new(vec![
        ModelCapability::Streaming,
        ModelCapability::Tools,
        ModelCapability::Streaming,
    ]);

    assert_eq!(set.as_slice().len(), 2);
    assert!(set.contains(ModelCapability::Streaming));
    assert!(set.contains(ModelCapability::Tools));
}

#[test]
fn capability_set_debug_output_is_stable_and_redacted() {
    let set = CapabilitySet::new(vec![ModelCapability::Streaming, ModelCapability::Tools]);

    let rendered = format!("{set:?}");
    assert!(!rendered.contains("Streaming"));
    assert!(!rendered.contains("Tools"));
    assert_eq!(rendered, "CapabilitySet { count: 2 }");
}

#[test]
fn capability_request_debug_output_is_stable_and_redacted() {
    let request = CapabilityRequest::new(CapabilitySet::new(vec![
        ModelCapability::ResponsesApi,
        ModelCapability::RemoteCompact,
    ]));

    let rendered = format!("{request:?}");
    assert!(!rendered.contains("ResponsesApi"));
    assert!(!rendered.contains("RemoteCompact"));
    assert_eq!(rendered, "CapabilityRequest { required: \"<redacted>\" }");
}

#[test]
fn negotiation_picks_first_compatible_candidate() {
    let request = CapabilityRequest::new(CapabilitySet::new(vec![
        ModelCapability::ResponsesApi,
        ModelCapability::Streaming,
    ]));
    let candidates = vec![
        candidate(
            "provider-a",
            "text-only",
            vec![ModelCapability::ResponsesApi],
        ),
        candidate(
            "provider-b",
            "streaming",
            vec![ModelCapability::ResponsesApi, ModelCapability::Streaming],
        ),
    ];

    assert_eq!(
        negotiate_capability(&request, &candidates),
        CapabilityDecision::Compatible(candidates[1].clone())
    );
}

#[test]
fn negotiation_skips_malformed_provider_or_model_names() {
    let request = CapabilityRequest::new(CapabilitySet::new(vec![ModelCapability::ResponsesApi]));
    let candidates = vec![
        candidate("", "valid-model", vec![ModelCapability::ResponsesApi]),
        candidate("  ", "valid-model", vec![ModelCapability::ResponsesApi]),
        candidate(
            "provider-a",
            "bad model",
            vec![ModelCapability::ResponsesApi],
        ),
        candidate(
            "provider-overlong",
            &"x".repeat(129),
            vec![ModelCapability::ResponsesApi],
        ),
        candidate(
            "provider-b",
            "valid-model",
            vec![ModelCapability::ResponsesApi],
        ),
    ];

    assert!(!candidates[0].is_well_formed());
    assert!(!candidates[1].is_well_formed());
    assert!(!candidates[2].is_well_formed());
    assert!(!candidates[3].is_well_formed());
    assert!(candidates[4].is_well_formed());
    assert_eq!(
        negotiate_capability(&request, &candidates),
        CapabilityDecision::Compatible(candidates[4].clone())
    );
}

#[test]
fn malformed_only_candidate_pool_reports_no_candidate() {
    let request = CapabilityRequest::new(CapabilitySet::new(vec![ModelCapability::ResponsesApi]));
    let candidates = vec![candidate(
        "provider\nsecret",
        "model-é",
        vec![ModelCapability::ResponsesApi],
    )];

    assert_eq!(
        negotiate_capability(&request, &candidates),
        CapabilityDecision::NoCandidate
    );
}

#[test]
fn model_route_candidate_debug_output_is_stable_and_redacted() {
    let candidate = candidate(
        "provider-secret-route",
        "internal-premium-model",
        vec![ModelCapability::ResponsesApi],
    );

    let rendered = format!("{candidate:?}");
    assert!(!rendered.contains("provider-secret-route"));
    assert!(!rendered.contains("internal-premium-model"));
    assert!(rendered.contains("provider: \"<redacted>\""));
    assert!(rendered.contains("model: \"<redacted>\""));
    assert!(rendered.contains("ResponsesApi"));
}

#[test]
fn negotiation_reports_missing_capabilities_without_rewriting_provider_behavior() {
    let request = CapabilityRequest::new(CapabilitySet::new(vec![
        ModelCapability::ResponsesApi,
        ModelCapability::Tools,
        ModelCapability::RemoteCompact,
    ]));
    let candidates = vec![candidate(
        "provider-a",
        "basic",
        vec![ModelCapability::ResponsesApi],
    )];

    assert_eq!(
        negotiate_capability(&request, &candidates),
        CapabilityDecision::Incompatible {
            candidate: candidates[0].clone(),
            missing: vec![ModelCapability::Tools, ModelCapability::RemoteCompact],
        }
    );
}

#[test]
fn capability_decision_debug_output_is_stable_and_redacted() {
    let decision = CapabilityDecision::Incompatible {
        candidate: candidate(
            "provider-secret-route",
            "internal-premium-model",
            vec![ModelCapability::ResponsesApi],
        ),
        missing: vec![ModelCapability::Tools, ModelCapability::RemoteCompact],
    };

    let rendered = format!("{decision:?}");
    for sensitive in [
        "provider-secret-route",
        "internal-premium-model",
        "Tools",
        "RemoteCompact",
    ] {
        assert!(
            !rendered.contains(sensitive),
            "capability decision debug output leaked sensitive token {sensitive}: {rendered}"
        );
    }
    assert_eq!(
        rendered,
        "Incompatible { candidate: \"<redacted>\", missing: \"<redacted>\" }"
    );
}

#[test]
fn negotiation_reports_no_candidate_for_empty_pool() {
    let request = CapabilityRequest::new(CapabilitySet::new(vec![ModelCapability::ResponsesApi]));

    assert_eq!(
        negotiate_capability(&request, &[]),
        CapabilityDecision::NoCandidate
    );
}

#[test]
fn capability_error_responses_are_stable_and_redacted() {
    let request = CapabilityRequest::new(CapabilitySet::new(vec![
        ModelCapability::ResponsesApi,
        ModelCapability::Tools,
        ModelCapability::RemoteCompact,
    ]));
    let candidates = vec![candidate(
        "provider-secret-route",
        "internal-premium-model",
        vec![ModelCapability::ResponsesApi],
    )];
    let incompatible = negotiate_capability(&request, &candidates);

    let response = plan_capability_decision_error_response(&incompatible).unwrap();
    assert_eq!(response.status, CapabilityErrorStatus::UnprocessableRequest);
    assert_eq!(response.code, "model_capability_unsupported");
    assert_eq!(
        response.message,
        "requested model capabilities are not supported"
    );

    let unavailable =
        plan_capability_decision_error_response(&CapabilityDecision::NoCandidate).unwrap();
    assert_eq!(
        unavailable.status,
        CapabilityErrorStatus::ServiceUnavailable
    );
    assert_eq!(unavailable.code, "model_route_unavailable");
    assert_eq!(
        unavailable.message,
        "no compatible model route is available"
    );

    assert!(
        plan_capability_decision_error_response(&CapabilityDecision::Compatible(
            candidates[0].clone()
        ))
        .is_none()
    );

    let rendered = format!("{response:?} {unavailable:?}");
    for sensitive in [
        "provider-secret-route",
        "internal-premium-model",
        "RemoteCompact",
        "Tools",
        "ResponsesApi",
    ] {
        assert!(
            !rendered.contains(sensitive),
            "capability response leaked sensitive token {sensitive}: {rendered}"
        );
    }
}
