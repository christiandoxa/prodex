use crate::{
    INITIAL_TENANT_ACCOUNTING_MIGRATION, PostgresBackendOpenMode, PostgresBackendOpenPlan,
    PostgresMigration, PostgresMigrationPhase, PostgresMigrationPlan, PostgresMigrationVersion,
    PostgresRuntimeMode, PostgresStoragePlanError,
};

pub const GROUPED_REQUEST_BUDGET_MIGRATION: PostgresMigration = PostgresMigration {
    version: PostgresMigrationVersion(2),
    phase: PostgresMigrationPhase::Expand,
    name: "002_grouped_request_budget_counter",
    sql: r#"
ALTER TABLE prodex_budget_counters
    ADD COLUMN IF NOT EXISTS request_count BIGINT NOT NULL DEFAULT 0
    CHECK (request_count >= 0);
"#,
};

pub const POSTGRES_MIGRATIONS: &[PostgresMigration] = &[
    INITIAL_TENANT_ACCOUNTING_MIGRATION,
    GROUPED_REQUEST_BUDGET_MIGRATION,
];
pub const REQUIRED_POSTGRES_SCHEMA_VERSION: PostgresMigrationVersion = PostgresMigrationVersion(2);

pub fn plan_postgres_migrations(
    mode: PostgresRuntimeMode,
) -> Result<PostgresMigrationPlan, PostgresStoragePlanError> {
    match mode {
        PostgresRuntimeMode::ExternalMigrator => Ok(PostgresMigrationPlan {
            migrations: POSTGRES_MIGRATIONS.to_vec(),
        }),
        PostgresRuntimeMode::GatewayRequestPath => {
            Err(PostgresStoragePlanError::DdlForbiddenOnRequestPath)
        }
    }
}

pub fn plan_postgres_backend_open(
    mode: PostgresBackendOpenMode,
    observed_schema_version: Option<PostgresMigrationVersion>,
) -> Result<PostgresBackendOpenPlan, PostgresStoragePlanError> {
    match mode {
        PostgresBackendOpenMode::ExternalMigrator => Ok(PostgresBackendOpenPlan {
            mode,
            required_schema_version: REQUIRED_POSTGRES_SCHEMA_VERSION,
            ddl_allowed: true,
            migration_count: POSTGRES_MIGRATIONS.len(),
        }),
        PostgresBackendOpenMode::GatewayStartup | PostgresBackendOpenMode::GatewayRequestPath => {
            let observed =
                observed_schema_version.ok_or(PostgresStoragePlanError::MissingSchemaVersion)?;
            if observed < REQUIRED_POSTGRES_SCHEMA_VERSION {
                return Err(PostgresStoragePlanError::SchemaVersionTooOld {
                    observed,
                    required: REQUIRED_POSTGRES_SCHEMA_VERSION,
                });
            }
            Ok(PostgresBackendOpenPlan {
                mode,
                required_schema_version: REQUIRED_POSTGRES_SCHEMA_VERSION,
                ddl_allowed: false,
                migration_count: 0,
            })
        }
    }
}
