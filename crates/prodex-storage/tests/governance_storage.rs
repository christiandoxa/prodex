use prodex_domain::{
    ApprovalAction, ApprovalFingerprint, ApprovalId, ApprovalKind, ApprovalRecord, ApprovalScope,
    ApprovalTransitionRequest, AuditAction, AuditEvent, AuditEventId, AuditOutcome, AuditResource,
    CredentialScope, IdempotencyKey, Principal, PrincipalId, PrincipalKind, Role, TenantContext,
    TenantId, compute_audit_chain_digest, transition_approval,
};
use prodex_storage::{
    AppendOnlyAuditCommand, AuditOutboxWriteCommand, GovernanceActivationAction,
    GovernanceActivationCurrent, GovernanceActivationRequest, GovernanceArtifactKind,
    GovernanceRevisionWriteCommand, GovernanceStorageError, SiemOutboxDeliveryDecision,
    SiemOutboxRetryPolicy, TenantStorageKey, plan_governance_activation,
    plan_governance_revision_write, plan_siem_outbox_delivery,
};

fn approved(tenant_id: TenantId, fingerprint: ApprovalFingerprint) -> (ApprovalRecord, Principal) {
    let maker = PrincipalId::new();
    let pending = ApprovalRecord::pending(
        ApprovalId::new("approval-1").unwrap(),
        tenant_id,
        ApprovalKind::PolicyRevision,
        ApprovalScope::new("policy/active").unwrap(),
        fingerprint,
        maker,
        1,
        10_000,
    )
    .unwrap();
    let checker = Principal::new(
        PrincipalId::new(),
        Some(tenant_id),
        PrincipalKind::User,
        Role::Admin,
        CredentialScope::ControlPlane,
    );
    let approval = transition_approval(ApprovalTransitionRequest {
        record: &pending,
        actor: &checker,
        action: ApprovalAction::Approve,
        expected_version: 1,
        now_unix_ms: 100,
        reason: None,
    })
    .unwrap()
    .record;
    (approval, checker)
}

fn audit(tenant_id: TenantId, actor: &Principal) -> AuditOutboxWriteCommand {
    let event = AuditEvent::new(
        200,
        TenantContext { tenant_id },
        actor,
        AuditAction::try_new("governance.activate").unwrap(),
        AuditResource::new("policy", None::<String>, Some(tenant_id)),
        AuditOutcome::Success,
        None::<String>,
    );
    AuditOutboxWriteCommand {
        outbox_event_id: AuditEventId::new(),
        audit: AppendOnlyAuditCommand {
            storage_key: TenantStorageKey::tenant(tenant_id),
            event_digest: compute_audit_chain_digest(None, &event),
            event,
            previous_digest: None,
        },
    }
}

#[test]
fn governance_revision_is_tenant_bound_bounded_and_content_free_in_debug() {
    let tenant_id = TenantId::new();
    let command = GovernanceRevisionWriteCommand {
        storage_key: TenantStorageKey::tenant(tenant_id),
        tenant_id,
        kind: GovernanceArtifactKind::Policy,
        revision_id: "policy-v1".to_string(),
        fingerprint: ApprovalFingerprint::new("sha256:fixture").unwrap(),
        compiled_artifact: b"compiled-policy-sentinel".to_vec(),
        created_by: PrincipalId::new(),
        created_at_unix_ms: 100,
    };
    let rendered = format!("{command:?}");
    assert!(!rendered.contains("compiled-policy-sentinel"));
    plan_governance_revision_write(command).unwrap();

    assert_eq!(
        plan_governance_revision_write(GovernanceRevisionWriteCommand {
            storage_key: TenantStorageKey::tenant(TenantId::new()),
            tenant_id,
            kind: GovernanceArtifactKind::Policy,
            revision_id: "policy-v1".to_string(),
            fingerprint: ApprovalFingerprint::new("sha256:fixture").unwrap(),
            compiled_artifact: vec![1],
            created_by: PrincipalId::new(),
            created_at_unix_ms: 100,
        })
        .unwrap_err(),
        GovernanceStorageError::TenantMismatch
    );
}

#[test]
fn governance_activation_requires_exact_approved_fingerprint_and_preserves_lkg() {
    let tenant_id = TenantId::new();
    let fingerprint = ApprovalFingerprint::new("sha256:fixture").unwrap();
    let (approval, actor) = approved(tenant_id, fingerprint.clone());
    let request = GovernanceActivationRequest {
        tenant_id,
        kind: GovernanceArtifactKind::Policy,
        revision_id: prodex_domain::PolicyRevisionId::new().to_string(),
        approval_id: approval.id.clone(),
        actor: actor.clone(),
        action: GovernanceActivationAction::Activate,
        expected_etag: Some("etag-v1".to_string()),
        idempotency_key: IdempotencyKey::new("activate-v2").unwrap(),
        request_fingerprint: "request-v2".to_string(),
        audit_outbox: audit(tenant_id, &actor),
        activated_at_unix_ms: 200,
    };
    let plan = plan_governance_activation(
        &request,
        GovernanceActivationCurrent {
            revision_checksum: fingerprint.as_str(),
            approval: &approval,
            active_revision_id: Some("policy-v1"),
            last_known_good_revision_id: None,
            etag: Some("etag-v1"),
        },
    )
    .unwrap();

    assert_eq!(plan.last_known_good_revision_id, "policy-v1");
    assert_eq!(plan.previous_revision_state, Some("superseded"));
    assert_eq!(
        plan.activated_approval.state,
        prodex_domain::ApprovalState::Active
    );
}

#[test]
fn siem_outbox_retry_is_bounded_and_dead_letters() {
    let policy = SiemOutboxRetryPolicy::bounded(3, 100, 1_000).unwrap();
    assert_eq!(
        plan_siem_outbox_delivery(policy, 0, false, 1_000),
        SiemOutboxDeliveryDecision::RetryAt(1_100)
    );
    assert_eq!(
        plan_siem_outbox_delivery(policy, 1, false, 1_000),
        SiemOutboxDeliveryDecision::RetryAt(1_200)
    );
    assert_eq!(
        plan_siem_outbox_delivery(policy, 2, false, 1_000),
        SiemOutboxDeliveryDecision::DeadLetter
    );
    assert_eq!(
        plan_siem_outbox_delivery(policy, 0, true, 1_000),
        SiemOutboxDeliveryDecision::Delivered
    );
}
