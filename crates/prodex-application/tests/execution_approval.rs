use std::sync::Mutex;

use prodex_application::{
    ApplicationExecutionApprovalDecision, ApplicationExecutionApprovalRequest,
    ApplicationExecutionApprovalService, ApplicationGovernanceLifecycleError,
    ApplicationGovernanceRepository,
};
use prodex_domain::{
    ApprovalAction, ApprovalFingerprint, ApprovalRecord, ApprovalState, AuditAction, AuditEvent,
    AuditEventId, AuditOutcome, AuditResource, CredentialScope, PolicyEffect, Principal,
    PrincipalId, PrincipalKind, Role, TenantContext, TenantId, compute_audit_chain_digest,
    transition_approval,
};
use prodex_storage::{
    AppendOnlyAuditCommand, ApprovalVoteRequest, AuditOutboxWriteCommand,
    GovernanceActivationRequest, GovernanceActivationResult, GovernanceRepositoryError,
    GovernanceRevisionWriteCommand, GovernanceWriteOutcome, TenantStorageKey,
};

#[derive(Default)]
struct Repository {
    approval: Mutex<Option<ApprovalRecord>>,
}

impl ApplicationGovernanceRepository for Repository {
    fn write_revision(
        &self,
        _: GovernanceRevisionWriteCommand,
        _: AuditOutboxWriteCommand,
    ) -> Result<GovernanceWriteOutcome, GovernanceRepositoryError> {
        Err(GovernanceRepositoryError::Unsupported)
    }

    fn create_approval(
        &self,
        approval: ApprovalRecord,
        _: AuditOutboxWriteCommand,
    ) -> Result<GovernanceWriteOutcome, GovernanceRepositoryError> {
        *self.approval.lock().unwrap() = Some(approval);
        Ok(GovernanceWriteOutcome::Applied)
    }

    fn transition_approval(
        &self,
        request: ApprovalVoteRequest,
        action: ApprovalAction,
    ) -> Result<ApprovalRecord, GovernanceRepositoryError> {
        let mut approval = self.approval.lock().unwrap();
        let current = approval
            .as_ref()
            .ok_or(GovernanceRepositoryError::NotFound)?;
        let transitioned = transition_approval(prodex_domain::ApprovalTransitionRequest {
            record: current,
            actor: &request.actor,
            action,
            expected_version: request.expected_version,
            now_unix_ms: request.now_unix_ms,
            reason: request.reason.as_ref(),
        })
        .map_err(|_| GovernanceRepositoryError::InvalidTransition)?
        .record;
        *approval = Some(transitioned.clone());
        Ok(transitioned)
    }

    fn get_approval(
        &self,
        tenant_id: TenantId,
        approval_id: &prodex_domain::ApprovalId,
    ) -> Result<ApprovalRecord, GovernanceRepositoryError> {
        self.approval
            .lock()
            .unwrap()
            .as_ref()
            .filter(|approval| approval.tenant_id == tenant_id && &approval.id == approval_id)
            .cloned()
            .ok_or(GovernanceRepositoryError::NotFound)
    }

    fn activate_revision(
        &self,
        _: GovernanceActivationRequest,
        _: &mut dyn FnMut(&[u8]) -> bool,
    ) -> Result<GovernanceActivationResult, GovernanceRepositoryError> {
        Err(GovernanceRepositoryError::Unsupported)
    }

    fn append_audit_outbox(
        &self,
        _: AuditOutboxWriteCommand,
    ) -> Result<(), GovernanceRepositoryError> {
        Ok(())
    }
}

fn audit(tenant_id: TenantId, principal: &Principal, action: &str) -> AuditOutboxWriteCommand {
    let event = AuditEvent::new(
        1_000,
        TenantContext { tenant_id },
        principal,
        AuditAction::try_new(action).unwrap(),
        AuditResource::new("execution_approval", None::<String>, Some(tenant_id)),
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

fn request(
    tenant_id: TenantId,
    principal: &Principal,
    effect: PolicyEffect,
) -> ApplicationExecutionApprovalRequest {
    ApplicationExecutionApprovalRequest {
        tenant_id,
        principal: principal.clone(),
        policy_effect: effect,
        fingerprint: ApprovalFingerprint::new("sha256:request-bound-fixture").unwrap(),
        now_unix_ms: 2_000,
        create_audit_outbox: audit(tenant_id, principal, "execution.create"),
        consume_audit_outbox: audit(tenant_id, principal, "execution.consume"),
    }
}

#[test]
fn execution_approval_is_policy_selected_quorum_gated_and_one_use() {
    let tenant_id = TenantId::new();
    let principal = Principal::new(
        PrincipalId::new(),
        Some(tenant_id),
        PrincipalKind::User,
        Role::Admin,
        CredentialScope::DataPlane,
    );
    let repository = Repository::default();
    let service = ApplicationExecutionApprovalService::new(&repository);
    assert_eq!(
        service.enforce(request(tenant_id, &principal, PolicyEffect::Allow)),
        Err(ApplicationGovernanceLifecycleError::InvalidAction)
    );
    let pending = service
        .enforce(request(
            tenant_id,
            &principal,
            PolicyEffect::RequireApproval,
        ))
        .unwrap();
    let ApplicationExecutionApprovalDecision::Pending(pending) = pending else {
        panic!("first request must be pending");
    };
    assert_eq!(pending.required_quorum, 2);
    let approved_id = pending.id.clone();
    {
        let mut stored = repository.approval.lock().unwrap();
        let approval = stored.as_mut().unwrap();
        approval.state = ApprovalState::Approved;
        approval.version = 2;
    }
    let authorized = service
        .enforce(request(
            tenant_id,
            &principal,
            PolicyEffect::RequireApproval,
        ))
        .unwrap();
    assert!(matches!(
        authorized,
        ApplicationExecutionApprovalDecision::Authorized(ApprovalRecord {
            state: ApprovalState::Active,
            ..
        })
    ));
    let replay = service
        .enforce(request(
            tenant_id,
            &principal,
            PolicyEffect::RequireApproval,
        ))
        .unwrap();
    assert!(matches!(
        replay,
        ApplicationExecutionApprovalDecision::Denied(ApprovalRecord {
            state: ApprovalState::Active,
            ..
        })
    ));
    let mut changed = request(tenant_id, &principal, PolicyEffect::RequireApproval);
    changed.fingerprint = ApprovalFingerprint::new("sha256:changed-request-body").unwrap();
    let changed = service.enforce(changed).unwrap();
    let ApplicationExecutionApprovalDecision::Pending(changed) = changed else {
        panic!("changed request must require a distinct approval");
    };
    assert_ne!(changed.id, approved_id);
}
