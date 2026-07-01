use std::path::PathBuf;

use prodex_storage_postgres::{PostgresRuntimeMode, plan_postgres_migrations};
use prodex_storage_sqlite::{SqliteRuntimeMode, plan_sqlite_migrations};

pub use prodex_app::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GatewayMigrationTarget {
    Sqlite { path: PathBuf },
    Postgres { url_env: String },
}

pub fn run_gateway_migrate(target: GatewayMigrationTarget) -> Result<String, String> {
    match target {
        GatewayMigrationTarget::Sqlite { path } => {
            let plan = plan_sqlite_migrations(SqliteRuntimeMode::ExternalMigrator)
                .map_err(|err| err.to_string())?;
            let conn = rusqlite::Connection::open(&path)
                .map_err(|err| format!("failed to open sqlite database {}: {err}", path.display()))?;
            for migration in &plan.migrations {
                conn.execute_batch(migration.sql).map_err(|err| {
                    format!("failed to apply sqlite migration {}: {err}", migration.name)
                })?;
            }
            Ok(format!(
                "applied {} sqlite migration(s)",
                plan.migrations.len()
            ))
        }
        GatewayMigrationTarget::Postgres { url_env } => {
            let url = std::env::var(&url_env)
                .map_err(|_| format!("missing postgres URL environment variable {url_env}"))?;
            let plan = plan_postgres_migrations(PostgresRuntimeMode::ExternalMigrator)
                .map_err(|err| err.to_string())?;
            let mut client = postgres::Client::connect(&url, postgres::NoTls)
                .map_err(|err| format!("failed to connect postgres from {url_env}: {err}"))?;
            for migration in &plan.migrations {
                client.batch_execute(migration.sql).map_err(|err| {
                    format!(
                        "failed to apply postgres migration {}: {err}",
                        migration.name
                    )
                })?;
            }
            Ok(format!(
                "applied {} postgres migration(s)",
                plan.migrations.len()
            ))
        }
    }
}
