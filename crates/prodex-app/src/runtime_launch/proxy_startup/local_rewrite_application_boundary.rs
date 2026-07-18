use prodex_application::{
    ApplicationAuthenticatedRequestContext, ApplicationAuthorizedRequestContext,
    ApplicationControlPlaneGovernanceScope, ApplicationRequestAuthorizationError,
    ApplicationRequestContext, ApplicationRequestContextError, ApplicationRequestDeadline,
    plan_application_control_plane_authorization, plan_application_data_plane_authorization,
    plan_application_request_authentication_from_evidence, plan_application_request_context,
};
use prodex_authn::{
    VerifiedCredentialAuthenticationError, VerifiedCredentialEvidence,
    VerifiedOidcCredentialEvidence, VerifiedOidcRoleEvidence,
};
use prodex_control_plane::{
    ControlPlaneActionPlan, ControlPlaneActionRequest, ControlPlaneResourceRef,
};
use prodex_domain::{
    CredentialScope, ExplicitRoleMapper, Principal, PrincipalId, PrincipalKind, RequestId, Role,
    TenantId,
};
use prodex_gateway_http::{CanonicalRequestTarget, GatewayHttpHeader, GatewayHttpRequestMeta};
use sha2::{Digest, Sha256};
use std::time::{SystemTime, UNIX_EPOCH};

use super::local_rewrite_gateway_admin_auth::{
    RuntimeGatewayAdminAuth, RuntimeGatewayAdminAuthentication,
    RuntimeGatewayAdminCredentialEvidence, RuntimeGatewayOidcAdminCredentialEvidence,
};
use super::local_rewrite_gateway_config::RuntimeGatewayAdminRole;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct RuntimeGatewayVerifiedCredential {
    pub(super) evidence: Option<VerifiedCredentialEvidence>,
    pub(super) anonymous_allowed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum RuntimeGatewayApplicationBoundaryError {
    Route,
    Authentication(VerifiedCredentialAuthenticationError),
    Authorization(ApplicationRequestAuthorizationError),
}

pub(super) struct RuntimeGatewayAdminPreauthorization<'a> {
    pub(super) auth: RuntimeGatewayAdminAuth,
    pub(super) application: Option<ApplicationAuthorizedRequestContext<'a>>,
}

impl RuntimeGatewayAdminPreauthorization<'_> {
    pub(super) fn control_plane_action(&self) -> Option<&ControlPlaneActionPlan> {
        self.application.as_ref()?.control_plane_action()
    }
}

pub(super) fn runtime_gateway_admin_governance_scope(
    admin_auth: &RuntimeGatewayAdminAuth,
) -> ApplicationControlPlaneGovernanceScope {
    ApplicationControlPlaneGovernanceScope::new(
        admin_auth.tenant_id.clone(),
        admin_auth.team_id.clone(),
        admin_auth.project_id.clone(),
        admin_auth.user_id.clone(),
        admin_auth.budget_id.clone(),
        admin_auth.allowed_key_prefixes.clone(),
    )
}

pub(super) fn runtime_gateway_application_request_context<'a>(
    target: &'a CanonicalRequestTarget,
    request_id: RequestId,
    deadline: ApplicationRequestDeadline,
    headers: &[(String, String)],
) -> Result<ApplicationRequestContext<'a>, ApplicationRequestContextError> {
    let headers = headers
        .iter()
        .map(|(name, value)| GatewayHttpHeader::new(name, value))
        .collect::<Vec<_>>();
    plan_application_request_context(target, request_id, deadline, &headers)
}

pub(super) fn runtime_gateway_application_authentication<'a>(
    request: &ApplicationRequestContext<'a>,
    credential: RuntimeGatewayVerifiedCredential,
) -> Result<ApplicationAuthenticatedRequestContext<'a>, VerifiedCredentialAuthenticationError> {
    plan_application_request_authentication_from_evidence(
        request.clone(),
        credential.evidence,
        credential.anonymous_allowed,
    )
}

pub(super) fn runtime_gateway_application_data_plane_authorization<'a>(
    request: &ApplicationRequestContext<'a>,
    credential: RuntimeGatewayVerifiedCredential,
) -> Result<ApplicationAuthorizedRequestContext<'a>, RuntimeGatewayApplicationBoundaryError> {
    let authenticated = runtime_gateway_application_authentication(request, credential)
        .map_err(RuntimeGatewayApplicationBoundaryError::Authentication)?;
    plan_application_data_plane_authorization(authenticated)
        .map_err(RuntimeGatewayApplicationBoundaryError::Authorization)
}

pub(super) fn runtime_gateway_application_control_plane_authorization<'a>(
    request: &ApplicationRequestContext<'a>,
    http: &GatewayHttpRequestMeta,
    authentication: &RuntimeGatewayAdminAuthentication,
) -> Result<ApplicationAuthorizedRequestContext<'a>, RuntimeGatewayApplicationBoundaryError> {
    let admin_auth = &authentication.auth;
    let action = runtime_gateway_admin_control_plane_action(http, admin_auth)
        .ok_or(RuntimeGatewayApplicationBoundaryError::Route)?;
    let authenticated = runtime_gateway_application_authentication(
        request,
        runtime_gateway_control_plane_credential(authentication),
    )
    .map_err(RuntimeGatewayApplicationBoundaryError::Authentication)?;
    plan_application_control_plane_authorization(authenticated, action)
        .map_err(RuntimeGatewayApplicationBoundaryError::Authorization)
}

pub(super) fn runtime_gateway_admin_preauthorization<'a>(
    request: &ApplicationRequestContext<'a>,
    http: &GatewayHttpRequestMeta,
    authentication: &RuntimeGatewayAdminAuthentication,
) -> Result<Option<ApplicationAuthorizedRequestContext<'a>>, RuntimeGatewayApplicationBoundaryError>
{
    let application = match runtime_gateway_application_control_plane_authorization(
        request,
        http,
        authentication,
    ) {
        Ok(application) => Some(application),
        Err(RuntimeGatewayApplicationBoundaryError::Route) => None,
        Err(error) => return Err(error),
    };
    Ok(application)
}

pub(super) fn runtime_gateway_data_plane_credential(
    virtual_key: Option<&runtime_proxy_crate::RuntimeGatewayVirtualKey>,
    legacy_data_plane_authorized: bool,
    authentication_configured: bool,
) -> RuntimeGatewayVerifiedCredential {
    let principal = virtual_key
        .map(runtime_gateway_virtual_key_principal)
        .or_else(|| legacy_data_plane_authorized.then(runtime_gateway_bearer_principal));
    RuntimeGatewayVerifiedCredential {
        evidence: principal.map(VerifiedCredentialEvidence::Principal),
        anonymous_allowed: virtual_key.is_none()
            && !legacy_data_plane_authorized
            && !authentication_configured,
    }
}

pub(super) fn runtime_gateway_control_plane_credential(
    authentication: &RuntimeGatewayAdminAuthentication,
) -> RuntimeGatewayVerifiedCredential {
    RuntimeGatewayVerifiedCredential {
        evidence: Some(runtime_gateway_admin_credential_evidence(authentication)),
        anonymous_allowed: false,
    }
}

fn runtime_gateway_admin_credential_evidence(
    authentication: &RuntimeGatewayAdminAuthentication,
) -> VerifiedCredentialEvidence {
    let principal = runtime_gateway_admin_principal(&authentication.auth);
    match &authentication.evidence {
        RuntimeGatewayAdminCredentialEvidence::Principal => {
            VerifiedCredentialEvidence::Principal(principal)
        }
        RuntimeGatewayAdminCredentialEvidence::Oidc(evidence) => {
            let RuntimeGatewayOidcAdminCredentialEvidence {
                token,
                subject_name,
                claimed_tenant_id,
                role_claim,
            } = evidence.as_ref();
            let tenant_id = claimed_tenant_id
                .as_deref()
                .map(runtime_gateway_control_plane_tenant_id_from_text);
            let resolved_tenant_id =
                runtime_gateway_admin_control_plane_tenant_id(&authentication.auth);
            let resolved_tenant_text = resolved_tenant_id.to_string();
            let oidc_name = format!("oidc:{subject_name}");
            let principal_id = PrincipalId::from_uuid(runtime_gateway_stable_id(
                "prodex:gateway-admin-control-plane-principal:v1",
                &[resolved_tenant_text.as_bytes(), oidc_name.as_bytes()],
            ));
            VerifiedCredentialEvidence::Oidc(Box::new(VerifiedOidcCredentialEvidence {
                policy: token.policy(),
                jwks_snapshot: token.jwks_snapshot(),
                claims: token.canonical_claims(principal_id, tenant_id, role_claim.clone()),
                role_evidence: runtime_gateway_oidc_role_evidence(
                    role_claim.as_deref(),
                    authentication.auth.role,
                ),
                resolved_principal: principal,
                now_unix_ms: token.now_unix_ms(),
            }))
        }
    }
}

fn runtime_gateway_oidc_role_evidence(
    role_claim: Option<&str>,
    resolved_role: RuntimeGatewayAdminRole,
) -> VerifiedOidcRoleEvidence {
    if role_claim.is_none() {
        return VerifiedOidcRoleEvidence::TrustedMissingClaimFallback(
            runtime_gateway_admin_domain_role(resolved_role),
        );
    }
    VerifiedOidcRoleEvidence::Claim(ExplicitRoleMapper::new([
        ("admin", Role::Admin),
        ("write", Role::Admin),
        ("writer", Role::Admin),
        ("viewer", Role::Viewer),
        ("read", Role::Viewer),
        ("readonly", Role::Viewer),
        ("read-only", Role::Viewer),
    ]))
}

pub(super) fn runtime_gateway_admin_control_plane_action(
    http: &GatewayHttpRequestMeta,
    admin_auth: &RuntimeGatewayAdminAuth,
) -> Option<ControlPlaneActionRequest> {
    let route = prodex_application::plan_application_control_plane_http_route(http).ok()?;
    runtime_gateway_admin_control_plane_action_for_operation(http, admin_auth, route.operation)
}

pub(super) fn runtime_gateway_admin_control_plane_action_for_operation(
    http: &GatewayHttpRequestMeta,
    admin_auth: &RuntimeGatewayAdminAuth,
    operation: prodex_control_plane::ControlPlaneOperation,
) -> Option<ControlPlaneActionRequest> {
    let route = prodex_application::plan_application_control_plane_http_route(http).ok()?;
    if route.operation != operation
        && !matches!(
            (route.operation, operation),
            (
                prodex_control_plane::ControlPlaneOperation::VirtualKeyUpdate,
                prodex_control_plane::ControlPlaneOperation::VirtualKeyRotateSecret,
            )
        )
    {
        return None;
    }
    let principal = runtime_gateway_admin_principal(admin_auth);
    let tenant_id = principal.tenant_id?;
    Some(ControlPlaneActionRequest {
        principal,
        operation,
        resource: ControlPlaneResourceRef::new(
            tenant_id,
            operation.requirement().resource,
            runtime_gateway_admin_resource_id(http, operation),
        ),
        occurred_at_unix_ms: runtime_gateway_now_unix_ms(),
    })
}

fn runtime_gateway_admin_resource_id(
    http: &GatewayHttpRequestMeta,
    operation: prodex_control_plane::ControlPlaneOperation,
) -> Option<String> {
    use prodex_control_plane::ControlPlaneOperation;

    let marker = match operation {
        ControlPlaneOperation::VirtualKeyUpdate
        | ControlPlaneOperation::VirtualKeyDelete
        | ControlPlaneOperation::VirtualKeyRotateSecret => "/keys/",
        ControlPlaneOperation::ScimUserUpdate | ControlPlaneOperation::ScimUserDelete => {
            "/scim/v2/Users/"
        }
        _ => return None,
    };
    let resource_id = http
        .path
        .split_once(marker)
        .map(|(_, resource_id)| resource_id.trim())?;
    let resource_id = if operation == ControlPlaneOperation::VirtualKeyRotateSecret {
        resource_id
            .strip_suffix("/secret")
            .or_else(|| resource_id.strip_suffix("/secrets"))
            .unwrap_or(resource_id)
    } else {
        resource_id
    };
    Some(resource_id)
        .filter(|resource_id| !resource_id.is_empty() && !resource_id.contains('/'))
        .map(str::to_string)
}

pub(super) fn runtime_gateway_now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

pub(super) fn runtime_gateway_admin_control_plane_tenant_id(
    admin_auth: &RuntimeGatewayAdminAuth,
) -> TenantId {
    match admin_auth.tenant_id.as_deref() {
        Some(value) => runtime_gateway_control_plane_tenant_id_from_text(value),
        None => TenantId::from_uuid(runtime_gateway_stable_id(
            "prodex:gateway-admin-control-plane-tenant:v1",
            &[admin_auth.name.as_bytes()],
        )),
    }
}

fn runtime_gateway_control_plane_tenant_id_from_text(value: &str) -> TenantId {
    value.parse::<TenantId>().unwrap_or_else(|_| {
        TenantId::from_uuid(runtime_gateway_stable_id(
            "prodex:gateway-admin-control-plane-tenant-text:v1",
            &[value.as_bytes()],
        ))
    })
}

pub(super) fn runtime_gateway_admin_principal(admin_auth: &RuntimeGatewayAdminAuth) -> Principal {
    let tenant_id = runtime_gateway_admin_control_plane_tenant_id(admin_auth);
    let tenant_text = tenant_id.to_string();
    Principal::new(
        PrincipalId::from_uuid(runtime_gateway_stable_id(
            "prodex:gateway-admin-control-plane-principal:v1",
            &[tenant_text.as_bytes(), admin_auth.name.as_bytes()],
        )),
        Some(tenant_id),
        PrincipalKind::User,
        runtime_gateway_admin_domain_role(admin_auth.role),
        CredentialScope::ControlPlane,
    )
}

fn runtime_gateway_admin_domain_role(role: RuntimeGatewayAdminRole) -> Role {
    match role {
        RuntimeGatewayAdminRole::Admin => Role::Admin,
        RuntimeGatewayAdminRole::Viewer => Role::Viewer,
    }
}

fn runtime_gateway_virtual_key_principal(
    key: &runtime_proxy_crate::RuntimeGatewayVirtualKey,
) -> Principal {
    let tenant_id = key
        .tenant_id
        .as_deref()
        .and_then(|value| value.parse::<TenantId>().ok())
        .unwrap_or_else(|| {
            TenantId::from_uuid(runtime_gateway_stable_id(
                "prodex:gateway-virtual-key-tenant:v1",
                &[key.name.as_bytes()],
            ))
        });
    let tenant_text = tenant_id.to_string();
    Principal::new(
        PrincipalId::from_uuid(runtime_gateway_stable_id(
            "prodex:gateway-virtual-key-principal:v1",
            &[tenant_text.as_bytes(), key.name.as_bytes()],
        )),
        Some(tenant_id),
        PrincipalKind::VirtualKey,
        Role::Operator,
        CredentialScope::DataPlane,
    )
}

fn runtime_gateway_bearer_principal() -> Principal {
    let tenant_id = TenantId::from_uuid(runtime_gateway_stable_id(
        "prodex:gateway-bearer-tenant:v1",
        &[],
    ));
    Principal::new(
        PrincipalId::from_uuid(runtime_gateway_stable_id(
            "prodex:gateway-bearer-principal:v1",
            &[],
        )),
        Some(tenant_id),
        PrincipalKind::ServiceAccount,
        Role::Operator,
        CredentialScope::DataPlane,
    )
}

pub(super) fn runtime_gateway_stable_id(namespace: &str, parts: &[&[u8]]) -> uuid::Uuid {
    let mut hasher = Sha256::new();
    hasher.update(namespace.as_bytes());
    for part in parts {
        hasher.update([0]);
        hasher.update(part);
    }
    let digest = hasher.finalize();
    let mut bytes = [0_u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x80;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    uuid::Uuid::from_bytes(bytes)
}

#[cfg(test)]
mod tests {
    use super::super::local_rewrite_gateway_admin_auth::runtime_gateway_test_verified_oidc_token;
    use super::*;
    use prodex_gateway_http::GatewayHttpMethod;
    use std::time::{Duration, Instant};

    fn application_request_context<'a>(
        target: &'a CanonicalRequestTarget,
    ) -> ApplicationRequestContext<'a> {
        runtime_gateway_application_request_context(
            target,
            RequestId::new(),
            ApplicationRequestDeadline::at(Instant::now() + Duration::from_secs(30)),
            &[],
        )
        .unwrap()
    }

    fn virtual_key() -> runtime_proxy_crate::RuntimeGatewayVirtualKey {
        runtime_proxy_crate::RuntimeGatewayVirtualKey {
            name: "boundary-key".to_string(),
            tenant_id: Some("00000000-0000-7000-8000-000000000010".to_string()),
            team_id: None,
            project_id: None,
            user_id: None,
            budget_id: None,
            token_hash: runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(
                "boundary-secret",
            ),
            allowed_models: Vec::new(),
            budget_microusd: None,
            request_budget: None,
            rpm_limit: None,
            tpm_limit: None,
        }
    }

    fn admin(role: RuntimeGatewayAdminRole) -> RuntimeGatewayAdminAuth {
        RuntimeGatewayAdminAuth {
            name: "boundary-admin".to_string(),
            role,
            tenant_id: Some("00000000-0000-7000-8000-000000000011".to_string()),
            team_id: None,
            project_id: None,
            user_id: None,
            budget_id: None,
            allowed_key_prefixes: Vec::new(),
        }
    }

    fn admin_authentication(role: RuntimeGatewayAdminRole) -> RuntimeGatewayAdminAuthentication {
        RuntimeGatewayAdminAuthentication {
            auth: admin(role),
            evidence: RuntimeGatewayAdminCredentialEvidence::Principal,
        }
    }

    #[test]
    fn admin_governance_scope_preserves_exact_credential_limits() {
        let mut auth = admin(RuntimeGatewayAdminRole::Admin);
        auth.team_id = Some("team-a".to_string());
        auth.allowed_key_prefixes = vec!["team-a-".to_string()];

        let scope = runtime_gateway_admin_governance_scope(&auth);
        assert!(scope.matches(auth.tenant_id.as_deref(), Some("team-a"), None, None, None,));
        assert!(scope.matches_resource_name("team-a-key"));
        assert!(!scope.matches_resource_name("team-b-key"));
    }

    fn oidc_admin_authentication(
        resolved_tenant: &str,
        claimed_tenant: &str,
    ) -> RuntimeGatewayAdminAuthentication {
        let mut auth = admin(RuntimeGatewayAdminRole::Admin);
        auth.name = "oidc:boundary-admin".to_string();
        auth.tenant_id = Some(resolved_tenant.to_string());
        RuntimeGatewayAdminAuthentication {
            auth,
            evidence: RuntimeGatewayAdminCredentialEvidence::Oidc(Box::new(
                RuntimeGatewayOidcAdminCredentialEvidence {
                    token: runtime_gateway_test_verified_oidc_token(),
                    subject_name: "boundary-admin".to_string(),
                    claimed_tenant_id: Some(claimed_tenant.to_string()),
                    role_claim: Some("admin".to_string()),
                },
            )),
        }
    }

    #[test]
    fn application_gate_matches_the_legacy_data_plane_decision_matrix() {
        let target = CanonicalRequestTarget::parse("/v1/responses").unwrap();
        let request = application_request_context(&target);

        for virtual_keys_empty in [false, true] {
            for legacy_data_plane_authorized in [false, true] {
                for authentication_configured in [false, true] {
                    let legacy_allows = !virtual_keys_empty
                        || legacy_data_plane_authorized
                        || !authentication_configured;
                    let virtual_key = (!virtual_keys_empty).then(virtual_key);
                    let credential = runtime_gateway_data_plane_credential(
                        virtual_key.as_ref(),
                        legacy_data_plane_authorized,
                        authentication_configured,
                    );
                    assert_eq!(
                        runtime_gateway_application_data_plane_authorization(&request, credential)
                            .is_ok(),
                        legacy_allows,
                        "virtual_keys_empty={virtual_keys_empty} legacy_authorized={legacy_data_plane_authorized} auth_configured={authentication_configured}",
                    );
                }
            }
        }
    }

    #[test]
    fn application_gate_binds_typed_tenant_and_control_plane_role() {
        let data_target = CanonicalRequestTarget::parse("/v1/responses").unwrap();
        let data = application_request_context(&data_target);
        let key = virtual_key();
        let authorized = runtime_gateway_application_data_plane_authorization(
            &data,
            runtime_gateway_data_plane_credential(Some(&key), false, true),
        )
        .unwrap();
        assert_eq!(
            authorized.tenant_context().unwrap().tenant_id,
            "00000000-0000-7000-8000-000000000010"
                .parse::<TenantId>()
                .unwrap(),
        );
        assert!(!format!("{authorized:?}").contains("boundary-secret"));

        let control_target = CanonicalRequestTarget::parse("/admin/keys").unwrap();
        let control = application_request_context(&control_target);
        let read = GatewayHttpRequestMeta {
            method: GatewayHttpMethod::Get,
            path: "/admin/keys".to_string(),
            body_len: 0,
            headers: Vec::new(),
        };
        let write = GatewayHttpRequestMeta {
            method: GatewayHttpMethod::Post,
            ..read.clone()
        };
        assert!(
            runtime_gateway_application_control_plane_authorization(
                &control,
                &read,
                &admin_authentication(RuntimeGatewayAdminRole::Viewer),
            )
            .is_ok(),
            "legacy viewer reads must remain authorized",
        );
        assert!(
            runtime_gateway_application_control_plane_authorization(
                &control,
                &write,
                &admin_authentication(RuntimeGatewayAdminRole::Viewer),
            )
            .is_err(),
            "legacy viewer mutations must remain denied",
        );
        assert!(
            runtime_gateway_application_control_plane_authorization(
                &control,
                &write,
                &admin_authentication(RuntimeGatewayAdminRole::Admin),
            )
            .is_ok(),
            "legacy admin mutations must remain authorized",
        );
    }

    #[test]
    fn non_uuid_oidc_tenant_claim_must_match_the_resolved_tenant_text() {
        let target = CanonicalRequestTarget::parse("/admin/keys").unwrap();
        let request = application_request_context(&target);
        let matched = oidc_admin_authentication("tenant-a", "tenant-a");
        let authenticated = runtime_gateway_application_authentication(
            &request,
            runtime_gateway_control_plane_credential(&matched),
        )
        .unwrap();
        assert_eq!(
            authenticated.principal().unwrap().tenant_id,
            Some(runtime_gateway_control_plane_tenant_id_from_text(
                "tenant-a"
            )),
        );

        let mismatched = oidc_admin_authentication("tenant-b", "tenant-a");
        assert_eq!(
            runtime_gateway_application_authentication(
                &request,
                runtime_gateway_control_plane_credential(&mismatched),
            ),
            Err(VerifiedCredentialAuthenticationError::OidcPrincipalMismatch),
        );
        assert_ne!(
            runtime_gateway_control_plane_tenant_id_from_text("tenant-a"),
            runtime_gateway_control_plane_tenant_id_from_text("tenant-b"),
        );
    }

    #[test]
    fn oidc_role_claim_must_match_resolved_admin_and_unknown_is_rejected() {
        let target = CanonicalRequestTarget::parse("/admin/keys").unwrap();
        let request = application_request_context(&target);
        for (role_claim, expected) in [
            (
                "viewer",
                VerifiedCredentialAuthenticationError::OidcPrincipalMismatch,
            ),
            (
                "owner",
                VerifiedCredentialAuthenticationError::Oidc(
                    prodex_authn::AuthenticationError::Role(prodex_domain::RoleClaimError::Unknown),
                ),
            ),
        ] {
            let mut authentication = oidc_admin_authentication("tenant-a", "tenant-a");
            let RuntimeGatewayAdminCredentialEvidence::Oidc(evidence) =
                &mut authentication.evidence
            else {
                unreachable!();
            };
            evidence.role_claim = Some(role_claim.to_string());
            assert_eq!(
                runtime_gateway_application_authentication(
                    &request,
                    runtime_gateway_control_plane_credential(&authentication),
                ),
                Err(expected),
            );
        }
    }

    #[test]
    fn application_adapter_preserves_request_identity_deadline_trace_and_exact_target() {
        let target = CanonicalRequestTarget::parse("/v1/responses?stream=true").unwrap();
        let request_id = "00000000-0000-7000-8000-000000000020"
            .parse::<RequestId>()
            .unwrap();
        let deadline = ApplicationRequestDeadline::at(Instant::now() + Duration::from_secs(30));
        let mut headers = vec![
            (
                "traceparent".to_string(),
                "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01".to_string(),
            ),
            (
                "authorization".to_string(),
                "Bearer private-boundary-secret".to_string(),
            ),
        ];
        let request =
            runtime_gateway_application_request_context(&target, request_id, deadline, &headers)
                .unwrap();

        headers.clear();
        assert!(std::ptr::eq(request.target(), &target));
        assert_eq!(request.request_id(), request_id);
        assert_eq!(request.deadline(), deadline);
        assert_eq!(
            request.trace_context().unwrap().traceparent(),
            "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"
        );
        assert!(request.metadata().credential_present());
        assert!(!format!("{request:?}").contains("private-boundary-secret"));

        let key = virtual_key();
        let authorized = runtime_gateway_application_data_plane_authorization(
            &request,
            runtime_gateway_data_plane_credential(Some(&key), false, true),
        )
        .unwrap();
        assert_eq!(authorized.request().request_id(), request_id);
        assert_eq!(authorized.request().deadline(), deadline);
        assert_eq!(authorized.correlation_context().request_id, request_id);
        assert_eq!(
            authorized.correlation_context().tenant_id,
            authorized.tenant_context().map(|tenant| tenant.tenant_id)
        );
        assert!(std::ptr::eq(authorized.request().target(), &target));
    }
}
