//! Executing policy-governance use case behind the existing control-plane boundary.

use std::error::Error;
use std::fmt;

use prodex_control_plane::{
    ControlPlaneActionRequest, ControlPlaneAuthorizationError, ControlPlaneDecision,
    ControlPlaneOperation, decide_control_plane_action,
};
use prodex_domain::{
    ApprovalAction, AuditAction, AuditDigest, AuditEvent, AuditEventId, AuditResource, ResourceKind,
};
use prodex_storage::{
    AppendOnlyAuditCommand, ApprovalVoteRequest, AuditOutboxWriteCommand,
    GovernanceActivationAction, GovernanceActivationRequest, GovernanceActivationResult,
    GovernanceArtifactKind, GovernanceRepositoryError, GovernanceRevisionWriteCommand,
    GovernanceWriteOutcome, TenantStorageKey,
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
        request.audit_outbox = self.authorize(
            action,
            audit,
            "governance.policy.approval.vote",
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
