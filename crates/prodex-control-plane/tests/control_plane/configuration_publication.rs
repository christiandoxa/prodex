use super::{principal, request};
use prodex_config::ConfigRevision;
use prodex_control_plane::{
    ConfigurationPublicationDecision, ConfigurationPublicationErrorStatus,
    ConfigurationPublicationRequest, ControlPlaneOperation, decide_configuration_publication,
    plan_configuration_publication_error_response,
};
use prodex_domain::{CredentialScope, PolicyRevisionId, ResourceKind, Role, TenantId};

#[test]
fn configuration_publication_requires_newer_same_tenant_revision() {
    let tenant_id = TenantId::new();
    let candidate = ConfigRevision {
        tenant_id,
        revision_id: PolicyRevisionId::new(),
        published_at_unix_ms: 10_000,
        payload: "payload",
    };
    let decision = decide_configuration_publication(ConfigurationPublicationRequest {
        action: request(
            tenant_id,
            principal(tenant_id, Role::Admin, CredentialScope::ControlPlane),
            ControlPlaneOperation::ConfigurationPublish,
            ResourceKind::Configuration,
        ),
        current_revision_id: None,
        candidate,
    });
    assert!(matches!(
        decision,
        ConfigurationPublicationDecision::Authorized(_)
    ));

    let rejected = decide_configuration_publication(ConfigurationPublicationRequest {
        action: request(
            tenant_id,
            principal(tenant_id, Role::Admin, CredentialScope::ControlPlane),
            ControlPlaneOperation::ConfigurationPublish,
            ResourceKind::Configuration,
        ),
        current_revision_id: None,
        candidate: ConfigRevision {
            tenant_id: TenantId::new(),
            revision_id: PolicyRevisionId::new(),
            published_at_unix_ms: 10_000,
            payload: "payload",
        },
    });
    assert!(matches!(
        rejected,
        ConfigurationPublicationDecision::Denied { .. }
    ));
}

#[test]
fn configuration_publication_error_responses_are_stable_and_redacted_at_control_plane_boundary() {
    let tenant_id = TenantId::new();
    let wrong_tenant = TenantId::new();
    let current_revision_id = PolicyRevisionId::new();

    let denied_by_authorization =
        decide_configuration_publication(ConfigurationPublicationRequest {
            action: request(
                tenant_id,
                principal(tenant_id, Role::Viewer, CredentialScope::ControlPlane),
                ControlPlaneOperation::ConfigurationPublish,
                ResourceKind::Configuration,
            ),
            current_revision_id: None,
            candidate: ConfigRevision {
                tenant_id,
                revision_id: PolicyRevisionId::new(),
                published_at_unix_ms: 10_000,
                payload: "sensitive-policy-payload",
            },
        });
    let ConfigurationPublicationDecision::Denied { error, .. } = denied_by_authorization else {
        panic!("expected authorization denial");
    };
    let authorization_response = plan_configuration_publication_error_response(&error);
    assert_eq!(
        authorization_response.status,
        ConfigurationPublicationErrorStatus::Forbidden
    );
    assert_eq!(authorization_response.code, "role_not_authorized");
    assert!(!authorization_response.message.contains("Viewer"));
    assert!(!authorization_response.message.contains("Admin"));

    let denied_by_tenant = decide_configuration_publication(ConfigurationPublicationRequest {
        action: request(
            tenant_id,
            principal(tenant_id, Role::Admin, CredentialScope::ControlPlane),
            ControlPlaneOperation::ConfigurationPublish,
            ResourceKind::Configuration,
        ),
        current_revision_id: None,
        candidate: ConfigRevision {
            tenant_id: wrong_tenant,
            revision_id: PolicyRevisionId::new(),
            published_at_unix_ms: 10_000,
            payload: "sensitive-policy-payload",
        },
    });
    let ConfigurationPublicationDecision::Denied { error, .. } = denied_by_tenant else {
        panic!("expected tenant publication denial");
    };
    let tenant_response = plan_configuration_publication_error_response(&error);
    assert_eq!(
        tenant_response.status,
        ConfigurationPublicationErrorStatus::BadRequest
    );
    assert_eq!(tenant_response.code, "configuration_tenant_mismatch");
    assert!(!tenant_response.message.contains(&tenant_id.to_string()));
    assert!(!tenant_response.message.contains(&wrong_tenant.to_string()));
    assert!(!tenant_response.message.contains("sensitive-policy-payload"));

    let denied_by_revision = decide_configuration_publication(ConfigurationPublicationRequest {
        action: request(
            tenant_id,
            principal(tenant_id, Role::Admin, CredentialScope::ControlPlane),
            ControlPlaneOperation::ConfigurationPublish,
            ResourceKind::Configuration,
        ),
        current_revision_id: Some(current_revision_id),
        candidate: ConfigRevision {
            tenant_id,
            revision_id: current_revision_id,
            published_at_unix_ms: 10_000,
            payload: "sensitive-policy-payload",
        },
    });
    let ConfigurationPublicationDecision::Denied { error, .. } = denied_by_revision else {
        panic!("expected stale revision denial");
    };
    let revision_response = plan_configuration_publication_error_response(&error);
    assert_eq!(
        revision_response.status,
        ConfigurationPublicationErrorStatus::Conflict
    );
    assert_eq!(revision_response.code, "configuration_revision_not_newer");
    assert!(
        !revision_response
            .message
            .contains(&current_revision_id.to_string())
    );
    assert!(
        !revision_response
            .message
            .contains("sensitive-policy-payload")
    );
}
