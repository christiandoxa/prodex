use prodex_domain::{SecretProvider as _, SecretPurpose, SecretRef, SecretResolutionRequest};
use prodex_storage_postgres::{PostgresRuntimeMode, plan_postgres_migrations};
use prodex_storage_sqlite::{SqliteRuntimeMode, plan_sqlite_migrations};
use secret_store::ProjectedSecretProvider;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GatewayMigrationTarget {
    Sqlite {
        path: PathBuf,
    },
    Postgres {
        url_env: String,
        tls: prodex_storage_postgres_runtime::PostgresTlsConfig,
    },
    PostgresProjected {
        reference: SecretRef,
        projected_root: PathBuf,
        tls: prodex_storage_postgres_runtime::PostgresTlsConfig,
    },
}

pub fn run_gateway_migrate(target: GatewayMigrationTarget) -> Result<String, String> {
    match target {
        GatewayMigrationTarget::Sqlite { path } => {
            let plan = plan_sqlite_migrations(SqliteRuntimeMode::ExternalMigrator)
                .map_err(|err| err.to_string())?;
            let conn = rusqlite::Connection::open(&path).map_err(|err| {
                format!("failed to open sqlite database {}: {err}", path.display())
            })?;
            for migration in &plan.migrations {
                conn.execute_batch(migration.sql).map_err(|err| {
                    format!("failed to apply sqlite migration {}: {err}", migration.name)
                })?;
            }
            prodex_app::migrate_gateway_compatibility_state_sqlite(&path).map_err(|err| {
                format!("failed to migrate gateway sqlite compatibility state: {err:#}")
            })?;
            Ok(format!(
                "applied {} sqlite migration(s) and ensured gateway compatibility schema",
                plan.migrations.len()
            ))
        }
        GatewayMigrationTarget::Postgres { url_env, tls } => {
            let url = std::env::var(&url_env)
                .map_err(|_| format!("missing postgres URL environment variable {url_env}"))?;
            run_postgres_gateway_migrations(&url, &tls, &format!("environment variable {url_env}"))
        }
        GatewayMigrationTarget::PostgresProjected {
            reference,
            projected_root,
            tls,
        } => {
            let provider = ProjectedSecretProvider::new(projected_root, reference.provider())
                .map_err(|_| "failed to initialize projected postgres URL provider".to_string())?;
            let material = provider
                .resolve(&SecretResolutionRequest::new(
                    reference,
                    SecretPurpose::DataPlaneCredential,
                ))
                .map_err(|_| "failed to resolve projected postgres URL".to_string())?;
            material.with_exposed_secret(|bytes| {
                let url = std::str::from_utf8(bytes)
                    .map_err(|_| "projected postgres URL must be UTF-8".to_string())?;
                run_postgres_gateway_migrations(url, &tls, "projected secret")
            })
        }
    }
}

fn run_postgres_gateway_migrations(
    url: &str,
    tls: &prodex_storage_postgres_runtime::PostgresTlsConfig,
    source: &str,
) -> Result<String, String> {
    let plan = plan_postgres_migrations(PostgresRuntimeMode::ExternalMigrator)
        .map_err(|err| err.to_string())?;
    let mut client = prodex_storage_postgres_runtime::connect_blocking(url, tls)
        .map_err(|err| format!("failed to connect postgres from {source}: {err}"))?;
    for migration in &plan.migrations {
        client.batch_execute(migration.sql).map_err(|err| {
            format!(
                "failed to apply postgres migration {}: {err}",
                migration.name
            )
        })?;
    }
    prodex_app::migrate_gateway_compatibility_state_postgres(url, tls).map_err(|err| {
        format!("failed to migrate gateway postgres compatibility state: {err:#}")
    })?;
    Ok(format!(
        "applied {} postgres migration(s) and ensured gateway compatibility schema",
        plan.migrations.len()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_sqlite_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "prodex-gateway-migrate-{name}-{}.sqlite",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    #[test]
    fn sqlite_gateway_migrate_also_ensures_compatibility_schema() {
        let path = temp_sqlite_path("compat");
        let message = run_gateway_migrate(GatewayMigrationTarget::Sqlite { path: path.clone() })
            .expect("sqlite gateway migrate should succeed");

        assert!(message.contains("ensured gateway compatibility schema"));
        let conn = rusqlite::Connection::open(&path).expect("sqlite database should open");
        let compatibility_table_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'prodex_gateway_virtual_keys'",
                [],
                |row| row.get(0),
            )
            .expect("compatibility table query should succeed");
        let enterprise_table_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'prodex_tenants'",
                [],
                |row| row.get(0),
            )
            .expect("enterprise table query should succeed");
        assert_eq!(compatibility_table_count, 1);
        assert_eq!(enterprise_table_count, 1);

        let _ = std::fs::remove_file(path);
    }
}
