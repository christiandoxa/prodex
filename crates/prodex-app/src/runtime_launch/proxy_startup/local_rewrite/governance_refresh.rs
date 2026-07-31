use super::governance_bundle::{
    RuntimeGovernanceRequestSnapshot, RuntimeGovernanceSnapshotBundleSet,
};
use super::{
    RuntimeGovernanceAuthority, RuntimeLocalRewriteProxyShared,
    runtime_gateway_load_governance_snapshot,
};
use crate::runtime_core_shared::runtime_proxy_log_to_path;
use arc_swap::ArcSwap;
use prodex_domain::TenantId;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

const GOVERNANCE_BUNDLE_LOAD_ATTEMPTS: usize = 2;
const GOVERNANCE_ARTIFACT_KINDS: [prodex_storage::GovernanceArtifactKind; 4] = [
    prodex_storage::GovernanceArtifactKind::Policy,
    prodex_storage::GovernanceArtifactKind::ClassificationRules,
    prodex_storage::GovernanceArtifactKind::ProviderRegistry,
    prodex_storage::GovernanceArtifactKind::RoutingScores,
];

struct RuntimeGatewayGovernanceRefreshInputs<'a> {
    authority: &'a RuntimeGovernanceAuthority,
    sqlite_repository: Option<&'a prodex_storage_sqlite_runtime::GovernanceSqliteRepository>,
    governance_policy: &'a prodex_runtime_policy::RuntimePolicyGovernanceSettings,
    deployment_mode: prodex_config::GovernanceMode,
    provider: &'a super::RuntimeLocalRewriteProviderOptions,
    provider_credential: Option<&'a super::RuntimeProjectedProviderCredential>,
}

struct RuntimeGatewayGovernanceRefreshContext<'a> {
    inputs: &'a RuntimeGatewayGovernanceRefreshInputs<'a>,
    tenant_id: TenantId,
}

fn runtime_gateway_governance_status(
    authority: &RuntimeGovernanceAuthority,
    sqlite_repository: Option<&prodex_storage_sqlite_runtime::GovernanceSqliteRepository>,
    tenant_id: prodex_domain::TenantId,
    kind: prodex_storage::GovernanceArtifactKind,
) -> Result<prodex_storage::GovernanceStatus, prodex_storage::GovernanceRepositoryError> {
    match authority {
        RuntimeGovernanceAuthority::Sqlite { .. } => sqlite_repository
            .ok_or(prodex_storage::GovernanceRepositoryError::Database)?
            .status(tenant_id, kind),
        RuntimeGovernanceAuthority::Postgres {
            repository,
            runtime,
            ..
        } => runtime.block_on(repository.governance_status(tenant_id, kind)),
    }
}

fn runtime_gateway_governance_statuses(
    authority: &RuntimeGovernanceAuthority,
    sqlite_repository: Option<&prodex_storage_sqlite_runtime::GovernanceSqliteRepository>,
    tenant_id: prodex_domain::TenantId,
) -> Result<[prodex_storage::GovernanceStatus; 4], prodex_storage::GovernanceRepositoryError> {
    let mut statuses = Vec::with_capacity(GOVERNANCE_ARTIFACT_KINDS.len());
    for kind in GOVERNANCE_ARTIFACT_KINDS {
        statuses.push(runtime_gateway_governance_status(
            authority,
            sqlite_repository,
            tenant_id,
            kind,
        )?);
    }
    statuses
        .try_into()
        .map_err(|_| prodex_storage::GovernanceRepositoryError::Database)
}

fn governance_status_has_revision(
    status: &prodex_storage::GovernanceStatus,
    revision_id: &str,
) -> bool {
    status.active_revision_id.as_deref() == Some(revision_id)
        || status.last_known_good_revision_id.as_deref() == Some(revision_id)
}

fn governance_load_error(error: &anyhow::Error) -> prodex_storage::GovernanceRepositoryError {
    error
        .downcast_ref::<prodex_storage::GovernanceRepositoryError>()
        .copied()
        .unwrap_or(prodex_storage::GovernanceRepositoryError::SnapshotUnavailable)
}

pub(super) fn runtime_gateway_load_compatible_governance_bundle(
    authority: &RuntimeGovernanceAuthority,
    sqlite_repository: Option<&prodex_storage_sqlite_runtime::GovernanceSqliteRepository>,
    tenant_id: prodex_domain::TenantId,
    governance_policy: &prodex_runtime_policy::RuntimePolicyGovernanceSettings,
    deployment_mode: prodex_config::GovernanceMode,
    provider: &super::RuntimeLocalRewriteProviderOptions,
    provider_credential: Option<&super::RuntimeProjectedProviderCredential>,
) -> Result<RuntimeGovernanceRequestSnapshot, prodex_storage::GovernanceRepositoryError> {
    let inputs = RuntimeGatewayGovernanceRefreshInputs {
        authority,
        sqlite_repository,
        governance_policy,
        deployment_mode,
        provider,
        provider_credential,
    };
    let context = RuntimeGatewayGovernanceRefreshContext {
        inputs: &inputs,
        tenant_id,
    };
    for _ in 0..GOVERNANCE_BUNDLE_LOAD_ATTEMPTS {
        let before = runtime_gateway_governance_statuses(authority, sqlite_repository, tenant_id)?;
        for policy_revision in runtime_gateway_policy_revision_candidates(&before[0]) {
            match runtime_gateway_load_compatible_bundle_candidate(
                &context,
                &before,
                policy_revision,
            )? {
                RuntimeGatewayCompatibleBundleCandidate::Snapshot(snapshot) => return Ok(snapshot),
                RuntimeGatewayCompatibleBundleCandidate::StatusChanged => break,
                RuntimeGatewayCompatibleBundleCandidate::Unavailable => continue,
            }
        }
        if before == runtime_gateway_governance_statuses(authority, sqlite_repository, tenant_id)? {
            return Err(prodex_storage::GovernanceRepositoryError::SnapshotUnavailable);
        }
    }
    Err(prodex_storage::GovernanceRepositoryError::SnapshotUnavailable)
}

enum RuntimeGatewayCompatibleBundleCandidate {
    Snapshot(RuntimeGovernanceRequestSnapshot),
    StatusChanged,
    Unavailable,
}

fn runtime_gateway_policy_revision_candidates(
    status: &prodex_storage::GovernanceStatus,
) -> Vec<&str> {
    let mut candidates = Vec::with_capacity(2);
    if let Some(active) = status.active_revision_id.as_deref() {
        candidates.push(active);
    }
    if let Some(last_known_good) = status.last_known_good_revision_id.as_deref()
        && !candidates.contains(&last_known_good)
    {
        candidates.push(last_known_good);
    }
    candidates
}

fn runtime_gateway_load_compatible_snapshot(
    authority: &RuntimeGovernanceAuthority,
    sqlite_repository: Option<&prodex_storage_sqlite_runtime::GovernanceSqliteRepository>,
    tenant_id: prodex_domain::TenantId,
    kind: prodex_storage::GovernanceArtifactKind,
    validate_artifact: impl FnMut(&prodex_storage::GovernanceArtifactValidationInput<'_>) -> bool,
) -> Result<Option<prodex_storage::GovernanceSnapshot>, prodex_storage::GovernanceRepositoryError> {
    match runtime_gateway_load_governance_snapshot(
        authority,
        sqlite_repository,
        tenant_id,
        kind,
        validate_artifact,
    ) {
        Ok(snapshot) => Ok(Some(snapshot)),
        Err(error)
            if governance_load_error(&error)
                == prodex_storage::GovernanceRepositoryError::SnapshotUnavailable =>
        {
            Ok(None)
        }
        Err(error) => Err(governance_load_error(&error)),
    }
}

fn runtime_gateway_load_compatible_bundle_candidate(
    context: &RuntimeGatewayGovernanceRefreshContext<'_>,
    before: &[prodex_storage::GovernanceStatus; 4],
    policy_revision: &str,
) -> Result<RuntimeGatewayCompatibleBundleCandidate, prodex_storage::GovernanceRepositoryError> {
    let inputs = context.inputs;
    let tenant_id = context.tenant_id;
    let Some(stored_policy) = runtime_gateway_load_compatible_snapshot(
        inputs.authority,
        inputs.sqlite_repository,
        tenant_id,
        prodex_storage::GovernanceArtifactKind::Policy,
        |input| {
            input.revision_id == policy_revision
                && runtime_gateway_governance_artifact_is_valid(
                    inputs.governance_policy,
                    inputs.deployment_mode,
                    inputs.provider,
                    inputs.provider_credential,
                    input,
                )
        },
    )?
    else {
        return Ok(RuntimeGatewayCompatibleBundleCandidate::Unavailable);
    };
    let policy = crate::runtime_governance::compile_runtime_governance_artifact_for_deployment(
        &stored_policy.compiled_artifact,
        inputs.deployment_mode,
    )
    .map_err(|_| prodex_storage::GovernanceRepositoryError::SnapshotUnavailable)?;
    let pinned_classification = &policy.application.classification_rules;
    let provider_revision = policy.provider_registry_revision.to_string();
    let routing_revision = policy.routing_score_revision.to_string();
    if !governance_status_has_revision(&before[1], pinned_classification.revision().as_str())
        || !governance_status_has_revision(&before[2], &provider_revision)
        || !governance_status_has_revision(&before[3], &routing_revision)
    {
        return Ok(RuntimeGatewayCompatibleBundleCandidate::Unavailable);
    }

    let Some(stored_classification) = runtime_gateway_load_compatible_snapshot(
        inputs.authority,
        inputs.sqlite_repository,
        tenant_id,
        prodex_storage::GovernanceArtifactKind::ClassificationRules,
        |input| {
            input.revision_id == pinned_classification.revision().as_str()
                && runtime_gateway_governance_artifact_is_valid(
                    inputs.governance_policy,
                    inputs.deployment_mode,
                    inputs.provider,
                    inputs.provider_credential,
                    input,
                )
                && super::super::local_rewrite_classification_rules::compile_runtime_classification_rules_artifact(
                    tenant_id,
                    input.compiled_artifact,
                )
                .is_ok_and(|snapshot| {
                    snapshot.classification_rules().checksum() == pinned_classification.checksum()
                })
        },
    )?
    else {
        return Ok(RuntimeGatewayCompatibleBundleCandidate::Unavailable);
    };
    let classification = super::super::local_rewrite_classification_rules::compile_runtime_classification_rules_artifact(
        tenant_id,
        &stored_classification.compiled_artifact,
    )
    .map_err(|_| prodex_storage::GovernanceRepositoryError::SnapshotUnavailable)?;

    let Some(stored_provider) = runtime_gateway_load_compatible_snapshot(
        inputs.authority,
        inputs.sqlite_repository,
        tenant_id,
        prodex_storage::GovernanceArtifactKind::ProviderRegistry,
        |input| {
            input.revision_id == provider_revision
                && runtime_gateway_governance_artifact_is_valid(
                    inputs.governance_policy,
                    inputs.deployment_mode,
                    inputs.provider,
                    inputs.provider_credential,
                    input,
                )
        },
    )?
    else {
        return Ok(RuntimeGatewayCompatibleBundleCandidate::Unavailable);
    };
    let provider_registry = super::super::local_rewrite_provider_registry::compile_runtime_gateway_provider_registry_artifact_for_deployment(
        &stored_provider.compiled_artifact,
        inputs.provider,
        inputs.provider_credential,
        inputs.deployment_mode,
    )
    .map_err(|_| prodex_storage::GovernanceRepositoryError::SnapshotUnavailable)?;

    let Some(stored_routing) = runtime_gateway_load_compatible_snapshot(
        inputs.authority,
        inputs.sqlite_repository,
        tenant_id,
        prodex_storage::GovernanceArtifactKind::RoutingScores,
        |input| {
            input.revision_id == routing_revision
                && runtime_gateway_governance_artifact_is_valid(
                    inputs.governance_policy,
                    inputs.deployment_mode,
                    inputs.provider,
                    inputs.provider_credential,
                    input,
                )
        },
    )?
    else {
        return Ok(RuntimeGatewayCompatibleBundleCandidate::Unavailable);
    };
    let routing_scores = super::super::local_rewrite_provider_registry::compile_runtime_gateway_routing_scores_artifact(
        &stored_routing.compiled_artifact,
    )
    .map_err(|_| prodex_storage::GovernanceRepositoryError::SnapshotUnavailable)?;
    if before
        != &runtime_gateway_governance_statuses(
            inputs.authority,
            inputs.sqlite_repository,
            tenant_id,
        )?
    {
        return Ok(RuntimeGatewayCompatibleBundleCandidate::StatusChanged);
    }
    Ok(RuntimeGatewayCompatibleBundleCandidate::Snapshot(
        RuntimeGovernanceRequestSnapshot {
            policy: Arc::new(policy),
            classification: Arc::new(classification),
            provider_registry: Arc::new(provider_registry),
            routing_scores: Arc::new(routing_scores),
        },
    ))
}

pub(super) fn runtime_gateway_governance_artifact_is_valid(
    governance_policy: &prodex_runtime_policy::RuntimePolicyGovernanceSettings,
    deployment_mode: prodex_config::GovernanceMode,
    provider: &super::RuntimeLocalRewriteProviderOptions,
    provider_credential: Option<&super::RuntimeProjectedProviderCredential>,
    input: &prodex_storage::GovernanceArtifactValidationInput<'_>,
) -> bool {
    super::super::local_rewrite_governance_artifact_authenticity::governance_artifact_authenticity_is_valid(
        governance_policy,
        input,
    ) && match input.kind {
        prodex_storage::GovernanceArtifactKind::Policy => {
            crate::runtime_governance::compile_runtime_governance_artifact_for_deployment(
                input.compiled_artifact,
                deployment_mode,
            )
            .is_ok_and(|snapshot| {
                snapshot.application.policy.revision().to_string() == input.revision_id
            })
        }
        prodex_storage::GovernanceArtifactKind::ClassificationRules => {
            super::super::local_rewrite_classification_rules::compile_runtime_classification_rules_artifact(
                input.tenant_id,
                input.compiled_artifact,
            )
            .is_ok_and(|snapshot| {
                snapshot.classification_rules().revision().as_str() == input.revision_id
            })
        }
        prodex_storage::GovernanceArtifactKind::ProviderRegistry => {
            super::super::local_rewrite_provider_registry::compile_runtime_gateway_provider_registry_artifact_for_deployment(
                input.compiled_artifact,
                provider,
                provider_credential,
                deployment_mode,
            )
            .is_ok_and(|snapshot| snapshot.revision().to_string() == input.revision_id)
        }
        prodex_storage::GovernanceArtifactKind::RoutingScores => {
            super::super::local_rewrite_provider_registry::compile_runtime_gateway_routing_scores_artifact(
                input.compiled_artifact,
            )
            .is_ok_and(|snapshot| snapshot.revision.to_string() == input.revision_id)
        }
    }
}

pub(super) fn runtime_gateway_refresh_policy_snapshot(
    authority: &RuntimeGovernanceAuthority,
    sqlite_repository: Option<&prodex_storage_sqlite_runtime::GovernanceSqliteRepository>,
    tenant_id: prodex_domain::TenantId,
    governance_policy: &prodex_runtime_policy::RuntimePolicyGovernanceSettings,
    deployment_mode: prodex_config::GovernanceMode,
    snapshots: &mut crate::runtime_governance::RuntimeGovernanceAuthoritySnapshotSet,
) -> (usize, usize) {
    let stored = runtime_gateway_load_governance_snapshot(
        authority,
        sqlite_repository,
        tenant_id,
        prodex_storage::GovernanceArtifactKind::Policy,
        |input| {
            super::super::local_rewrite_governance_artifact_authenticity::governance_artifact_authenticity_is_valid(
                governance_policy,
                input,
            ) && crate::runtime_governance::compile_runtime_governance_artifact_for_deployment(
                input.compiled_artifact,
                deployment_mode,
            )
            .is_ok_and(|snapshot| {
                snapshot.application.policy.revision().to_string() == input.revision_id
            })
        },
    );
    match stored {
        Ok(stored) => {
            let snapshot =
                crate::runtime_governance::compile_runtime_governance_artifact_for_deployment(
                    &stored.compiled_artifact,
                    deployment_mode,
                );
            if let Ok(snapshot) = snapshot
                && snapshot.application.policy.revision().to_string() == stored.revision_id
                && let Ok(updated) = snapshots.with_tenant_snapshot(tenant_id, snapshot)
            {
                *snapshots = updated;
                (1, 0)
            } else {
                (0, 0)
            }
        }
        Err(error) => (
            0,
            runtime_gateway_invalidate_unavailable_snapshot(&error, snapshots, |snapshots| {
                snapshots.without_tenant_snapshot(tenant_id)
            }),
        ),
    }
}

pub(super) fn runtime_gateway_invalidate_unavailable_snapshot<T>(
    error: &anyhow::Error,
    snapshots: &mut T,
    invalidate: impl FnOnce(&T) -> Option<T>,
) -> usize {
    if error.downcast_ref::<prodex_storage::GovernanceRepositoryError>()
        != Some(&prodex_storage::GovernanceRepositoryError::SnapshotUnavailable)
    {
        return 0;
    }
    let Some(updated) = invalidate(snapshots) else {
        return 0;
    };
    *snapshots = updated;
    1
}

#[derive(Clone, Copy, Default)]
struct RuntimeGatewayGovernanceRefreshCounts {
    policy_refreshed: usize,
    policy_unavailable: usize,
    classification_refreshed: usize,
    classification_unavailable: usize,
    provider_refreshed: usize,
    provider_unavailable: usize,
    routing_refreshed: usize,
    routing_unavailable: usize,
}

impl RuntimeGatewayGovernanceRefreshCounts {
    fn add(&mut self, other: Self) {
        self.policy_refreshed += other.policy_refreshed;
        self.policy_unavailable += other.policy_unavailable;
        self.classification_refreshed += other.classification_refreshed;
        self.classification_unavailable += other.classification_unavailable;
        self.provider_refreshed += other.provider_refreshed;
        self.provider_unavailable += other.provider_unavailable;
        self.routing_refreshed += other.routing_refreshed;
        self.routing_unavailable += other.routing_unavailable;
    }

    fn changed(self) -> bool {
        self.policy_refreshed
            + self.policy_unavailable
            + self.classification_refreshed
            + self.classification_unavailable
            + self.provider_refreshed
            + self.provider_unavailable
            + self.routing_refreshed
            + self.routing_unavailable
            > 0
    }
}

fn runtime_gateway_discover_governance_tenants(
    authority: &RuntimeGovernanceAuthority,
    sqlite_repository: Option<&prodex_storage_sqlite_runtime::GovernanceSqliteRepository>,
) -> Result<Vec<TenantId>, prodex_storage::GovernanceRepositoryError> {
    let discovered = match authority {
        RuntimeGovernanceAuthority::Sqlite { .. } => sqlite_repository
            .ok_or(prodex_storage::GovernanceRepositoryError::Database)?
            .governance_list_tenant_ids(
                (crate::runtime_governance::MAX_RUNTIME_GOVERNANCE_AUTHORITY_TENANTS + 1) as u16,
            ),
        RuntimeGovernanceAuthority::Postgres {
            repository,
            runtime,
            ..
        } => runtime.block_on(repository.governance_list_tenant_ids(
            (crate::runtime_governance::MAX_RUNTIME_GOVERNANCE_AUTHORITY_TENANTS + 1) as u16,
        )),
    }?;
    authority.merge_tenant_ids(discovered)?;
    authority.tenant_ids()
}

fn runtime_gateway_refresh_enforcing_tenant(
    context: &RuntimeGatewayGovernanceRefreshContext<'_>,
    governance_snapshots: &Arc<ArcSwap<RuntimeGovernanceSnapshotBundleSet>>,
) -> Result<RuntimeGatewayGovernanceRefreshCounts, prodex_storage::GovernanceRepositoryError> {
    let mut next = (*governance_snapshots.load_full()).clone();
    match runtime_gateway_load_compatible_governance_bundle(
        context.inputs.authority,
        context.inputs.sqlite_repository,
        context.tenant_id,
        context.inputs.governance_policy,
        context.inputs.deployment_mode,
        context.inputs.provider,
        context.inputs.provider_credential,
    ) {
        Ok(snapshot) => {
            next = next
                .with_tenant_snapshot(context.tenant_id, snapshot)
                .map_err(|_| prodex_storage::GovernanceRepositoryError::SnapshotUnavailable)?;
            governance_snapshots.store(Arc::new(next));
            Ok(RuntimeGatewayGovernanceRefreshCounts {
                policy_refreshed: 1,
                classification_refreshed: 1,
                provider_refreshed: 1,
                routing_refreshed: 1,
                ..Default::default()
            })
        }
        Err(prodex_storage::GovernanceRepositoryError::SnapshotUnavailable) => {
            if let Some(updated) = next.without_tenant_snapshot(context.tenant_id) {
                governance_snapshots.store(Arc::new(updated));
            }
            Ok(RuntimeGatewayGovernanceRefreshCounts {
                policy_unavailable: 1,
                classification_unavailable: 1,
                provider_unavailable: 1,
                routing_unavailable: 1,
                ..Default::default()
            })
        }
        Err(error) => Err(error),
    }
}

fn runtime_gateway_refresh_classification_snapshot(
    context: &RuntimeGatewayGovernanceRefreshContext<'_>,
    snapshots: &mut super::super::local_rewrite_classification_rules::RuntimeClassificationRulesSnapshotSet,
) -> (usize, usize) {
    let inputs = context.inputs;
    let stored = runtime_gateway_load_governance_snapshot(
        inputs.authority,
        inputs.sqlite_repository,
        context.tenant_id,
        prodex_storage::GovernanceArtifactKind::ClassificationRules,
        |input| {
            runtime_gateway_governance_artifact_is_valid(
                inputs.governance_policy,
                inputs.deployment_mode,
                inputs.provider,
                inputs.provider_credential,
                input,
            )
        },
    );
    match stored {
        Ok(stored) => {
            if let Ok(snapshot) = super::super::local_rewrite_classification_rules::compile_runtime_classification_rules_artifact(
                context.tenant_id,
                &stored.compiled_artifact,
            ) && snapshot.classification_rules().revision().as_str() == stored.revision_id
                && let Ok(updated) = snapshots.with_tenant_snapshot(context.tenant_id, snapshot)
            {
                *snapshots = updated;
                (1, 0)
            } else {
                (0, 0)
            }
        }
        Err(error) => (
            0,
            runtime_gateway_invalidate_unavailable_snapshot(&error, snapshots, |snapshots| {
                snapshots.without_tenant_snapshot(context.tenant_id)
            }),
        ),
    }
}

fn runtime_gateway_refresh_provider_snapshot(
    context: &RuntimeGatewayGovernanceRefreshContext<'_>,
    snapshots: &mut super::super::local_rewrite_provider_registry::RuntimeGatewayProviderRegistrySnapshotSet,
) -> (usize, usize) {
    let inputs = context.inputs;
    let stored = runtime_gateway_load_governance_snapshot(
        inputs.authority,
        inputs.sqlite_repository,
        context.tenant_id,
        prodex_storage::GovernanceArtifactKind::ProviderRegistry,
        |input| {
            runtime_gateway_governance_artifact_is_valid(
                inputs.governance_policy,
                inputs.deployment_mode,
                inputs.provider,
                inputs.provider_credential,
                input,
            )
        },
    );
    match stored {
        Ok(stored) => {
            if let Ok(snapshot) = super::super::local_rewrite_provider_registry::compile_runtime_gateway_provider_registry_artifact_for_deployment(
                &stored.compiled_artifact,
                inputs.provider,
                inputs.provider_credential,
                inputs.deployment_mode,
            ) && snapshot.revision().to_string() == stored.revision_id
                && let Ok(updated) = snapshots.with_tenant_snapshot(context.tenant_id, snapshot)
            {
                *snapshots = updated;
                (1, 0)
            } else {
                (0, 0)
            }
        }
        Err(error) => (
            0,
            runtime_gateway_invalidate_unavailable_snapshot(&error, snapshots, |snapshots| {
                snapshots.without_tenant_snapshot(context.tenant_id)
            }),
        ),
    }
}

fn runtime_gateway_refresh_routing_snapshot(
    context: &RuntimeGatewayGovernanceRefreshContext<'_>,
    snapshots: &mut super::super::local_rewrite_provider_registry::RuntimeGatewayRoutingScoresSnapshotSet,
) -> (usize, usize) {
    let inputs = context.inputs;
    let stored = runtime_gateway_load_governance_snapshot(
        inputs.authority,
        inputs.sqlite_repository,
        context.tenant_id,
        prodex_storage::GovernanceArtifactKind::RoutingScores,
        |input| {
            runtime_gateway_governance_artifact_is_valid(
                inputs.governance_policy,
                inputs.deployment_mode,
                inputs.provider,
                inputs.provider_credential,
                input,
            )
        },
    );
    match stored {
        Ok(stored) => {
            if let Ok(snapshot) = super::super::local_rewrite_provider_registry::compile_runtime_gateway_routing_scores_artifact(
                &stored.compiled_artifact,
            ) && snapshot.revision.to_string() == stored.revision_id
                && let Ok(updated) = snapshots.with_tenant_snapshot(context.tenant_id, snapshot)
            {
                *snapshots = updated;
                (1, 0)
            } else {
                (0, 0)
            }
        }
        Err(error) => (
            0,
            runtime_gateway_invalidate_unavailable_snapshot(&error, snapshots, |snapshots| {
                snapshots.without_tenant_snapshot(context.tenant_id)
            }),
        ),
    }
}

fn runtime_gateway_refresh_non_enforcing_tenant(
    context: &RuntimeGatewayGovernanceRefreshContext<'_>,
    governance_snapshots: &Arc<ArcSwap<RuntimeGovernanceSnapshotBundleSet>>,
) -> Result<RuntimeGatewayGovernanceRefreshCounts, prodex_storage::GovernanceRepositoryError> {
    let mut next = (*governance_snapshots.load_full()).clone();
    let (policy_refreshed, policy_unavailable) = runtime_gateway_refresh_policy_snapshot(
        context.inputs.authority,
        context.inputs.sqlite_repository,
        context.tenant_id,
        context.inputs.governance_policy,
        context.inputs.deployment_mode,
        &mut next.policy,
    );
    let (classification_refreshed, classification_unavailable) =
        runtime_gateway_refresh_classification_snapshot(context, &mut next.classification);
    let (provider_refreshed, provider_unavailable) =
        runtime_gateway_refresh_provider_snapshot(context, &mut next.provider_registry);
    let (routing_refreshed, routing_unavailable) =
        runtime_gateway_refresh_routing_snapshot(context, &mut next.routing_scores);
    let counts = RuntimeGatewayGovernanceRefreshCounts {
        policy_refreshed,
        policy_unavailable,
        classification_refreshed,
        classification_unavailable,
        provider_refreshed,
        provider_unavailable,
        routing_refreshed,
        routing_unavailable,
    };
    if counts.changed() {
        governance_snapshots.store(Arc::new(next));
    }
    Ok(counts)
}

fn runtime_gateway_refresh_tenant(
    context: &RuntimeGatewayGovernanceRefreshContext<'_>,
    governance_snapshots: &Arc<ArcSwap<RuntimeGovernanceSnapshotBundleSet>>,
) -> Result<RuntimeGatewayGovernanceRefreshCounts, prodex_storage::GovernanceRepositoryError> {
    context
        .inputs
        .authority
        .commit_for_tenant(context.tenant_id, || {
            if context.inputs.deployment_mode.is_enforcing() {
                return runtime_gateway_refresh_enforcing_tenant(context, governance_snapshots);
            }
            runtime_gateway_refresh_non_enforcing_tenant(context, governance_snapshots)
        })
}

fn runtime_gateway_governance_refresh_wait(
    shutdown: &AtomicBool,
    refresh_requested: Option<&AtomicBool>,
) {
    for _ in 0..50 {
        if shutdown.load(Ordering::SeqCst) {
            break;
        }
        if let Some(refresh_requested) = refresh_requested
            && refresh_requested.swap(false, Ordering::AcqRel)
        {
            break;
        }
        thread::sleep(Duration::from_millis(100));
    }
}

fn runtime_gateway_log_governance_refresh(
    log_path: &std::path::Path,
    counts: RuntimeGatewayGovernanceRefreshCounts,
    tenant_count: usize,
) {
    if counts.changed() {
        runtime_proxy_log_to_path(
            log_path,
            &format!(
                "governance_snapshot_refresh status=success policy={} policy_unavailable={} classification_rules={} classification_rules_unavailable={} provider_registry={} provider_registry_unavailable={} routing_scores={} routing_scores_unavailable={} configured={}",
                counts.policy_refreshed,
                counts.policy_unavailable,
                counts.classification_refreshed,
                counts.classification_unavailable,
                counts.provider_refreshed,
                counts.provider_unavailable,
                counts.routing_refreshed,
                counts.routing_unavailable,
                tenant_count,
            ),
        );
    } else {
        runtime_proxy_log_to_path(
            log_path,
            "governance_snapshot_refresh status=error action=retain_lkg",
        );
    }
}

pub(super) fn spawn_runtime_gateway_governance_refresh_worker(
    shared: &RuntimeLocalRewriteProxyShared,
    shutdown: &Arc<AtomicBool>,
) -> Option<thread::JoinHandle<()>> {
    let authority = shared.governance_authority.clone()?;
    let governance_snapshots = Arc::clone(&shared.process.governance_snapshots);
    let provider = shared.provider.clone();
    let provider_credential = shared.provider_credential.clone();
    let shutdown = Arc::clone(shutdown);
    let log_path = shared.runtime_shared.log_path.clone();
    let deployment_mode = shared.runtime_shared.runtime_config.governance.mode;
    let governance_policy = shared
        .runtime_shared
        .runtime_config
        .governance_policy
        .clone();
    let governance_refresh_requested = Arc::clone(&shared.governance_refresh_requested);
    Some(thread::spawn(move || {
        let sqlite_repository = match &authority {
            RuntimeGovernanceAuthority::Sqlite { path, .. } => {
                prodex_storage_sqlite_runtime::GovernanceSqliteRepository::open(path).ok()
            }
            RuntimeGovernanceAuthority::Postgres { .. } => None,
        };
        let inputs = RuntimeGatewayGovernanceRefreshInputs {
            authority: &authority,
            sqlite_repository: sqlite_repository.as_ref(),
            governance_policy: &governance_policy,
            deployment_mode,
            provider: &provider,
            provider_credential: provider_credential.as_ref(),
        };
        while !shutdown.load(Ordering::SeqCst) {
            let tenant_ids = match runtime_gateway_discover_governance_tenants(
                &authority,
                sqlite_repository.as_ref(),
            ) {
                Ok(tenant_ids) => tenant_ids,
                Err(_) => {
                    runtime_proxy_log_to_path(
                        &log_path,
                        "governance_snapshot_refresh status=error phase=tenant_discovery action=retain_lkg",
                    );
                    runtime_gateway_governance_refresh_wait(&shutdown, None);
                    continue;
                }
            };
            let mut counts = RuntimeGatewayGovernanceRefreshCounts::default();
            for tenant_id in tenant_ids.iter().copied() {
                let context = RuntimeGatewayGovernanceRefreshContext {
                    inputs: &inputs,
                    tenant_id,
                };
                if let Ok(refresh) = runtime_gateway_refresh_tenant(&context, &governance_snapshots)
                {
                    counts.add(refresh);
                }
            }
            runtime_gateway_log_governance_refresh(&log_path, counts, tenant_ids.len());
            runtime_gateway_governance_refresh_wait(&shutdown, Some(&governance_refresh_requested));
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::runtime_gateway_invalidate_unavailable_snapshot;
    use prodex_storage::GovernanceRepositoryError;

    #[test]
    fn unavailable_snapshot_is_invalidated_but_database_errors_retain_cache() {
        let mut snapshot = Some("active");
        let unavailable = anyhow::Error::new(GovernanceRepositoryError::SnapshotUnavailable);
        assert_eq!(
            runtime_gateway_invalidate_unavailable_snapshot(&unavailable, &mut snapshot, |_| Some(
                None
            ),),
            1
        );
        assert_eq!(snapshot, None);

        let mut snapshot = Some("last-known-good");
        let database = anyhow::Error::new(GovernanceRepositoryError::Database);
        assert_eq!(
            runtime_gateway_invalidate_unavailable_snapshot(&database, &mut snapshot, |_| {
                Some(None)
            }),
            0
        );
        assert_eq!(snapshot, Some("last-known-good"));
    }
}
