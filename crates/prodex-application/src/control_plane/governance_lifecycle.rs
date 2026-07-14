//! Executing policy-governance use case behind the existing control-plane boundary.

use std::error::Error;
use std::fmt;

use prodex_control_plane::{
    ControlPlaneActionRequest, ControlPlaneAuthorizationError, ControlPlaneDecision,
    ControlPlaneOperation, decide_control_plane_action,
};
use prodex_domain::{
    ApprovalAction, ApprovalFingerprint, ApprovalKind, ApprovalReasonCode, ApprovalRecord,
    ApprovalScope, ApprovalState, AuditAction, AuditDigest, AuditEvent, AuditEventId,
    AuditResource, PolicyEffect, Principal, ResourceKind, TenantId, execution_approval_id,
};
use prodex_storage::{
    AppendOnlyAuditCommand, ApprovalVoteIdempotency, ApprovalVoteMutationOutcome,
    ApprovalVoteRequest, AuditOutboxWriteCommand, GovernanceActivationAction,
    GovernanceActivationRequest, GovernanceActivationResult, GovernanceArtifactKind,
    GovernanceRepositoryError, GovernanceRevisionWriteCommand, GovernanceWriteOutcome,
    TenantStorageKey,
};

#[derive(Clone, PartialEq, Eq)]
pub struct ApplicationGovernanceAuditLink {
    pub previous_digest: Option<AuditDigest>,
    pub event_digest: AuditDigest,
    pub outbox_event_id: AuditEventId,
}

impl fmt::Debug for ApplicationGovernanceAuditLink {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationGovernanceAuditLink")
            .field(
                "previous_digest",
                &self.previous_digest.as_ref().map(|_| "<redacted>"),
            )
            .field("event_digest", &"<redacted>")
            .field("outbox_event_id", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApplicationGovernanceLifecycleError {
    InvalidAction,
    Authorization(ControlPlaneAuthorizationError),
    Repository(GovernanceRepositoryError),
}

impl fmt::Display for ApplicationGovernanceLifecycleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "policy governance operation failed")
    }
}

impl Error for ApplicationGovernanceLifecycleError {}

pub trait ApplicationGovernanceRepository {
    fn write_revision(
        &self,
        command: GovernanceRevisionWriteCommand,
        audit_outbox: AuditOutboxWriteCommand,
    ) -> Result<GovernanceWriteOutcome, GovernanceRepositoryError>;

    fn create_approval(
        &self,
        approval: prodex_domain::ApprovalRecord,
        audit_outbox: AuditOutboxWriteCommand,
    ) -> Result<GovernanceWriteOutcome, GovernanceRepositoryError>;

    fn transition_approval(
        &self,
        request: ApprovalVoteRequest,
        action: ApprovalAction,
    ) -> Result<prodex_domain::ApprovalRecord, GovernanceRepositoryError>;

    fn transition_approval_idempotent(
        &self,
        request: ApprovalVoteRequest,
        action: ApprovalAction,
        _idempotency: ApprovalVoteIdempotency,
    ) -> Result<ApprovalVoteMutationOutcome, GovernanceRepositoryError> {
        self.transition_approval(request, action)
            .map(ApprovalVoteMutationOutcome::Applied)
    }

    fn get_approval(
        &self,
        tenant_id: TenantId,
        approval_id: &prodex_domain::ApprovalId,
    ) -> Result<ApprovalRecord, GovernanceRepositoryError>;

    fn activate_revision(
        &self,
        request: GovernanceActivationRequest,
        validate_artifact: &mut dyn FnMut(&[u8]) -> bool,
    ) -> Result<GovernanceActivationResult, GovernanceRepositoryError>;

    fn append_audit_outbox(
        &self,
        command: AuditOutboxWriteCommand,
    ) -> Result<(), GovernanceRepositoryError>;
}

pub const EXECUTION_APPROVAL_TTL_MS: u64 = 15 * 60 * 1_000;

#[derive(Clone)]
pub struct ApplicationExecutionApprovalRequest {
    pub tenant_id: TenantId,
    pub principal: Principal,
    pub policy_effect: PolicyEffect,
    pub fingerprint: ApprovalFingerprint,
    pub now_unix_ms: u64,
    pub create_audit_outbox: AuditOutboxWriteCommand,
    pub consume_audit_outbox: AuditOutboxWriteCommand,
}

impl fmt::Debug for ApplicationExecutionApprovalRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationExecutionApprovalRequest")
            .field("tenant_id", &"<redacted>")
            .field("principal", &"<redacted>")
            .field("policy_effect", &self.policy_effect)
            .field("fingerprint", &self.fingerprint)
            .field("now_unix_ms", &"<redacted>")
            .field("create_audit_outbox", &"<redacted>")
            .field("consume_audit_outbox", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApplicationExecutionApprovalDecision {
    Pending(ApprovalRecord),
    Authorized(ApprovalRecord),
    Denied(ApprovalRecord),
}

pub struct ApplicationExecutionApprovalService<'a, R: ApplicationGovernanceRepository + ?Sized> {
    repository: &'a R,
}

impl<'a, R: ApplicationGovernanceRepository + ?Sized> ApplicationExecutionApprovalService<'a, R> {
    pub fn new(repository: &'a R) -> Self {
        Self { repository }
    }

    pub fn enforce(
        &self,
        request: ApplicationExecutionApprovalRequest,
    ) -> Result<ApplicationExecutionApprovalDecision, ApplicationGovernanceLifecycleError> {
        if request.policy_effect != PolicyEffect::RequireApproval
            || request.principal.tenant_id != Some(request.tenant_id)
        {
            return Err(ApplicationGovernanceLifecycleError::InvalidAction);
        }
        let approval_id = execution_approval_id(&request.fingerprint)
            .map_err(|_| ApplicationGovernanceLifecycleError::InvalidAction)?;
        let approval = match self
            .repository
            .get_approval(request.tenant_id, &approval_id)
        {
            Ok(approval) => approval,
            Err(GovernanceRepositoryError::NotFound) => {
                let approval = ApprovalRecord::pending(
                    approval_id,
                    request.tenant_id,
                    ApprovalKind::Execution,
                    ApprovalScope::new("execution")
                        .map_err(|_| ApplicationGovernanceLifecycleError::InvalidAction)?,
                    request.fingerprint,
                    request.principal.id,
                    prodex_domain::HIGH_RISK_MINIMUM_APPROVAL_QUORUM,
                    request
                        .now_unix_ms
                        .saturating_add(EXECUTION_APPROVAL_TTL_MS),
                )
                .map_err(|_| ApplicationGovernanceLifecycleError::InvalidAction)?;
                self.repository
                    .create_approval(approval.clone(), request.create_audit_outbox)
                    .map_err(ApplicationGovernanceLifecycleError::Repository)?;
                return Ok(ApplicationExecutionApprovalDecision::Pending(approval));
            }
            Err(error) => return Err(ApplicationGovernanceLifecycleError::Repository(error)),
        };
        if approval.kind != ApprovalKind::Execution
            || approval.fingerprint != request.fingerprint
            || approval.maker != request.principal.id
        {
            return Err(ApplicationGovernanceLifecycleError::InvalidAction);
        }
        if matches!(
            approval.state,
            ApprovalState::PendingApproval | ApprovalState::Approved
        ) && request.now_unix_ms >= approval.expires_at_unix_ms
        {
            return self
                .consume(approval, request)
                .map(ApplicationExecutionApprovalDecision::Denied);
        }
        match approval.state {
            ApprovalState::PendingApproval => {
                Ok(ApplicationExecutionApprovalDecision::Pending(approval))
            }
            ApprovalState::Approved => self
                .consume(approval, request)
                .map(ApplicationExecutionApprovalDecision::Authorized),
            ApprovalState::Draft
            | ApprovalState::Rejected
            | ApprovalState::Expired
            | ApprovalState::Cancelled
            | ApprovalState::Active
            | ApprovalState::Superseded
            | ApprovalState::RolledBack => {
                Ok(ApplicationExecutionApprovalDecision::Denied(approval))
            }
        }
    }

    pub fn review(
        &self,
        request: ApprovalVoteRequest,
        action: ApprovalAction,
    ) -> Result<ApprovalRecord, ApplicationGovernanceLifecycleError> {
        if !matches!(
            action,
            ApprovalAction::Approve | ApprovalAction::Reject | ApprovalAction::Cancel
        ) {
            return Err(ApplicationGovernanceLifecycleError::InvalidAction);
        }
        let approval = self
            .repository
            .get_approval(request.tenant_id, &request.approval_id)
            .map_err(ApplicationGovernanceLifecycleError::Repository)?;
        if approval.kind != ApprovalKind::Execution {
            return Err(ApplicationGovernanceLifecycleError::InvalidAction);
        }
        self.repository
            .transition_approval(request, action)
            .map_err(ApplicationGovernanceLifecycleError::Repository)
    }

    pub fn review_idempotent(
        &self,
        request: ApprovalVoteRequest,
        action: ApprovalAction,
        idempotency: ApprovalVoteIdempotency,
    ) -> Result<ApprovalVoteMutationOutcome, ApplicationGovernanceLifecycleError> {
        if !matches!(
            action,
            ApprovalAction::Approve | ApprovalAction::Reject | ApprovalAction::Cancel
        ) {
            return Err(ApplicationGovernanceLifecycleError::InvalidAction);
        }
        let approval = self
            .repository
            .get_approval(request.tenant_id, &request.approval_id)
            .map_err(ApplicationGovernanceLifecycleError::Repository)?;
        if approval.kind != ApprovalKind::Execution {
            return Err(ApplicationGovernanceLifecycleError::InvalidAction);
        }
        self.repository
            .transition_approval_idempotent(request, action, idempotency)
            .map_err(ApplicationGovernanceLifecycleError::Repository)
    }

    fn consume(
        &self,
        approval: ApprovalRecord,
        request: ApplicationExecutionApprovalRequest,
    ) -> Result<ApprovalRecord, ApplicationGovernanceLifecycleError> {
        self.repository
            .transition_approval(
                ApprovalVoteRequest {
                    tenant_id: request.tenant_id,
                    approval_id: approval.id.clone(),
                    actor: request.principal,
                    expected_version: approval.version,
                    now_unix_ms: request.now_unix_ms,
                    reason: None,
                    audit_outbox: request.consume_audit_outbox,
                },
                ApprovalAction::Activate,
            )
            .map_err(ApplicationGovernanceLifecycleError::Repository)
    }
}

pub struct ApplicationPolicyGovernanceService<'a, R: ApplicationGovernanceRepository + ?Sized> {
    repository: &'a R,
}

impl<R: ApplicationGovernanceRepository + ?Sized> fmt::Debug
    for ApplicationPolicyGovernanceService<'_, R>
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationPolicyGovernanceService")
            .field("repository", &"<redacted>")
            .finish()
    }
}

impl<'a, R: ApplicationGovernanceRepository + ?Sized> ApplicationPolicyGovernanceService<'a, R> {
    pub fn new(repository: &'a R) -> Self {
        Self { repository }
    }

    pub fn write_revision(
        &self,
        action: ControlPlaneActionRequest,
        revision: GovernanceRevisionWriteCommand,
        audit: ApplicationGovernanceAuditLink,
    ) -> Result<GovernanceWriteOutcome, ApplicationGovernanceLifecycleError> {
        if revision.kind != GovernanceArtifactKind::Policy
            || revision.tenant_id != action.resource.tenant_id
        {
            return Err(ApplicationGovernanceLifecycleError::InvalidAction);
        }
        let audit_outbox = self.authorize(
            action,
            audit,
            "governance.policy.revision.write",
            Some(revision.revision_id.clone()),
        )?;
        self.repository
            .write_revision(revision, audit_outbox)
            .map_err(ApplicationGovernanceLifecycleError::Repository)
    }

    pub fn create_approval(
        &self,
        action: ControlPlaneActionRequest,
        approval: prodex_domain::ApprovalRecord,
        audit: ApplicationGovernanceAuditLink,
    ) -> Result<GovernanceWriteOutcome, ApplicationGovernanceLifecycleError> {
        if approval.kind != prodex_domain::ApprovalKind::PolicyRevision
            || approval.tenant_id != action.resource.tenant_id
        {
            return Err(ApplicationGovernanceLifecycleError::InvalidAction);
        }
        let audit_outbox = self.authorize(
            action,
            audit,
            "governance.policy.approval.create",
            Some(approval.id.as_str().to_string()),
        )?;
        self.repository
            .create_approval(approval, audit_outbox)
            .map_err(ApplicationGovernanceLifecycleError::Repository)
    }

    pub fn vote_approval(
        &self,
        action: ControlPlaneActionRequest,
        request: ApprovalVoteRequest,
        audit: ApplicationGovernanceAuditLink,
    ) -> Result<prodex_domain::ApprovalRecord, ApplicationGovernanceLifecycleError> {
        self.transition_approval(action, request, ApprovalAction::Approve, audit)
    }

    pub fn reject_approval(
        &self,
        action: ControlPlaneActionRequest,
        mut request: ApprovalVoteRequest,
        reason: ApprovalReasonCode,
        audit: ApplicationGovernanceAuditLink,
    ) -> Result<prodex_domain::ApprovalRecord, ApplicationGovernanceLifecycleError> {
        request.reason = Some(reason);
        self.transition_approval(action, request, ApprovalAction::Reject, audit)
    }

    pub fn cancel_approval(
        &self,
        action: ControlPlaneActionRequest,
        mut request: ApprovalVoteRequest,
        reason: ApprovalReasonCode,
        audit: ApplicationGovernanceAuditLink,
    ) -> Result<prodex_domain::ApprovalRecord, ApplicationGovernanceLifecycleError> {
        request.reason = Some(reason);
        self.transition_approval(action, request, ApprovalAction::Cancel, audit)
    }

    pub fn transition_approval(
        &self,
        action: ControlPlaneActionRequest,
        mut request: ApprovalVoteRequest,
        approval_action: ApprovalAction,
        audit: ApplicationGovernanceAuditLink,
    ) -> Result<prodex_domain::ApprovalRecord, ApplicationGovernanceLifecycleError> {
        if request.tenant_id != action.resource.tenant_id {
            return Err(ApplicationGovernanceLifecycleError::InvalidAction);
        }
        request.actor = action.principal.clone();
        let audit_action = approval_transition_audit_action(approval_action)
            .ok_or(ApplicationGovernanceLifecycleError::InvalidAction)?;
        request.audit_outbox = self.authorize(
            action,
            audit,
            audit_action,
            Some(request.approval_id.as_str().to_string()),
        )?;
        self.repository
            .transition_approval(request, approval_action)
            .map_err(ApplicationGovernanceLifecycleError::Repository)
    }

    pub fn activate_revision(
        &self,
        action: ControlPlaneActionRequest,
        mut request: GovernanceActivationRequest,
        audit: ApplicationGovernanceAuditLink,
        validate_artifact: impl FnMut(&[u8]) -> bool,
    ) -> Result<GovernanceActivationResult, ApplicationGovernanceLifecycleError> {
        if request.kind != GovernanceArtifactKind::Policy
            || request.tenant_id != action.resource.tenant_id
        {
            return Err(ApplicationGovernanceLifecycleError::InvalidAction);
        }
        let audit_action = match request.action {
            GovernanceActivationAction::Activate => "governance.policy.revision.activate",
            GovernanceActivationAction::Rollback => "governance.policy.revision.rollback",
        };
        request.actor = action.principal.clone();
        request.audit_outbox = self.authorize(
            action,
            audit,
            audit_action,
            Some(request.revision_id.clone()),
        )?;
        let mut validate_artifact = validate_artifact;
        self.repository
            .activate_revision(request, &mut validate_artifact)
            .map_err(ApplicationGovernanceLifecycleError::Repository)
    }

    fn authorize(
        &self,
        action: ControlPlaneActionRequest,
        audit: ApplicationGovernanceAuditLink,
        audit_action: &'static str,
        resource_id: Option<String>,
    ) -> Result<AuditOutboxWriteCommand, ApplicationGovernanceLifecycleError> {
        if action.operation != ControlPlaneOperation::PolicyPublish
            || action.resource.kind != ResourceKind::Policy
        {
            return Err(ApplicationGovernanceLifecycleError::InvalidAction);
        }
        match decide_control_plane_action(action) {
            ControlPlaneDecision::Authorized(plan) => Ok(audit_command(
                plan.audit_event,
                audit,
                audit_action,
                resource_id,
            )),
            ControlPlaneDecision::Denied {
                error, audit_event, ..
            } => {
                let command = audit_command(audit_event, audit, audit_action, resource_id);
                self.repository
                    .append_audit_outbox(command)
                    .map_err(ApplicationGovernanceLifecycleError::Repository)?;
                Err(ApplicationGovernanceLifecycleError::Authorization(error))
            }
        }
    }
}

const fn approval_transition_audit_action(action: ApprovalAction) -> Option<&'static str> {
    match action {
        ApprovalAction::Approve => Some("governance.policy.approval.vote"),
        ApprovalAction::Reject => Some("governance.policy.approval.reject"),
        ApprovalAction::Cancel => Some("governance.policy.approval.cancel"),
        ApprovalAction::Activate | ApprovalAction::Supersede | ApprovalAction::RollBack => None,
    }
}

fn audit_command(
    mut event: AuditEvent,
    audit: ApplicationGovernanceAuditLink,
    action: &'static str,
    resource_id: Option<String>,
) -> AuditOutboxWriteCommand {
    event.action = AuditAction::new(action);
    event.resource = AuditResource::new(
        "governance_policy_revision",
        resource_id,
        Some(event.tenant_id),
    );
    AuditOutboxWriteCommand {
        outbox_event_id: audit.outbox_event_id,
        audit: AppendOnlyAuditCommand {
            storage_key: TenantStorageKey::tenant(event.tenant_id),
            event,
            previous_digest: audit.previous_digest,
            event_digest: audit.event_digest,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn application_approval_actions_have_explicit_audit_contracts() {
        assert_eq!(
            approval_transition_audit_action(ApprovalAction::Approve),
            Some("governance.policy.approval.vote")
        );
        assert_eq!(
            approval_transition_audit_action(ApprovalAction::Reject),
            Some("governance.policy.approval.reject")
        );
        assert_eq!(
            approval_transition_audit_action(ApprovalAction::Cancel),
            Some("governance.policy.approval.cancel")
        );
        assert_eq!(
            approval_transition_audit_action(ApprovalAction::Activate),
            None
        );
    }
}
