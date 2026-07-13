use prodex_domain::{
    ApprovalAction, ApprovalError, ApprovalFingerprint, ApprovalId, ApprovalKind, ApprovalRecord,
    ApprovalScope, ApprovalState, ApprovalTransitionRequest, CredentialScope, Principal,
    PrincipalId, PrincipalKind, Role, TenantId, transition_approval,
};

fn principal(id: PrincipalId, tenant_id: TenantId) -> Principal {
    Principal::new(
        id,
        Some(tenant_id),
        PrincipalKind::User,
        Role::Admin,
        CredentialScope::ControlPlane,
    )
}

fn pending(tenant_id: TenantId, maker: PrincipalId, quorum: u8) -> ApprovalRecord {
    ApprovalRecord::pending(
        ApprovalId::new("approval-1").unwrap(),
        tenant_id,
        ApprovalKind::PolicyRevision,
        ApprovalScope::new("policy/active").unwrap(),
        ApprovalFingerprint::new("sha256:fixture").unwrap(),
        maker,
        quorum,
        10_000,
    )
    .unwrap()
}

#[test]
fn maker_checker_quorum_and_activation_are_enforced() {
    let tenant_id = TenantId::new();
    let maker_id = PrincipalId::new();
    let maker = principal(maker_id, tenant_id);
    let record = pending(tenant_id, maker_id, 2);
    assert_eq!(
        transition_approval(ApprovalTransitionRequest {
            record: &record,
            actor: &maker,
            action: ApprovalAction::Approve,
            expected_version: 1,
            now_unix_ms: 100,
        })
        .unwrap_err(),
        ApprovalError::SelfApprovalDenied
    );

    let first = transition_approval(ApprovalTransitionRequest {
        record: &record,
        actor: &principal(PrincipalId::new(), tenant_id),
        action: ApprovalAction::Approve,
        expected_version: 1,
        now_unix_ms: 100,
    })
    .unwrap();
    assert_eq!(first.record.state, ApprovalState::PendingApproval);

    let second = transition_approval(ApprovalTransitionRequest {
        record: &first.record,
        actor: &principal(PrincipalId::new(), tenant_id),
        action: ApprovalAction::Approve,
        expected_version: 2,
        now_unix_ms: 101,
    })
    .unwrap();
    assert_eq!(second.record.state, ApprovalState::Approved);

    let active = transition_approval(ApprovalTransitionRequest {
        record: &second.record,
        actor: &maker,
        action: ApprovalAction::Activate,
        expected_version: 3,
        now_unix_ms: 102,
    })
    .unwrap();
    assert_eq!(active.record.state, ApprovalState::Active);
}

#[test]
fn approval_replay_is_idempotent_and_stale_versions_fail() {
    let tenant_id = TenantId::new();
    let record = pending(tenant_id, PrincipalId::new(), 2);
    let checker = principal(PrincipalId::new(), tenant_id);
    let first = transition_approval(ApprovalTransitionRequest {
        record: &record,
        actor: &checker,
        action: ApprovalAction::Approve,
        expected_version: 1,
        now_unix_ms: 100,
    })
    .unwrap();
    let replay = transition_approval(ApprovalTransitionRequest {
        record: &first.record,
        actor: &checker,
        action: ApprovalAction::Approve,
        expected_version: 2,
        now_unix_ms: 101,
    })
    .unwrap();
    assert!(!replay.changed);
    assert_eq!(replay.record.version, 2);

    assert_eq!(
        transition_approval(ApprovalTransitionRequest {
            record: &first.record,
            actor: &checker,
            action: ApprovalAction::Approve,
            expected_version: 1,
            now_unix_ms: 101,
        })
        .unwrap_err(),
        ApprovalError::StaleVersion
    );
}

#[test]
fn expired_and_cross_tenant_requests_fail_closed() {
    let tenant_id = TenantId::new();
    let record = pending(tenant_id, PrincipalId::new(), 1);
    let other = principal(PrincipalId::new(), TenantId::new());
    assert_eq!(
        transition_approval(ApprovalTransitionRequest {
            record: &record,
            actor: &other,
            action: ApprovalAction::Approve,
            expected_version: 1,
            now_unix_ms: 100,
        })
        .unwrap_err(),
        ApprovalError::TenantMismatch
    );

    let expired = transition_approval(ApprovalTransitionRequest {
        record: &record,
        actor: &principal(PrincipalId::new(), tenant_id),
        action: ApprovalAction::Approve,
        expected_version: 1,
        now_unix_ms: 10_000,
    })
    .unwrap();
    assert_eq!(expired.record.state, ApprovalState::Expired);
}
