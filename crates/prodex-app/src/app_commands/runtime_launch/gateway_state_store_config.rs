use super::*;
use prodex_domain::{
    ProductionReadinessTopology, plan_deployment_security_error_response,
    plan_production_deployment_readiness,
};
use std::{env, path::PathBuf};

const PRODEX_GATEWAY_REPLICA_COUNT_ENV: &str = "PRODEX_GATEWAY_REPLICA_COUNT";
const PRODEX_GATEWAY_REDIS_URL_ENV: &str = "PRODEX_GATEWAY_REDIS_URL";
const PRODEX_REQUIRE_MULTI_REPLICA_ACCOUNTING_CHECKS_ENV: &str =
    "PRODEX_REQUIRE_MULTI_REPLICA_ACCOUNTING_CHECKS";

pub(crate) fn gateway_state_store_config(
    paths: &AppPaths,
    policy: &prodex_runtime_policy::RuntimePolicyGatewaySettings,
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
            let env_name = policy
                .state
                .postgres_url_env
                .as_deref()
                .map(|value| {
                    gateway_exact_policy_identifier(value)
                        .then_some(value)
                        .context(
                            "gateway.state.postgres_url_env must be non-empty without whitespace",
                        )
                })
                .transpose()?
                .unwrap_or("PRODEX_GATEWAY_POSTGRES_URL");
            let url = gateway_state_url_from_env("gateway.state.backend=postgres", env_name)?;
            Ok(RuntimeGatewayStateStore::postgres(
                env_name.to_string(),
                url,
            ))
        }
        "redis" => {
            let env_name = policy
                .state
                .redis_url_env
                .as_deref()
                .map(|value| {
                    gateway_exact_policy_identifier(value)
                        .then_some(value)
                        .context("gateway.state.redis_url_env must be non-empty without whitespace")
                })
                .transpose()?
                .unwrap_or("PRODEX_GATEWAY_REDIS_URL");
            let url = gateway_state_url_from_env("gateway.state.backend=redis", env_name)?;
            Ok(RuntimeGatewayStateStore::redis(env_name.to_string(), url))
        }
        other => {
            bail!("gateway.state.backend must be file, sqlite, postgres, or redis, got {other:?}")
        }
    }
}

fn gateway_state_url_from_env(context: &str, env_name: &str) -> Result<String> {
    let url = env::var(env_name).with_context(|| format!("{context} requires {env_name}"))?;
    if url.is_empty() {
        bail!("{context} env {env_name} cannot be empty");
    }
    if url.chars().any(char::is_whitespace) {
        bail!("{context} env {env_name} must not contain whitespace");
    }
    Ok(url)
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
    if require_multi_replica_accounting_checks {
        let response =
            prodex_application::plan_application_runtime_accounting_verification_required_response(
            );
        bail!(
            "{}: {}: legacy prodex gateway admission still uses local usage state; durable reservation backend is not wired yet",
            response.code,
            response.message,
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
        shared_redis_configured: gateway_has_shared_redis_url()?,
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
    let shared_redis_configured = gateway_has_shared_redis_url()?;
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

fn gateway_has_shared_redis_url() -> Result<bool> {
    match env::var(PRODEX_GATEWAY_REDIS_URL_ENV) {
        Ok(value) => {
            if value.is_empty() {
                bail!("{PRODEX_GATEWAY_REDIS_URL_ENV} cannot be empty");
            }
            if value.chars().any(char::is_whitespace) {
                bail!("{PRODEX_GATEWAY_REDIS_URL_ENV} must not contain whitespace");
            }
            Ok(true)
        }
        Err(_) => Ok(false),
    }
}

fn gateway_exact_policy_identifier(value: &str) -> bool {
    !value.is_empty() && !value.chars().any(char::is_whitespace)
}
