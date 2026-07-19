use prodex_domain::ApprovalKind;
use prodex_observability::{PolicyLifecycleOperation, PolicyLifecycleResult};
use prodex_storage::GovernanceArtifactKind;

#[derive(Clone, Copy)]
pub(super) enum RuntimeGovernanceResource {
    Policy,
    ClassificationRules,
    ProviderRegistry,
    RoutingScores,
}

impl RuntimeGovernanceResource {
    pub(super) const ALL: [Self; 4] = [
        Self::Policy,
        Self::ClassificationRules,
        Self::ProviderRegistry,
        Self::RoutingScores,
    ];

    pub(super) fn prefix_suffix(self) -> &'static str {
        match self {
            Self::Policy => "/policies",
            Self::ClassificationRules => "/classification-rules",
            Self::ProviderRegistry => "/provider-registries",
            Self::RoutingScores => "/routing-scores",
        }
    }

    pub(super) fn kind(self) -> GovernanceArtifactKind {
        match self {
            Self::Policy => GovernanceArtifactKind::Policy,
            Self::ClassificationRules => GovernanceArtifactKind::ClassificationRules,
            Self::ProviderRegistry => GovernanceArtifactKind::ProviderRegistry,
            Self::RoutingScores => GovernanceArtifactKind::RoutingScores,
        }
    }

    pub(super) fn approval_kind(self) -> ApprovalKind {
        match self {
            Self::Policy => ApprovalKind::PolicyRevision,
            Self::ClassificationRules => ApprovalKind::ClassificationRuleRevision,
            Self::ProviderRegistry => ApprovalKind::ProviderRegistryRevision,
            Self::RoutingScores => ApprovalKind::RoutingScoreRevision,
        }
    }

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Policy => "policy",
            Self::ClassificationRules => "classification_rules",
            Self::ProviderRegistry => "provider_registry",
            Self::RoutingScores => "routing_scores",
        }
    }
}

pub(super) fn policy_activation_operation(action: &str) -> PolicyLifecycleOperation {
    if action == "activate" {
        PolicyLifecycleOperation::Publish
    } else {
        PolicyLifecycleOperation::Invalidate
    }
}

pub(super) fn record_policy_lifecycle(
    resource: RuntimeGovernanceResource,
    operation: PolicyLifecycleOperation,
    result: PolicyLifecycleResult,
) {
    if matches!(resource, RuntimeGovernanceResource::Policy) {
        crate::record_runtime_policy_lifecycle_metric(operation, result);
    }
}
