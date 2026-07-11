use super::gateway_secret_config::GatewaySecretResolver;
use super::*;
use prodex_domain::{
    ProductionReadinessTopology, SecretPurpose, plan_deployment_security_error_response,
    plan_production_deployment_readiness,
};
use std::{env, path::PathBuf};

const PRODEX_GATEWAY_REPLICA_COUNT_ENV: &str = "PRODEX_GATEWAY_REPLICA_COUNT";
const PRODEX_GATEWAY_REDIS_URL_ENV: &str = "PRODEX_GATEWAY_REDIS_URL";
const PRODEX_REQUIRE_MULTI_REPLICA_ACCOUNTING_CHECKS_ENV: &str =
    "PRODEX_REQUIRE_MULTI_REPLICA_ACCOUNTING_CHECKS";

#[cfg(test)]
pub(crate) fn gateway_state_store_config(
    paths: &AppPaths,
    policy: &prodex_runtime_policy::RuntimePolicyGatewaySettings,
) -> Result<RuntimeGatewayStateStore> {
    let resolver = GatewaySecretResolver::from_policy(&Default::default())?;
    gateway_state_store_config_with_resolver(paths, policy, &resolver)
}

pub(crate) fn gateway_state_store_config_with_resolver(
    paths: &AppPaths,
    policy: &prodex_runtime_policy::RuntimePolicyGatewaySettings,
    resolver: &GatewaySecretResolver,
) -> Result<RuntimeGatewayStateStore> {
    let backend = match policy.state.backend.as_deref() {
        Some(value) if gateway_exact_policy_identifier(value) => value,
        Some(_) => bail!("gateway.state.backend must be non-empty without whitespace"),
        None => "file",
    };
    match backend.to_ascii_lowercase().as_str() {
        "file" => Ok(RuntimeGatewayStateStore::file(paths)),
        "sqlite" => {
            let configured = policy
                .state
                .sqlite_path
                .as_deref()
                .map(|value| {
                    if value.trim().is_empty() {
                        bail!("gateway.state.sqlite_path cannot be empty");
                    }
                    Ok(value)
                })
                .transpose()?
                .unwrap_or("gateway-state.sqlite");
            let path = PathBuf::from(configured);
            let path = if path.is_absolute() {
                path
            } else {
                paths.root.join(path)
            };
            Ok(RuntimeGatewayStateStore::sqlite(path))
        }
        "postgres" => {
            if policy
                .state
                .postgres_url_env
                .as_deref()
                .is_some_and(|value| !gateway_exact_policy_identifier(value))
            {
                bail!("gateway.state.postgres_url_env must be non-empty without whitespace");
            }
            let env_name = policy.state.postgres_url_env.as_deref().or_else(|| {
                (!resolver.production() && policy.state.postgres_url_ref.is_none())
                    .then_some("PRODEX_GATEWAY_POSTGRES_URL")
            });
            let url = resolver
                .resolve_static(
                    "gateway.state.backend=postgres",
                    policy.state.postgres_url_ref.as_ref(),
                    env_name,
                    None,
                    SecretPurpose::DataPlaneCredential,
                )?
                .context("gateway.state.backend=postgres requires a secret source")?;
            let coordination_redis_url = gateway_coordination_redis_url(policy, resolver)?;
            Ok(RuntimeGatewayStateStore::postgres_with_coordination(
                env_name.unwrap_or("projected-secret").to_string(),
                url,
                coordination_redis_url,
            ))
        }
        "redis" => {
            if policy
                .state
                .redis_url_env
                .as_deref()
                .is_some_and(|value| !gateway_exact_policy_identifier(value))
            {
                bail!("gateway.state.redis_url_env must be non-empty without whitespace");
            }
            let env_name = policy.state.redis_url_env.as_deref().or_else(|| {
                (!resolver.production() && policy.state.redis_url_ref.is_none())
                    .then_some(PRODEX_GATEWAY_REDIS_URL_ENV)
            });
            let url = resolver
                .resolve_static(
                    "gateway.state.backend=redis",
                    policy.state.redis_url_ref.as_ref(),
                    env_name,
                    None,
                    SecretPurpose::DataPlaneCredential,
                )?
                .context("gateway.state.backend=redis requires a secret source")?;
            Ok(RuntimeGatewayStateStore::redis(
                env_name.unwrap_or("projected-secret").to_string(),
                url,
            ))
        }
        other => {
            bail!("gateway.state.backend must be file, sqlite, postgres, or redis, got {other:?}")
        }
    }
}

pub(crate) fn gateway_validate_runtime_topology(
    state_store: &RuntimeGatewayStateStore,
) -> Result<()> {
    let gateway_replica_count = gateway_runtime_replica_count()?;
    let require_multi_replica_accounting_checks =
        gateway_require_multi_replica_accounting_checks()?;
    if require_multi_replica_accounting_checks {
        let topology = gateway_production_readiness_topology(state_store, gateway_replica_count)?;
        plan_production_deployment_readiness(&topology).map_err(|report| {
            let response = plan_deployment_security_error_response(&report);
            anyhow::anyhow!("{}: {}: {:?}", response.code, response.message, report)
        })?;
    }
    let Some(topology) = gateway_application_runtime_topology(
        state_store,
        gateway_replica_count,
        require_multi_replica_accounting_checks,
    )?
    else {
        return Ok(());
    };
    prodex_application::plan_application_runtime(topology).map_err(|error| {
        let response = prodex_application::plan_application_runtime_error_response(&error);
        anyhow::anyhow!("{}: {}: {}", response.code, response.message, error)
    })?;
    Ok(())
}

fn gateway_production_readiness_topology(
    state_store: &RuntimeGatewayStateStore,
    gateway_replica_count: u16,
) -> Result<ProductionReadinessTopology> {
    Ok(ProductionReadinessTopology {
        gateway_replica_count,
        require_multi_replica_accounting_checks: true,
        shared_postgres_configured: matches!(
            state_store,
            RuntimeGatewayStateStore::Postgres { .. }
        ),
        shared_redis_configured: state_store.coordination_redis_url().is_some(),
    })
}

fn gateway_application_runtime_topology(
    state_store: &RuntimeGatewayStateStore,
    gateway_replica_count: u16,
    require_multi_replica_accounting_checks: bool,
) -> Result<Option<prodex_application::ApplicationRuntimeTopology>> {
    let durable_store = match state_store {
        RuntimeGatewayStateStore::Sqlite { .. } => prodex_storage::DurableStoreKind::Sqlite,
        RuntimeGatewayStateStore::Postgres { .. } => prodex_storage::DurableStoreKind::Postgres,
        RuntimeGatewayStateStore::File { .. } | RuntimeGatewayStateStore::Redis { .. } => {
            return Ok(None);
        }
    };
    let shared_redis_configured = state_store.coordination_redis_url().is_some();
    Ok(Some(prodex_application::ApplicationRuntimeTopology {
        storage: prodex_storage::StorageTopology {
            durable_store,
            cache_store: shared_redis_configured.then_some(prodex_storage::CacheStoreKind::Redis),
            migration_policy: prodex_storage::MigrationRuntimePolicy::RequestPathForbidden,
        },
        migrations: prodex_application::ApplicationMigrationMode::ExternalOnly,
        gateway_replica_count,
        require_multi_replica_accounting_checks,
        redis_rate_limit: None,
        redis_cache: vec![],
        redis_coordination: vec![],
    }))
}

fn gateway_runtime_replica_count() -> Result<u16> {
    match env::var(PRODEX_GATEWAY_REPLICA_COUNT_ENV) {
        Ok(value) => {
            if value.is_empty() {
                bail!("{PRODEX_GATEWAY_REPLICA_COUNT_ENV} cannot be empty");
            }
            if value.chars().any(char::is_whitespace) {
                bail!("{PRODEX_GATEWAY_REPLICA_COUNT_ENV} must not contain whitespace");
            }
            let count = value.parse::<u16>().with_context(|| {
                format!("{PRODEX_GATEWAY_REPLICA_COUNT_ENV} must be a positive integer")
            })?;
            if count == 0 {
                bail!("{PRODEX_GATEWAY_REPLICA_COUNT_ENV} must be at least 1");
            }
            Ok(count)
        }
        Err(_) => Ok(1),
    }
}

fn gateway_require_multi_replica_accounting_checks() -> Result<bool> {
    match env::var(PRODEX_REQUIRE_MULTI_REPLICA_ACCOUNTING_CHECKS_ENV) {
        Ok(value) => {
            if value.is_empty() {
                bail!("{PRODEX_REQUIRE_MULTI_REPLICA_ACCOUNTING_CHECKS_ENV} cannot be empty");
            }
            if value.chars().any(char::is_whitespace) {
                bail!(
                    "{PRODEX_REQUIRE_MULTI_REPLICA_ACCOUNTING_CHECKS_ENV} must not contain whitespace"
                );
            }
            match value.to_ascii_lowercase().as_str() {
                "1" | "true" | "yes" | "on" => Ok(true),
                "0" | "false" | "no" | "off" => Ok(false),
                _ => bail!(
                    "{PRODEX_REQUIRE_MULTI_REPLICA_ACCOUNTING_CHECKS_ENV} must be one of true,false,1,0,yes,no,on,off"
                ),
            }
        }
        Err(_) => Ok(false),
    }
}

fn gateway_coordination_redis_url(
    policy: &prodex_runtime_policy::RuntimePolicyGatewaySettings,
    resolver: &GatewaySecretResolver,
) -> Result<Option<String>> {
    if policy
        .state
        .redis_url_env
        .as_deref()
        .is_some_and(|value| !gateway_exact_policy_identifier(value))
    {
        bail!("gateway.state.redis_url_env must be non-empty without whitespace");
    }
    let development_env = (!resolver.production()
        && policy.state.redis_url_ref.is_none()
        && policy.state.redis_url_env.is_none()
        && env::var_os(PRODEX_GATEWAY_REDIS_URL_ENV).is_some())
    .then_some(PRODEX_GATEWAY_REDIS_URL_ENV);
    resolver.resolve_static(
        "gateway.state.redis_url",
        policy.state.redis_url_ref.as_ref(),
        policy.state.redis_url_env.as_deref().or(development_env),
        None,
        SecretPurpose::DataPlaneCredential,
    )
}

fn gateway_exact_policy_identifier(value: &str) -> bool {
    !value.is_empty() && !value.chars().any(char::is_whitespace)
}
