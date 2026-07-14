use prodex_domain::{
    CallId, CapabilityErrorStatus, CapabilityRequest, CapabilitySet, CredentialScope,
    ModelCapability, Principal, PrincipalId, PrincipalKind, RequestId, Role, SecretRef,
    TenantContext, TenantId, UsageAmount,
};
use prodex_provider_core::{ProviderEndpoint, ProviderErrorClass, ProviderId, ProviderWireFormat};
use prodex_provider_spi::{
    ProviderCapabilityNegotiationDecision, ProviderCircuitBreakerDecision,
    ProviderCircuitBreakerEvent, ProviderCircuitBreakerPolicy, ProviderCircuitBreakerState,
    ProviderInvocation, ProviderInvocationError, ProviderInvocationErrorStatus, ProviderRetryCause,
    ProviderRetryDecision, ProviderRetryDecisionStatus, ProviderRetryPolicy, ProviderRetryStage,
    ProviderRoute, ProviderRouteCapabilityCandidate, ProviderRouteError, ProviderRouteErrorStatus,
    ProviderStreamMode, evaluate_provider_retry, negotiate_provider_route_capability,
    plan_provider_capability_negotiation_error_response, plan_provider_circuit_breaker,
    plan_provider_circuit_breaker_event, plan_provider_invocation_error_response,
    plan_provider_retry, plan_provider_retry_decision_response, plan_provider_route_error_response,
    validate_provider_invocation,
};

fn principal(tenant_id: Option<TenantId>, scope: CredentialScope) -> Principal {
    Principal::new(
        PrincipalId::new(),
        tenant_id,
        PrincipalKind::VirtualKey,
        Role::Operator,
        scope,
    )
}

fn invocation(tenant_id: TenantId, principal: Principal) -> ProviderInvocation {
    ProviderInvocation {
        tenant: TenantContext { tenant_id },
        principal,
        request_id: RequestId::new(),
        call_id: CallId::new(),
        route: ProviderRoute::new(
            ProviderId::OpenAi,
            ProviderEndpoint::Responses,
            ProviderWireFormat::OpenAiResponses,
            "gpt-5.3-codex",
        )
        .unwrap(),
        credential_ref: SecretRef::new("external-secret", "openai-prod", Some("v7")),
        stream_mode: ProviderStreamMode::Streaming,
        estimated_usage: UsageAmount::new(100, 1_000),
    }
}

fn route(provider: ProviderId, model: &str) -> ProviderRoute {
    ProviderRoute::new(
        provider,
        ProviderEndpoint::Responses,
        ProviderWireFormat::OpenAiResponses,
        model,
    )
    .unwrap()
}

#[test]
fn provider_route_requires_stable_non_empty_model() {
    assert_eq!(
        ProviderRoute::new(
            ProviderId::OpenAi,
            ProviderEndpoint::Responses,
            ProviderWireFormat::OpenAiResponses,
            " ",
        ),
        Err(ProviderRouteError::MissingModel)
    );
    assert_eq!(
        ProviderRoute::new(
            ProviderId::OpenAi,
            ProviderEndpoint::Responses,
            ProviderWireFormat::OpenAiResponses,
            "x".repeat(257),
        ),
        Err(ProviderRouteError::ModelTooLong { length: 257 })
    );
}

#[test]
fn provider_capability_negotiation_selects_compatible_route_before_dispatch() {
    let request = CapabilityRequest::new(CapabilitySet::new(vec![
        ModelCapability::ResponsesApi,
        ModelCapability::Streaming,
        ModelCapability::Tools,
    ]));
    let candidates = vec![
        ProviderRouteCapabilityCandidate::new(
            route(ProviderId::OpenAi, "gpt-base"),
            CapabilitySet::new(vec![ModelCapability::ResponsesApi]),
        ),
        ProviderRouteCapabilityCandidate::new(
            route(ProviderId::Copilot, "gpt-5.3-codex"),
            CapabilitySet::new(vec![
                ModelCapability::ResponsesApi,
                ModelCapability::Streaming,
                ModelCapability::Tools,
            ]),
        ),
    ];

    let decision = negotiate_provider_route_capability(&request, &candidates);

    let ProviderCapabilityNegotiationDecision::Compatible(plan) = decision else {
        panic!("expected compatible provider route");
    };
    assert_eq!(plan.route.provider, ProviderId::Copilot);
    assert_eq!(plan.route.model, "gpt-5.3-codex");
    assert!(plan.capabilities.contains(ModelCapability::Streaming));
}

#[test]
fn provider_capability_negotiation_errors_are_stable_and_redacted() {
    let request = CapabilityRequest::new(CapabilitySet::new(vec![
        ModelCapability::ResponsesApi,
        ModelCapability::RemoteCompact,
    ]));
    let candidates = vec![ProviderRouteCapabilityCandidate::new(
        route(ProviderId::OpenAi, "internal-premium-model"),
        CapabilitySet::new(vec![ModelCapability::ResponsesApi]),
    )];

    let decision = negotiate_provider_route_capability(&request, &candidates);
    let response = plan_provider_capability_negotiation_error_response(&decision).unwrap();

    assert_eq!(response.status, CapabilityErrorStatus::UnprocessableRequest);
    assert_eq!(response.code, "model_capability_unsupported");
    assert_eq!(
        response.message,
        "requested model capabilities are not supported"
    );
    assert!(!response.message.contains("internal-premium-model"));
    assert!(!response.message.contains("OpenAi"));

    let unavailable = plan_provider_capability_negotiation_error_response(
        &negotiate_provider_route_capability(&request, &[]),
    )
    .unwrap();
    assert_eq!(
        unavailable.status,
        CapabilityErrorStatus::ServiceUnavailable
    );
    assert_eq!(unavailable.code, "model_route_unavailable");
}

#[test]
fn provider_route_error_responses_are_stable_and_redacted() {
    let error = ProviderRouteError::ModelTooLong { length: 257 };
    assert!(!error.to_string().contains("257"));
    assert!(!error.to_string().contains("model"));
    assert_eq!(error.to_string(), "provider route is invalid");
    let response = plan_provider_route_error_response(&error);

    assert_eq!(response.status, ProviderRouteErrorStatus::BadRequest);
    assert_eq!(response.code, "provider_route_invalid");
    assert_eq!(response.message, "provider route is invalid");
    assert!(!response.message.contains("257"));
    assert!(!response.message.contains("gpt"));
}

#[test]
fn provider_invocation_accepts_matching_tenant_data_plane_principal() {
    let tenant_id = TenantId::new();
    let invocation = invocation(
        tenant_id,
        principal(Some(tenant_id), CredentialScope::DataPlane),
    );

    assert_eq!(validate_provider_invocation(&invocation), Ok(()));
}

#[test]
fn provider_invocation_rejects_cross_tenant_or_missing_tenant_principal() {
    let tenant_id = TenantId::new();

    assert_eq!(
        validate_provider_invocation(&invocation(
            tenant_id,
            principal(None, CredentialScope::DataPlane)
        )),
        Err(ProviderInvocationError::PrincipalMissingTenant)
    );
    assert!(matches!(
        validate_provider_invocation(&invocation(
            tenant_id,
            principal(Some(TenantId::new()), CredentialScope::DataPlane),
        )),
        Err(ProviderInvocationError::CrossTenantPrincipal { .. })
    ));
}

#[test]
fn provider_invocation_rejects_control_plane_or_break_glass_scope_for_inference() {
    let tenant_id = TenantId::new();

    assert_eq!(
        validate_provider_invocation(&invocation(
            tenant_id,
            principal(Some(tenant_id), CredentialScope::ControlPlane),
        )),
        Err(ProviderInvocationError::CredentialScopeMismatch {
            actual: CredentialScope::ControlPlane,
        })
    );
    assert_eq!(
        validate_provider_invocation(&invocation(
            tenant_id,
            principal(Some(tenant_id), CredentialScope::BreakGlass),
        )),
        Err(ProviderInvocationError::CredentialScopeMismatch {
            actual: CredentialScope::BreakGlass,
        })
    );
}

#[test]
fn provider_invocation_error_responses_are_stable_and_redacted() {
    let tenant_id = TenantId::new();
    let other_tenant = TenantId::new();

    let cross_tenant = ProviderInvocationError::CrossTenantPrincipal {
        tenant: TenantContext { tenant_id },
        principal_tenant: TenantContext {
            tenant_id: other_tenant,
        },
    };
    let response = plan_provider_invocation_error_response(&cross_tenant);
    assert_eq!(response.status, ProviderInvocationErrorStatus::Forbidden);
    assert_eq!(response.code, "provider_invocation_not_authorized");
    assert_eq!(response.message, "provider invocation is not authorized");
    assert_eq!(
        cross_tenant.to_string(),
        "provider invocation is not authorized"
    );
    assert!(!response.message.contains(&tenant_id.to_string()));
    assert!(!response.message.contains(&other_tenant.to_string()));
    assert!(!cross_tenant.to_string().contains(&tenant_id.to_string()));
    assert!(!cross_tenant.to_string().contains(&other_tenant.to_string()));

    let scope = ProviderInvocationError::CredentialScopeMismatch {
        actual: CredentialScope::ControlPlane,
    };
    assert!(!scope.to_string().contains("ControlPlane"));
    assert!(!scope.to_string().contains("DataPlane"));
    assert!(!scope.to_string().contains("credential"));
    assert_eq!(scope.to_string(), "provider invocation is not authorized");
    let scope_response = plan_provider_invocation_error_response(&scope);
    assert_eq!(scope_response, response);
    assert!(!scope_response.message.contains("ControlPlane"));
    assert!(!scope_response.message.contains("openai-prod"));

    let break_glass_scope = ProviderInvocationError::CredentialScopeMismatch {
        actual: CredentialScope::BreakGlass,
    };
    assert!(!break_glass_scope.to_string().contains("BreakGlass"));
}

#[test]
fn provider_retry_is_denied_after_stream_commit_or_cancellation() {
    assert_eq!(
        evaluate_provider_retry(ProviderRetryStage::BeforeDispatch),
        ProviderRetryDecision::Allowed,
    );
    assert_eq!(
        evaluate_provider_retry(ProviderRetryStage::BeforeFirstByte),
        ProviderRetryDecision::Allowed,
    );
    assert_eq!(
        evaluate_provider_retry(ProviderRetryStage::AfterFirstByte),
        ProviderRetryDecision::DeniedCommitted,
    );
    assert_eq!(
        evaluate_provider_retry(ProviderRetryStage::AfterCancellation),
        ProviderRetryDecision::DeniedCommitted,
    );
}

#[test]
fn provider_retry_plan_is_bounded_to_precommit_attempts() {
    let policy = ProviderRetryPolicy::single_retry();
    let allowed = plan_provider_retry(
        policy,
        ProviderRetryStage::BeforeFirstByte,
        ProviderRetryCause::NextModel,
        ProviderErrorClass::Transient,
        0,
    );
    assert_eq!(allowed.stage, ProviderRetryStage::BeforeFirstByte);
    assert_eq!(allowed.decision, ProviderRetryDecision::Allowed);
    assert_eq!(allowed.remaining_precommit_retries, 1);

    let exhausted = plan_provider_retry(
        policy,
        ProviderRetryStage::BeforeDispatch,
        ProviderRetryCause::RotateCredential,
        ProviderErrorClass::Auth,
        1,
    );
    assert_eq!(
        exhausted.decision,
        ProviderRetryDecision::DeniedBudgetExhausted
    );
    assert_eq!(exhausted.remaining_precommit_retries, 0);

    let committed = plan_provider_retry(
        policy,
        ProviderRetryStage::AfterFirstByte,
        ProviderRetryCause::RotateCredential,
        ProviderErrorClass::Auth,
        0,
    );
    assert_eq!(committed.decision, ProviderRetryDecision::DeniedCommitted);
    assert_eq!(committed.remaining_precommit_retries, 1);
}

#[test]
fn provider_retry_eligibility_is_cause_specific() {
    for (cause, class, expected) in [
        (
            ProviderRetryCause::NextModel,
            ProviderErrorClass::NotFound,
            ProviderRetryDecision::Allowed,
        ),
        (
            ProviderRetryCause::NextModel,
            ProviderErrorClass::Auth,
            ProviderRetryDecision::DeniedNotRetryable,
        ),
        (
            ProviderRetryCause::RotateCredential,
            ProviderErrorClass::Auth,
            ProviderRetryDecision::Allowed,
        ),
        (
            ProviderRetryCause::RotateCredential,
            ProviderErrorClass::NotFound,
            ProviderRetryDecision::DeniedNotRetryable,
        ),
        (
            ProviderRetryCause::NextProvider,
            ProviderErrorClass::Transient,
            ProviderRetryDecision::Allowed,
        ),
        (
            ProviderRetryCause::NextProvider,
            ProviderErrorClass::Auth,
            ProviderRetryDecision::DeniedNotRetryable,
        ),
    ] {
        assert_eq!(
            plan_provider_retry(
                ProviderRetryPolicy::single_retry(),
                ProviderRetryStage::BeforeFirstByte,
                cause,
                class,
                0,
            )
            .decision,
            expected,
            "cause={cause:?} class={class:?}",
        );
    }

    assert_eq!(
        plan_provider_retry(
            ProviderRetryPolicy::single_retry(),
            ProviderRetryStage::AfterFirstByte,
            ProviderRetryCause::NextProvider,
            ProviderErrorClass::Transient,
            0,
        )
        .decision,
        ProviderRetryDecision::DeniedCommitted,
    );
}

#[test]
fn provider_retry_decision_responses_are_stable_and_redacted() {
    assert_eq!(
        plan_provider_retry_decision_response(ProviderRetryDecision::Allowed),
        None
    );

    let committed =
        plan_provider_retry_decision_response(ProviderRetryDecision::DeniedCommitted).unwrap();
    assert_eq!(
        committed.status,
        ProviderRetryDecisionStatus::ServiceUnavailable
    );
    assert_eq!(committed.code, "provider_retry_not_safe");
    assert_eq!(committed.message, "provider retry is not safe");

    let exhausted =
        plan_provider_retry_decision_response(ProviderRetryDecision::DeniedBudgetExhausted)
            .unwrap();
    assert_eq!(
        exhausted.status,
        ProviderRetryDecisionStatus::ServiceUnavailable
    );
    assert_eq!(exhausted.code, "provider_retry_budget_exhausted");
    assert_eq!(exhausted.message, "provider retry budget is exhausted");

    let not_retryable =
        plan_provider_retry_decision_response(ProviderRetryDecision::DeniedNotRetryable).unwrap();
    assert_eq!(not_retryable.code, "provider_retry_not_eligible");
    assert_eq!(
        not_retryable.message,
        "provider response is not eligible for retry"
    );

    let rendered = format!("{committed:?} {exhausted:?} {not_retryable:?}");
    for sensitive in [
        "BeforeFirstByte",
        "AfterFirstByte",
        "AfterCancellation",
        "gpt-5.3-codex",
        "openai-prod",
    ] {
        assert!(
            !rendered.contains(sensitive),
            "provider retry response leaked sensitive token {sensitive}: {rendered}"
        );
    }
}

#[test]
fn provider_circuit_breaker_opens_cools_down_and_half_opens() {
    let policy = ProviderCircuitBreakerPolicy {
        failure_threshold: 2,
        cooldown_ms: 1_000,
    };
    let state = ProviderCircuitBreakerState::default();
    assert_eq!(
        plan_provider_circuit_breaker(policy, state, 10_000),
        ProviderCircuitBreakerDecision::Closed
    );

    let first = plan_provider_circuit_breaker_event(
        policy,
        state,
        ProviderCircuitBreakerEvent::Failure {
            now_unix_ms: 10_000,
        },
    );
    assert_eq!(first.next_state.consecutive_failures, 1);
    assert_eq!(first.next_state.open_until_unix_ms, None);
    assert_eq!(first.decision, ProviderCircuitBreakerDecision::Closed);

    let second = plan_provider_circuit_breaker_event(
        policy,
        first.next_state,
        ProviderCircuitBreakerEvent::Failure {
            now_unix_ms: 10_100,
        },
    );
    assert_eq!(second.next_state.consecutive_failures, 2);
    assert_eq!(second.next_state.open_until_unix_ms, Some(11_100));
    assert_eq!(
        plan_provider_circuit_breaker(policy, second.next_state, 10_500),
        ProviderCircuitBreakerDecision::Open {
            retry_after_ms: 600,
        }
    );
    assert_eq!(
        plan_provider_circuit_breaker(policy, second.next_state, 11_100),
        ProviderCircuitBreakerDecision::HalfOpenProbe
    );
}

#[test]
fn provider_circuit_breaker_success_resets_state_and_threshold_is_minimum_one() {
    let policy = ProviderCircuitBreakerPolicy {
        failure_threshold: 0,
        cooldown_ms: 500,
    };
    let opened = plan_provider_circuit_breaker_event(
        policy,
        ProviderCircuitBreakerState::default(),
        ProviderCircuitBreakerEvent::Failure {
            now_unix_ms: 20_000,
        },
    );
    assert_eq!(opened.next_state.open_until_unix_ms, Some(20_500));

    let reset = plan_provider_circuit_breaker_event(
        policy,
        opened.next_state,
        ProviderCircuitBreakerEvent::Success,
    );
    assert_eq!(reset.next_state, ProviderCircuitBreakerState::default());
    assert_eq!(reset.decision, ProviderCircuitBreakerDecision::Closed);
}

#[test]
fn invocation_debug_redacts_provider_secret_reference() {
    let tenant_id = TenantId::new();
    let invocation = invocation(
        tenant_id,
        principal(Some(tenant_id), CredentialScope::DataPlane),
    );
    let debug = format!("{invocation:?}");

    assert!(!debug.contains("openai-prod"));
    assert!(debug.contains("<redacted>"));
}
