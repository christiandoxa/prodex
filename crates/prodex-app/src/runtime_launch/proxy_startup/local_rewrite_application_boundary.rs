use prodex_application::{
    ApplicationAuthenticatedRequestContext, ApplicationAuthorizedRequestContext,
    ApplicationRequestAuthorizationError, ApplicationRequestContext,
    ApplicationRequestContextError,
    plan_application_control_plane_authorization_from_compatibility,
    plan_application_data_plane_authorization_from_compatibility,
    plan_application_request_authentication_from_compatibility, plan_application_request_context,
};
use prodex_authn::CompatibilityAuthenticationError;
use prodex_control_plane::{ControlPlaneActionRequest, ControlPlaneResourceRef};
use prodex_domain::{CredentialScope, Principal, PrincipalId, PrincipalKind, Role, TenantId};
use prodex_gateway_http::{CanonicalRequestTarget, GatewayHttpRequestMeta};
use sha2::{Digest, Sha256};

use super::local_rewrite_gateway_admin_auth::RuntimeGatewayAdminAuth;
use super::local_rewrite_gateway_config::RuntimeGatewayAdminRole;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct RuntimeGatewayCompatibilityCredential {
    pub(super) principal: Option<Principal>,
    pub(super) anonymous_allowed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum RuntimeGatewayApplicationBoundaryError {
    Route,
    Authentication(CompatibilityAuthenticationError),
    Authorization(ApplicationRequestAuthorizationError),
}

pub(super) struct RuntimeGatewayAdminPreauthorization<'a> {
    pub(super) auth: RuntimeGatewayAdminAuth,
    pub(super) application: Option<ApplicationAuthorizedRequestContext<'a>>,
}

pub(super) fn runtime_gateway_application_request_context(
    target: &CanonicalRequestTarget,
) -> Result<ApplicationRequestContext<'_>, ApplicationRequestContextError> {
    plan_application_request_context(target)
}

pub(super) fn runtime_gateway_application_authentication(
    request: ApplicationRequestContext<'_>,
    credential: RuntimeGatewayCompatibilityCredential,
) -> Result<ApplicationAuthenticatedRequestContext<'_>, CompatibilityAuthenticationError> {
    plan_application_request_authentication_from_compatibility(
        request,
        credential.principal,
        credential.anonymous_allowed,
    )
}

pub(super) fn runtime_gateway_application_data_plane_authorization(
    request: ApplicationRequestContext<'_>,
    credential: RuntimeGatewayCompatibilityCredential,
) -> Result<ApplicationAuthorizedRequestContext<'_>, RuntimeGatewayApplicationBoundaryError> {
    let authenticated = runtime_gateway_application_authentication(request, credential)
        .map_err(RuntimeGatewayApplicationBoundaryError::Authentication)?;
    plan_application_data_plane_authorization_from_compatibility(authenticated)
        .map_err(RuntimeGatewayApplicationBoundaryError::Authorization)
}

pub(super) fn runtime_gateway_application_control_plane_authorization<'a>(
    request: ApplicationRequestContext<'a>,
    http: &GatewayHttpRequestMeta,
    admin_auth: &RuntimeGatewayAdminAuth,
) -> Result<ApplicationAuthorizedRequestContext<'a>, RuntimeGatewayApplicationBoundaryError> {
    let action = runtime_gateway_admin_control_plane_action(http, admin_auth)
        .ok_or(RuntimeGatewayApplicationBoundaryError::Route)?;
    let authenticated = runtime_gateway_application_authentication(
        request,
        runtime_gateway_control_plane_compatibility_credential(admin_auth),
    )
    .map_err(RuntimeGatewayApplicationBoundaryError::Authentication)?;
    plan_application_control_plane_authorization_from_compatibility(authenticated, action)
        .map_err(RuntimeGatewayApplicationBoundaryError::Authorization)
}

pub(super) fn runtime_gateway_admin_preauthorization<'a>(
    request: ApplicationRequestContext<'a>,
    http: &GatewayHttpRequestMeta,
    auth: &RuntimeGatewayAdminAuth,
) -> Result<Option<ApplicationAuthorizedRequestContext<'a>>, RuntimeGatewayApplicationBoundaryError>
{
    let application =
        match runtime_gateway_application_control_plane_authorization(request, http, auth) {
            Ok(application) => Some(application),
            Err(RuntimeGatewayApplicationBoundaryError::Route) => None,
            Err(error) => return Err(error),
        };
    Ok(application)
}

pub(super) fn runtime_gateway_data_plane_compatibility_credential(
    virtual_key: Option<&runtime_proxy_crate::RuntimeGatewayVirtualKey>,
    legacy_data_plane_authorized: bool,
    authentication_configured: bool,
) -> RuntimeGatewayCompatibilityCredential {
    let principal = virtual_key
        .map(runtime_gateway_virtual_key_principal)
        .or_else(|| legacy_data_plane_authorized.then(runtime_gateway_bearer_principal));
    RuntimeGatewayCompatibilityCredential {
        principal,
        anonymous_allowed: virtual_key.is_none()
            && !legacy_data_plane_authorized
            && !authentication_configured,
    }
}

pub(super) fn runtime_gateway_control_plane_compatibility_credential(
    admin_auth: &RuntimeGatewayAdminAuth,
) -> RuntimeGatewayCompatibilityCredential {
    RuntimeGatewayCompatibilityCredential {
        principal: Some(runtime_gateway_admin_principal(admin_auth)),
        anonymous_allowed: false,
    }
}

pub(super) fn runtime_gateway_admin_control_plane_action(
    http: &GatewayHttpRequestMeta,
    admin_auth: &RuntimeGatewayAdminAuth,
) -> Option<ControlPlaneActionRequest> {
    let route = prodex_application::plan_application_control_plane_http_route(http).ok()?;
    let principal = runtime_gateway_admin_principal(admin_auth);
    let tenant_id = principal.tenant_id?;
    Some(ControlPlaneActionRequest {
        principal,
        operation: route.operation,
        resource: ControlPlaneResourceRef::new(
            tenant_id,
            route.operation.requirement().resource,
            None::<String>,
        ),
        occurred_at_unix_ms: 0,
    })
}

pub(super) fn runtime_gateway_admin_control_plane_tenant_id(
    admin_auth: &RuntimeGatewayAdminAuth,
) -> TenantId {
    admin_auth
        .tenant_id
        .as_deref()
        .and_then(|value| value.parse::<TenantId>().ok())
        .unwrap_or_else(|| {
            TenantId::from_uuid(runtime_gateway_stable_id(
                "prodex:gateway-admin-control-plane-tenant:v1",
                &[admin_auth.name.as_bytes()],
            ))
        })
}

fn runtime_gateway_admin_principal(admin_auth: &RuntimeGatewayAdminAuth) -> Principal {
    let tenant_id = runtime_gateway_admin_control_plane_tenant_id(admin_auth);
    let tenant_text = tenant_id.to_string();
    Principal::new(
        PrincipalId::from_uuid(runtime_gateway_stable_id(
            "prodex:gateway-admin-control-plane-principal:v1",
            &[tenant_text.as_bytes(), admin_auth.name.as_bytes()],
        )),
        Some(tenant_id),
        PrincipalKind::User,
        match admin_auth.role {
            RuntimeGatewayAdminRole::Admin => Role::Admin,
            RuntimeGatewayAdminRole::Viewer => Role::Viewer,
        },
        CredentialScope::ControlPlane,
    )
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

fn runtime_gateway_stable_id(namespace: &str, parts: &[&[u8]]) -> uuid::Uuid {
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
    use super::*;
    use prodex_gateway_http::GatewayHttpMethod;

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

    #[test]
    fn application_gate_matches_the_legacy_data_plane_decision_matrix() {
        let target = CanonicalRequestTarget::parse("/v1/responses").unwrap();
        let request = runtime_gateway_application_request_context(&target).unwrap();

        for virtual_keys_empty in [false, true] {
            for legacy_data_plane_authorized in [false, true] {
                for authentication_configured in [false, true] {
                    let legacy_allows = !virtual_keys_empty
                        || legacy_data_plane_authorized
                        || !authentication_configured;
                    let virtual_key = (!virtual_keys_empty).then(virtual_key);
                    let credential = runtime_gateway_data_plane_compatibility_credential(
                        virtual_key.as_ref(),
                        legacy_data_plane_authorized,
                        authentication_configured,
                    );
                    assert_eq!(
                        runtime_gateway_application_data_plane_authorization(request, credential)
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
        let data = runtime_gateway_application_request_context(&data_target).unwrap();
        let key = virtual_key();
        let authorized = runtime_gateway_application_data_plane_authorization(
            data,
            runtime_gateway_data_plane_compatibility_credential(Some(&key), false, true),
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
        let control = runtime_gateway_application_request_context(&control_target).unwrap();
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
                control,
                &read,
                &admin(RuntimeGatewayAdminRole::Viewer),
            )
            .is_ok(),
            "legacy viewer reads must remain authorized",
        );
        assert!(
            runtime_gateway_application_control_plane_authorization(
                control,
                &write,
                &admin(RuntimeGatewayAdminRole::Viewer),
            )
            .is_err(),
            "legacy viewer mutations must remain denied",
        );
        assert!(
            runtime_gateway_application_control_plane_authorization(
                control,
                &write,
                &admin(RuntimeGatewayAdminRole::Admin),
            )
            .is_ok(),
            "legacy admin mutations must remain authorized",
        );
    }
}
