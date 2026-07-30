use super::{
    RuntimeGovernanceArtifactRefreshOutcome, RuntimeGovernanceAuthority,
    RuntimeLocalRewriteRequestContext, governance_refresh,
    runtime_gateway_load_governance_snapshot,
};
use anyhow::Result;
use std::sync::Arc;

impl RuntimeLocalRewriteRequestContext {
    pub(in super::super) fn refresh_committed_governance_artifact_kind(
        &self,
        tenant_id: prodex_domain::TenantId,
        kind: prodex_storage::GovernanceArtifactKind,
    ) -> std::result::Result<
        RuntimeGovernanceArtifactRefreshOutcome,
        prodex_storage::GovernanceRepositoryError,
    > {
        let authority = self
            .governance_authority
            .as_ref()
            .ok_or(prodex_storage::GovernanceRepositoryError::Unsupported)?;
        let sqlite_repository = match authority {
            RuntimeGovernanceAuthority::Sqlite { path, .. } => {
                Some(prodex_storage_sqlite_runtime::GovernanceSqliteRepository::open(path)?)
            }
            RuntimeGovernanceAuthority::Postgres { .. } => None,
        };
        if !authority.tenant_ids()?.contains(&tenant_id) {
            return Err(prodex_storage::GovernanceRepositoryError::NotFound);
        }
        authority.commit_for_tenant(tenant_id, || {
            if self
                .runtime_shared
                .runtime_config
                .governance
                .mode
                .is_enforcing()
            {
                return match governance_refresh::runtime_gateway_load_compatible_governance_bundle(
                    authority,
                    sqlite_repository.as_ref(),
                    tenant_id,
                    &self.runtime_shared.runtime_config.governance_policy,
                    self.runtime_shared.runtime_config.governance.mode,
                    &self.provider,
                    self.provider_credential.as_ref(),
                ) {
                    Ok(snapshot) => {
                        let next = self
                            .process
                            .governance_snapshots
                            .load_full()
                            .with_tenant_snapshot(tenant_id, snapshot)
                            .map_err(|_| {
                                prodex_storage::GovernanceRepositoryError::SnapshotUnavailable
                            })?;
                        self.process.governance_snapshots.store(Arc::new(next));
                        Ok(RuntimeGovernanceArtifactRefreshOutcome::Published)
                    }
                    Err(prodex_storage::GovernanceRepositoryError::SnapshotUnavailable) => {
                        self.invalidate_committed_governance_artifact_kind(tenant_id, kind);
                        Ok(RuntimeGovernanceArtifactRefreshOutcome::Invalidated)
                    }
                    Err(error) => {
                        self.invalidate_committed_governance_artifact_kind(tenant_id, kind);
                        Err(error)
                    }
                };
            }
            let stored = runtime_gateway_load_governance_snapshot(
                authority,
                sqlite_repository.as_ref(),
                tenant_id,
                kind,
                |input| {
                    governance_refresh::runtime_gateway_governance_artifact_is_valid(
                        &self.runtime_shared.runtime_config.governance_policy,
                        self.runtime_shared.runtime_config.governance.mode,
                        &self.provider,
                        self.provider_credential.as_ref(),
                        input,
                    )
                },
            );
            match stored {
                Ok(snapshot) => {
                    if self
                        .swap_committed_governance_artifact_kind(
                            tenant_id,
                            kind,
                            &snapshot.compiled_artifact,
                        )
                        .is_err()
                    {
                        self.invalidate_committed_governance_artifact_kind(tenant_id, kind);
                        return Err(prodex_storage::GovernanceRepositoryError::SnapshotUnavailable);
                    }
                    Ok(RuntimeGovernanceArtifactRefreshOutcome::Published)
                }
                Err(error)
                    if error.downcast_ref::<prodex_storage::GovernanceRepositoryError>()
                        == Some(
                            &prodex_storage::GovernanceRepositoryError::SnapshotUnavailable,
                        ) =>
                {
                    self.invalidate_committed_governance_artifact_kind(tenant_id, kind);
                    Ok(RuntimeGovernanceArtifactRefreshOutcome::Invalidated)
                }
                Err(error) => {
                    self.invalidate_committed_governance_artifact_kind(tenant_id, kind);
                    Err(error
                        .downcast_ref::<prodex_storage::GovernanceRepositoryError>()
                        .copied()
                        .unwrap_or(prodex_storage::GovernanceRepositoryError::Database))
                }
            }
        })
    }

    fn swap_committed_governance_artifact_kind(
        &self,
        tenant_id: prodex_domain::TenantId,
        kind: prodex_storage::GovernanceArtifactKind,
        artifact: &[u8],
    ) -> Result<()> {
        let mut next = (*self.process.governance_snapshots.load_full()).clone();
        match kind {
            prodex_storage::GovernanceArtifactKind::Policy => {
                let snapshot =
                    crate::runtime_governance::compile_runtime_governance_artifact_for_deployment(
                        artifact,
                        self.runtime_shared.runtime_config.governance.mode,
                    )?;
                next.policy = next.policy.with_tenant_snapshot(tenant_id, snapshot)?;
            }
            prodex_storage::GovernanceArtifactKind::ClassificationRules => {
                let snapshot = super::super::local_rewrite_classification_rules::compile_runtime_classification_rules_artifact(
                    tenant_id,
                    artifact,
                )?;
                next.classification = next
                    .classification
                    .with_tenant_snapshot(tenant_id, snapshot)?;
            }
            prodex_storage::GovernanceArtifactKind::ProviderRegistry => {
                let snapshot = super::super::local_rewrite_provider_registry::compile_runtime_gateway_provider_registry_artifact_for_deployment(
                    artifact,
                    &self.provider,
                    self.provider_credential.as_ref(),
                    self.runtime_shared.runtime_config.governance.mode,
                )?;
                next.provider_registry = next
                    .provider_registry
                    .with_tenant_snapshot(tenant_id, snapshot)?;
            }
            prodex_storage::GovernanceArtifactKind::RoutingScores => {
                let snapshot = super::super::local_rewrite_provider_registry::compile_runtime_gateway_routing_scores_artifact(artifact)?;
                next.routing_scores = next
                    .routing_scores
                    .with_tenant_snapshot(tenant_id, snapshot)?;
            }
        }
        self.process.governance_snapshots.store(Arc::new(next));
        Ok(())
    }

    fn invalidate_committed_governance_artifact_kind(
        &self,
        tenant_id: prodex_domain::TenantId,
        kind: prodex_storage::GovernanceArtifactKind,
    ) {
        let current = self.process.governance_snapshots.load_full();
        if self
            .runtime_shared
            .runtime_config
            .governance
            .mode
            .is_enforcing()
        {
            if let Some(next) = current.without_tenant_snapshot(tenant_id) {
                self.process.governance_snapshots.store(Arc::new(next));
            }
            return;
        }
        let mut next = (*current).clone();
        let mut changed = false;
        match kind {
            prodex_storage::GovernanceArtifactKind::Policy => {
                if let Some(updated) = next.policy.without_tenant_snapshot(tenant_id) {
                    next.policy = updated;
                    changed = true;
                }
            }
            prodex_storage::GovernanceArtifactKind::ClassificationRules => {
                if let Some(updated) = next.classification.without_tenant_snapshot(tenant_id) {
                    next.classification = updated;
                    changed = true;
                }
            }
            prodex_storage::GovernanceArtifactKind::ProviderRegistry => {
                if let Some(updated) = next.provider_registry.without_tenant_snapshot(tenant_id) {
                    next.provider_registry = updated;
                    changed = true;
                }
            }
            prodex_storage::GovernanceArtifactKind::RoutingScores => {
                if let Some(updated) = next.routing_scores.without_tenant_snapshot(tenant_id) {
                    next.routing_scores = updated;
                    changed = true;
                }
            }
        }
        if changed {
            self.process.governance_snapshots.store(Arc::new(next));
        }
    }
}
