//! Executing governance lifecycle use cases behind the control-plane boundary.

use std::error::Error;
use std::fmt;

use prodex_control_plane::{ControlPlaneActionPlan, ControlPlaneOperation};
use prodex_domain::{
    ApprovalAction, ApprovalFingerprint, ApprovalKind, ApprovalRecord, ApprovalScope,
    ApprovalState, PolicyEffect, Principal, TenantId, execution_approval_id,
};
use prodex_storage::{
    ApprovalVoteIdempotency, ApprovalVoteMutationOutcome, ApprovalVoteRequest,
    AuditOutboxWriteCommand, GovernanceActivationAction, GovernanceActivationRequest,
    GovernanceActivationResult, GovernanceArtifactKind, GovernanceArtifactValidationInput,
    GovernanceMutationIdempotency, GovernanceRepositoryError, GovernanceRevisionWriteCommand,
    GovernanceWriteOutcome,
    governance_support::{
        approval_kind_for_artifact, artifact_kind_for_approval, artifact_kind_label,
    },
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApplicationGovernanceLifecycleError {
    InvalidAction,
    Repository(GovernanceRepositoryError),
}

impl fmt::Display for ApplicationGovernanceLifecycleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "governance lifecycle operation failed")
    }
}

impl Error for ApplicationGovernanceLifecycleError {}

pub trait ApplicationGovernanceRepository {
    fn write_revision_idempotent(
        &self,
        command: GovernanceRevisionWriteCommand,
        audit_outbox: AuditOutboxWriteCommand,
        idempotency: GovernanceMutationIdempotency,
    ) -> Result<GovernanceWriteOutcome, GovernanceRepositoryError>;

    fn create_approval(
        &self,
        approval: prodex_domain::ApprovalRecord,
        audit_outbox: AuditOutboxWriteCommand,
    ) -> Result<GovernanceWriteOutcome, GovernanceRepositoryError>;

    fn create_approval_idempotent(
        &self,
        approval: prodex_domain::ApprovalRecord,
        audit_outbox: AuditOutboxWriteCommand,
        idempotency: GovernanceMutationIdempotency,
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
        idempotency: ApprovalVoteIdempotency,
    ) -> Result<ApprovalVoteMutationOutcome, GovernanceRepositoryError>;

    fn get_approval(
        &self,
        tenant_id: TenantId,
        approval_id: &prodex_domain::ApprovalId,
    ) -> Result<ApprovalRecord, GovernanceRepositoryError>;

    fn activate_revision(
        &self,
        request: GovernanceActivationRequest,
        validate_artifact: &mut dyn FnMut(&GovernanceArtifactValidationInput<'_>) -> bool,
    ) -> Result<GovernanceActivationResult, GovernanceRepositoryError>;
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

pub struct ApplicationGovernanceLifecycleService<'a, R: ApplicationGovernanceRepository + ?Sized> {
    repository: &'a R,
}

impl<R: ApplicationGovernanceRepository + ?Sized> fmt::Debug
    for ApplicationGovernanceLifecycleService<'_, R>
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationGovernanceLifecycleService")
            .field("repository", &"<redacted>")
            .finish()
    }
}

impl<'a, R: ApplicationGovernanceRepository + ?Sized> ApplicationGovernanceLifecycleService<'a, R> {
    pub fn new(repository: &'a R) -> Self {
        Self { repository }
    }

    pub fn write_revision(
        &self,
        action: &ControlPlaneActionPlan,
        revision: GovernanceRevisionWriteCommand,
        idempotency: GovernanceMutationIdempotency,
        audit_outbox: AuditOutboxWriteCommand,
    ) -> Result<GovernanceWriteOutcome, ApplicationGovernanceLifecycleError> {
        if revision.tenant_id != action.tenant.tenant_id
            || idempotency.operation.tenant_id != revision.tenant_id
        {
            return Err(ApplicationGovernanceLifecycleError::InvalidAction);
        }
        let label = artifact_kind_label(revision.kind);
        validate_action(
            action,
            ControlPlaneOperation::PolicyCreate,
            revision.tenant_id,
            &audit_outbox,
            &format!("governance.{label}.revision.write"),
            &format!("governance_{label}_revision"),
            Some(&revision.revision_id),
        )?;
        self.repository
            .write_revision_idempotent(revision, audit_outbox, idempotency)
            .map_err(ApplicationGovernanceLifecycleError::Repository)
    }

    pub fn create_approval(
        &self,
        action: &ControlPlaneActionPlan,
        approval: prodex_domain::ApprovalRecord,
        idempotency: GovernanceMutationIdempotency,
        audit_outbox: AuditOutboxWriteCommand,
    ) -> Result<GovernanceWriteOutcome, ApplicationGovernanceLifecycleError> {
        let kind = artifact_kind_for_approval(approval.kind)
            .map_err(|_| ApplicationGovernanceLifecycleError::InvalidAction)?;
        if approval.tenant_id != action.tenant.tenant_id
            || idempotency.operation.tenant_id != approval.tenant_id
            || approval.maker != action.audit_event.principal_id
        {
            return Err(ApplicationGovernanceLifecycleError::InvalidAction);
        }
        let label = artifact_kind_label(kind);
        validate_action(
            action,
            ControlPlaneOperation::PolicySubmit,
            approval.tenant_id,
            &audit_outbox,
            &format!("governance.{label}.approval.create"),
            &format!("governance_{label}_revision"),
            Some(approval.id.as_str()),
        )?;
        self.repository
            .create_approval_idempotent(approval, audit_outbox, idempotency)
            .map_err(ApplicationGovernanceLifecycleError::Repository)
    }

    pub fn transition_approval(
        &self,
        action: &ControlPlaneActionPlan,
        kind: GovernanceArtifactKind,
        request: ApprovalVoteRequest,
        approval_action: ApprovalAction,
        idempotency: GovernanceMutationIdempotency,
    ) -> Result<ApprovalVoteMutationOutcome, ApplicationGovernanceLifecycleError> {
        if request.tenant_id != action.tenant.tenant_id
            || idempotency.operation.tenant_id != request.tenant_id
            || request.actor.id != action.audit_event.principal_id
            || self
                .repository
                .get_approval(request.tenant_id, &request.approval_id)
                .map_err(ApplicationGovernanceLifecycleError::Repository)?
                .kind
                != approval_kind_for_artifact(kind)
        {
            return Err(ApplicationGovernanceLifecycleError::InvalidAction);
        }
        let audit_action = approval_transition_audit_action(kind, approval_action)
            .ok_or(ApplicationGovernanceLifecycleError::InvalidAction)?;
        let label = artifact_kind_label(kind);
        validate_action(
            action,
            ControlPlaneOperation::PolicyVote,
            request.tenant_id,
            &request.audit_outbox,
            &audit_action,
            &format!("governance_{label}_revision"),
            Some(request.approval_id.as_str()),
        )?;
        self.repository
            .transition_approval_idempotent(request, approval_action, idempotency)
            .map_err(ApplicationGovernanceLifecycleError::Repository)
    }

    pub fn activate_revision(
        &self,
        action: &ControlPlaneActionPlan,
        request: GovernanceActivationRequest,
        validate_artifact: impl FnMut(&GovernanceArtifactValidationInput<'_>) -> bool,
    ) -> Result<GovernanceActivationResult, ApplicationGovernanceLifecycleError> {
        if request.tenant_id != action.tenant.tenant_id
            || request.actor.id != action.audit_event.principal_id
        {
            return Err(ApplicationGovernanceLifecycleError::InvalidAction);
        }
        let label = artifact_kind_label(request.kind);
        let audit_action = match request.action {
            GovernanceActivationAction::Activate => {
                format!("governance.{label}.revision.activate")
            }
            GovernanceActivationAction::Rollback => {
                format!("governance.{label}.revision.rollback")
            }
            GovernanceActivationAction::Revoke => {
                format!("governance.{label}.revision.revoke")
            }
        };
        validate_action(
            action,
            match request.action {
                GovernanceActivationAction::Activate => ControlPlaneOperation::PolicyActivate,
                GovernanceActivationAction::Rollback => ControlPlaneOperation::PolicyRollback,
                GovernanceActivationAction::Revoke => ControlPlaneOperation::PolicyRevoke,
            },
            request.tenant_id,
            &request.audit_outbox,
            &audit_action,
            &format!("governance_{label}_revision"),
            Some(&request.revision_id),
        )?;
        let mut validate_artifact = validate_artifact;
        self.repository
            .activate_revision(request, &mut validate_artifact)
            .map_err(ApplicationGovernanceLifecycleError::Repository)
    }
}

fn approval_transition_audit_action(
    kind: GovernanceArtifactKind,
    action: ApprovalAction,
) -> Option<String> {
    let action = match action {
        ApprovalAction::Approve => "approve",
        ApprovalAction::Reject => "reject",
        ApprovalAction::Cancel => "cancel",
        ApprovalAction::Activate | ApprovalAction::Supersede | ApprovalAction::RollBack => {
            return None;
        }
    };
    Some(format!(
        "governance.{}.approval.{action}",
        artifact_kind_label(kind)
    ))
}

fn validate_action(
    action: &ControlPlaneActionPlan,
    operation: ControlPlaneOperation,
    tenant_id: TenantId,
    audit: &AuditOutboxWriteCommand,
    audit_action: &str,
    resource_kind: &str,
    resource_id: Option<&str>,
) -> Result<(), ApplicationGovernanceLifecycleError> {
    let event = &audit.audit.event;
    if action.operation != operation
        || action.requirement != operation.requirement()
        || action.tenant.tenant_id != tenant_id
        || action.audit_write.tenant_partition_key != tenant_id
        || action.audit_write.event != action.audit_event
        || action.audit_event.tenant_id != tenant_id
        || audit.audit.storage_key.tenant_id != tenant_id
        || event.tenant_id != tenant_id
        || event.principal_id != action.audit_event.principal_id
        || event.action.as_str() != audit_action
        || event.resource.kind != resource_kind
        || event.resource.id.as_deref() != resource_id
        || event.resource.tenant_id != Some(tenant_id)
    {
        return Err(ApplicationGovernanceLifecycleError::InvalidAction);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn application_approval_actions_have_explicit_audit_contracts() {
        assert_eq!(
            approval_transition_audit_action(
                GovernanceArtifactKind::Policy,
                ApprovalAction::Approve
            ),
            Some("governance.policy.approval.approve".to_string())
        );
        assert_eq!(
            approval_transition_audit_action(
                GovernanceArtifactKind::ClassificationRules,
                ApprovalAction::Reject
            ),
            Some("governance.classification_rules.approval.reject".to_string())
        );
        assert_eq!(
            approval_transition_audit_action(
                GovernanceArtifactKind::ProviderRegistry,
                ApprovalAction::Cancel
            ),
            Some("governance.provider_registry.approval.cancel".to_string())
        );
        assert_eq!(
            approval_transition_audit_action(
                GovernanceArtifactKind::RoutingScores,
                ApprovalAction::Activate
            ),
            None
        );
    }
}
