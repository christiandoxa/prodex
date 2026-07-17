use prodex_domain::{SecretProvider as _, SecretPurpose, SecretRef, SecretResolutionRequest};
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
            let applied = prodex_app::migrate_gateway_enterprise_state_sqlite(&path)
                .map_err(|err| format!("failed to migrate gateway sqlite state: {err:#}"))?;
            prodex_app::migrate_gateway_compatibility_state_sqlite(&path).map_err(|err| {
                format!("failed to migrate gateway sqlite compatibility state: {err:#}")
            })?;
            Ok(format!(
                "applied {} sqlite migration(s) and ensured gateway compatibility schema",
                applied
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
    let applied = prodex_app::migrate_gateway_enterprise_state_postgres(url, tls)
        .map_err(|err| format!("failed to migrate gateway postgres from {source}: {err:#}"))?;
    prodex_app::migrate_gateway_compatibility_state_postgres(url, tls).map_err(|err| {
        format!("failed to migrate gateway postgres compatibility state: {err:#}")
    })?;
    Ok(format!(
        "applied {} postgres migration(s) and ensured gateway compatibility schema",
        applied
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
        let second = run_gateway_migrate(GatewayMigrationTarget::Sqlite { path: path.clone() })
            .expect("repeated sqlite gateway migrate should succeed");

        assert!(message.contains("ensured gateway compatibility schema"));
        assert!(second.contains("applied 0 sqlite migration(s)"));
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
        let enterprise_migration_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM prodex_enterprise_schema_migrations",
                [],
                |row| row.get(0),
            )
            .expect("enterprise migration ledger query should succeed");
        assert_eq!(
            enterprise_migration_count,
            i64::from(prodex_storage_sqlite::REQUIRED_SQLITE_SCHEMA_VERSION.0)
        );

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn sqlite_gateway_migration_rolls_back_failed_migration() {
        let path = temp_sqlite_path("rollback");
        let conn = rusqlite::Connection::open(&path).expect("sqlite database should open");
        conn.execute_batch(
            "CREATE TABLE prodex_enterprise_schema_migrations (
                version INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                checksum TEXT NOT NULL,
                applied_at_epoch INTEGER NOT NULL
            );
            CREATE TABLE prodex_governance_sessions (
                tenant_id TEXT,
                principal_id TEXT,
                absolute_expires_at_unix_ms INTEGER,
                idle_expires_at_unix_ms INTEGER,
                session_id_hash TEXT,
                last_seen_at_unix_ms INTEGER,
                registry_revision_id TEXT,
                provider_descriptor_revision INTEGER
            );",
        )
        .expect("conflicting table should be created");
        drop(conn);

        let error = run_gateway_migrate(GatewayMigrationTarget::Sqlite { path: path.clone() })
            .expect_err("conflicting schema should fail migration");
        assert!(
            error.contains("006_governance_session_provider_revisions"),
            "unexpected migration error: {error}"
        );
        let conn = rusqlite::Connection::open(&path).expect("sqlite database should reopen");
        let max_version: i64 = conn
            .query_row(
                "SELECT MAX(version) FROM prodex_enterprise_schema_migrations",
                [],
                |row| row.get(0),
            )
            .expect("migration ledger should retain the committed first version");
        let legacy_column_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('prodex_governance_sessions')
                 WHERE name = 'registry_revision_id'",
                [],
                |row| row.get(0),
            )
            .expect("rolled-back column query should succeed");
        let renamed_column_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('prodex_governance_sessions')
                 WHERE name = 'provider_registry_revision'",
                [],
                |row| row.get(0),
            )
            .expect("renamed column query should succeed");
        assert_eq!(max_version, 5);
        assert_eq!(legacy_column_count, 1);
        assert_eq!(renamed_column_count, 0);

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn sqlite_gateway_migration_rejects_checksum_drift() {
        let path = temp_sqlite_path("checksum");
        run_gateway_migrate(GatewayMigrationTarget::Sqlite { path: path.clone() })
            .expect("initial migration should succeed");
        let conn = rusqlite::Connection::open(&path).expect("sqlite database should reopen");
        conn.execute(
            "UPDATE prodex_enterprise_schema_migrations
             SET checksum = 'tampered' WHERE version = 4",
            [],
        )
        .expect("migration checksum should be modified for the test");
        drop(conn);

        let error = run_gateway_migrate(GatewayMigrationTarget::Sqlite { path: path.clone() })
            .expect_err("checksum drift should fail migration");
        assert!(error.contains("checksum does not match"));

        let _ = std::fs::remove_file(path);
    }
}
