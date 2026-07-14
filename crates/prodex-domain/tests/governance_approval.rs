use prodex_domain::{
    ApprovalAction, ApprovalError, ApprovalFingerprint, ApprovalId, ApprovalKind,
    ApprovalReasonCode, ApprovalRecord, ApprovalScope, ApprovalState, ApprovalTransitionRequest,
    CredentialScope, ExecutionApprovalBinding, GovernedAction, PolicyRevisionId, Principal,
    PrincipalId, PrincipalKind, Role, TenantId, execution_approval_id, transition_approval,
};
use sha2::{Digest, Sha256};

fn principal(id: PrincipalId, tenant_id: TenantId) -> Principal {
    Principal::new(
        id,
        Some(tenant_id),
        PrincipalKind::User,
        Role::Admin,
        CredentialScope::ControlPlane,
    )
}

#[test]
fn execution_fingerprint_is_stable_and_bound_to_the_request_context() {
    let tenant_id = TenantId::new();
    let principal_id = PrincipalId::new();
    let policy_revision_id = PolicyRevisionId::new();
    let tools = vec!["shell".to_string(), "browser".to_string()];
    let reversed = vec!["browser".to_string(), "shell".to_string()];
    let request_digest: [u8; 32] = Sha256::digest(b"request-a").into();
    let other_request_digest: [u8; 32] = Sha256::digest(b"request-b").into();
    let fingerprint = ExecutionApprovalBinding {
        tenant_id,
        principal_id,
        session_id_hash: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        action: GovernedAction::InvokeModel,
        model: Some("model-a"),
        tools: &tools,
        request_body_digest: &request_digest,
        policy_revision_id,
    }
    .fingerprint()
    .unwrap();
    let reordered = ExecutionApprovalBinding {
        tools: &reversed,
        model: Some("model-a"),
        tenant_id,
        principal_id,
        session_id_hash: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        action: GovernedAction::InvokeModel,
        request_body_digest: &request_digest,
        policy_revision_id,
    }
    .fingerprint()
    .unwrap();
    let other_model = ExecutionApprovalBinding {
        tools: &tools,
        model: Some("model-b"),
        tenant_id,
        principal_id,
        session_id_hash: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        action: GovernedAction::InvokeModel,
        request_body_digest: &request_digest,
        policy_revision_id,
    }
    .fingerprint()
    .unwrap();
    let other_request = ExecutionApprovalBinding {
        tools: &tools,
        model: Some("model-a"),
        tenant_id,
        principal_id,
        session_id_hash: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        action: GovernedAction::InvokeModel,
        request_body_digest: &other_request_digest,
        policy_revision_id,
    }
    .fingerprint()
    .unwrap();

    assert_eq!(fingerprint, reordered);
    assert_ne!(fingerprint, other_model);
    assert_ne!(fingerprint, other_request);
    assert_ne!(
        execution_approval_id(&fingerprint).unwrap(),
        execution_approval_id(&other_request).unwrap()
    );
    assert!(
        execution_approval_id(&fingerprint)
            .unwrap()
            .as_str()
            .starts_with("execution:sha256:")
    );
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
            reason: None,
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
        reason: None,
    })
    .unwrap();
    assert_eq!(first.record.state, ApprovalState::PendingApproval);

    let second = transition_approval(ApprovalTransitionRequest {
        record: &first.record,
        actor: &principal(PrincipalId::new(), tenant_id),
        action: ApprovalAction::Approve,
        expected_version: 2,
        now_unix_ms: 101,
        reason: None,
    })
    .unwrap();
    assert_eq!(second.record.state, ApprovalState::Approved);

    let active = transition_approval(ApprovalTransitionRequest {
        record: &second.record,
        actor: &maker,
        action: ApprovalAction::Activate,
        expected_version: 3,
        now_unix_ms: 102,
        reason: None,
    })
    .unwrap();
    assert_eq!(active.record.state, ApprovalState::Active);
    assert_eq!(
        transition_approval(ApprovalTransitionRequest {
            record: &active.record,
            actor: &principal(PrincipalId::new(), tenant_id),
            action: ApprovalAction::Approve,
            expected_version: 4,
            now_unix_ms: 103,
            reason: None,
        })
        .unwrap_err(),
        ApprovalError::InvalidTransition
    );
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
        reason: None,
    })
    .unwrap();
    let replay = transition_approval(ApprovalTransitionRequest {
        record: &first.record,
        actor: &checker,
        action: ApprovalAction::Approve,
        expected_version: 2,
        now_unix_ms: 101,
        reason: None,
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
            reason: None,
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
            reason: None,
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
        reason: None,
    })
    .unwrap();
    assert_eq!(expired.record.state, ApprovalState::Expired);
}

#[test]
fn high_risk_quorum_and_terminal_replays_fail_closed() {
    let tenant_id = TenantId::new();
    let maker = PrincipalId::new();
    let record = ApprovalRecord::pending(
        ApprovalId::new("high-risk").unwrap(),
        tenant_id,
        ApprovalKind::Execution,
        ApprovalScope::new("execution/tool").unwrap(),
        ApprovalFingerprint::new("sha256:execution").unwrap(),
        maker,
        1,
        10_000,
    )
    .unwrap();
    assert_eq!(record.required_quorum, 2);

    let cancellation_reason = ApprovalReasonCode::new("operator.cancelled").unwrap();
    let cancelled = transition_approval(ApprovalTransitionRequest {
        record: &record,
        actor: &principal(maker, tenant_id),
        action: ApprovalAction::Cancel,
        expected_version: 1,
        now_unix_ms: 100,
        reason: Some(&cancellation_reason),
    })
    .unwrap();
    assert_eq!(cancelled.record.state, ApprovalState::Cancelled);
    assert_eq!(
        cancelled.record.termination_reason,
        Some(cancellation_reason.clone())
    );

    let replay = transition_approval(ApprovalTransitionRequest {
        record: &cancelled.record,
        actor: &principal(maker, tenant_id),
        action: ApprovalAction::Cancel,
        expected_version: 2,
        now_unix_ms: 101,
        reason: Some(&cancellation_reason),
    })
    .unwrap();
    assert!(!replay.changed);
    let other_reason = ApprovalReasonCode::new("operator.changed_reason").unwrap();
    assert_eq!(
        transition_approval(ApprovalTransitionRequest {
            record: &cancelled.record,
            actor: &principal(maker, tenant_id),
            action: ApprovalAction::Cancel,
            expected_version: 2,
            now_unix_ms: 101,
            reason: Some(&other_reason),
        })
        .unwrap_err(),
        ApprovalError::ReplayMismatch
    );
}

#[test]
fn quorum_one_vote_and_expiry_replays_are_idempotent_and_debug_is_redacted() {
    let tenant_id = TenantId::new();
    let record = pending(tenant_id, PrincipalId::new(), 1);
    let checker = principal(PrincipalId::new(), tenant_id);
    let approved = transition_approval(ApprovalTransitionRequest {
        record: &record,
        actor: &checker,
        action: ApprovalAction::Approve,
        expected_version: 1,
        now_unix_ms: 100,
        reason: None,
    })
    .unwrap();
    assert_eq!(approved.record.state, ApprovalState::Approved);
    let replay = transition_approval(ApprovalTransitionRequest {
        record: &approved.record,
        actor: &checker,
        action: ApprovalAction::Approve,
        expected_version: 2,
        now_unix_ms: 101,
        reason: None,
    })
    .unwrap();
    assert!(!replay.changed);

    let expired = transition_approval(ApprovalTransitionRequest {
        record: &record,
        actor: &checker,
        action: ApprovalAction::Approve,
        expected_version: 1,
        now_unix_ms: 10_000,
        reason: None,
    })
    .unwrap();
    let expiry_replay = transition_approval(ApprovalTransitionRequest {
        record: &expired.record,
        actor: &checker,
        action: ApprovalAction::Approve,
        expected_version: 2,
        now_unix_ms: 10_001,
        reason: None,
    })
    .unwrap();
    assert!(!expiry_replay.changed);
    let rendered = format!("{:?}", expired.record);
    assert!(!rendered.contains("approval.expired"));
    assert_eq!(
        ApprovalError::ReplayMismatch.to_string(),
        "approval transition is not permitted"
    );
}
