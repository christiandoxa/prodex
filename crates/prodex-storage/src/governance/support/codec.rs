use crate::{GovernanceArtifactKind, GovernanceRepositoryError};
use prodex_domain::{ApprovalKind, ApprovalState, Channel, CredentialScope, DataClassification};

pub fn revision_table(kind: GovernanceArtifactKind) -> &'static str {
    match kind {
        GovernanceArtifactKind::Policy => "prodex_policy_revisions",
        GovernanceArtifactKind::ClassificationRules => "prodex_classification_rule_revisions",
        GovernanceArtifactKind::ProviderRegistry => "prodex_provider_registry_revisions",
        GovernanceArtifactKind::RoutingScores => "prodex_routing_score_revisions",
    }
}

pub fn artifact_kind_for_approval(
    kind: ApprovalKind,
) -> Result<GovernanceArtifactKind, GovernanceRepositoryError> {
    match kind {
        ApprovalKind::PolicyRevision => Ok(GovernanceArtifactKind::Policy),
        ApprovalKind::ClassificationRuleRevision => Ok(GovernanceArtifactKind::ClassificationRules),
        ApprovalKind::ProviderRegistryRevision => Ok(GovernanceArtifactKind::ProviderRegistry),
        ApprovalKind::RoutingScoreRevision => Ok(GovernanceArtifactKind::RoutingScores),
        ApprovalKind::HighImpactConfiguration
        | ApprovalKind::Execution
        | ApprovalKind::BreakGlass => Err(GovernanceRepositoryError::InvalidInput),
    }
}

pub fn approval_artifact_kind(
    approval_kind: ApprovalKind,
) -> Result<Option<GovernanceArtifactKind>, GovernanceRepositoryError> {
    match approval_kind {
        ApprovalKind::Execution | ApprovalKind::BreakGlass => Ok(None),
        ApprovalKind::HighImpactConfiguration => Err(GovernanceRepositoryError::InvalidInput),
        _ => artifact_kind_for_approval(approval_kind).map(Some),
    }
}

pub fn approval_kind_for_artifact(kind: GovernanceArtifactKind) -> ApprovalKind {
    match kind {
        GovernanceArtifactKind::Policy => ApprovalKind::PolicyRevision,
        GovernanceArtifactKind::ClassificationRules => ApprovalKind::ClassificationRuleRevision,
        GovernanceArtifactKind::ProviderRegistry => ApprovalKind::ProviderRegistryRevision,
        GovernanceArtifactKind::RoutingScores => ApprovalKind::RoutingScoreRevision,
    }
}

pub fn artifact_kind_label(kind: GovernanceArtifactKind) -> &'static str {
    match kind {
        GovernanceArtifactKind::Policy => "policy",
        GovernanceArtifactKind::ClassificationRules => "classification_rules",
        GovernanceArtifactKind::ProviderRegistry => "provider_registry",
        GovernanceArtifactKind::RoutingScores => "routing_scores",
    }
}

pub fn channel_label(channel: Channel) -> &'static str {
    match channel {
        Channel::Cli => "cli",
        Channel::Ide => "ide",
        Channel::Api => "api",
        Channel::Mcp => "mcp",
        Channel::InternalService => "internal_service",
    }
}

pub fn channel_from_label(value: &str) -> Result<Channel, GovernanceRepositoryError> {
    match value {
        "cli" => Ok(Channel::Cli),
        "ide" => Ok(Channel::Ide),
        "api" => Ok(Channel::Api),
        "mcp" => Ok(Channel::Mcp),
        "internal_service" => Ok(Channel::InternalService),
        _ => Err(GovernanceRepositoryError::Database),
    }
}

pub fn credential_scope_label(scope: CredentialScope) -> &'static str {
    match scope {
        CredentialScope::DataPlane => "data_plane",
        CredentialScope::ControlPlane => "control_plane",
        CredentialScope::BreakGlass => "break_glass",
    }
}

pub fn credential_scope_from_label(
    value: &str,
) -> Result<CredentialScope, GovernanceRepositoryError> {
    match value {
        "data_plane" => Ok(CredentialScope::DataPlane),
        "control_plane" => Ok(CredentialScope::ControlPlane),
        "break_glass" => Ok(CredentialScope::BreakGlass),
        _ => Err(GovernanceRepositoryError::Database),
    }
}

pub fn classification_from_label(
    value: &str,
) -> Result<DataClassification, GovernanceRepositoryError> {
    match value {
        "public" => Ok(DataClassification::Public),
        "internal" => Ok(DataClassification::Internal),
        "confidential" => Ok(DataClassification::Confidential),
        "restricted" => Ok(DataClassification::Restricted),
        _ => Err(GovernanceRepositoryError::Database),
    }
}

pub fn approval_kind_label(kind: ApprovalKind) -> &'static str {
    match kind {
        ApprovalKind::PolicyRevision => "policy_revision",
        ApprovalKind::ClassificationRuleRevision => "classification_rule_revision",
        ApprovalKind::ProviderRegistryRevision => "provider_registry_revision",
        ApprovalKind::RoutingScoreRevision => "routing_score_revision",
        ApprovalKind::HighImpactConfiguration => "high_impact_configuration",
        ApprovalKind::Execution => "execution",
        ApprovalKind::BreakGlass => "break_glass",
    }
}

pub fn approval_kind_from_label(value: &str) -> Result<ApprovalKind, GovernanceRepositoryError> {
    match value {
        "policy_revision" => Ok(ApprovalKind::PolicyRevision),
        "classification_rule_revision" => Ok(ApprovalKind::ClassificationRuleRevision),
        "provider_registry_revision" => Ok(ApprovalKind::ProviderRegistryRevision),
        "routing_score_revision" => Ok(ApprovalKind::RoutingScoreRevision),
        "high_impact_configuration" => Ok(ApprovalKind::HighImpactConfiguration),
        "execution" => Ok(ApprovalKind::Execution),
        "break_glass" => Ok(ApprovalKind::BreakGlass),
        _ => Err(GovernanceRepositoryError::Database),
    }
}

pub fn approval_state_label(state: ApprovalState) -> &'static str {
    match state {
        ApprovalState::Draft => "draft",
        ApprovalState::PendingApproval => "pending_approval",
        ApprovalState::Approved => "approved",
        ApprovalState::Rejected => "rejected",
        ApprovalState::Expired => "expired",
        ApprovalState::Cancelled => "cancelled",
        ApprovalState::Active => "active",
        ApprovalState::Superseded => "superseded",
        ApprovalState::RolledBack => "rolled_back",
    }
}

pub fn approval_state_from_label(value: &str) -> Result<ApprovalState, GovernanceRepositoryError> {
    match value {
        "draft" => Ok(ApprovalState::Draft),
        "pending_approval" => Ok(ApprovalState::PendingApproval),
        "approved" => Ok(ApprovalState::Approved),
        "rejected" => Ok(ApprovalState::Rejected),
        "expired" => Ok(ApprovalState::Expired),
        "cancelled" => Ok(ApprovalState::Cancelled),
        "active" => Ok(ApprovalState::Active),
        "superseded" => Ok(ApprovalState::Superseded),
        "rolled_back" => Ok(ApprovalState::RolledBack),
        _ => Err(GovernanceRepositoryError::Database),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn storage_codecs_round_trip_all_variants() {
        for channel in [
            Channel::Cli,
            Channel::Ide,
            Channel::Api,
            Channel::Mcp,
            Channel::InternalService,
        ] {
            assert_eq!(channel_from_label(channel_label(channel)), Ok(channel));
        }
        for scope in [
            CredentialScope::DataPlane,
            CredentialScope::ControlPlane,
            CredentialScope::BreakGlass,
        ] {
            assert_eq!(
                credential_scope_from_label(credential_scope_label(scope)),
                Ok(scope)
            );
        }
        for kind in [
            ApprovalKind::PolicyRevision,
            ApprovalKind::ClassificationRuleRevision,
            ApprovalKind::ProviderRegistryRevision,
            ApprovalKind::RoutingScoreRevision,
            ApprovalKind::HighImpactConfiguration,
            ApprovalKind::Execution,
            ApprovalKind::BreakGlass,
        ] {
            assert_eq!(
                approval_kind_from_label(approval_kind_label(kind)),
                Ok(kind)
            );
        }
        for state in [
            ApprovalState::Draft,
            ApprovalState::PendingApproval,
            ApprovalState::Approved,
            ApprovalState::Rejected,
            ApprovalState::Expired,
            ApprovalState::Cancelled,
            ApprovalState::Active,
            ApprovalState::Superseded,
            ApprovalState::RolledBack,
        ] {
            assert_eq!(
                approval_state_from_label(approval_state_label(state)),
                Ok(state)
            );
        }
        assert_eq!(
            classification_from_label("restricted"),
            Ok(DataClassification::Restricted)
        );
        assert_eq!(
            classification_from_label("unknown"),
            Err(GovernanceRepositoryError::Database)
        );
    }

    #[test]
    fn non_revision_approval_kinds_are_explicit() {
        assert_eq!(approval_artifact_kind(ApprovalKind::Execution), Ok(None));
        assert_eq!(approval_artifact_kind(ApprovalKind::BreakGlass), Ok(None));
        assert_eq!(
            approval_artifact_kind(ApprovalKind::HighImpactConfiguration),
            Err(GovernanceRepositoryError::InvalidInput)
        );
    }
}
