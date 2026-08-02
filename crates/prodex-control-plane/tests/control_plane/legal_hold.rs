use super::{principal, request};
use prodex_control_plane::{
    ControlPlaneDecision, ControlPlaneOperation, decide_control_plane_action,
};
use prodex_domain::{CredentialScope, ResourceAction, ResourceKind, Role, TenantId};

#[test]
fn audit_legal_hold_delete_is_distinct_from_retention_purge() {
    let tenant_id = TenantId::new();
    let decision = decide_control_plane_action(request(
        tenant_id,
        principal(tenant_id, Role::Admin, CredentialScope::ControlPlane),
        ControlPlaneOperation::AuditLegalHoldDelete,
        ResourceKind::AuditLog,
    ));
    let ControlPlaneDecision::Authorized(plan) = decision else {
        panic!("expected authorized audit legal-hold deletion");
    };
    assert_eq!(plan.requirement.action, ResourceAction::Delete);
    assert_eq!(
        plan.audit_event.action.as_str(),
        "control_plane.audit.legal_hold.delete"
    );
    assert_ne!(
        plan.audit_event.action,
        ControlPlaneOperation::AuditRetentionPurge.audit_action()
    );
}

#[test]
fn legal_hold_operations_preserve_idempotency_requirements() {
    assert!(!ControlPlaneOperation::AuditLegalHoldRead.requires_idempotency());
    assert!(ControlPlaneOperation::AuditLegalHoldUpsert.requires_idempotency());
    assert!(ControlPlaneOperation::AuditLegalHoldDelete.requires_idempotency());
}

#[test]
fn legal_hold_operations_have_explicit_lifecycle_requirements() {
    for (operation, action, role) in [
        (
            ControlPlaneOperation::AuditLegalHoldRead,
            ResourceAction::Read,
            Role::Viewer,
        ),
        (
            ControlPlaneOperation::AuditLegalHoldUpsert,
            ResourceAction::Update,
            Role::Admin,
        ),
        (
            ControlPlaneOperation::AuditLegalHoldDelete,
            ResourceAction::Delete,
            Role::Admin,
        ),
    ] {
        let requirement = operation.requirement();
        assert_eq!(requirement.resource, ResourceKind::AuditLog);
        assert_eq!(requirement.action, action);
        assert_eq!(requirement.required_role, role);
    }
}
