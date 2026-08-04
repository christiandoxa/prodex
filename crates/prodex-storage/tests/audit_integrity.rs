use prodex_domain::{
    AuditAction, AuditEvent, AuditOutcome, AuditReasonDetail, AuditResource, CredentialScope,
    Principal, PrincipalId, PrincipalKind, Role, TenantContext, TenantId,
    compute_audit_chain_digest,
};
use prodex_storage::{GovernanceAuditExportRecord, verify_governance_audit_integrity};

fn audit_record(tenant_id: TenantId, detail: Option<String>) -> GovernanceAuditExportRecord {
    let principal = Principal::new(
        PrincipalId::new(),
        Some(tenant_id),
        PrincipalKind::ServiceAccount,
        Role::Admin,
        CredentialScope::ControlPlane,
    );
    let mut event = AuditEvent::new(
        2_000,
        TenantContext { tenant_id },
        &principal,
        AuditAction::new("control_plane.provider_credential.rotate"),
        AuditResource::new("provider_credential", Some("credential-1"), Some(tenant_id)),
        AuditOutcome::Success,
        None::<String>,
    );
    event.reason_detail = detail.map(|detail| AuditReasonDetail::new(detail).unwrap());
    let digest = compute_audit_chain_digest(None, &event);
    GovernanceAuditExportRecord {
        audit_event_id: event.id.to_string(),
        occurred_at_unix_ms: event.occurred_at_unix_ms,
        principal_id: event.principal_id.to_string(),
        action: event.action.as_str().to_string(),
        resource_kind: event.resource.kind,
        resource_id: event.resource.id,
        outcome: event.outcome.as_str().to_string(),
        reason_code: event.reason_code,
        reason_detail: event
            .reason_detail
            .map(|detail| detail.as_str().to_string()),
        previous_digest: None,
        event_digest: digest.as_str().to_string(),
    }
}

#[test]
fn audit_reason_detail_round_trips_and_tampering_breaks_digest() {
    let tenant_id = TenantId::new();
    let detail =
        prodex_domain::normalize_audit_reason_detail(" incident response api_key=top-secret ")
            .unwrap();
    assert_eq!(detail, "incident response api_key=<redacted>");

    let record = audit_record(tenant_id, Some(detail.clone()));
    assert!(
        verify_governance_audit_integrity(tenant_id, std::slice::from_ref(&record)).chain_valid
    );
    assert!(!format!("{record:?}").contains("top-secret"));

    let mut tampered = record.clone();
    tampered.reason_detail = Some("incident response api_key=other-secret".to_string());
    assert!(!verify_governance_audit_integrity(tenant_id, &[tampered]).chain_valid);

    let mut legacy_event = audit_record(tenant_id, None);
    let legacy_digest = {
        let id = legacy_event.audit_event_id.parse().unwrap();
        let principal_id = legacy_event.principal_id.parse().unwrap();
        let action = AuditAction::new(legacy_event.action.clone());
        let resource = AuditResource::new(
            legacy_event.resource_kind.clone(),
            legacy_event.resource_id.clone(),
            Some(tenant_id),
        );
        let event = AuditEvent {
            id,
            occurred_at_unix_ms: legacy_event.occurred_at_unix_ms,
            tenant_id,
            principal_id,
            action,
            resource,
            outcome: AuditOutcome::parse(&legacy_event.outcome).unwrap(),
            reason_code: legacy_event.reason_code.clone(),
            reason_detail: None,
        };
        compute_audit_chain_digest(None, &event)
    };
    legacy_event.event_digest = legacy_digest.as_str().to_string();
    assert!(verify_governance_audit_integrity(tenant_id, &[legacy_event]).chain_valid);
}
