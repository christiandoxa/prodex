use prodex_control_plane::{
    ControlPlaneActionRequest, ControlPlaneAuditWriteMode, ControlPlaneAuthorizationError,
    ControlPlaneAuthorizationErrorStatus, ControlPlaneDecision, ControlPlaneOperation,
    ControlPlaneResourceRef, decide_control_plane_action,
    plan_control_plane_authorization_error_response,
};
use prodex_domain::{
    AuditOutcome, CredentialScope, IdempotencyKey, Principal, PrincipalId, PrincipalKind,
    ResourceAction, ResourceKind, Role, TenantId,
};

#[path = "control_plane/break_glass.rs"]
mod break_glass;
#[path = "control_plane/configuration_publication.rs"]
mod configuration_publication;

fn principal(tenant_id: TenantId, role: Role, scope: CredentialScope) -> Principal {
    Principal::new(
        PrincipalId::new(),
        Some(tenant_id),
        PrincipalKind::User,
        role,
        scope,
    )
}

fn request(
    tenant_id: TenantId,
    principal: Principal,
    operation: ControlPlaneOperation,
    kind: ResourceKind,
) -> ControlPlaneActionRequest {
    ControlPlaneActionRequest {
        principal,
        operation,
        resource: ControlPlaneResourceRef::new(tenant_id, kind, Some("resource-1")),
        occurred_at_unix_ms: 10_000,
    }
}

#[test]
fn control_plane_mutation_requires_control_plane_admin_and_emits_success_audit() {
    let tenant_id = TenantId::new();
    let decision = decide_control_plane_action(request(
        tenant_id,
        principal(tenant_id, Role::Admin, CredentialScope::ControlPlane),
        ControlPlaneOperation::PolicyPublish,
        ResourceKind::Policy,
    ));
    let ControlPlaneDecision::Authorized(plan) = decision else {
        panic!("expected authorized decision");
    };
    assert_eq!(plan.tenant.tenant_id, tenant_id);
    assert_eq!(plan.requirement.resource, ResourceKind::Policy);
    assert_eq!(plan.audit_event.outcome, AuditOutcome::Success);
    assert_eq!(plan.audit_event.tenant_id, tenant_id);
    assert_eq!(
        plan.audit_write.mode,
        ControlPlaneAuditWriteMode::AppendOnlyHashChain
    );
    assert_eq!(plan.audit_write.event, plan.audit_event);
    assert_eq!(plan.audit_write.tenant_partition_key, tenant_id);
    assert_eq!(
        plan.audit_event.action.as_str(),
        "control_plane.policy.publish"
    );
}

#[test]
fn data_plane_credential_cannot_call_control_plane_and_denial_is_audited() {
    let tenant_id = TenantId::new();
    let decision = decide_control_plane_action(request(
        tenant_id,
        principal(tenant_id, Role::Admin, CredentialScope::DataPlane),
        ControlPlaneOperation::VirtualKeyCreate,
        ResourceKind::VirtualKey,
    ));
    let ControlPlaneDecision::Denied {
        error,
        audit_write,
        audit_event,
    } = decision
    else {
        panic!("expected denied decision");
    };
    assert_eq!(
        error,
        ControlPlaneAuthorizationError::CredentialScopeMismatch {
            expected: CredentialScope::ControlPlane,
            actual: CredentialScope::DataPlane,
        }
    );
    assert_eq!(audit_event.outcome, AuditOutcome::Denied);
    assert_eq!(
        audit_write.mode,
        ControlPlaneAuditWriteMode::AppendOnlyHashChain
    );
    assert_eq!(audit_write.event, audit_event);
    assert_eq!(audit_write.tenant_partition_key, tenant_id);
    assert_eq!(
        audit_event.reason_code.as_deref(),
        Some("credential_scope_mismatch")
    );
}

#[test]
fn viewer_cannot_perform_vertical_privilege_escalation() {
    let tenant_id = TenantId::new();
    let decision = decide_control_plane_action(request(
        tenant_id,
        principal(tenant_id, Role::Viewer, CredentialScope::ControlPlane),
        ControlPlaneOperation::BudgetUpdate,
        ResourceKind::Budget,
    ));
    assert!(matches!(
        decision,
        ControlPlaneDecision::Denied {
            error: ControlPlaneAuthorizationError::InsufficientRole { .. },
            ..
        }
    ));
}

#[test]
fn cross_tenant_control_plane_access_is_denied() {
    let tenant_id = TenantId::new();
    let resource_tenant_id = TenantId::new();
    let decision = decide_control_plane_action(request(
        resource_tenant_id,
        principal(tenant_id, Role::Admin, CredentialScope::ControlPlane),
        ControlPlaneOperation::AuditExport,
        ResourceKind::AuditLog,
    ));
    assert!(matches!(
        decision,
        ControlPlaneDecision::Denied {
            error: ControlPlaneAuthorizationError::Tenant(_),
            ..
        }
    ));
}

#[test]
fn audit_retention_purge_requires_admin_delete_and_emits_audit() {
    let tenant_id = TenantId::new();
    let decision = decide_control_plane_action(request(
        tenant_id,
        principal(tenant_id, Role::Admin, CredentialScope::ControlPlane),
        ControlPlaneOperation::AuditRetentionPurge,
        ResourceKind::AuditLog,
    ));
    let ControlPlaneDecision::Authorized(plan) = decision else {
        panic!("expected authorized audit retention purge");
    };
    assert_eq!(plan.tenant.tenant_id, tenant_id);
    assert_eq!(plan.requirement.resource, ResourceKind::AuditLog);
    assert_eq!(
        plan.requirement.action,
        prodex_domain::ResourceAction::Delete
    );
    assert_eq!(plan.requirement.required_role, Role::Admin);
    assert_eq!(plan.audit_event.outcome, AuditOutcome::Success);
    assert_eq!(
        plan.audit_event.action.as_str(),
        "control_plane.audit.retention_purge"
    );
    assert_eq!(plan.audit_write.tenant_partition_key, tenant_id);
}

#[test]
fn mutating_control_plane_operations_require_idempotency() {
    assert!(ControlPlaneOperation::AuditRetentionPurge.requires_idempotency());
    assert!(ControlPlaneOperation::PolicyPublish.requires_idempotency());
    assert!(ControlPlaneOperation::ConfigurationPublish.requires_idempotency());
    assert!(ControlPlaneOperation::ScimUserCreate.requires_idempotency());
    assert!(ControlPlaneOperation::ScimUserUpdate.requires_idempotency());
    assert!(ControlPlaneOperation::ScimUserDelete.requires_idempotency());
    assert!(ControlPlaneOperation::VirtualKeyUpdate.requires_idempotency());
    assert!(ControlPlaneOperation::VirtualKeyDelete.requires_idempotency());
    assert!(ControlPlaneOperation::RoleBindingGrant.requires_idempotency());
    assert!(ControlPlaneOperation::RoleBindingRevoke.requires_idempotency());
    assert!(!ControlPlaneOperation::GatewayAdminRead.requires_idempotency());
    assert!(!ControlPlaneOperation::RouteExplain.requires_idempotency());
    assert!(!ControlPlaneOperation::ScimUserRead.requires_idempotency());
    assert!(!ControlPlaneOperation::VirtualKeyRead.requires_idempotency());
    assert!(!ControlPlaneOperation::BillingRead.requires_idempotency());
    assert!(!ControlPlaneOperation::AuditExport.requires_idempotency());
}

#[test]
fn all_control_plane_operations_require_immutable_audit_on_success_and_denial() {
    assert_eq!(ControlPlaneOperation::ALL.len(), 24);
    for operation in ControlPlaneOperation::ALL {
        let audit = operation.audit_requirement();
        assert_eq!(audit.operation, operation);
        assert_eq!(audit.action, operation.audit_action());
        assert_eq!(
            audit.write_mode,
            ControlPlaneAuditWriteMode::AppendOnlyHashChain
        );
        assert!(operation.requires_immutable_audit());
        assert!(audit.success_required);
        assert!(audit.denial_required);
    }
}

#[test]
fn all_control_plane_operations_have_explicit_lifecycle_requirements() {
    let expected = [
        (
            ControlPlaneOperation::GatewayAdminRead,
            ResourceKind::Configuration,
            ResourceAction::Read,
            Role::Viewer,
        ),
        (
            ControlPlaneOperation::RouteExplain,
            ResourceKind::Configuration,
            ResourceAction::Read,
            Role::Viewer,
        ),
        (
            ControlPlaneOperation::TenantCreate,
            ResourceKind::Tenant,
            ResourceAction::Create,
            Role::Admin,
        ),
        (
            ControlPlaneOperation::TenantUpdate,
            ResourceKind::Tenant,
            ResourceAction::Update,
            Role::Admin,
        ),
        (
            ControlPlaneOperation::UserInvite,
            ResourceKind::User,
            ResourceAction::Create,
            Role::Admin,
        ),
        (
            ControlPlaneOperation::ScimUserRead,
            ResourceKind::User,
            ResourceAction::Read,
            Role::Viewer,
        ),
        (
            ControlPlaneOperation::ScimUserCreate,
            ResourceKind::User,
            ResourceAction::Create,
            Role::Admin,
        ),
        (
            ControlPlaneOperation::ScimUserUpdate,
            ResourceKind::User,
            ResourceAction::Update,
            Role::Admin,
        ),
        (
            ControlPlaneOperation::ScimUserDelete,
            ResourceKind::User,
            ResourceAction::Delete,
            Role::Admin,
        ),
        (
            ControlPlaneOperation::RoleBindingGrant,
            ResourceKind::RoleBinding,
            ResourceAction::Create,
            Role::Admin,
        ),
        (
            ControlPlaneOperation::RoleBindingRevoke,
            ResourceKind::RoleBinding,
            ResourceAction::Delete,
            Role::Admin,
        ),
        (
            ControlPlaneOperation::ServiceIdentityCreate,
            ResourceKind::ServiceIdentity,
            ResourceAction::Create,
            Role::Admin,
        ),
        (
            ControlPlaneOperation::VirtualKeyRead,
            ResourceKind::VirtualKey,
            ResourceAction::Read,
            Role::Viewer,
        ),
        (
            ControlPlaneOperation::VirtualKeyCreate,
            ResourceKind::VirtualKey,
            ResourceAction::Create,
            Role::Admin,
        ),
        (
            ControlPlaneOperation::VirtualKeyUpdate,
            ResourceKind::VirtualKey,
            ResourceAction::Update,
            Role::Admin,
        ),
        (
            ControlPlaneOperation::VirtualKeyDelete,
            ResourceKind::VirtualKey,
            ResourceAction::Delete,
            Role::Admin,
        ),
        (
            ControlPlaneOperation::VirtualKeyRotateSecret,
            ResourceKind::VirtualKey,
            ResourceAction::RotateSecret,
            Role::Admin,
        ),
        (
            ControlPlaneOperation::PolicyPublish,
            ResourceKind::Policy,
            ResourceAction::PublishRevision,
            Role::Admin,
        ),
        (
            ControlPlaneOperation::ProviderCredentialRotate,
            ResourceKind::ProviderCredential,
            ResourceAction::RotateSecret,
            Role::Admin,
        ),
        (
            ControlPlaneOperation::BudgetUpdate,
            ResourceKind::Budget,
            ResourceAction::Update,
            Role::Admin,
        ),
        (
            ControlPlaneOperation::BillingRead,
            ResourceKind::Billing,
            ResourceAction::Read,
            Role::Viewer,
        ),
        (
            ControlPlaneOperation::AuditExport,
            ResourceKind::AuditLog,
            ResourceAction::Export,
            Role::Admin,
        ),
        (
            ControlPlaneOperation::AuditRetentionPurge,
            ResourceKind::AuditLog,
            ResourceAction::Delete,
            Role::Admin,
        ),
        (
            ControlPlaneOperation::ConfigurationPublish,
            ResourceKind::Configuration,
            ResourceAction::PublishRevision,
            Role::Admin,
        ),
    ];
    assert_eq!(expected.len(), ControlPlaneOperation::ALL.len());
    for (operation, resource, action, role) in expected {
        let requirement = operation.requirement();
        assert_eq!(requirement.resource, resource);
        assert_eq!(requirement.action, action);
        assert_eq!(requirement.required_role, role);
    }
}

#[test]
fn role_binding_lifecycle_operations_are_admin_mutations_and_audited() {
    let tenant_id = TenantId::new();
    let operations = [
        (
            ControlPlaneOperation::RoleBindingGrant,
            ResourceAction::Create,
            "control_plane.role_binding.grant",
        ),
        (
            ControlPlaneOperation::RoleBindingRevoke,
            ResourceAction::Delete,
            "control_plane.role_binding.revoke",
        ),
    ];

    for (operation, action, audit_action) in operations {
        let decision = decide_control_plane_action(request(
            tenant_id,
            principal(tenant_id, Role::Admin, CredentialScope::ControlPlane),
            operation,
            ResourceKind::RoleBinding,
        ));

        let ControlPlaneDecision::Authorized(plan) = decision else {
            panic!("expected authorized role-binding lifecycle operation for {operation:?}");
        };
        assert_eq!(plan.requirement.resource, ResourceKind::RoleBinding);
        assert_eq!(plan.requirement.action, action);
        assert_eq!(plan.requirement.required_role, Role::Admin);
        assert_eq!(plan.audit_event.outcome, AuditOutcome::Success);
        assert_eq!(plan.audit_event.action.as_str(), audit_action);
        assert_eq!(plan.audit_write.tenant_partition_key, tenant_id);
        assert!(operation.requires_idempotency());
    }
}

#[test]
fn viewer_cannot_grant_or_revoke_role_bindings() {
    let tenant_id = TenantId::new();

    for operation in [
        ControlPlaneOperation::RoleBindingGrant,
        ControlPlaneOperation::RoleBindingRevoke,
    ] {
        let decision = decide_control_plane_action(request(
            tenant_id,
            principal(tenant_id, Role::Viewer, CredentialScope::ControlPlane),
            operation,
            ResourceKind::RoleBinding,
        ));

        assert!(matches!(
            decision,
            ControlPlaneDecision::Denied {
                error: ControlPlaneAuthorizationError::InsufficientRole { .. },
                ..
            }
        ));
    }
}

#[test]
fn scim_user_lifecycle_operations_are_admin_mutations_and_audited() {
    let tenant_id = TenantId::new();
    let operations = [
        (
            ControlPlaneOperation::ScimUserCreate,
            ResourceAction::Create,
            "control_plane.scim_user.create",
        ),
        (
            ControlPlaneOperation::ScimUserUpdate,
            ResourceAction::Update,
            "control_plane.scim_user.update",
        ),
        (
            ControlPlaneOperation::ScimUserDelete,
            ResourceAction::Delete,
            "control_plane.scim_user.delete",
        ),
    ];

    for (operation, action, audit_action) in operations {
        let decision = decide_control_plane_action(request(
            tenant_id,
            principal(tenant_id, Role::Admin, CredentialScope::ControlPlane),
            operation,
            ResourceKind::User,
        ));

        let ControlPlaneDecision::Authorized(plan) = decision else {
            panic!("expected authorized SCIM user lifecycle operation for {operation:?}");
        };
        assert_eq!(plan.requirement.resource, ResourceKind::User);
        assert_eq!(plan.requirement.action, action);
        assert_eq!(plan.requirement.required_role, Role::Admin);
        assert_eq!(plan.audit_event.outcome, AuditOutcome::Success);
        assert_eq!(plan.audit_event.action.as_str(), audit_action);
        assert_eq!(plan.audit_write.tenant_partition_key, tenant_id);
        assert!(operation.requires_idempotency());
    }
}

#[test]
fn lifecycle_resource_kind_mismatch_is_denied_and_audited_for_every_operation() {
    let tenant_id = TenantId::new();

    for operation in ControlPlaneOperation::ALL {
        let expected = operation.requirement().resource;
        let wrong_kind = if expected == ResourceKind::Tenant {
            ResourceKind::Budget
        } else {
            ResourceKind::Tenant
        };
        let decision = decide_control_plane_action(request(
            tenant_id,
            principal(tenant_id, Role::Admin, CredentialScope::ControlPlane),
            operation,
            wrong_kind,
        ));

        let ControlPlaneDecision::Denied {
            error,
            audit_event,
            audit_write,
        } = decision
        else {
            panic!("expected resource-kind mismatch denial for {operation:?}");
        };
        assert_eq!(
            error,
            ControlPlaneAuthorizationError::ResourceKindMismatch {
                operation,
                expected,
                actual: wrong_kind,
            }
        );
        assert_eq!(audit_event.outcome, AuditOutcome::Denied);
        assert_eq!(
            audit_event.resource.kind,
            format!("{wrong_kind:?}").to_lowercase()
        );
        assert!(!audit_event.resource.kind.contains(char::is_uppercase));
        assert_eq!(
            audit_event.reason_code.as_deref(),
            Some("resource_kind_mismatch")
        );
        assert_eq!(audit_write.event, audit_event);
        assert_eq!(
            audit_write.mode,
            ControlPlaneAuditWriteMode::AppendOnlyHashChain
        );
    }
}

#[test]
fn audit_retention_purge_idempotency_operation_is_tenant_scoped() {
    let tenant_id = TenantId::new();
    let key = IdempotencyKey::new("retention-purge-1").unwrap();
    let action = request(
        tenant_id,
        principal(tenant_id, Role::Admin, CredentialScope::ControlPlane),
        ControlPlaneOperation::AuditRetentionPurge,
        ResourceKind::AuditLog,
    );

    let operation = action
        .idempotent_operation(key.clone(), "sha256:retention-purge")
        .unwrap()
        .unwrap();

    assert_eq!(operation.tenant_id, tenant_id);
    assert_eq!(operation.key, key);
    assert_eq!(operation.request_fingerprint, "sha256:retention-purge");
}

#[test]
fn read_only_control_plane_operations_do_not_create_idempotency_operation() {
    let tenant_id = TenantId::new();
    let action = request(
        tenant_id,
        principal(tenant_id, Role::Admin, CredentialScope::ControlPlane),
        ControlPlaneOperation::AuditExport,
        ResourceKind::AuditLog,
    );

    assert_eq!(
        action.idempotent_operation(
            IdempotencyKey::new("audit-export-1").unwrap(),
            "sha256:audit-export"
        ),
        Ok(None)
    );
}

#[test]
fn viewer_cannot_purge_audit_retention() {
    let tenant_id = TenantId::new();
    let decision = decide_control_plane_action(request(
        tenant_id,
        principal(tenant_id, Role::Viewer, CredentialScope::ControlPlane),
        ControlPlaneOperation::AuditRetentionPurge,
        ResourceKind::AuditLog,
    ));

    assert!(matches!(
        decision,
        ControlPlaneDecision::Denied {
            error: ControlPlaneAuthorizationError::InsufficientRole { .. },
            ..
        }
    ));
}

#[test]
fn control_plane_authorization_error_responses_are_stable_and_redacted() {
    let tenant_id = TenantId::new();
    let resource_tenant_id = TenantId::new();

    let scope = ControlPlaneAuthorizationError::CredentialScopeMismatch {
        expected: CredentialScope::ControlPlane,
        actual: CredentialScope::DataPlane,
    };
    assert_eq!(
        scope.to_string(),
        "control-plane authorization request is denied"
    );
    assert!(!scope.to_string().contains("ControlPlane"));
    assert!(!scope.to_string().contains("DataPlane"));
    let scope_response = plan_control_plane_authorization_error_response(&scope);
    assert_eq!(
        scope_response.status,
        ControlPlaneAuthorizationErrorStatus::Forbidden
    );
    assert_eq!(scope_response.code, "credential_scope_not_allowed");
    assert_eq!(
        scope_response.message,
        "credential scope is not allowed for this control-plane operation"
    );
    assert!(!scope_response.message.contains("ControlPlane"));
    assert!(!scope_response.message.contains("DataPlane"));

    let role = ControlPlaneAuthorizationError::InsufficientRole {
        required: Role::Admin,
        actual: Role::Viewer,
    };
    assert_eq!(
        role.to_string(),
        "control-plane authorization request is denied"
    );
    assert!(!role.to_string().contains("Admin"));
    assert!(!role.to_string().contains("Viewer"));
    let role_response = plan_control_plane_authorization_error_response(&role);
    assert_eq!(role_response.code, "role_not_authorized");
    assert!(!role_response.message.contains("Admin"));
    assert!(!role_response.message.contains("Viewer"));

    let tenant = decide_control_plane_action(request(
        resource_tenant_id,
        principal(tenant_id, Role::Admin, CredentialScope::ControlPlane),
        ControlPlaneOperation::AuditExport,
        ResourceKind::AuditLog,
    ));
    let ControlPlaneDecision::Denied { error, .. } = tenant else {
        panic!("expected tenant denial");
    };
    assert_eq!(
        error.to_string(),
        "control-plane authorization request is denied"
    );
    let tenant_response = plan_control_plane_authorization_error_response(&error);
    assert_eq!(tenant_response.code, "tenant_access_denied");
    assert_eq!(tenant_response.message, "tenant access is denied");
    assert!(!tenant_response.message.contains(&tenant_id.to_string()));
    assert!(
        !tenant_response
            .message
            .contains(&resource_tenant_id.to_string())
    );

    let resource = ControlPlaneAuthorizationError::ResourceKindMismatch {
        operation: ControlPlaneOperation::VirtualKeyCreate,
        expected: ResourceKind::VirtualKey,
        actual: ResourceKind::Budget,
    };
    assert_eq!(
        resource.to_string(),
        "control-plane authorization request is denied"
    );
    assert!(!resource.to_string().contains("VirtualKeyCreate"));
    assert!(!resource.to_string().contains("VirtualKey"));
    assert!(!resource.to_string().contains("Budget"));
    let resource_response = plan_control_plane_authorization_error_response(&resource);
    assert_eq!(resource_response.code, "resource_not_authorized");
    assert!(!resource_response.message.contains("VirtualKeyCreate"));
    assert!(!resource_response.message.contains("VirtualKey"));
    assert!(!resource_response.message.contains("Budget"));

    let break_glass = ControlPlaneAuthorizationError::BreakGlassExpired {
        now_unix_ms: 10_000,
        expires_at_unix_ms: 9_999,
    };
    assert_eq!(
        break_glass.to_string(),
        "control-plane authorization request is denied"
    );
    assert!(!break_glass.to_string().contains("10000"));
    assert!(!break_glass.to_string().contains("9999"));
    let break_glass_response = plan_control_plane_authorization_error_response(&break_glass);
    assert_eq!(break_glass_response.code, "break_glass_not_authorized");
    assert_eq!(
        break_glass_response.message,
        "break-glass access is not authorized"
    );
    assert!(!break_glass_response.message.contains("9999"));
    assert_eq!(
        plan_control_plane_authorization_error_response(
            &ControlPlaneAuthorizationError::BreakGlassReasonMissing
        ),
        break_glass_response
    );
    assert_eq!(
        plan_control_plane_authorization_error_response(
            &ControlPlaneAuthorizationError::BreakGlassReasonMalformed
        ),
        break_glass_response
    );
}
