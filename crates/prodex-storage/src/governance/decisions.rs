use super::*;
use prodex_domain::{
    ApprovalAction, ApprovalKind, ApprovalRecord, ApprovalState, ApprovalTransition,
    ApprovalTransitionRequest, AuditOutcome, compute_audit_chain_digest, transition_approval,
};
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApprovalVoteTransitionDecision {
    Denied(ApprovalVoteStableDenial),
    Transition(Box<ApprovalTransition>),
}

pub fn plan_approval_vote_transition(
    current: &ApprovalRecord,
    request: &ApprovalVoteRequest,
    action: ApprovalAction,
) -> Result<ApprovalVoteTransitionDecision, GovernanceRepositoryError> {
    let supported = match action {
        ApprovalAction::Approve | ApprovalAction::Reject | ApprovalAction::Cancel => true,
        ApprovalAction::Activate => matches!(
            current.kind,
            ApprovalKind::Execution | ApprovalKind::BreakGlass
        ),
        ApprovalAction::Supersede => current.kind == ApprovalKind::BreakGlass,
        ApprovalAction::RollBack => false,
    };
    if !supported {
        return Err(GovernanceRepositoryError::InvalidInput);
    }
    if !matches!(
        (current.kind, action),
        (ApprovalKind::Execution, ApprovalAction::Activate)
    ) {
        crate::governance_support::require_control_plane_admin(request.tenant_id, &request.actor)?;
    }
    match transition_approval(ApprovalTransitionRequest {
        record: current,
        actor: &request.actor,
        action,
        expected_version: request.expected_version,
        now_unix_ms: request.now_unix_ms,
        reason: request.reason.as_ref(),
    }) {
        Ok(transition) => Ok(ApprovalVoteTransitionDecision::Transition(Box::new(
            transition,
        ))),
        Err(error) => match ApprovalVoteStableDenial::from_approval_error(error) {
            Some(denial) => Ok(ApprovalVoteTransitionDecision::Denied(denial)),
            None => Err(crate::governance_support::approval_transition_error(error)),
        },
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GovernanceRevisionLifecycleUpdate {
    pub kind: GovernanceArtifactKind,
    pub state: &'static str,
}

pub fn plan_approval_revision_lifecycle_update(
    record: &ApprovalRecord,
) -> Result<Option<GovernanceRevisionLifecycleUpdate>, GovernanceRepositoryError> {
    let Some(kind) = crate::governance_support::approval_artifact_kind(record.kind)? else {
        return Ok(None);
    };
    let state = match record.state {
        ApprovalState::Approved => "approved",
        ApprovalState::Rejected => "rejected",
        _ => return Ok(None),
    };
    Ok(Some(GovernanceRevisionLifecycleUpdate { kind, state }))
}

pub fn denied_approval_audit_outbox(
    mut command: AuditOutboxWriteCommand,
    denial: ApprovalVoteStableDenial,
) -> AuditOutboxWriteCommand {
    command.audit.event.outcome = AuditOutcome::Denied;
    command.audit.event.reason_code = Some(denial.reason_code().to_string());
    command.audit.event_digest =
        compute_audit_chain_digest(command.audit.previous_digest.as_ref(), &command.audit.event);
    command
}

#[derive(Clone, Copy)]
pub struct GovernanceActivationCurrent<'a> {
    pub revision_checksum: &'a str,
    pub approval: &'a ApprovalRecord,
    pub active_revision_id: Option<&'a str>,
    pub last_known_good_revision_id: Option<&'a str>,
    pub etag: Option<&'a str>,
}

impl fmt::Debug for GovernanceActivationCurrent<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GovernanceActivationCurrent")
            .field("revision_checksum", &"<redacted>")
            .field("approval", &"<redacted>")
            .field("active_revision_id", &"<redacted>")
            .field("last_known_good_revision_id", &"<redacted>")
            .field("etag", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct GovernanceActivationPlan {
    pub previous_active_revision_id: Option<String>,
    pub last_known_good_revision_id: String,
    pub etag: String,
    pub previous_revision_state: Option<&'static str>,
    pub activated_approval: ApprovalRecord,
}

impl fmt::Debug for GovernanceActivationPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GovernanceActivationPlan")
            .field("previous_active_revision_id", &"<redacted>")
            .field("last_known_good_revision_id", &"<redacted>")
            .field("etag", &"<redacted>")
            .field("previous_revision_state", &self.previous_revision_state)
            .field("activated_approval", &"<redacted>")
            .finish()
    }
}

pub fn plan_governance_activation(
    request: &GovernanceActivationRequest,
    current: GovernanceActivationCurrent<'_>,
) -> Result<GovernanceActivationPlan, GovernanceRepositoryError> {
    crate::governance_support::validate_governance_activation_request(request)?;
    if current.approval.tenant_id != request.tenant_id {
        return Err(GovernanceRepositoryError::TenantMismatch);
    }
    if current.approval.state != ApprovalState::Approved
        || current.approval.kind
            != crate::governance_support::approval_kind_for_artifact(request.kind)
        || current.approval.fingerprint.as_str() != current.revision_checksum
    {
        return Err(GovernanceRepositoryError::ApprovalRequired);
    }
    if current.etag != request.expected_etag.as_deref() {
        return Err(GovernanceRepositoryError::EtagMismatch);
    }
    if request.action == GovernanceActivationAction::Rollback
        && current.last_known_good_revision_id != Some(request.revision_id.as_str())
    {
        return Err(GovernanceRepositoryError::Conflict);
    }

    let previous_active_revision_id = current.active_revision_id.map(str::to_owned);
    let last_known_good_revision_id = match request.action {
        GovernanceActivationAction::Activate => previous_active_revision_id
            .clone()
            .or_else(|| current.last_known_good_revision_id.map(str::to_owned))
            .unwrap_or_else(|| request.revision_id.clone()),
        GovernanceActivationAction::Rollback => request.revision_id.clone(),
    };
    let activated_approval = transition_approval(ApprovalTransitionRequest {
        record: current.approval,
        actor: &request.actor,
        action: ApprovalAction::Activate,
        expected_version: current.approval.version,
        now_unix_ms: request.activated_at_unix_ms,
        reason: None,
    })
    .map_err(|_| GovernanceRepositoryError::ApprovalRequired)?
    .record;
    let previous_revision_state = current
        .active_revision_id
        .filter(|previous| *previous != request.revision_id)
        .map(|_| match request.action {
            GovernanceActivationAction::Activate => "superseded",
            GovernanceActivationAction::Rollback => "rolled_back",
        });
    Ok(GovernanceActivationPlan {
        previous_active_revision_id,
        last_known_good_revision_id,
        etag: crate::governance_support::activation_etag(request, current.etag),
        previous_revision_state,
        activated_approval,
    })
}
