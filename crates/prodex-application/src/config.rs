//! Configuration publication, activation, readiness, and runtime topology plans.

use super::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationConfigurationPublicationRequest<T> {
    pub durable_store: DurableStoreKind,
    pub publication: ConfigurationPublicationRequest<T>,
    pub previous_digest: Option<AuditDigest>,
    pub event_digest: AuditDigest,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApplicationConfigurationPublicationAuditStoragePlan {
    Postgres(PostgresAppendOnlyAuditSqlPlan),
    Sqlite(SqliteAppendOnlyAuditSqlPlan),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationConfigurationPublicationPlan<T> {
    pub decision: ConfigurationPublicationDecision<T>,
    pub audit_storage: ApplicationConfigurationPublicationAuditStoragePlan,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationConfigurationActivationRequest<'a, T> {
    pub cache_state: &'a ConfigCacheState,
    pub publication_decision: &'a ConfigurationPublicationDecision<T>,
    pub redis_cache_ttl_seconds: Option<u64>,
    pub event_delivery: ConfigPublicationEventDelivery,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationConfigurationActivationPlan {
    pub activation: Option<ConfigActivationPlan>,
    pub redis_cache: Option<RedisCachePlan>,
    pub publication_event: Option<ConfigPublicationEventPlan>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationConfigurationReadinessRequest {
    pub live: bool,
    pub startup_complete: bool,
    pub draining: bool,
    pub cache_state: ConfigCacheState,
    pub now_unix_ms: u64,
    pub checks: Vec<HealthCheck>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationConfigurationReadinessPlan {
    pub refresh_decision: ConfigRefreshDecision,
    pub snapshot: HealthSnapshot,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApplicationConfigurationActivationError {
    Publication(ConfigPublicationError),
    PublicationEvent(ConfigPublicationEventError),
    Redis(prodex_storage_redis::RedisPlanError),
}

impl fmt::Display for ApplicationConfigurationActivationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Publication(err) => err.fmt(f),
            Self::PublicationEvent(err) => err.fmt(f),
            Self::Redis(err) => err.fmt(f),
        }
    }
}

impl Error for ApplicationConfigurationActivationError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationConfigurationActivationErrorStatus {
    BadRequest,
    Conflict,
    ServiceUnavailable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationConfigurationActivationErrorResponsePlan {
    pub status: ApplicationConfigurationActivationErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_application_configuration_activation_error_response(
    error: &ApplicationConfigurationActivationError,
) -> ApplicationConfigurationActivationErrorResponsePlan {
    match error {
        ApplicationConfigurationActivationError::Publication(error) => {
            application_configuration_activation_response_from_config(
                plan_config_activation_error_response(error),
            )
        }
        ApplicationConfigurationActivationError::PublicationEvent(error) => {
            let response = plan_config_publication_event_error_response(error);
            ApplicationConfigurationActivationErrorResponsePlan {
                status: match response.status {
                    prodex_config::ConfigPublicationEventErrorStatus::InvalidConfiguration => {
                        ApplicationConfigurationActivationErrorStatus::ServiceUnavailable
                    }
                },
                code: response.code,
                message: response.message,
            }
        }
        ApplicationConfigurationActivationError::Redis(error) => {
            application_configuration_activation_response_from_redis(plan_redis_error_response(
                error,
            ))
        }
    }
}

pub fn plan_application_configuration_readiness_snapshot(
    request: ApplicationConfigurationReadinessRequest,
) -> ApplicationConfigurationReadinessPlan {
    let refresh_decision = evaluate_config_refresh(&request.cache_state, request.now_unix_ms);
    let active_policy_revision = match refresh_decision {
        ConfigRefreshDecision::UseActive | ConfigRefreshDecision::RefreshAsync => {
            request.cache_state.active_revision_id
        }
        ConfigRefreshDecision::UseLastKnownGood => request.cache_state.last_known_good_revision_id,
        ConfigRefreshDecision::RefreshRequired | ConfigRefreshDecision::RejectedInvalidated => None,
    };
    let mut checks = request.checks;
    checks.push(application_configuration_readiness_check(refresh_decision));
    let snapshot = HealthSnapshot::new(
        request.live,
        request.startup_complete,
        request.draining,
        active_policy_revision,
        checks,
    );
    ApplicationConfigurationReadinessPlan {
        refresh_decision,
        snapshot,
    }
}

fn application_configuration_readiness_check(
    refresh_decision: ConfigRefreshDecision,
) -> HealthCheck {
    match refresh_decision {
        ConfigRefreshDecision::UseActive => {
            HealthCheck::new("configuration", HealthState::Passing, None::<String>)
        }
        ConfigRefreshDecision::RefreshAsync => HealthCheck::new(
            "configuration",
            HealthState::Degraded,
            Some("configuration refresh is pending"),
        ),
        ConfigRefreshDecision::UseLastKnownGood => HealthCheck::new(
            "configuration",
            HealthState::Degraded,
            Some("using last known good configuration"),
        ),
        ConfigRefreshDecision::RefreshRequired => HealthCheck::new(
            "configuration",
            HealthState::Failing,
            Some("configuration refresh is required"),
        ),
        ConfigRefreshDecision::RejectedInvalidated => HealthCheck::new(
            "configuration",
            HealthState::Failing,
            Some("configuration revision is invalidated"),
        ),
    }
}

fn application_configuration_activation_response_from_config(
    response: ConfigPublicationErrorResponsePlan,
) -> ApplicationConfigurationActivationErrorResponsePlan {
    ApplicationConfigurationActivationErrorResponsePlan {
        status: match response.status {
            ConfigPublicationErrorStatus::BadRequest => {
                ApplicationConfigurationActivationErrorStatus::BadRequest
            }
            ConfigPublicationErrorStatus::Conflict => {
                ApplicationConfigurationActivationErrorStatus::Conflict
            }
        },
        code: response.code,
        message: response.message,
    }
}

fn application_configuration_activation_response_from_redis(
    response: RedisPlanErrorResponsePlan,
) -> ApplicationConfigurationActivationErrorResponsePlan {
    ApplicationConfigurationActivationErrorResponsePlan {
        status: match response.status {
            RedisPlanErrorStatus::ServiceUnavailable => {
                ApplicationConfigurationActivationErrorStatus::ServiceUnavailable
            }
        },
        code: "configuration_cache_unavailable",
        message: "configuration cache is temporarily unavailable",
    }
}

pub fn plan_application_configuration_activation<T>(
    request: ApplicationConfigurationActivationRequest<'_, T>,
) -> Result<ApplicationConfigurationActivationPlan, ApplicationConfigurationActivationError> {
    match request.publication_decision {
        ConfigurationPublicationDecision::Authorized(publication) => {
            let activation = plan_config_activation(request.cache_state, &publication.candidate)
                .map_err(ApplicationConfigurationActivationError::Publication)?;
            let redis_cache = request
                .redis_cache_ttl_seconds
                .map(|ttl_seconds| {
                    plan_policy_revision_cache(
                        activation.next_state.tenant_id,
                        activation.activated_revision_id,
                        ttl_seconds,
                    )
                    .map_err(ApplicationConfigurationActivationError::Redis)
                })
                .transpose()?;
            let publication_event =
                plan_config_publication_event(&activation, request.event_delivery)
                    .map_err(ApplicationConfigurationActivationError::PublicationEvent)?;
            Ok(ApplicationConfigurationActivationPlan {
                activation: Some(activation),
                redis_cache,
                publication_event: Some(publication_event),
            })
        }
        ConfigurationPublicationDecision::Denied { .. } => {
            Ok(ApplicationConfigurationActivationPlan {
                activation: None,
                redis_cache: None,
                publication_event: None,
            })
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApplicationConfigurationPublicationError {
    Postgres(prodex_storage_postgres::PostgresStoragePlanError),
    Sqlite(prodex_storage_sqlite::SqliteStoragePlanError),
}

impl fmt::Display for ApplicationConfigurationPublicationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Postgres(err) => err.fmt(f),
            Self::Sqlite(err) => err.fmt(f),
        }
    }
}

impl Error for ApplicationConfigurationPublicationError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationConfigurationPublicationErrorStatus {
    BadRequest,
    Conflict,
    Forbidden,
    ServiceUnavailable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationConfigurationPublicationErrorResponsePlan {
    pub status: ApplicationConfigurationPublicationErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_application_configuration_publication_decision_error_response<T>(
    decision: &ConfigurationPublicationDecision<T>,
) -> Option<ApplicationConfigurationPublicationErrorResponsePlan> {
    match decision {
        ConfigurationPublicationDecision::Authorized(_) => None,
        ConfigurationPublicationDecision::Denied { error, .. } => Some(
            application_configuration_publication_response_from_control_plane(
                plan_configuration_publication_error_response(error),
            ),
        ),
    }
}

pub fn plan_application_configuration_publication_error_response(
    error: &ApplicationConfigurationPublicationError,
) -> ApplicationConfigurationPublicationErrorResponsePlan {
    match error {
        ApplicationConfigurationPublicationError::Postgres(error) => {
            application_configuration_publication_response_from_postgres(
                plan_postgres_storage_error_response(error),
                "audit_storage_unavailable",
                "audit storage is temporarily unavailable",
            )
        }
        ApplicationConfigurationPublicationError::Sqlite(error) => {
            application_configuration_publication_response_from_sqlite(
                plan_sqlite_storage_error_response(error),
                "audit_storage_unavailable",
                "audit storage is temporarily unavailable",
            )
        }
    }
}

fn application_configuration_publication_response_from_postgres(
    response: PostgresStorageErrorResponsePlan,
    code: &'static str,
    message: &'static str,
) -> ApplicationConfigurationPublicationErrorResponsePlan {
    ApplicationConfigurationPublicationErrorResponsePlan {
        status: match response.status {
            PostgresStorageErrorStatus::ServiceUnavailable => {
                ApplicationConfigurationPublicationErrorStatus::ServiceUnavailable
            }
        },
        code,
        message,
    }
}

fn application_configuration_publication_response_from_sqlite(
    response: SqliteStorageErrorResponsePlan,
    code: &'static str,
    message: &'static str,
) -> ApplicationConfigurationPublicationErrorResponsePlan {
    ApplicationConfigurationPublicationErrorResponsePlan {
        status: match response.status {
            SqliteStorageErrorStatus::ServiceUnavailable => {
                ApplicationConfigurationPublicationErrorStatus::ServiceUnavailable
            }
        },
        code,
        message,
    }
}

fn application_configuration_publication_response_from_control_plane(
    response: ConfigurationPublicationErrorResponsePlan,
) -> ApplicationConfigurationPublicationErrorResponsePlan {
    ApplicationConfigurationPublicationErrorResponsePlan {
        status: match response.status {
            ConfigurationPublicationErrorStatus::BadRequest => {
                ApplicationConfigurationPublicationErrorStatus::BadRequest
            }
            ConfigurationPublicationErrorStatus::Conflict => {
                ApplicationConfigurationPublicationErrorStatus::Conflict
            }
            ConfigurationPublicationErrorStatus::Forbidden => {
                ApplicationConfigurationPublicationErrorStatus::Forbidden
            }
        },
        code: response.code,
        message: response.message,
    }
}

pub fn plan_application_configuration_publication<T>(
    request: ApplicationConfigurationPublicationRequest<T>,
) -> Result<ApplicationConfigurationPublicationPlan<T>, ApplicationConfigurationPublicationError> {
    let decision = decide_configuration_publication(request.publication);
    let audit_write = configuration_publication_audit_write(&decision);
    let audit_command = AppendOnlyAuditCommand {
        storage_key: TenantStorageKey::tenant(audit_write.tenant_partition_key),
        event: audit_write.event.clone(),
        previous_digest: request.previous_digest,
        event_digest: request.event_digest,
    };
    let audit_storage = match request.durable_store {
        DurableStoreKind::Postgres => {
            ApplicationConfigurationPublicationAuditStoragePlan::Postgres(
                plan_postgres_append_only_audit(audit_command)
                    .map_err(ApplicationConfigurationPublicationError::Postgres)?,
            )
        }
        DurableStoreKind::Sqlite => ApplicationConfigurationPublicationAuditStoragePlan::Sqlite(
            plan_sqlite_append_only_audit(audit_command)
                .map_err(ApplicationConfigurationPublicationError::Sqlite)?,
        ),
    };
    Ok(ApplicationConfigurationPublicationPlan {
        decision,
        audit_storage,
    })
}

fn configuration_publication_audit_write<T>(
    decision: &ConfigurationPublicationDecision<T>,
) -> &ControlPlaneAuditWritePlan {
    match decision {
        ConfigurationPublicationDecision::Authorized(plan) => &plan.action.audit_write,
        ConfigurationPublicationDecision::Denied { audit_write, .. } => audit_write,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationRuntimeTopology {
    pub storage: StorageTopology,
    pub migrations: ApplicationMigrationMode,
    pub gateway_replica_count: u16,
    pub require_multi_replica_accounting_checks: bool,
    pub redis_rate_limit: Option<RedisRateLimitPlan>,
    pub redis_cache: Vec<RedisCachePlan>,
    pub redis_coordination: Vec<RedisCoordinationPlan>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationMigrationMode {
    ExternalOnly,
    RequestPath,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationRuntimePlan {
    pub durable_store: DurableStoreKind,
    pub migrations_are_external: bool,
    pub redis_is_rebuildable: bool,
    pub accounting_concurrency: Option<MultiReplicaAccountingConcurrencySpec>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApplicationRuntimePlanError {
    MigrationOnRequestPath,
    Storage(prodex_storage::StorageTopologyError),
    AccountingConcurrency(prodex_storage::MultiReplicaAccountingConcurrencySpecError),
    Postgres(prodex_storage_postgres::PostgresStoragePlanError),
    Sqlite(prodex_storage_sqlite::SqliteStoragePlanError),
}

impl fmt::Display for ApplicationRuntimePlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MigrationOnRequestPath => write!(
                f,
                "application topology cannot run migrations on request paths"
            ),
            Self::Storage(err) => err.fmt(f),
            Self::AccountingConcurrency(err) => err.fmt(f),
            Self::Postgres(err) => err.fmt(f),
            Self::Sqlite(err) => err.fmt(f),
        }
    }
}

impl Error for ApplicationRuntimePlanError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationRuntimePlanErrorStatus {
    InvalidConfiguration,
    ServiceUnavailable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationRuntimePlanErrorResponsePlan {
    pub status: ApplicationRuntimePlanErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_application_runtime_error_response(
    error: &ApplicationRuntimePlanError,
) -> ApplicationRuntimePlanErrorResponsePlan {
    match error {
        ApplicationRuntimePlanError::MigrationOnRequestPath => {
            ApplicationRuntimePlanErrorResponsePlan {
                status: ApplicationRuntimePlanErrorStatus::InvalidConfiguration,
                code: "runtime_topology_invalid",
                message: "runtime topology configuration is invalid",
            }
        }
        ApplicationRuntimePlanError::Storage(error) => application_runtime_response_from_storage(
            plan_storage_topology_error_response(error),
            "runtime_topology_invalid",
            "runtime topology configuration is invalid",
        ),
        ApplicationRuntimePlanError::AccountingConcurrency(error) => {
            application_runtime_response_from_storage(
                plan_multi_replica_accounting_error_response(error),
                "runtime_topology_invalid",
                "runtime topology configuration is invalid",
            )
        }
        ApplicationRuntimePlanError::Postgres(error) => application_runtime_response_from_postgres(
            plan_postgres_storage_error_response(error),
            "runtime_storage_plan_unavailable",
            "runtime storage planning is temporarily unavailable",
        ),
        ApplicationRuntimePlanError::Sqlite(error) => application_runtime_response_from_sqlite(
            plan_sqlite_storage_error_response(error),
            "runtime_storage_plan_unavailable",
            "runtime storage planning is temporarily unavailable",
        ),
    }
}

pub(crate) fn application_runtime_response_from_storage(
    response: StoragePlanErrorResponsePlan,
    code: &'static str,
    message: &'static str,
) -> ApplicationRuntimePlanErrorResponsePlan {
    ApplicationRuntimePlanErrorResponsePlan {
        status: match response.status {
            StoragePlanErrorStatus::BadRequest | StoragePlanErrorStatus::InvalidConfiguration => {
                ApplicationRuntimePlanErrorStatus::InvalidConfiguration
            }
        },
        code,
        message,
    }
}

fn application_runtime_response_from_postgres(
    response: PostgresStorageErrorResponsePlan,
    code: &'static str,
    message: &'static str,
) -> ApplicationRuntimePlanErrorResponsePlan {
    ApplicationRuntimePlanErrorResponsePlan {
        status: match response.status {
            PostgresStorageErrorStatus::ServiceUnavailable => {
                ApplicationRuntimePlanErrorStatus::ServiceUnavailable
            }
        },
        code,
        message,
    }
}

fn application_runtime_response_from_sqlite(
    response: SqliteStorageErrorResponsePlan,
    code: &'static str,
    message: &'static str,
) -> ApplicationRuntimePlanErrorResponsePlan {
    ApplicationRuntimePlanErrorResponsePlan {
        status: match response.status {
            SqliteStorageErrorStatus::ServiceUnavailable => {
                ApplicationRuntimePlanErrorStatus::ServiceUnavailable
            }
        },
        code,
        message,
    }
}

pub fn plan_application_runtime(
    topology: ApplicationRuntimeTopology,
) -> Result<ApplicationRuntimePlan, ApplicationRuntimePlanError> {
    topology
        .storage
        .validate_for_gateway()
        .map_err(ApplicationRuntimePlanError::Storage)?;
    if topology.migrations != ApplicationMigrationMode::ExternalOnly {
        return Err(ApplicationRuntimePlanError::MigrationOnRequestPath);
    }
    match topology.storage.durable_store {
        DurableStoreKind::Postgres => {
            plan_postgres_migrations(PostgresRuntimeMode::ExternalMigrator)
                .map_err(ApplicationRuntimePlanError::Postgres)?;
        }
        DurableStoreKind::Sqlite => {
            plan_sqlite_migrations(SqliteRuntimeMode::ExternalMigrator)
                .map_err(ApplicationRuntimePlanError::Sqlite)?;
        }
    }
    let accounting_concurrency = if topology.require_multi_replica_accounting_checks {
        Some(
            plan_multi_replica_accounting_concurrency_spec(
                topology.storage,
                topology.gateway_replica_count,
            )
            .map_err(ApplicationRuntimePlanError::AccountingConcurrency)?,
        )
    } else {
        None
    };
    Ok(ApplicationRuntimePlan {
        durable_store: topology.storage.durable_store,
        migrations_are_external: true,
        redis_is_rebuildable: topology.redis_rate_limit.is_some()
            || !topology.redis_cache.is_empty()
            || !topology.redis_coordination.is_empty(),
        accounting_concurrency,
    })
}
