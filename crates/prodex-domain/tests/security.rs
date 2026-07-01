use prodex_domain::{
    AuthorizationError, CredentialScope, ExplicitRoleMapper, Principal, PrincipalId, PrincipalKind,
    Role, RoleClaimError, SecurityErrorStatus, TenantId, TenantMode, TenantResolutionError,
    authorize_min_role, authorize_scope, plan_domain_authorization_error_response,
    plan_resource_authorization_error_response, plan_role_claim_error_response,
    plan_tenant_access_error_response, plan_tenant_resolution_error_response,
};

#[test]
fn missing_or_unknown_role_claim_never_maps_to_admin() {
    let mapper = ExplicitRoleMapper::new([("ops", Role::Operator), ("admins", Role::Admin)]);

    assert_eq!(mapper.role_for_claim(None), Err(RoleClaimError::Missing));
    assert_eq!(
        mapper.role_for_claim(Some("unmapped")),
        Err(RoleClaimError::Unknown)
    );
    assert_eq!(mapper.viewer_for_missing_or_unknown(None), Role::Viewer);
    assert_eq!(
        mapper.viewer_for_missing_or_unknown(Some("unmapped")),
        Role::Viewer
    );
}

#[test]
fn empty_or_whitespace_role_claim_never_maps_to_admin() {
    let mapper = ExplicitRoleMapper::new([("", Role::Admin), (" admins ", Role::Admin)]);

    assert_eq!(
        mapper.role_for_claim(Some("")),
        Err(RoleClaimError::Missing)
    );
    assert_eq!(
        mapper.role_for_claim(Some("   ")),
        Err(RoleClaimError::Unknown)
    );
    assert_eq!(
        mapper.role_for_claim(Some(" admins ")),
        Err(RoleClaimError::Unknown)
    );
    assert_eq!(
        mapper.viewer_for_missing_or_unknown(Some(" admins ")),
        Role::Viewer
    );
    assert_eq!(mapper.viewer_for_missing_or_unknown(Some("")), Role::Viewer);
    assert_eq!(
        mapper.role_for_claim(Some("admins")),
        Err(RoleClaimError::Unknown)
    );
}

#[test]
fn admin_role_requires_explicit_claim_mapping() {
    let mapper = ExplicitRoleMapper::new([("admins", Role::Admin)]);

    assert_eq!(mapper.role_for_claim(Some("admins")), Ok(Role::Admin));
    assert_ne!(
        mapper.viewer_for_missing_or_unknown(Some("admin")),
        Role::Admin
    );
}

#[test]
fn explicit_role_mapper_debug_output_is_stable_and_redacted() {
    let mapper = ExplicitRoleMapper::new([("secret-admins", Role::Admin)]);

    let rendered = format!("{mapper:?}");
    assert!(!rendered.contains("secret-admins"));
    assert!(!rendered.contains("Admin"));
    assert_eq!(rendered, "ExplicitRoleMapper { mappings: \"<redacted>\" }");
}

#[test]
fn tenant_context_requires_tenant_claim_in_multi_tenant_mode() {
    let principal = Principal::new(
        PrincipalId::new(),
        None,
        PrincipalKind::User,
        Role::Viewer,
        CredentialScope::ControlPlane,
    );

    assert_eq!(
        principal.tenant_context(TenantMode::MultiTenant),
        Err(TenantResolutionError::MissingTenant)
    );
}

#[test]
fn tenant_context_preserves_canonical_tenant_id() {
    let tenant_id = TenantId::new();
    let principal = Principal::new(
        PrincipalId::new(),
        Some(tenant_id),
        PrincipalKind::ServiceAccount,
        Role::Operator,
        CredentialScope::ControlPlane,
    );

    let context = principal
        .tenant_context(TenantMode::MultiTenant)
        .expect("tenant context");
    assert_eq!(context.tenant_id, tenant_id);
}

#[test]
fn principal_and_tenant_context_debug_output_is_stable_and_redacted() {
    let principal_id = PrincipalId::new();
    let tenant_id = TenantId::new();
    let principal = Principal::new(
        principal_id,
        Some(tenant_id),
        PrincipalKind::ServiceAccount,
        Role::Operator,
        CredentialScope::ControlPlane,
    );
    let context = prodex_domain::TenantContext { tenant_id };

    let rendered = format!("{principal:?} {context:?}");
    for sensitive in [principal_id.to_string(), tenant_id.to_string()] {
        assert!(
            !rendered.contains(&sensitive),
            "security debug output leaked sensitive identifier {sensitive}: {rendered}"
        );
    }
    assert!(rendered.contains("kind: ServiceAccount"));
    assert!(rendered.contains("role: Operator"));
    assert!(rendered.contains("credential_scope: ControlPlane"));
    assert!(rendered.contains("\"<redacted>\""));
}

#[test]
fn data_plane_credential_cannot_access_control_plane_scope() {
    let principal = Principal::new(
        PrincipalId::new(),
        Some(TenantId::new()),
        PrincipalKind::VirtualKey,
        Role::Viewer,
        CredentialScope::DataPlane,
    );

    assert_eq!(
        authorize_scope(&principal, CredentialScope::ControlPlane),
        Err(AuthorizationError::CredentialScopeMismatch {
            expected: CredentialScope::ControlPlane,
            actual: CredentialScope::DataPlane,
        })
    );
}

#[test]
fn control_plane_credential_cannot_access_data_plane_scope() {
    let principal = Principal::new(
        PrincipalId::new(),
        Some(TenantId::new()),
        PrincipalKind::User,
        Role::Admin,
        CredentialScope::ControlPlane,
    );

    assert_eq!(
        authorize_scope(&principal, CredentialScope::DataPlane),
        Err(AuthorizationError::CredentialScopeMismatch {
            expected: CredentialScope::DataPlane,
            actual: CredentialScope::ControlPlane,
        })
    );
}

#[test]
fn break_glass_credential_is_not_implicit_scope_bypass() {
    let tenant_id = TenantId::new();
    let principal = Principal::new(
        PrincipalId::new(),
        Some(tenant_id),
        PrincipalKind::BreakGlass,
        Role::Admin,
        CredentialScope::BreakGlass,
    );

    assert_eq!(
        authorize_scope(&principal, CredentialScope::DataPlane),
        Err(AuthorizationError::CredentialScopeMismatch {
            expected: CredentialScope::DataPlane,
            actual: CredentialScope::BreakGlass,
        })
    );
    assert_eq!(
        authorize_scope(&principal, CredentialScope::ControlPlane),
        Err(AuthorizationError::CredentialScopeMismatch {
            expected: CredentialScope::ControlPlane,
            actual: CredentialScope::BreakGlass,
        })
    );
    assert_eq!(
        authorize_scope(&principal, CredentialScope::BreakGlass),
        Ok(())
    );
}

#[test]
fn control_plane_mutation_requires_admin_role() {
    let principal = Principal::new(
        PrincipalId::new(),
        Some(TenantId::new()),
        PrincipalKind::User,
        Role::Operator,
        CredentialScope::ControlPlane,
    );

    assert_eq!(
        authorize_min_role(&principal, Role::Admin),
        Err(AuthorizationError::InsufficientRole {
            required: Role::Admin,
            actual: Role::Operator,
        })
    );
}

#[derive(Clone, Copy, Debug)]
struct TenantOwnedFixture {
    tenant_id: TenantId,
}

impl prodex_domain::TenantScopedResource for TenantOwnedFixture {
    fn tenant_id(&self) -> TenantId {
        self.tenant_id
    }
}

#[test]
fn tenant_authorization_allows_same_tenant_resource_access() {
    let tenant_id = TenantId::new();
    let principal = Principal::new(
        PrincipalId::new(),
        Some(tenant_id),
        PrincipalKind::User,
        Role::Viewer,
        CredentialScope::ControlPlane,
    );
    let resource = TenantOwnedFixture { tenant_id };

    let context = prodex_domain::authorize_tenant_access(&principal, &resource).unwrap();

    assert_eq!(context.tenant_id, tenant_id);
}

#[test]
fn tenant_authorization_denies_cross_tenant_resource_access() {
    let principal_tenant_id = TenantId::new();
    let resource_tenant_id = TenantId::new();
    let principal = Principal::new(
        PrincipalId::new(),
        Some(principal_tenant_id),
        PrincipalKind::User,
        Role::Admin,
        CredentialScope::ControlPlane,
    );
    let resource = TenantOwnedFixture {
        tenant_id: resource_tenant_id,
    };

    assert_eq!(
        prodex_domain::authorize_tenant_access(&principal, &resource),
        Err(prodex_domain::TenantAccessError::CrossTenantAccess {
            principal_tenant_id,
            resource_tenant_id,
        })
    );
}

#[test]
fn tenant_authorization_denies_principal_without_tenant_context() {
    let principal = Principal::new(
        PrincipalId::new(),
        None,
        PrincipalKind::User,
        Role::Admin,
        CredentialScope::ControlPlane,
    );
    let resource = TenantOwnedFixture {
        tenant_id: TenantId::new(),
    };

    assert_eq!(
        prodex_domain::authorize_tenant_access(&principal, &resource),
        Err(prodex_domain::TenantAccessError::PrincipalMissingTenant)
    );
}

#[test]
fn resource_action_authorization_allows_matching_scope_role_and_tenant() {
    let tenant_id = TenantId::new();
    let principal = Principal::new(
        PrincipalId::new(),
        Some(tenant_id),
        PrincipalKind::User,
        Role::Admin,
        CredentialScope::ControlPlane,
    );
    let resource = TenantOwnedFixture { tenant_id };
    let requirement = prodex_domain::AuthorizationRequirement::new(
        prodex_domain::ResourceKind::Policy,
        prodex_domain::ResourceAction::PublishRevision,
        CredentialScope::ControlPlane,
        Role::Admin,
    );

    let context = prodex_domain::authorize_resource_action(&principal, requirement, &resource)
        .expect("authorized");

    assert_eq!(context.tenant_id, tenant_id);
}

#[test]
fn authorization_requirement_debug_output_is_stable_and_redacted() {
    let requirement = prodex_domain::AuthorizationRequirement::new(
        prodex_domain::ResourceKind::ProviderCredential,
        prodex_domain::ResourceAction::RotateSecret,
        CredentialScope::ControlPlane,
        Role::Admin,
    );

    let rendered = format!("{requirement:?}");
    for sensitive in [
        "ProviderCredential",
        "RotateSecret",
        "ControlPlane",
        "Admin",
    ] {
        assert!(
            !rendered.contains(sensitive),
            "authorization requirement debug output leaked sensitive token {sensitive}: {rendered}"
        );
    }
    assert_eq!(
        rendered,
        "AuthorizationRequirement { resource: \"<redacted>\", action: \"<redacted>\", required_scope: \"<redacted>\", required_role: \"<redacted>\" }"
    );
}

#[test]
fn resource_action_authorization_denies_vertical_privilege_escalation() {
    let tenant_id = TenantId::new();
    let principal = Principal::new(
        PrincipalId::new(),
        Some(tenant_id),
        PrincipalKind::User,
        Role::Operator,
        CredentialScope::ControlPlane,
    );
    let resource = TenantOwnedFixture { tenant_id };
    let requirement = prodex_domain::AuthorizationRequirement::new(
        prodex_domain::ResourceKind::ProviderCredential,
        prodex_domain::ResourceAction::RotateSecret,
        CredentialScope::ControlPlane,
        Role::Admin,
    );

    assert!(matches!(
        prodex_domain::authorize_resource_action(&principal, requirement, &resource),
        Err(prodex_domain::ResourceAuthorizationError::Role(
            prodex_domain::AuthorizationError::InsufficientRole { .. }
        ))
    ));
}

#[test]
fn resource_action_authorization_denies_horizontal_tenant_escalation() {
    let principal_tenant_id = TenantId::new();
    let resource_tenant_id = TenantId::new();
    let principal = Principal::new(
        PrincipalId::new(),
        Some(principal_tenant_id),
        PrincipalKind::User,
        Role::Admin,
        CredentialScope::ControlPlane,
    );
    let resource = TenantOwnedFixture {
        tenant_id: resource_tenant_id,
    };
    let requirement = prodex_domain::AuthorizationRequirement::new(
        prodex_domain::ResourceKind::Budget,
        prodex_domain::ResourceAction::Update,
        CredentialScope::ControlPlane,
        Role::Admin,
    );

    assert!(matches!(
        prodex_domain::authorize_resource_action(&principal, requirement, &resource),
        Err(prodex_domain::ResourceAuthorizationError::Tenant(
            prodex_domain::TenantAccessError::CrossTenantAccess { .. }
        ))
    ));
}

#[test]
fn resource_action_authorization_denies_scope_bypass() {
    let tenant_id = TenantId::new();
    let principal = Principal::new(
        PrincipalId::new(),
        Some(tenant_id),
        PrincipalKind::VirtualKey,
        Role::Admin,
        CredentialScope::DataPlane,
    );
    let resource = TenantOwnedFixture { tenant_id };
    let requirement = prodex_domain::AuthorizationRequirement::new(
        prodex_domain::ResourceKind::AuditLog,
        prodex_domain::ResourceAction::Export,
        CredentialScope::ControlPlane,
        Role::Admin,
    );

    assert!(matches!(
        prodex_domain::authorize_resource_action(&principal, requirement, &resource),
        Err(prodex_domain::ResourceAuthorizationError::Scope(
            prodex_domain::AuthorizationError::CredentialScopeMismatch { .. }
        ))
    ));
}

#[test]
fn resource_action_authorization_denies_break_glass_scope_bypass() {
    let tenant_id = TenantId::new();
    let principal = Principal::new(
        PrincipalId::new(),
        Some(tenant_id),
        PrincipalKind::BreakGlass,
        Role::Admin,
        CredentialScope::BreakGlass,
    );
    let resource = TenantOwnedFixture { tenant_id };
    let requirement = prodex_domain::AuthorizationRequirement::new(
        prodex_domain::ResourceKind::VirtualKey,
        prodex_domain::ResourceAction::Read,
        CredentialScope::DataPlane,
        Role::Operator,
    );

    assert!(matches!(
        prodex_domain::authorize_resource_action(&principal, requirement, &resource),
        Err(prodex_domain::ResourceAuthorizationError::Scope(
            prodex_domain::AuthorizationError::CredentialScopeMismatch { .. }
        ))
    ));
}

#[test]
fn domain_security_error_responses_are_stable_and_redacted() {
    assert_eq!(
        RoleClaimError::Missing.to_string(),
        "role claim is not authorized"
    );
    assert_eq!(
        RoleClaimError::Unknown.to_string(),
        "role claim is not authorized"
    );
    assert!(!RoleClaimError::Unknown.to_string().contains("secret-admin"));
    assert!(!format!("{:?}", RoleClaimError::Unknown).contains("secret-admin"));

    let role = plan_role_claim_error_response(&RoleClaimError::Unknown);
    assert_eq!(role.status, SecurityErrorStatus::Forbidden);
    assert_eq!(role.code, "role_claim_denied");
    assert_eq!(role.message, "role claim is not authorized");
    assert!(!role.message.contains("secret-admin"));

    let tenant = plan_tenant_resolution_error_response(&TenantResolutionError::MissingTenant);
    assert_eq!(
        TenantResolutionError::MissingTenant.to_string(),
        "tenant context is required"
    );
    assert_eq!(tenant.status, SecurityErrorStatus::BadRequest);
    assert_eq!(tenant.code, "tenant_required");
    assert_eq!(tenant.message, "tenant context is required");

    let principal_tenant_id = TenantId::new();
    let resource_tenant_id = TenantId::new();
    let access_error = prodex_domain::TenantAccessError::CrossTenantAccess {
        principal_tenant_id,
        resource_tenant_id,
    };
    assert_eq!(access_error.to_string(), "tenant access is denied");
    assert!(
        !access_error
            .to_string()
            .contains(&principal_tenant_id.to_string())
    );
    assert!(
        !access_error
            .to_string()
            .contains(&resource_tenant_id.to_string())
    );
    assert!(!format!("{access_error:?}").contains(&principal_tenant_id.to_string()));
    assert!(!format!("{access_error:?}").contains(&resource_tenant_id.to_string()));
    let resource_access_error =
        prodex_domain::ResourceAuthorizationError::Tenant(access_error.clone());
    assert_eq!(
        resource_access_error.to_string(),
        "resource authorization request is denied"
    );
    assert!(!format!("{resource_access_error:?}").contains(&principal_tenant_id.to_string()));
    assert!(!format!("{resource_access_error:?}").contains(&resource_tenant_id.to_string()));
    let access = plan_tenant_access_error_response(&access_error);
    assert_eq!(access.status, SecurityErrorStatus::Forbidden);
    assert_eq!(access.code, "tenant_access_denied");
    assert_eq!(access.message, "tenant access is denied");
    assert!(!access.message.contains(&principal_tenant_id.to_string()));
    assert!(!access.message.contains(&resource_tenant_id.to_string()));

    let scope =
        plan_domain_authorization_error_response(&AuthorizationError::CredentialScopeMismatch {
            expected: CredentialScope::ControlPlane,
            actual: CredentialScope::DataPlane,
        });
    let scope_error = AuthorizationError::CredentialScopeMismatch {
        expected: CredentialScope::ControlPlane,
        actual: CredentialScope::DataPlane,
    };
    assert_eq!(scope_error.to_string(), "authorization request is denied");
    let scope_rendered = format!("{scope_error:?}");
    assert!(!scope_rendered.contains("ControlPlane"));
    assert!(!scope_rendered.contains("DataPlane"));
    assert!(scope_rendered.contains("\"<redacted>\""));
    assert_eq!(scope.status, SecurityErrorStatus::Forbidden);
    assert_eq!(scope.code, "credential_scope_denied");
    assert_eq!(scope.message, "credential scope is not allowed");
    assert!(!scope.message.contains("DataPlane"));

    let resource = plan_resource_authorization_error_response(
        &prodex_domain::ResourceAuthorizationError::Role(AuthorizationError::InsufficientRole {
            required: Role::Admin,
            actual: Role::Viewer,
        }),
    );
    let role_error = AuthorizationError::InsufficientRole {
        required: Role::Admin,
        actual: Role::Viewer,
    };
    let resource_role_error = prodex_domain::ResourceAuthorizationError::Role(role_error.clone());
    assert_eq!(
        resource_role_error.to_string(),
        "resource authorization request is denied"
    );
    assert_eq!(role_error.to_string(), "authorization request is denied");
    let role_rendered = format!("{role_error:?}");
    assert!(!role_rendered.contains("Admin"));
    assert!(!role_rendered.contains("Viewer"));
    assert!(role_rendered.contains("\"<redacted>\""));
    assert_eq!(resource.status, SecurityErrorStatus::Forbidden);
    assert_eq!(resource.code, "role_denied");
    assert_eq!(resource.message, "principal role is not sufficient");
    assert!(!resource.message.contains("Admin"));
    assert!(!resource.message.contains("Viewer"));
}
