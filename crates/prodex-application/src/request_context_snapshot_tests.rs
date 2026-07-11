use super::*;
use prodex_authn::VerifiedCredentialEvidence;
use prodex_domain::{
    CredentialScope, Principal, PrincipalId, PrincipalKind, RequestId, Role, TenantId,
};
use prodex_gateway_http::{CanonicalRequestTarget, GatewayHttpHeader, GatewayHttpPlanError};
use std::fmt;
use std::time::{Duration, Instant};

const REQUEST_ID: &str = "00000000-0000-7000-8000-000000000004";
const TRACEPARENT: &str = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01";

fn id<T: std::str::FromStr>(value: &str) -> T
where
    T::Err: fmt::Debug,
{
    value.parse().unwrap()
}

fn principal() -> Principal {
    Principal::new(
        id::<PrincipalId>("00000000-0000-7000-8000-000000000001"),
        Some(id::<TenantId>("00000000-0000-7000-8000-000000000002")),
        PrincipalKind::ServiceAccount,
        Role::Operator,
        CredentialScope::DataPlane,
    )
}

fn context_with_headers<'a>(
    target: &'a CanonicalRequestTarget,
    headers: &[GatewayHttpHeader],
) -> Result<ApplicationRequestContext<'a>, ApplicationRequestContextError> {
    plan_application_request_context(
        target,
        id::<RequestId>(REQUEST_ID),
        ApplicationRequestDeadline::at(Instant::now() + Duration::from_secs(30)),
        headers,
    )
}

#[test]
fn request_context_is_an_immutable_redacted_snapshot() {
    let target = CanonicalRequestTarget::parse("/v1/responses?private=secret-value").unwrap();
    let request_id = id::<RequestId>(REQUEST_ID);
    let deadline_instant = Instant::now() + Duration::from_secs(30);
    let deadline = ApplicationRequestDeadline::at(deadline_instant);
    let mut headers = vec![
        GatewayHttpHeader::new("traceparent", TRACEPARENT),
        GatewayHttpHeader::new("authorization", "Bearer request-secret"),
        GatewayHttpHeader::new("session_id", "private-session"),
        GatewayHttpHeader::new("x-codex-turn-metadata", "private-metadata"),
        GatewayHttpHeader::new("user-agent", "private-agent"),
        GatewayHttpHeader::new("x-private", "private-value"),
    ];

    let context =
        plan_application_request_context(&target, request_id, deadline, &headers).unwrap();
    assert!(std::ptr::eq(context.target(), &target));
    assert_eq!(context.request_id(), request_id);
    assert_eq!(context.deadline().instant(), deadline_instant);
    assert!(
        !context
            .deadline()
            .is_expired_at(deadline_instant - Duration::from_millis(1))
    );
    assert_eq!(context.trace_context().unwrap().traceparent(), TRACEPARENT);
    assert_eq!(context.correlation_context().request_id, request_id);
    assert_eq!(
        context
            .correlation_context()
            .trace_id
            .as_ref()
            .unwrap()
            .as_str(),
        "4bf92f3577b34da6a3ce929d0e0e4736"
    );

    headers[0].value = "mutated-after-construction".to_string();
    headers.clear();
    assert_eq!(context.trace_context().unwrap().traceparent(), TRACEPARENT);

    let debug = format!("{context:?}");
    assert_eq!(
        debug,
        "ApplicationRequestContext { target: \"<redacted>\", request_id: \"<redacted>\", deadline: \"<redacted>\", route: DataPlaneResponses, plane: DataPlane, required_credential_scope: Some(DataPlane), trace_context: Some(\"<redacted>\"), correlation: \"<redacted>\", metadata: ApplicationRequestMetadata { observed_header_count: 6, headers_truncated: false, trace_context_present: true, credential_present: true, affinity_present: true, codex_metadata_present: true, user_agent_present: true } }"
    );
    for secret in [
        REQUEST_ID,
        TRACEPARENT,
        "/v1/responses",
        "secret-value",
        "request-secret",
        "private-session",
        "private-metadata",
        "private-agent",
        "private-value",
    ] {
        assert!(!debug.contains(secret));
    }
}

#[test]
fn request_context_rejects_invalid_and_duplicate_trace_headers_without_leaking_them() {
    let target = CanonicalRequestTarget::parse("/v1/responses").unwrap();
    let invalid_value = "private-invalid-traceparent";
    let invalid = context_with_headers(
        &target,
        &[GatewayHttpHeader::new("traceparent", invalid_value)],
    )
    .unwrap_err();
    assert!(matches!(
        &invalid,
        ApplicationRequestContextError::Trace(GatewayHttpPlanError::InvalidTraceContext(_))
    ));
    assert!(!format!("{invalid:?}").contains(invalid_value));

    let duplicate = context_with_headers(
        &target,
        &[
            GatewayHttpHeader::new("traceparent", TRACEPARENT),
            GatewayHttpHeader::new("TraceParent", TRACEPARENT),
        ],
    )
    .unwrap_err();
    assert_eq!(
        duplicate,
        ApplicationRequestContextError::Trace(GatewayHttpPlanError::DuplicateTraceContext),
    );
    assert!(!format!("{duplicate:?}").contains(TRACEPARENT));
}

#[test]
fn request_metadata_is_constant_size_and_presence_only() {
    let target = CanonicalRequestTarget::parse("/v1/responses").unwrap();
    let mut headers = vec![
        GatewayHttpHeader::new("traceparent", TRACEPARENT),
        GatewayHttpHeader::new("authorization", "Bearer private-token"),
        GatewayHttpHeader::new("x-codex-turn-state", "private-state"),
        GatewayHttpHeader::new("x-openai-subagent", "private-subagent"),
        GatewayHttpHeader::new("user-agent", "private-agent"),
    ];
    headers.extend((headers.len()..70).map(|index| {
        GatewayHttpHeader::new(format!("x-padding-{index}"), format!("private-{index}"))
    }));

    let metadata = context_with_headers(&target, &headers).unwrap().metadata();
    assert_eq!(
        metadata.observed_header_count(),
        APPLICATION_REQUEST_METADATA_HEADER_LIMIT
    );
    assert!(metadata.headers_truncated());
    assert!(metadata.trace_context_present());
    assert!(metadata.credential_present());
    assert!(metadata.affinity_present());
    assert!(metadata.codex_metadata_present());
    assert!(metadata.user_agent_present());
    let debug = format!("{metadata:?}");
    for secret in ["private-token", "private-state", "private-subagent"] {
        assert!(!debug.contains(secret));
    }
}

#[test]
fn request_identity_deadline_trace_and_tenant_propagate_through_authorization() {
    let target = CanonicalRequestTarget::parse("/v1/responses").unwrap();
    let request = context_with_headers(
        &target,
        &[GatewayHttpHeader::new("traceparent", TRACEPARENT)],
    )
    .unwrap();
    let request_id = request.request_id();
    let deadline = request.deadline();
    let operator = principal();
    let tenant_id = operator.tenant_id.unwrap();
    let authenticated = plan_application_request_authentication_from_evidence(
        request.clone(),
        Some(VerifiedCredentialEvidence::Principal(operator)),
        false,
    )
    .unwrap();
    assert_eq!(authenticated.request().request_id(), request_id);
    assert_eq!(authenticated.request().deadline(), deadline);
    assert!(std::ptr::eq(authenticated.request().target(), &target));

    let authorized = plan_application_data_plane_authorization(authenticated).unwrap();
    assert_eq!(authorized.request().request_id(), request_id);
    assert_eq!(authorized.request().deadline(), deadline);
    assert_eq!(authorized.correlation_context().request_id, request_id);
    assert_eq!(authorized.correlation_context().tenant_id, Some(tenant_id));
    assert_eq!(
        authorized
            .correlation_context()
            .trace_id
            .as_ref()
            .unwrap()
            .as_str(),
        "4bf92f3577b34da6a3ce929d0e0e4736"
    );
    assert!(std::ptr::eq(authorized.request().target(), &target));
}
