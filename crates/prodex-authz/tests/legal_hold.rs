use prodex_authz::{
    BoundaryAuthorizationError, BoundaryKind, authorize_boundary_resource,
    control_plane_boundary_for_requirement,
};
use prodex_domain::{
    AuthorizationRequirement, CredentialScope, Principal, PrincipalId, PrincipalKind,
    ResourceAction, ResourceKind, Role, TenantId, TenantScopedResource,
};

struct TenantResource(TenantId);

impl TenantScopedResource for TenantResource {
    fn tenant_id(&self) -> TenantId {
        self.0
    }
}

fn principal(tenant_id: TenantId, role: Role) -> Principal {
    Principal::new(
        PrincipalId::new(),
        Some(tenant_id),
        PrincipalKind::ServiceAccount,
        role,
        CredentialScope::ControlPlane,
    )
}

#[test]
fn legal_hold_boundaries_preserve_viewer_read_and_admin_upsert() {
    let cases = [
        (
            BoundaryKind::ControlPlaneAuditLegalHoldRead,
            ResourceAction::Read,
            Role::Viewer,
        ),
        (
            BoundaryKind::ControlPlaneAuditLegalHoldUpsert,
            ResourceAction::Update,
            Role::Admin,
        ),
    ];
    for (boundary, action, role) in cases {
        let requirement = boundary.requirement();
        let expected = AuthorizationRequirement::new(
            ResourceKind::AuditLog,
            action,
            CredentialScope::ControlPlane,
            role,
        );
        assert_eq!(
            control_plane_boundary_for_requirement(expected),
            Some(boundary)
        );
        assert_eq!(requirement, expected);
    }

    let tenant_id = TenantId::new();
    let resource = TenantResource(tenant_id);
    let viewer = principal(tenant_id, Role::Viewer);
    let admin = principal(tenant_id, Role::Admin);
    assert!(
        authorize_boundary_resource(
            BoundaryKind::ControlPlaneAuditLegalHoldRead,
            &viewer,
            &resource,
        )
        .is_ok()
    );
    assert!(matches!(
        authorize_boundary_resource(
            BoundaryKind::ControlPlaneAuditLegalHoldUpsert,
            &viewer,
            &resource,
        ),
        Err(BoundaryAuthorizationError::InsufficientRole { .. })
    ));
    assert!(
        authorize_boundary_resource(
            BoundaryKind::ControlPlaneAuditLegalHoldUpsert,
            &admin,
            &resource,
        )
        .is_ok()
    );
}
