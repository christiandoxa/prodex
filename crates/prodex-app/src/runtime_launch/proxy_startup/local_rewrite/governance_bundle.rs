use super::super::local_rewrite_classification_rules::{
    RuntimeClassificationRulesSnapshot, RuntimeClassificationRulesSnapshotSet,
};
use super::super::local_rewrite_provider_registry::{
    RuntimeGatewayGovernedProviderRegistrySnapshot, RuntimeGatewayProviderRegistrySnapshotSet,
    RuntimeGatewayRoutingScoresSnapshot, RuntimeGatewayRoutingScoresSnapshotSet,
};
use crate::runtime_governance::{
    RuntimeGovernanceAuthoritySnapshot, RuntimeGovernanceAuthoritySnapshotSet,
};
use anyhow::Result;
use prodex_domain::TenantId;
use std::sync::Arc;

#[derive(Clone, Debug)]
pub(in crate::runtime_launch::proxy_startup) struct RuntimeGovernanceSnapshotBundleSet {
    pub(super) policy: RuntimeGovernanceAuthoritySnapshotSet,
    pub(super) classification: RuntimeClassificationRulesSnapshotSet,
    pub(super) provider_registry: RuntimeGatewayProviderRegistrySnapshotSet,
    pub(super) routing_scores: RuntimeGatewayRoutingScoresSnapshotSet,
}

#[derive(Clone)]
pub(in crate::runtime_launch::proxy_startup) struct RuntimeGovernanceRequestSnapshot {
    pub(in crate::runtime_launch::proxy_startup) policy: Arc<RuntimeGovernanceAuthoritySnapshot>,
    pub(in crate::runtime_launch::proxy_startup) classification:
        Arc<RuntimeClassificationRulesSnapshot>,
    pub(in crate::runtime_launch::proxy_startup) provider_registry:
        Arc<RuntimeGatewayGovernedProviderRegistrySnapshot>,
    pub(in crate::runtime_launch::proxy_startup) routing_scores:
        Arc<RuntimeGatewayRoutingScoresSnapshot>,
}

impl RuntimeGovernanceSnapshotBundleSet {
    pub(super) fn new(
        policy: RuntimeGovernanceAuthoritySnapshotSet,
        classification: RuntimeClassificationRulesSnapshotSet,
        provider_registry: RuntimeGatewayProviderRegistrySnapshotSet,
        routing_scores: RuntimeGatewayRoutingScoresSnapshotSet,
    ) -> Self {
        Self {
            policy,
            classification,
            provider_registry,
            routing_scores,
        }
    }

    pub(in crate::runtime_launch::proxy_startup) fn snapshot_for(
        &self,
        tenant_id: TenantId,
    ) -> Option<RuntimeGovernanceRequestSnapshot> {
        let snapshot = RuntimeGovernanceRequestSnapshot {
            policy: self.policy.snapshot_for(tenant_id)?,
            classification: self.classification.snapshot_for(tenant_id)?,
            provider_registry: self.provider_registry.snapshot_for(tenant_id)?,
            routing_scores: self.routing_scores.snapshot_for(tenant_id)?,
        };
        (!snapshot.policy.config.mode.is_enforcing() || snapshot.pins_match()).then_some(snapshot)
    }

    pub(super) fn with_tenant_snapshot(
        &self,
        tenant_id: TenantId,
        snapshot: RuntimeGovernanceRequestSnapshot,
    ) -> Result<Self> {
        Ok(Self {
            policy: self
                .policy
                .with_tenant_snapshot(tenant_id, Arc::unwrap_or_clone(snapshot.policy))?,
            classification: self
                .classification
                .with_tenant_snapshot(tenant_id, Arc::unwrap_or_clone(snapshot.classification))?,
            provider_registry: self.provider_registry.with_tenant_snapshot(
                tenant_id,
                Arc::unwrap_or_clone(snapshot.provider_registry),
            )?,
            routing_scores: self
                .routing_scores
                .with_tenant_snapshot(tenant_id, Arc::unwrap_or_clone(snapshot.routing_scores))?,
        })
    }

    pub(super) fn without_tenant_snapshot(&self, tenant_id: TenantId) -> Option<Self> {
        let policy = self.policy.without_tenant_snapshot(tenant_id);
        let classification = self.classification.without_tenant_snapshot(tenant_id);
        let provider_registry = self.provider_registry.without_tenant_snapshot(tenant_id);
        let routing_scores = self.routing_scores.without_tenant_snapshot(tenant_id);
        (policy.is_some()
            || classification.is_some()
            || provider_registry.is_some()
            || routing_scores.is_some())
        .then(|| Self {
            policy: policy.unwrap_or_else(|| self.policy.clone()),
            classification: classification.unwrap_or_else(|| self.classification.clone()),
            provider_registry: provider_registry.unwrap_or_else(|| self.provider_registry.clone()),
            routing_scores: routing_scores.unwrap_or_else(|| self.routing_scores.clone()),
        })
    }

    pub(in crate::runtime_launch::proxy_startup) fn policies_are_servable(
        &self,
        tenant_ids: &[TenantId],
        now_unix_ms: u64,
    ) -> bool {
        !tenant_ids.is_empty()
            && tenant_ids.iter().all(|tenant_id| {
                self.snapshot_for(*tenant_id).is_some_and(|snapshot| {
                    snapshot.policy.application.policy.is_valid_at(now_unix_ms)
                })
            })
    }
}

impl RuntimeGovernanceRequestSnapshot {
    fn pins_match(&self) -> bool {
        let pinned_classification = &self.policy.application.classification_rules;
        self.classification.classification_rules().revision() == pinned_classification.revision()
            && self.classification.classification_rules().checksum()
                == pinned_classification.checksum()
            && self.provider_registry.revision() == self.policy.provider_registry_revision
            && self.routing_scores.revision == self.policy.routing_score_revision
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime_launch::proxy_startup::local_rewrite::RuntimeLocalRewriteProviderOptions;
    use prodex_runtime_policy::{
        RuntimeGovernanceDataClassification, RuntimeGovernanceMode,
        RuntimeGovernancePolicyFailureMode, RuntimeGovernanceProviderTrustTier,
        RuntimeGovernanceRolloutMode, RuntimeGovernanceUnknownClassificationBehavior,
        RuntimePolicyGovernanceProviderSettings, RuntimePolicyGovernanceSessionSettings,
        RuntimePolicyGovernanceSettings,
    };

    fn settings(
        mode: RuntimeGovernanceMode,
        classification_revision: &str,
        classification_checksum: &str,
        provider_registry_revision: u64,
        routing_score_revision: u64,
    ) -> RuntimePolicyGovernanceSettings {
        let policy_revision = prodex_domain::PolicyRevisionId::new();
        let enforcing = matches!(
            mode,
            RuntimeGovernanceMode::EnterpriseEnforce | RuntimeGovernanceMode::BankEnforce
        );
        let rollout = if enforcing {
            RuntimeGovernanceRolloutMode::Enforce
        } else {
            RuntimeGovernanceRolloutMode::default()
        };
        RuntimePolicyGovernanceSettings {
            mode,
            inspection: rollout,
            classification: rollout,
            policy: rollout,
            routing: rollout,
            mandatory_audit: enforcing,
            policy_revision: Some(policy_revision),
            policy_valid_until_unix_ms: Some(u64::MAX),
            classification_revision: Some(classification_revision.to_string()),
            classification_checksum: Some(classification_checksum.to_string()),
            provider_registry_revision: Some(provider_registry_revision),
            routing_score_revision: Some(routing_score_revision),
            provider: Some(RuntimePolicyGovernanceProviderSettings {
                descriptor_revision: 1,
                enabled: true,
                revoked: false,
                trust_tier: RuntimeGovernanceProviderTrustTier::Enterprise,
                local_execution: false,
                maximum_classification: RuntimeGovernanceDataClassification::Restricted,
                regions: vec!["test-region".to_string()],
                retention_seconds: 0,
                training_use: false,
            }),
            classification_unknown: if enforcing {
                RuntimeGovernanceUnknownClassificationBehavior::Deny
            } else {
                RuntimeGovernanceUnknownClassificationBehavior::UseDefault
            },
            policy_failure_mode: if enforcing {
                RuntimeGovernancePolicyFailureMode::Closed
            } else {
                RuntimeGovernancePolicyFailureMode::Open
            },
            active_policy_revision: enforcing.then_some(policy_revision),
            session: RuntimePolicyGovernanceSessionSettings {
                absolute_timeout_seconds: enforcing.then_some(3_600),
                idle_timeout_seconds: enforcing.then_some(900),
                max_concurrent: enforcing.then_some(10),
            },
            ..RuntimePolicyGovernanceSettings::default()
        }
    }

    fn bundle(settings: &RuntimePolicyGovernanceSettings) -> RuntimeGovernanceSnapshotBundleSet {
        let provider = RuntimeLocalRewriteProviderOptions::OpenAiResponses {
            api_keys: vec!["test-key".to_string()],
        };
        RuntimeGovernanceSnapshotBundleSet::new(
            RuntimeGovernanceAuthoritySnapshotSet::bootstrap(
                crate::runtime_governance::compile_runtime_governance_settings(settings).unwrap(),
                true,
            ),
            RuntimeClassificationRulesSnapshotSet::bootstrap(settings, true).unwrap(),
            RuntimeGatewayProviderRegistrySnapshotSet::bootstrap(
                super::super::super::local_rewrite_provider_registry::runtime_gateway_bootstrap_provider_registry_snapshot(
                    settings,
                    &provider,
                    None,
                )
                .unwrap(),
                true,
            ),
            RuntimeGatewayRoutingScoresSnapshotSet::bootstrap(
                super::super::super::local_rewrite_provider_registry::runtime_gateway_bootstrap_routing_scores_snapshot(
                    settings,
                ),
                true,
            ),
        )
    }

    #[test]
    fn enforcing_bundle_rejects_mixed_dependency_pins_but_observe_remains_available() {
        let tenant_id = TenantId::new();
        let policy = bundle(&settings(
            RuntimeGovernanceMode::EnterpriseEnforce,
            "classification-a",
            "checksum-a",
            1,
            1,
        ));
        let dependencies = bundle(&settings(
            RuntimeGovernanceMode::EnterpriseEnforce,
            "classification-b",
            "checksum-b",
            2,
            2,
        ));
        let mixed = RuntimeGovernanceSnapshotBundleSet::new(
            policy.policy,
            dependencies.classification,
            dependencies.provider_registry,
            dependencies.routing_scores,
        );
        assert!(mixed.snapshot_for(tenant_id).is_none());

        let observe = bundle(&settings(
            RuntimeGovernanceMode::EnterpriseObserve,
            "classification-a",
            "checksum-a",
            1,
            1,
        ));
        let dependencies = bundle(&settings(
            RuntimeGovernanceMode::EnterpriseObserve,
            "classification-b",
            "checksum-b",
            2,
            2,
        ));
        let mixed = RuntimeGovernanceSnapshotBundleSet::new(
            observe.policy,
            dependencies.classification,
            dependencies.provider_registry,
            dependencies.routing_scores,
        );
        assert!(mixed.snapshot_for(tenant_id).is_some());
    }

    #[test]
    fn pinned_request_keeps_one_bundle_while_next_request_observes_atomic_publish() {
        let tenant_id = TenantId::new();
        let first = bundle(&settings(
            RuntimeGovernanceMode::EnterpriseEnforce,
            "classification-a",
            "checksum-a",
            1,
            1,
        ));
        let second = bundle(&settings(
            RuntimeGovernanceMode::EnterpriseEnforce,
            "classification-b",
            "checksum-b",
            2,
            2,
        ));
        let current = arc_swap::ArcSwap::from_pointee(first);
        let pinned = current.load_full();
        current.store(Arc::new(second));

        let pinned = pinned.snapshot_for(tenant_id).unwrap();
        assert_eq!(
            pinned
                .classification
                .classification_rules()
                .revision()
                .as_str(),
            "classification-a"
        );
        assert_eq!(pinned.provider_registry.revision(), 1);
        assert_eq!(pinned.routing_scores.revision, 1);

        let next = current.load().snapshot_for(tenant_id).unwrap();
        assert_eq!(
            next.classification
                .classification_rules()
                .revision()
                .as_str(),
            "classification-b"
        );
        assert_eq!(next.provider_registry.revision(), 2);
        assert_eq!(next.routing_scores.revision, 2);
    }
}
