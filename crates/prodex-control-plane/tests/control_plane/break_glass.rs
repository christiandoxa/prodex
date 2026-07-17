use super::{principal, request};
use prodex_control_plane::{
    BreakGlassAuthorization, ControlPlaneAuthorizationError, ControlPlaneDecision,
    ControlPlaneOperation, decide_break_glass_action, decide_control_plane_action,
};
use prodex_domain::{
    CredentialScope, Principal, PrincipalId, PrincipalKind, ResourceKind, Role, TenantId,
};

fn break_glass_principal(tenant_id: TenantId) -> Principal {
    Principal::new(
        PrincipalId::new(),
        Some(tenant_id),
        PrincipalKind::BreakGlass,
        Role::Admin,
        CredentialScope::BreakGlass,
    )
}

#[test]
fn break_glass_requires_separate_scope_reason_expiry_and_audit() {
    let tenant_id = TenantId::new();
    let normal_control_plane = request(
        tenant_id,
        break_glass_principal(tenant_id),
        ControlPlaneOperation::ProviderCredentialRotate,
        ResourceKind::ProviderCredential,
    );

    assert!(matches!(
        decide_control_plane_action(normal_control_plane.clone()),
        ControlPlaneDecision::Denied {
            error: ControlPlaneAuthorizationError::CredentialScopeMismatch { .. },
            ..
        }
    ));
    assert!(matches!(
        decide_break_glass_action(
            normal_control_plane,
            BreakGlassAuthorization {
                reason: "incident response".to_string(),
                expires_at_unix_ms: 11_000,
            },
        ),
        ControlPlaneDecision::Authorized(_)
    ));

    let wrong_principal_kind = decide_break_glass_action(
        request(
            tenant_id,
            principal(tenant_id, Role::Admin, CredentialScope::BreakGlass),
            ControlPlaneOperation::ProviderCredentialRotate,
            ResourceKind::ProviderCredential,
        ),
        BreakGlassAuthorization {
            reason: "incident response".to_string(),
            expires_at_unix_ms: 11_000,
        },
    );
    assert!(matches!(
        wrong_principal_kind,
        ControlPlaneDecision::Denied {
            error: ControlPlaneAuthorizationError::BreakGlassPrincipalKindMismatch { .. },
            ..
        }
    ));

    let expired = decide_break_glass_action(
        request(
            tenant_id,
            break_glass_principal(tenant_id),
            ControlPlaneOperation::ProviderCredentialRotate,
            ResourceKind::ProviderCredential,
        ),
        BreakGlassAuthorization {
            reason: "incident response".to_string(),
            expires_at_unix_ms: 10_000,
        },
    );
    assert!(matches!(
        expired,
        ControlPlaneDecision::Denied {
            error: ControlPlaneAuthorizationError::BreakGlassExpired { .. },
            ..
        }
    ));

    for reason in ["", "   ", "incident\nresponse", &"x".repeat(513)] {
        let denied = decide_break_glass_action(
            request(
                tenant_id,
                break_glass_principal(tenant_id),
                ControlPlaneOperation::ProviderCredentialRotate,
                ResourceKind::ProviderCredential,
            ),
            BreakGlassAuthorization {
                reason: reason.to_string(),
                expires_at_unix_ms: 11_000,
            },
        );
        assert!(matches!(
            denied,
            ControlPlaneDecision::Denied {
                error: ControlPlaneAuthorizationError::BreakGlassReasonMissing
                    | ControlPlaneAuthorizationError::BreakGlassReasonMalformed,
                ..
            }
        ));
    }
}
