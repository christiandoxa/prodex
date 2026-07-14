use prodex_domain::{
    ApprovalAction, ApprovalFingerprint, ApprovalId, ApprovalKind, ApprovalRecord, ApprovalScope,
    ApprovalTransitionRequest, CredentialScope, Principal, PrincipalId, PrincipalKind, Role,
    TenantId, transition_approval,
};
use prodex_storage::{
    GovernanceActivationCommand, GovernanceArtifactKind, GovernanceRevisionWriteCommand,
    GovernanceStorageError, SiemOutboxDeliveryDecision, SiemOutboxRetryPolicy, TenantStorageKey,
    plan_governance_activation, plan_governance_revision_write, plan_siem_outbox_delivery,
};

fn approved(tenant_id: TenantId, fingerprint: ApprovalFingerprint) -> ApprovalRecord {
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
    transition_approval(ApprovalTransitionRequest {
        record: &pending,
        actor: &checker,
        action: ApprovalAction::Approve,
        expected_version: 1,
        now_unix_ms: 100,
        reason: None,
    })
    .unwrap()
    .record
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
    let plan = plan_governance_activation(GovernanceActivationCommand {
        storage_key: TenantStorageKey::tenant(tenant_id),
        tenant_id,
        kind: GovernanceArtifactKind::Policy,
        revision_id: "policy-v2".to_string(),
        fingerprint: fingerprint.clone(),
        approval: approved(tenant_id, fingerprint),
        expected_etag: "etag-v1".to_string(),
        current_active_revision_id: Some("policy-v1".to_string()),
        current_last_known_good_revision_id: None,
        activated_at_unix_ms: 200,
    })
    .unwrap();

    assert_eq!(plan.activated_revision_id, "policy-v2");
    assert_eq!(
        plan.last_known_good_revision_id.as_deref(),
        Some("policy-v1")
    );
    assert!(plan.transactional_audit_required);
    assert!(plan.invalidation_required);
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
