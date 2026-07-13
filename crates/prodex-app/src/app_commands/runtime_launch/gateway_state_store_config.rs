use super::gateway_secret_config::GatewaySecretResolver;
use super::*;
use prodex_domain::{
    ProductionReadinessTopology, SecretPurpose, plan_deployment_security_error_response,
    plan_production_deployment_readiness,
};
#[cfg(test)]
use std::env;
use std::path::PathBuf;

const PRODEX_GATEWAY_REDIS_URL_ENV: &str = "PRODEX_GATEWAY_REDIS_URL";

#[cfg(test)]
pub(crate) fn gateway_state_store_config(
    paths: &AppPaths,
    policy: &prodex_runtime_policy::RuntimePolicyGatewaySettings,
) -> Result<RuntimeGatewayStateStore> {
    let resolver = GatewaySecretResolver::from_policy(&Default::default())?;
    gateway_state_store_config_with_redis_presence(
        paths,
        policy,
        &resolver,
        env::var_os(PRODEX_GATEWAY_REDIS_URL_ENV).is_some(),
    )
}

pub(crate) fn gateway_state_store_config_with_resolver(
    paths: &AppPaths,
    policy: &prodex_runtime_policy::RuntimePolicyGatewaySettings,
    resolver: &GatewaySecretResolver,
    environment: &RuntimeGatewayLaunchEnvironment,
) -> Result<RuntimeGatewayStateStore> {
    gateway_state_store_config_with_redis_presence(
        paths,
        policy,
        resolver,
        environment.secret_env_present(PRODEX_GATEWAY_REDIS_URL_ENV),
    )
}

fn gateway_state_store_config_with_redis_presence(
    paths: &AppPaths,
    policy: &prodex_runtime_policy::RuntimePolicyGatewaySettings,
    resolver: &GatewaySecretResolver,
    development_redis_present: bool,
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
            let coordination_redis_url =
                gateway_coordination_redis_url(policy, resolver, development_redis_present)?;
            let tls = gateway_postgres_tls_config(paths, policy, resolver)?;
            Ok(
                RuntimeGatewayStateStore::postgres_with_coordination_and_tls(
                    env_name.unwrap_or("projected-secret").to_string(),
                    url,
                    coordination_redis_url,
                    tls,
                ),
            )
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

fn gateway_postgres_tls_config(
    paths: &AppPaths,
    policy: &prodex_runtime_policy::RuntimePolicyGatewaySettings,
    resolver: &GatewaySecretResolver,
) -> Result<prodex_storage_postgres_runtime::PostgresTlsConfig> {
    let mode = policy
        .state
        .postgres_tls_mode
        .as_deref()
        .unwrap_or(if resolver.production() {
            "verify-full"
        } else {
            "disable"
        });
    match mode {
        "verify-full" => {
            let ca_path = policy.state.postgres_tls_ca_path.as_deref().map(|value| {
                let path = PathBuf::from(value);
                if path.is_absolute() {
                    path
                } else {
                    paths.root.join(path)
                }
            });
            Ok(prodex_storage_postgres_runtime::PostgresTlsConfig::verify_full(ca_path))
        }
        "disable" if resolver.production() => {
            bail!("gateway.state.postgres_tls_mode=disable is forbidden in production")
        }
        "disable" if policy.state.postgres_tls_ca_path.is_some() => {
            bail!("gateway.state.postgres_tls_ca_path requires postgres_tls_mode=verify-full")
        }
        "disable" => Ok(prodex_storage_postgres_runtime::PostgresTlsConfig::explicit_disable()),
        _ => bail!("gateway.state.postgres_tls_mode must be verify-full or disable"),
    }
}

pub(crate) fn gateway_validate_runtime_topology(
    state_store: &RuntimeGatewayStateStore,
    config: &RuntimeGatewayConfig,
    policy: &prodex_runtime_policy::RuntimePolicyGatewaySettings,
) -> Result<()> {
    gateway_validate_runtime_deployment_policy(config, policy)?;
    let gateway_replica_count = config.replica_count;
    let require_multi_replica_accounting_checks = config.require_multi_replica_accounting_checks;
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

fn gateway_validate_runtime_deployment_policy(
    config: &RuntimeGatewayConfig,
    policy: &prodex_runtime_policy::RuntimePolicyGatewaySettings,
) -> Result<()> {
    if policy
        .replica_count
        .is_some_and(|expected| expected != config.replica_count)
    {
        bail!("gateway.replica_count does not match PRODEX_GATEWAY_REPLICA_COUNT runtime topology");
    }
    if policy
        .require_multi_replica_accounting_checks
        .is_some_and(|expected| expected != config.require_multi_replica_accounting_checks)
    {
        bail!(
            "gateway.require_multi_replica_accounting_checks does not match PRODEX_REQUIRE_MULTI_REPLICA_ACCOUNTING_CHECKS runtime topology"
        );
    }
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

fn gateway_coordination_redis_url(
    policy: &prodex_runtime_policy::RuntimePolicyGatewaySettings,
    resolver: &GatewaySecretResolver,
    development_redis_present: bool,
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
        && development_redis_present)
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

#[cfg(test)]
mod deployment_policy_tests {
    use super::*;

    fn runtime_config(replicas: u16, checks: bool) -> RuntimeGatewayConfig {
        RuntimeGatewayConfig {
            replica_count: replicas,
            require_multi_replica_accounting_checks: checks,
            launch: RuntimeGatewayLaunchEnvironment::default(),
        }
    }

    #[test]
    fn runtime_topology_must_match_deployment_policy() {
        let policy = prodex_runtime_policy::RuntimePolicyGatewaySettings {
            replica_count: Some(2),
            require_multi_replica_accounting_checks: Some(true),
            ..Default::default()
        };
        assert!(
            gateway_validate_runtime_deployment_policy(&runtime_config(2, true), &policy).is_ok()
        );
        assert!(
            gateway_validate_runtime_deployment_policy(&runtime_config(1, true), &policy).is_err()
        );
        assert!(
            gateway_validate_runtime_deployment_policy(&runtime_config(2, false), &policy).is_err()
        );
    }
}
