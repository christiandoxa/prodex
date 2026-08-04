use super::{
    Arc, ArcSwap, BTreeSet, Context as _, Mutex, Result, RuntimeConfig, RuntimeGatewayAdminToken,
    RuntimeGatewayStateStore, RuntimeGovernanceAuthority, RuntimeGovernanceSnapshotBundleSet,
    RuntimeLocalRewriteProviderOptions, RuntimeProjectedProviderCredential, governance_refresh,
};

type RuntimeGatewayGovernanceAuthorityState = (
    Arc<ArcSwap<RuntimeGovernanceSnapshotBundleSet>>,
    Option<RuntimeGovernanceAuthority>,
);

pub(in crate::runtime_launch::proxy_startup) fn runtime_gateway_governance_authority(
    runtime_config: &RuntimeConfig,
    state_store: &RuntimeGatewayStateStore,
    admin_tokens: &[RuntimeGatewayAdminToken],
    postgres_repository: Option<&prodex_storage_postgres_runtime::PostgresRepository>,
    async_runtime: &Arc<tokio::runtime::Runtime>,
    provider: &RuntimeLocalRewriteProviderOptions,
    provider_credential: Option<&RuntimeProjectedProviderCredential>,
) -> Result<RuntimeGatewayGovernanceAuthorityState> {
    let enforcing = runtime_config.governance.mode.is_enforcing();
    let deployment_mode = runtime_config.governance.mode;
    let policy_bootstrap = crate::runtime_governance::compile_runtime_governance_settings(
        &runtime_config.governance_policy,
    )?;
    let mut policy_snapshots =
        crate::runtime_governance::RuntimeGovernanceAuthoritySnapshotSet::bootstrap(
            policy_bootstrap,
            !enforcing,
        );
    let mut classification_snapshots =
        super::super::local_rewrite_classification_rules::RuntimeClassificationRulesSnapshotSet::bootstrap(
            &runtime_config.governance_policy,
            !enforcing,
        )?;
    let provider_bootstrap = super::super::local_rewrite_provider_registry::runtime_gateway_bootstrap_provider_registry_snapshot(
        &runtime_config.governance_policy,
        provider,
        provider_credential,
    )?;
    let mut provider_snapshots =
        super::super::local_rewrite_provider_registry::RuntimeGatewayProviderRegistrySnapshotSet::bootstrap(
            provider_bootstrap,
            !enforcing,
        );
    let routing_bootstrap =
        super::super::local_rewrite_provider_registry::runtime_gateway_bootstrap_routing_scores_snapshot(
            &runtime_config.governance_policy,
        );
    let mut routing_snapshots =
        super::super::local_rewrite_provider_registry::RuntimeGatewayRoutingScoresSnapshotSet::bootstrap(
            routing_bootstrap,
            !enforcing,
        );

    let wrap = |policy_snapshots,
                classification_snapshots,
                provider_snapshots,
                routing_snapshots,
                authority| {
        (
            Arc::new(ArcSwap::from_pointee(
                RuntimeGovernanceSnapshotBundleSet::new(
                    policy_snapshots,
                    classification_snapshots,
                    provider_snapshots,
                    routing_snapshots,
                ),
            )),
            authority,
        )
    };
    if !matches!(
        state_store,
        RuntimeGatewayStateStore::Sqlite { .. } | RuntimeGatewayStateStore::Postgres { .. }
    ) {
        if enforcing {
            anyhow::bail!("enforcing governance requires SQLite or PostgreSQL authority");
        }
        return Ok(wrap(
            policy_snapshots,
            classification_snapshots,
            provider_snapshots,
            routing_snapshots,
            None,
        ));
    }
    let mut tenants = runtime_config
        .governance_policy
        .authority_tenants
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    for value in admin_tokens
        .iter()
        .filter_map(|token| token.tenant_id.as_deref())
    {
        let tenant = value
            .parse::<prodex_domain::TenantId>()
            .context("gateway admin tenant is invalid for governance authority")?;
        tenants.insert(tenant);
    }
    let sqlite_repository = match state_store {
        RuntimeGatewayStateStore::Sqlite { path } => Some(
            prodex_storage_sqlite_runtime::GovernanceSqliteRepository::open(path)
                .map_err(|_| anyhow::anyhow!("failed to open authoritative governance store"))?,
        ),
        _ => None,
    };
    let discovery_limit =
        (crate::runtime_governance::MAX_RUNTIME_GOVERNANCE_AUTHORITY_TENANTS + 1) as u16;
    let discovered = match state_store {
        RuntimeGatewayStateStore::Sqlite { .. } => sqlite_repository
            .as_ref()
            .expect("SQLite governance repository must be initialized")
            .governance_list_tenant_ids(discovery_limit),
        // PostgreSQL forces tenant RLS, so its runtime role cannot safely enumerate tenants.
        // `authority_tenants` and tenant-scoped admin tokens are the bounded source of truth.
        RuntimeGatewayStateStore::Postgres { .. } => Ok(Vec::new()),
        _ => unreachable!(),
    }
    .map_err(|_| anyhow::anyhow!("failed to discover authoritative governance tenants"))?;
    tenants.extend(discovered);
    if tenants.len() > crate::runtime_governance::MAX_RUNTIME_GOVERNANCE_AUTHORITY_TENANTS {
        anyhow::bail!("governance authority tenant limit exceeded");
    }
    if tenants.is_empty() && enforcing {
        anyhow::bail!("enforcing governance requires configured authority tenants");
    }
    let tenant_ids = Arc::new(Mutex::new(tenants));
    let authority = match state_store {
        RuntimeGatewayStateStore::Sqlite { path } => RuntimeGovernanceAuthority::Sqlite {
            path: path.clone(),
            tenant_ids: Arc::clone(&tenant_ids),
        },
        RuntimeGatewayStateStore::Postgres { .. } => RuntimeGovernanceAuthority::Postgres {
            repository: postgres_repository
                .context("authoritative PostgreSQL governance repository is unavailable")?
                .clone(),
            runtime: Arc::clone(async_runtime),
            tenant_ids,
        },
        _ => unreachable!(),
    };
    let tenants = authority
        .tenant_ids()
        .map_err(|_| anyhow::anyhow!("failed to read authoritative governance tenants"))?;
    let snapshot_inputs = RuntimeGatewayAuthoritySnapshotInputs {
        authority: &authority,
        sqlite_repository: sqlite_repository.as_ref(),
        governance_policy: &runtime_config.governance_policy,
        deployment_mode,
        provider,
        provider_credential,
    };
    runtime_gateway_load_authority_tenant_snapshots(
        &snapshot_inputs,
        &tenants,
        &mut policy_snapshots,
        &mut classification_snapshots,
        &mut provider_snapshots,
        &mut routing_snapshots,
    )?;
    Ok(wrap(
        policy_snapshots,
        classification_snapshots,
        provider_snapshots,
        routing_snapshots,
        Some(authority),
    ))
}

struct RuntimeGatewayAuthoritySnapshotInputs<'a> {
    authority: &'a RuntimeGovernanceAuthority,
    sqlite_repository: Option<&'a prodex_storage_sqlite_runtime::GovernanceSqliteRepository>,
    governance_policy: &'a prodex_runtime_policy::RuntimePolicyGovernanceSettings,
    deployment_mode: prodex_config::GovernanceMode,
    provider: &'a RuntimeLocalRewriteProviderOptions,
    provider_credential: Option<&'a RuntimeProjectedProviderCredential>,
}

fn runtime_gateway_load_authority_tenant_snapshots(
    inputs: &RuntimeGatewayAuthoritySnapshotInputs<'_>,
    tenants: &[prodex_domain::TenantId],
    policy_snapshots: &mut crate::runtime_governance::RuntimeGovernanceAuthoritySnapshotSet,
    classification_snapshots: &mut super::super::local_rewrite_classification_rules::RuntimeClassificationRulesSnapshotSet,
    provider_snapshots: &mut super::super::local_rewrite_provider_registry::RuntimeGatewayProviderRegistrySnapshotSet,
    routing_snapshots: &mut super::super::local_rewrite_provider_registry::RuntimeGatewayRoutingScoresSnapshotSet,
) -> Result<()> {
    for tenant_id in tenants {
        if inputs.deployment_mode.is_enforcing() {
            let snapshot = governance_refresh::runtime_gateway_load_compatible_governance_bundle(
                inputs.authority,
                inputs.sqlite_repository,
                *tenant_id,
                inputs.governance_policy,
                inputs.deployment_mode,
                inputs.provider,
                inputs.provider_credential,
            )
            .map_err(|_| {
                anyhow::anyhow!(
                    "authoritative governance store has no compatible active or last-known-good bundle"
                )
            })?;
            *policy_snapshots = policy_snapshots
                .clone()
                .with_tenant_snapshot(*tenant_id, Arc::unwrap_or_clone(snapshot.policy))?;
            *classification_snapshots = classification_snapshots
                .clone()
                .with_tenant_snapshot(*tenant_id, Arc::unwrap_or_clone(snapshot.classification))?;
            *provider_snapshots = provider_snapshots.clone().with_tenant_snapshot(
                *tenant_id,
                Arc::unwrap_or_clone(snapshot.provider_registry),
            )?;
            *routing_snapshots = routing_snapshots
                .clone()
                .with_tenant_snapshot(*tenant_id, Arc::unwrap_or_clone(snapshot.routing_scores))?;
            continue;
        }
        let tenant_inputs = RuntimeGatewayAuthorityTenantSnapshotInputs {
            common: inputs,
            tenant_id: *tenant_id,
        };
        runtime_gateway_load_non_enforcing_authority_tenant_snapshots(
            &tenant_inputs,
            policy_snapshots,
            classification_snapshots,
            provider_snapshots,
            routing_snapshots,
        )?;
    }
    Ok(())
}

struct RuntimeGatewayAuthorityTenantSnapshotInputs<'a> {
    common: &'a RuntimeGatewayAuthoritySnapshotInputs<'a>,
    tenant_id: prodex_domain::TenantId,
}

fn runtime_gateway_load_non_enforcing_authority_tenant_snapshots(
    inputs: &RuntimeGatewayAuthorityTenantSnapshotInputs<'_>,
    policy_snapshots: &mut crate::runtime_governance::RuntimeGovernanceAuthoritySnapshotSet,
    classification_snapshots: &mut super::super::local_rewrite_classification_rules::RuntimeClassificationRulesSnapshotSet,
    provider_snapshots: &mut super::super::local_rewrite_provider_registry::RuntimeGatewayProviderRegistrySnapshotSet,
    routing_snapshots: &mut super::super::local_rewrite_provider_registry::RuntimeGatewayRoutingScoresSnapshotSet,
) -> Result<()> {
    let common = inputs.common;
    let authority = common.authority;
    let sqlite_repository = common.sqlite_repository;
    let tenant_id = inputs.tenant_id;
    let governance_policy = common.governance_policy;
    let deployment_mode = common.deployment_mode;
    let provider = common.provider;
    let provider_credential = common.provider_credential;
    let policy = runtime_gateway_load_governance_snapshot(
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
    )
    .and_then(|stored| {
        let snapshot =
            crate::runtime_governance::compile_runtime_governance_artifact_for_deployment(
                &stored.compiled_artifact,
                deployment_mode,
            )?;
        anyhow::ensure!(
            snapshot.application.policy.revision().to_string() == stored.revision_id,
            "policy artifact revision does not match stored revision"
        );
        Ok(snapshot)
    });
    let classification = runtime_gateway_load_governance_snapshot(
        authority,
        sqlite_repository,
        tenant_id,
        prodex_storage::GovernanceArtifactKind::ClassificationRules,
        |input| {
            governance_refresh::runtime_gateway_governance_artifact_is_valid(
                governance_policy,
                deployment_mode,
                provider,
                provider_credential,
                input,
            )
        },
    )
    .and_then(|stored| {
        let snapshot = super::super::local_rewrite_classification_rules::compile_runtime_classification_rules_artifact(
            tenant_id,
            &stored.compiled_artifact,
        )?;
        anyhow::ensure!(
            snapshot.classification_rules().revision().as_str() == stored.revision_id,
            "classification artifact revision does not match stored revision"
        );
        Ok(snapshot)
    });
    let registry = runtime_gateway_load_governance_snapshot(
        authority,
        sqlite_repository,
        tenant_id,
        prodex_storage::GovernanceArtifactKind::ProviderRegistry,
        |input| {
            governance_refresh::runtime_gateway_governance_artifact_is_valid(
                governance_policy,
                deployment_mode,
                provider,
                provider_credential,
                input,
            )
        },
    )
    .and_then(|stored| {
        let snapshot = super::super::local_rewrite_provider_registry::compile_runtime_gateway_provider_registry_artifact_for_deployment(
            &stored.compiled_artifact,
            provider,
            provider_credential,
            deployment_mode,
        )?;
        anyhow::ensure!(
            snapshot.revision().to_string() == stored.revision_id,
            "provider registry artifact revision does not match stored revision"
        );
        Ok(snapshot)
    });
    let routing = runtime_gateway_load_governance_snapshot(
        authority,
        sqlite_repository,
        tenant_id,
        prodex_storage::GovernanceArtifactKind::RoutingScores,
        |input| {
            governance_refresh::runtime_gateway_governance_artifact_is_valid(
                governance_policy,
                deployment_mode,
                provider,
                provider_credential,
                input,
            )
        },
    )
    .and_then(|stored| {
        let snapshot = super::super::local_rewrite_provider_registry::compile_runtime_gateway_routing_scores_artifact(
            &stored.compiled_artifact,
        )?;
        anyhow::ensure!(
            snapshot.revision.to_string() == stored.revision_id,
            "routing scores artifact revision does not match stored revision"
        );
        Ok(snapshot)
    });

    if let Ok(policy) = policy {
        *policy_snapshots = policy_snapshots
            .clone()
            .with_tenant_snapshot(tenant_id, policy)?;
    }
    if let Ok(classification) = classification {
        *classification_snapshots = classification_snapshots
            .clone()
            .with_tenant_snapshot(tenant_id, classification)?;
    }
    if let Ok(registry) = registry {
        *provider_snapshots = provider_snapshots
            .clone()
            .with_tenant_snapshot(tenant_id, registry)?;
    }
    if let Ok(routing) = routing {
        *routing_snapshots = routing_snapshots
            .clone()
            .with_tenant_snapshot(tenant_id, routing)?;
    }
    Ok(())
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_gateway_load_governance_snapshot(
    authority: &RuntimeGovernanceAuthority,
    sqlite_repository: Option<&prodex_storage_sqlite_runtime::GovernanceSqliteRepository>,
    tenant_id: prodex_domain::TenantId,
    kind: prodex_storage::GovernanceArtifactKind,
    validate_artifact: impl FnMut(&prodex_storage::GovernanceArtifactValidationInput<'_>) -> bool,
) -> Result<prodex_storage::GovernanceSnapshot> {
    match authority {
        RuntimeGovernanceAuthority::Sqlite { .. } => sqlite_repository
            .context("authoritative SQLite governance repository is unavailable")?
            .load_snapshot(tenant_id, kind, validate_artifact)
            .map_err(anyhow::Error::from),
        RuntimeGovernanceAuthority::Postgres {
            repository,
            runtime,
            ..
        } => runtime
            .block_on(repository.governance_load_snapshot(tenant_id, kind, validate_artifact))
            .map_err(anyhow::Error::from),
    }
}
