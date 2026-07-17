use super::{
    runtime_gateway_application_provider_retry_precommit, runtime_gateway_compatibility_http_route,
    runtime_gateway_compatibility_provider_invocation, runtime_gateway_provider_credential_ref,
};
use crate::RuntimeProxyRequest;
use prodex_domain::SecretRef;
use prodex_gateway_http::GatewayHttpRouteKind;
use prodex_provider_core::{ProviderEndpoint, ProviderErrorClass, ProviderId};
use prodex_provider_spi::{ProviderRetryCause, ProviderStreamMode};

#[test]
fn configured_provider_reference_reaches_application_invocation() {
    let configured = SecretRef::new("external", "provider-key", Some("v2"));

    let selected = runtime_gateway_provider_credential_ref(Some(&configured), ProviderId::OpenAi);

    assert_eq!(selected, configured);
    assert_ne!(selected.provider(), "runtime-provider");
}

fn captured(stream: bool) -> RuntimeProxyRequest {
    RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/responses".to_string(),
        headers: Vec::new(),
        body: serde_json::to_vec(&serde_json::json!({ "stream": stream })).unwrap(),
    }
}

#[test]
fn anonymous_compatibility_owns_typed_provider_dispatch() {
    let invocation = runtime_gateway_compatibility_provider_invocation(
        ProviderId::Gemini,
        GatewayHttpRouteKind::DataPlaneResponses,
        &captured(true),
    )
    .unwrap();
    assert_eq!(invocation.provider, ProviderId::Gemini);
    assert_eq!(invocation.endpoint, ProviderEndpoint::Responses);
    assert_eq!(invocation.stream_mode, ProviderStreamMode::Streaming);

    assert!(
        runtime_gateway_compatibility_provider_invocation(
            ProviderId::Gemini,
            GatewayHttpRouteKind::Unknown,
            &captured(false),
        )
        .is_err()
    );
}

#[test]
fn anonymous_compatibility_rejects_invalid_http_method() {
    let mut request = captured(false);
    request.method = "GET".to_string();

    assert!(
        runtime_gateway_compatibility_http_route(
            prodex_gateway_http::GatewayHttpPolicy::production_default(),
            GatewayHttpRouteKind::DataPlaneResponses,
            &request,
        )
        .is_err()
    );
}

#[test]
fn only_personal_mode_allows_anonymous_provider_dispatch() {
    for (mode, requires_identity) in [
        (prodex_config::GovernanceMode::Personal, false),
        (prodex_config::GovernanceMode::EnterpriseObserve, true),
        (prodex_config::GovernanceMode::EnterpriseEnforce, true),
        (prodex_config::GovernanceMode::BankEnforce, true),
    ] {
        assert_eq!(!mode.allows_anonymous_compatibility(), requires_identity);
    }
}

#[test]
fn application_retry_uses_nonzero_bounded_candidate_counts() {
    for (attempt_index, candidate_count, expected) in [
        (0, 0, false),
        (0, 1, false),
        (0, 2, true),
        (1, 2, false),
        (2, 2, false),
        (254, 257, true),
        (255, 257, false),
    ] {
        assert_eq!(
            runtime_gateway_application_provider_retry_precommit(
                ProviderRetryCause::NextModel,
                ProviderErrorClass::Transient,
                attempt_index,
                candidate_count,
            ),
            expected,
            "attempt_index={attempt_index} candidate_count={candidate_count}",
        );
    }
    assert!(!runtime_gateway_application_provider_retry_precommit(
        ProviderRetryCause::RotateCredential,
        ProviderErrorClass::NotFound,
        0,
        2,
    ));
}
