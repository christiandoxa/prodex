use prodex_authz::{
    AuthorizationErrorStatus, BoundaryAuthorizationError, BoundaryKind,
    authorize_boundary_resource, authorize_boundary_scope, boundary_for_requirement,
    break_glass_boundary_for_requirement, control_plane_boundary_for_requirement,
    data_plane_boundary_for_requirement, plan_authorization_error_response,
};
use prodex_domain::{
    AuthorizationRequirement, CredentialScope, Principal, PrincipalId, PrincipalKind,
    ResourceAction, ResourceKind, Role, TenantId, TenantScopedResource,
};

#[derive(Clone, Copy)]
struct TenantResource {
    tenant_id: TenantId,
}

impl TenantScopedResource for TenantResource {
    fn tenant_id(&self) -> TenantId {
        self.tenant_id
    }
}

fn principal(tenant_id: Option<TenantId>, role: Role, scope: CredentialScope) -> Principal {
    principal_with_kind(tenant_id, PrincipalKind::ServiceAccount, role, scope)
}

fn principal_with_kind(
    tenant_id: Option<TenantId>,
    kind: PrincipalKind,
    role: Role,
    scope: CredentialScope,
) -> Principal {
    Principal::new(PrincipalId::new(), tenant_id, kind, role, scope)
}

fn lifecycle_admin_boundaries() -> [(BoundaryKind, ResourceKind, ResourceAction); 8] {
    [
        (
            BoundaryKind::ControlPlaneTenantCreate,
            ResourceKind::Tenant,
            ResourceAction::Create,
        ),
        (
            BoundaryKind::ControlPlaneTenantUpdate,
            ResourceKind::Tenant,
            ResourceAction::Update,
        ),
        (
            BoundaryKind::ControlPlaneUserCreate,
            ResourceKind::User,
            ResourceAction::Create,
        ),
        (
            BoundaryKind::ControlPlaneUserUpdate,
            ResourceKind::User,
            ResourceAction::Update,
        ),
        (
            BoundaryKind::ControlPlaneUserDelete,
            ResourceKind::User,
            ResourceAction::Delete,
        ),
        (
            BoundaryKind::ControlPlaneRoleBindingGrant,
            ResourceKind::RoleBinding,
            ResourceAction::Create,
        ),
        (
            BoundaryKind::ControlPlaneRoleBindingRevoke,
            ResourceKind::RoleBinding,
            ResourceAction::Delete,
        ),
        (
            BoundaryKind::ControlPlaneServiceIdentityCreate,
            ResourceKind::ServiceIdentity,
            ResourceAction::Create,
        ),
    ]
}

fn secret_budget_admin_boundaries() -> [(BoundaryKind, ResourceKind, ResourceAction); 4] {
    [
        (
            BoundaryKind::ControlPlaneVirtualKeyCreate,
            ResourceKind::VirtualKey,
            ResourceAction::Create,
        ),
        (
            BoundaryKind::ControlPlaneVirtualKeyRotateSecret,
            ResourceKind::VirtualKey,
            ResourceAction::RotateSecret,
        ),
        (
            BoundaryKind::ControlPlaneProviderCredentialRotate,
            ResourceKind::ProviderCredential,
            ResourceAction::RotateSecret,
        ),
        (
            BoundaryKind::ControlPlaneBudgetUpdate,
            ResourceKind::Budget,
            ResourceAction::Update,
        ),
    ]
}

fn publish_admin_boundaries() -> [(BoundaryKind, ResourceKind, ResourceAction); 2] {
    [
        (
            BoundaryKind::ControlPlanePolicyPublish,
            ResourceKind::Policy,
            ResourceAction::PublishRevision,
        ),
        (
            BoundaryKind::ControlPlaneConfigurationPublish,
            ResourceKind::Configuration,
            ResourceAction::PublishRevision,
        ),
    ]
}

fn audit_admin_boundaries() -> [(BoundaryKind, ResourceKind, ResourceAction); 2] {
    [
        (
            BoundaryKind::ControlPlaneAuditExport,
            ResourceKind::AuditLog,
            ResourceAction::Export,
        ),
        (
            BoundaryKind::ControlPlaneAuditRetentionPurge,
            ResourceKind::AuditLog,
            ResourceAction::Delete,
        ),
    ]
}

#[test]
fn data_plane_inference_requires_data_plane_scope_and_operator_role() {
    let requirement = BoundaryKind::DataPlaneInference.requirement();

    assert_eq!(requirement.resource, ResourceKind::VirtualKey);
    assert_eq!(requirement.action, ResourceAction::Read);
    assert_eq!(requirement.required_scope, CredentialScope::DataPlane);
    assert_eq!(requirement.required_role, Role::Operator);
}

#[test]
fn explicit_data_plane_requirements_resolve_to_authz_boundaries() {
    let inference = data_plane_boundary_for_requirement(AuthorizationRequirement::new(
        ResourceKind::VirtualKey,
        ResourceAction::Read,
        CredentialScope::DataPlane,
        Role::Operator,
    ))
    .unwrap();
    let quota = data_plane_boundary_for_requirement(AuthorizationRequirement::new(
        ResourceKind::Budget,
        ResourceAction::Read,
        CredentialScope::DataPlane,
        Role::Operator,
    ))
    .unwrap();

    assert_eq!(inference, BoundaryKind::DataPlaneInference);
    assert_eq!(
        inference.requirement(),
        BoundaryKind::DataPlaneInference.requirement()
    );
    assert_eq!(quota, BoundaryKind::DataPlaneQuota);
    assert_eq!(
        quota.requirement(),
        BoundaryKind::DataPlaneQuota.requirement()
    );
}

#[test]
fn data_plane_boundary_resolver_rejects_control_plane_or_wrong_requirements() {
    assert_eq!(
        data_plane_boundary_for_requirement(AuthorizationRequirement::new(
            ResourceKind::VirtualKey,
            ResourceAction::Read,
            CredentialScope::ControlPlane,
            Role::Operator,
        )),
        None
    );
    assert_eq!(
        data_plane_boundary_for_requirement(AuthorizationRequirement::new(
            ResourceKind::VirtualKey,
            ResourceAction::Read,
            CredentialScope::BreakGlass,
            Role::Admin,
        )),
        None
    );
    assert_eq!(
        data_plane_boundary_for_requirement(AuthorizationRequirement::new(
            ResourceKind::Budget,
            ResourceAction::Update,
            CredentialScope::DataPlane,
            Role::Operator,
        )),
        None
    );
    assert_eq!(
        data_plane_boundary_for_requirement(AuthorizationRequirement::new(
            ResourceKind::VirtualKey,
            ResourceAction::Read,
            CredentialScope::DataPlane,
            Role::Viewer,
        )),
        None
    );
}

#[test]
fn universal_boundary_resolver_maps_data_control_and_break_glass_requirements() {
    for boundary in [
        BoundaryKind::DataPlaneInference,
        BoundaryKind::DataPlaneQuota,
        BoundaryKind::ControlPlaneTenantCreate,
        BoundaryKind::ControlPlaneVirtualKeyRotateSecret,
        BoundaryKind::ControlPlanePolicyPublish,
        BoundaryKind::ControlPlaneBillingRead,
        BoundaryKind::ControlPlaneAuditExport,
        BoundaryKind::BreakGlassAdmin,
    ] {
        assert_eq!(
            boundary_for_requirement(boundary.requirement()),
            Some(boundary)
        );
    }
}

#[test]
fn explicit_control_plane_requirements_resolve_to_authz_boundaries() {
    let mut cases = Vec::new();
    cases.extend(lifecycle_admin_boundaries());
    cases.extend(secret_budget_admin_boundaries());
    cases.extend(publish_admin_boundaries());
    cases.push((
        BoundaryKind::ControlPlaneBillingRead,
        ResourceKind::Billing,
        ResourceAction::Read,
    ));
    cases.extend(audit_admin_boundaries());

    for (expected_boundary, resource, action) in cases {
        let requirement = expected_boundary.requirement();
        let boundary = control_plane_boundary_for_requirement(AuthorizationRequirement::new(
            resource,
            action,
            CredentialScope::ControlPlane,
            requirement.required_role,
        ))
        .expect("explicit control-plane requirement must resolve");

        assert_eq!(boundary, expected_boundary);
        assert_eq!(boundary.requirement(), requirement);
    }
}

#[test]
fn explicit_break_glass_requirement_resolves_only_to_break_glass_boundary() {
    assert_eq!(
        break_glass_boundary_for_requirement(BoundaryKind::BreakGlassAdmin.requirement()),
        Some(BoundaryKind::BreakGlassAdmin)
    );
    assert_eq!(
        break_glass_boundary_for_requirement(AuthorizationRequirement::new(
            ResourceKind::Configuration,
            ResourceAction::Update,
            CredentialScope::ControlPlane,
            Role::Admin,
        )),
        None
    );
    assert_eq!(
        break_glass_boundary_for_requirement(AuthorizationRequirement::new(
            ResourceKind::VirtualKey,
            ResourceAction::Read,
            CredentialScope::BreakGlass,
            Role::Admin,
        )),
        None
    );
}

#[test]
fn control_plane_boundary_resolver_rejects_generic_or_wrong_scope_requirements() {
    assert_eq!(
        control_plane_boundary_for_requirement(AuthorizationRequirement::new(
            ResourceKind::Configuration,
            ResourceAction::Update,
            CredentialScope::ControlPlane,
            Role::Admin,
        )),
        None
    );
    assert_eq!(
        control_plane_boundary_for_requirement(AuthorizationRequirement::new(
            ResourceKind::VirtualKey,
            ResourceAction::Read,
            CredentialScope::DataPlane,
            Role::Operator,
        )),
        None
    );
    assert_eq!(
        control_plane_boundary_for_requirement(AuthorizationRequirement::new(
            ResourceKind::Configuration,
            ResourceAction::Update,
            CredentialScope::BreakGlass,
            Role::Admin,
        )),
        None
    );
}

#[test]
fn control_plane_admin_requires_control_plane_admin_scope() {
    let tenant_id = TenantId::new();
    let resource = TenantResource { tenant_id };
    let principal = principal(Some(tenant_id), Role::Admin, CredentialScope::ControlPlane);

    let context =
        authorize_boundary_resource(BoundaryKind::ControlPlaneAdmin, &principal, &resource)
            .unwrap();
    assert_eq!(context.tenant_id, tenant_id);
}

#[test]
fn control_plane_lifecycle_boundaries_use_specific_resource_actions() {
    let tenant_id = TenantId::new();
    let resource = TenantResource { tenant_id };
    let principal = principal(Some(tenant_id), Role::Admin, CredentialScope::ControlPlane);

    for (boundary, resource_kind, action) in lifecycle_admin_boundaries() {
        let requirement = boundary.requirement();

        assert_eq!(requirement.resource, resource_kind);
        assert_eq!(requirement.action, action);
        assert_eq!(requirement.required_scope, CredentialScope::ControlPlane);
        assert_eq!(requirement.required_role, Role::Admin);
        assert_eq!(
            authorize_boundary_resource(boundary, &principal, &resource)
                .unwrap()
                .tenant_id,
            tenant_id
        );
    }
}

#[test]
fn control_plane_secret_and_budget_boundaries_use_specific_resource_actions() {
    let tenant_id = TenantId::new();
    let resource = TenantResource { tenant_id };
    let principal = principal(Some(tenant_id), Role::Admin, CredentialScope::ControlPlane);

    for (boundary, resource_kind, action) in secret_budget_admin_boundaries() {
        let requirement = boundary.requirement();

        assert_eq!(requirement.resource, resource_kind);
        assert_eq!(requirement.action, action);
        assert_eq!(requirement.required_scope, CredentialScope::ControlPlane);
        assert_eq!(requirement.required_role, Role::Admin);
        assert_eq!(
            authorize_boundary_resource(boundary, &principal, &resource)
                .unwrap()
                .tenant_id,
            tenant_id
        );
    }
}

#[test]
fn control_plane_publish_boundaries_use_specific_resource_actions() {
    let tenant_id = TenantId::new();
    let resource = TenantResource { tenant_id };
    let principal = principal(Some(tenant_id), Role::Admin, CredentialScope::ControlPlane);
    let policy = BoundaryKind::ControlPlanePolicyPublish.requirement();
    let config = BoundaryKind::ControlPlaneConfigurationPublish.requirement();

    assert_eq!(policy.resource, ResourceKind::Policy);
    assert_eq!(policy.action, ResourceAction::PublishRevision);
    assert_eq!(policy.required_scope, CredentialScope::ControlPlane);
    assert_eq!(policy.required_role, Role::Admin);
    assert_eq!(config.resource, ResourceKind::Configuration);
    assert_eq!(config.action, ResourceAction::PublishRevision);
    assert_eq!(config.required_scope, CredentialScope::ControlPlane);
    assert_eq!(config.required_role, Role::Admin);
    assert_eq!(
        authorize_boundary_resource(
            BoundaryKind::ControlPlanePolicyPublish,
            &principal,
            &resource,
        )
        .unwrap()
        .tenant_id,
        tenant_id
    );
    assert_eq!(
        authorize_boundary_resource(
            BoundaryKind::ControlPlaneConfigurationPublish,
            &principal,
            &resource,
        )
        .unwrap()
        .tenant_id,
        tenant_id
    );
}

#[test]
fn control_plane_billing_read_allows_viewer_without_admin_escalation() {
    let tenant_id = TenantId::new();
    let resource = TenantResource { tenant_id };
    let principal = principal(Some(tenant_id), Role::Viewer, CredentialScope::ControlPlane);
    let requirement = BoundaryKind::ControlPlaneBillingRead.requirement();

    assert_eq!(requirement.resource, ResourceKind::Billing);
    assert_eq!(requirement.action, ResourceAction::Read);
    assert_eq!(requirement.required_scope, CredentialScope::ControlPlane);
    assert_eq!(requirement.required_role, Role::Viewer);
    let context =
        authorize_boundary_resource(BoundaryKind::ControlPlaneBillingRead, &principal, &resource)
            .unwrap();
    assert_eq!(context.tenant_id, tenant_id);
}

#[test]
fn control_plane_audit_boundaries_use_export_and_delete_actions() {
    let tenant_id = TenantId::new();
    let resource = TenantResource { tenant_id };
    let principal = principal(Some(tenant_id), Role::Admin, CredentialScope::ControlPlane);
    let export = BoundaryKind::ControlPlaneAuditExport.requirement();
    let purge = BoundaryKind::ControlPlaneAuditRetentionPurge.requirement();

    assert_eq!(export.resource, ResourceKind::AuditLog);
    assert_eq!(export.action, ResourceAction::Export);
    assert_eq!(export.required_scope, CredentialScope::ControlPlane);
    assert_eq!(export.required_role, Role::Admin);
    assert_eq!(purge.resource, ResourceKind::AuditLog);
    assert_eq!(purge.action, ResourceAction::Delete);
    assert_eq!(purge.required_scope, CredentialScope::ControlPlane);
    assert_eq!(purge.required_role, Role::Admin);
    assert_eq!(
        authorize_boundary_resource(BoundaryKind::ControlPlaneAuditExport, &principal, &resource)
            .unwrap()
            .tenant_id,
        tenant_id
    );
    assert_eq!(
        authorize_boundary_resource(
            BoundaryKind::ControlPlaneAuditRetentionPurge,
            &principal,
            &resource,
        )
        .unwrap()
        .tenant_id,
        tenant_id
    );
}

#[test]
fn control_plane_credential_cannot_bypass_data_plane_inference() {
    let tenant_id = TenantId::new();
    let principal = principal(Some(tenant_id), Role::Admin, CredentialScope::ControlPlane);

    assert_eq!(
        authorize_boundary_scope(BoundaryKind::DataPlaneInference, &principal),
        Err(BoundaryAuthorizationError::CredentialScopeMismatch {
            boundary: BoundaryKind::DataPlaneInference,
            expected: CredentialScope::DataPlane,
            actual: CredentialScope::ControlPlane,
        })
    );
}

#[test]
fn data_plane_credential_cannot_call_control_plane_admin() {
    let tenant_id = TenantId::new();
    let principal = principal(Some(tenant_id), Role::Admin, CredentialScope::DataPlane);

    assert_eq!(
        authorize_boundary_scope(BoundaryKind::ControlPlaneAdmin, &principal),
        Err(BoundaryAuthorizationError::CredentialScopeMismatch {
            boundary: BoundaryKind::ControlPlaneAdmin,
            expected: CredentialScope::ControlPlane,
            actual: CredentialScope::DataPlane,
        })
    );
}

#[test]
fn data_plane_credential_cannot_call_control_plane_lifecycle_boundaries() {
    let tenant_id = TenantId::new();
    let principal = principal(Some(tenant_id), Role::Admin, CredentialScope::DataPlane);

    for (boundary, _, _) in lifecycle_admin_boundaries() {
        assert_eq!(
            authorize_boundary_scope(boundary, &principal),
            Err(BoundaryAuthorizationError::CredentialScopeMismatch {
                boundary,
                expected: CredentialScope::ControlPlane,
                actual: CredentialScope::DataPlane,
            })
        );
    }
}

#[test]
fn data_plane_credential_cannot_call_secret_or_budget_admin_boundaries() {
    let tenant_id = TenantId::new();
    let principal = principal(Some(tenant_id), Role::Admin, CredentialScope::DataPlane);

    for (boundary, _, _) in secret_budget_admin_boundaries() {
        assert_eq!(
            authorize_boundary_scope(boundary, &principal),
            Err(BoundaryAuthorizationError::CredentialScopeMismatch {
                boundary,
                expected: CredentialScope::ControlPlane,
                actual: CredentialScope::DataPlane,
            })
        );
    }
}

#[test]
fn data_plane_credential_cannot_publish_control_plane_policy_or_config() {
    let tenant_id = TenantId::new();
    let principal = principal(Some(tenant_id), Role::Admin, CredentialScope::DataPlane);

    assert_eq!(
        authorize_boundary_scope(BoundaryKind::ControlPlanePolicyPublish, &principal),
        Err(BoundaryAuthorizationError::CredentialScopeMismatch {
            boundary: BoundaryKind::ControlPlanePolicyPublish,
            expected: CredentialScope::ControlPlane,
            actual: CredentialScope::DataPlane,
        })
    );
    assert_eq!(
        authorize_boundary_scope(BoundaryKind::ControlPlaneConfigurationPublish, &principal,),
        Err(BoundaryAuthorizationError::CredentialScopeMismatch {
            boundary: BoundaryKind::ControlPlaneConfigurationPublish,
            expected: CredentialScope::ControlPlane,
            actual: CredentialScope::DataPlane,
        })
    );
}

#[test]
fn data_plane_credential_cannot_read_control_plane_billing() {
    let tenant_id = TenantId::new();
    let principal = principal(Some(tenant_id), Role::Admin, CredentialScope::DataPlane);

    assert_eq!(
        authorize_boundary_scope(BoundaryKind::ControlPlaneBillingRead, &principal),
        Err(BoundaryAuthorizationError::CredentialScopeMismatch {
            boundary: BoundaryKind::ControlPlaneBillingRead,
            expected: CredentialScope::ControlPlane,
            actual: CredentialScope::DataPlane,
        })
    );
}

#[test]
fn data_plane_credential_cannot_export_or_purge_control_plane_audit() {
    let tenant_id = TenantId::new();
    let principal = principal(Some(tenant_id), Role::Admin, CredentialScope::DataPlane);

    assert_eq!(
        authorize_boundary_scope(BoundaryKind::ControlPlaneAuditExport, &principal),
        Err(BoundaryAuthorizationError::CredentialScopeMismatch {
            boundary: BoundaryKind::ControlPlaneAuditExport,
            expected: CredentialScope::ControlPlane,
            actual: CredentialScope::DataPlane,
        })
    );
    assert_eq!(
        authorize_boundary_scope(BoundaryKind::ControlPlaneAuditRetentionPurge, &principal,),
        Err(BoundaryAuthorizationError::CredentialScopeMismatch {
            boundary: BoundaryKind::ControlPlaneAuditRetentionPurge,
            expected: CredentialScope::ControlPlane,
            actual: CredentialScope::DataPlane,
        })
    );
}

#[test]
fn break_glass_scope_is_not_implicit_data_plane_or_control_plane_bypass() {
    let tenant_id = TenantId::new();
    let principal = principal_with_kind(
        Some(tenant_id),
        PrincipalKind::BreakGlass,
        Role::Admin,
        CredentialScope::BreakGlass,
    );

    assert!(authorize_boundary_scope(BoundaryKind::BreakGlassAdmin, &principal).is_ok());
    assert_eq!(
        authorize_boundary_scope(BoundaryKind::DataPlaneInference, &principal),
        Err(BoundaryAuthorizationError::CredentialScopeMismatch {
            boundary: BoundaryKind::DataPlaneInference,
            expected: CredentialScope::DataPlane,
            actual: CredentialScope::BreakGlass,
        })
    );
    assert_eq!(
        authorize_boundary_scope(BoundaryKind::ControlPlaneAdmin, &principal),
        Err(BoundaryAuthorizationError::CredentialScopeMismatch {
            boundary: BoundaryKind::ControlPlaneAdmin,
            expected: CredentialScope::ControlPlane,
            actual: CredentialScope::BreakGlass,
        })
    );
    for (boundary, _, _) in lifecycle_admin_boundaries() {
        assert_eq!(
            authorize_boundary_scope(boundary, &principal),
            Err(BoundaryAuthorizationError::CredentialScopeMismatch {
                boundary,
                expected: CredentialScope::ControlPlane,
                actual: CredentialScope::BreakGlass,
            })
        );
    }
    for (boundary, _, _) in secret_budget_admin_boundaries() {
        assert_eq!(
            authorize_boundary_scope(boundary, &principal),
            Err(BoundaryAuthorizationError::CredentialScopeMismatch {
                boundary,
                expected: CredentialScope::ControlPlane,
                actual: CredentialScope::BreakGlass,
            })
        );
    }
    assert_eq!(
        authorize_boundary_scope(BoundaryKind::ControlPlanePolicyPublish, &principal),
        Err(BoundaryAuthorizationError::CredentialScopeMismatch {
            boundary: BoundaryKind::ControlPlanePolicyPublish,
            expected: CredentialScope::ControlPlane,
            actual: CredentialScope::BreakGlass,
        })
    );
    assert_eq!(
        authorize_boundary_scope(BoundaryKind::ControlPlaneConfigurationPublish, &principal,),
        Err(BoundaryAuthorizationError::CredentialScopeMismatch {
            boundary: BoundaryKind::ControlPlaneConfigurationPublish,
            expected: CredentialScope::ControlPlane,
            actual: CredentialScope::BreakGlass,
        })
    );
    assert_eq!(
        authorize_boundary_scope(BoundaryKind::ControlPlaneBillingRead, &principal),
        Err(BoundaryAuthorizationError::CredentialScopeMismatch {
            boundary: BoundaryKind::ControlPlaneBillingRead,
            expected: CredentialScope::ControlPlane,
            actual: CredentialScope::BreakGlass,
        })
    );
    assert_eq!(
        authorize_boundary_scope(BoundaryKind::ControlPlaneAuditExport, &principal),
        Err(BoundaryAuthorizationError::CredentialScopeMismatch {
            boundary: BoundaryKind::ControlPlaneAuditExport,
            expected: CredentialScope::ControlPlane,
            actual: CredentialScope::BreakGlass,
        })
    );
    assert_eq!(
        authorize_boundary_scope(BoundaryKind::ControlPlaneAuditRetentionPurge, &principal,),
        Err(BoundaryAuthorizationError::CredentialScopeMismatch {
            boundary: BoundaryKind::ControlPlaneAuditRetentionPurge,
            expected: CredentialScope::ControlPlane,
            actual: CredentialScope::BreakGlass,
        })
    );
}

#[test]
fn break_glass_admin_requires_break_glass_principal_kind() {
    let tenant_id = TenantId::new();
    let service_account = principal(Some(tenant_id), Role::Admin, CredentialScope::BreakGlass);
    let user = principal_with_kind(
        Some(tenant_id),
        PrincipalKind::User,
        Role::Admin,
        CredentialScope::BreakGlass,
    );

    for principal in [service_account, user] {
        assert_eq!(
            authorize_boundary_scope(BoundaryKind::BreakGlassAdmin, &principal),
            Err(BoundaryAuthorizationError::PrincipalKindMismatch {
                boundary: BoundaryKind::BreakGlassAdmin,
                expected: PrincipalKind::BreakGlass,
                actual: principal.kind,
            })
        );
    }
}

#[test]
fn horizontal_and_vertical_privilege_escalation_are_rejected() {
    let tenant_id = TenantId::new();
    let wrong_tenant = TenantId::new();
    let data_plane_viewer = principal(Some(tenant_id), Role::Viewer, CredentialScope::DataPlane);
    let data_plane_operator = principal(
        Some(wrong_tenant),
        Role::Operator,
        CredentialScope::DataPlane,
    );

    assert!(matches!(
        authorize_boundary_resource(
            BoundaryKind::DataPlaneInference,
            &data_plane_viewer,
            &TenantResource { tenant_id },
        ),
        Err(BoundaryAuthorizationError::InsufficientRole { .. })
    ));
    assert!(matches!(
        authorize_boundary_resource(
            BoundaryKind::DataPlaneInference,
            &data_plane_operator,
            &TenantResource { tenant_id },
        ),
        Err(BoundaryAuthorizationError::Tenant(_))
    ));
}

#[test]
fn viewer_cannot_call_control_plane_lifecycle_boundaries() {
    let tenant_id = TenantId::new();
    let viewer = principal(Some(tenant_id), Role::Viewer, CredentialScope::ControlPlane);

    for (boundary, _, _) in lifecycle_admin_boundaries() {
        assert!(matches!(
            authorize_boundary_resource(boundary, &viewer, &TenantResource { tenant_id }),
            Err(BoundaryAuthorizationError::InsufficientRole { .. })
        ));
    }
}

#[test]
fn viewer_cannot_call_secret_or_budget_admin_boundaries() {
    let tenant_id = TenantId::new();
    let viewer = principal(Some(tenant_id), Role::Viewer, CredentialScope::ControlPlane);

    for (boundary, _, _) in secret_budget_admin_boundaries() {
        assert!(matches!(
            authorize_boundary_resource(boundary, &viewer, &TenantResource { tenant_id }),
            Err(BoundaryAuthorizationError::InsufficientRole { .. })
        ));
    }
}

#[test]
fn viewer_cannot_publish_policy_or_configuration() {
    let tenant_id = TenantId::new();
    let viewer = principal(Some(tenant_id), Role::Viewer, CredentialScope::ControlPlane);

    assert!(matches!(
        authorize_boundary_resource(
            BoundaryKind::ControlPlanePolicyPublish,
            &viewer,
            &TenantResource { tenant_id },
        ),
        Err(BoundaryAuthorizationError::InsufficientRole { .. })
    ));
    assert!(matches!(
        authorize_boundary_resource(
            BoundaryKind::ControlPlaneConfigurationPublish,
            &viewer,
            &TenantResource { tenant_id },
        ),
        Err(BoundaryAuthorizationError::InsufficientRole { .. })
    ));
}

#[test]
fn viewer_cannot_export_or_purge_audit_logs() {
    let tenant_id = TenantId::new();
    let viewer = principal(Some(tenant_id), Role::Viewer, CredentialScope::ControlPlane);

    assert!(matches!(
        authorize_boundary_resource(
            BoundaryKind::ControlPlaneAuditExport,
            &viewer,
            &TenantResource { tenant_id },
        ),
        Err(BoundaryAuthorizationError::InsufficientRole { .. })
    ));
    assert!(matches!(
        authorize_boundary_resource(
            BoundaryKind::ControlPlaneAuditRetentionPurge,
            &viewer,
            &TenantResource { tenant_id },
        ),
        Err(BoundaryAuthorizationError::InsufficientRole { .. })
    ));
}

#[test]
fn authorization_error_responses_are_stable_and_redacted() {
    let tenant_id = TenantId::new();
    let wrong_tenant = TenantId::new();

    let scope_error = BoundaryAuthorizationError::CredentialScopeMismatch {
        boundary: BoundaryKind::DataPlaneInference,
        expected: CredentialScope::DataPlane,
        actual: CredentialScope::ControlPlane,
    };
    assert_eq!(scope_error.to_string(), "authorization request is denied");
    assert!(!scope_error.to_string().contains("DataPlaneInference"));
    assert!(!scope_error.to_string().contains("ControlPlane"));
    assert!(!scope_error.to_string().contains("DataPlane"));
    let scope_response = plan_authorization_error_response(&scope_error);
    assert_eq!(scope_response.status, AuthorizationErrorStatus::Forbidden);
    assert_eq!(scope_response.code, "credential_scope_not_allowed");
    assert_eq!(
        scope_response.message,
        "credential scope is not allowed for this operation"
    );
    assert!(!scope_response.message.contains("DataPlaneInference"));
    assert!(!scope_response.message.contains("ControlPlane"));

    let role_error = BoundaryAuthorizationError::InsufficientRole {
        boundary: BoundaryKind::ControlPlaneAdmin,
        required: Role::Admin,
        actual: Role::Viewer,
    };
    assert_eq!(role_error.to_string(), "authorization request is denied");
    assert!(!role_error.to_string().contains("ControlPlaneAdmin"));
    assert!(!role_error.to_string().contains("Admin"));
    assert!(!role_error.to_string().contains("Viewer"));
    let role_response = plan_authorization_error_response(&role_error);
    assert_eq!(role_response.status, AuthorizationErrorStatus::Forbidden);
    assert_eq!(role_response.code, "role_not_authorized");
    assert_eq!(
        role_response.message,
        "principal role is not authorized for this operation"
    );
    assert!(!role_response.message.contains("Admin"));
    assert!(!role_response.message.contains("Viewer"));

    let kind_error = BoundaryAuthorizationError::PrincipalKindMismatch {
        boundary: BoundaryKind::BreakGlassAdmin,
        expected: PrincipalKind::BreakGlass,
        actual: PrincipalKind::ServiceAccount,
    };
    assert_eq!(kind_error.to_string(), "authorization request is denied");
    assert!(!kind_error.to_string().contains("BreakGlassAdmin"));
    assert!(!kind_error.to_string().contains("ServiceAccount"));
    let kind_response = plan_authorization_error_response(&kind_error);
    assert_eq!(kind_response.status, AuthorizationErrorStatus::Forbidden);
    assert_eq!(kind_response.code, "principal_kind_not_allowed");
    assert_eq!(
        kind_response.message,
        "principal kind is not allowed for this operation"
    );
    assert!(!kind_response.message.contains("BreakGlassAdmin"));
    assert!(!kind_response.message.contains("ServiceAccount"));

    let tenant_error = authorize_boundary_resource(
        BoundaryKind::DataPlaneInference,
        &principal(
            Some(wrong_tenant),
            Role::Operator,
            CredentialScope::DataPlane,
        ),
        &TenantResource { tenant_id },
    )
    .unwrap_err();
    assert_eq!(tenant_error.to_string(), "authorization request is denied");
    let tenant_response = plan_authorization_error_response(&tenant_error);
    assert_eq!(tenant_response.status, AuthorizationErrorStatus::Forbidden);
    assert_eq!(tenant_response.code, "tenant_access_denied");
    assert_eq!(tenant_response.message, "tenant access is denied");
    assert!(!tenant_response.message.contains(&tenant_id.to_string()));
    assert!(!tenant_response.message.contains(&wrong_tenant.to_string()));
}
