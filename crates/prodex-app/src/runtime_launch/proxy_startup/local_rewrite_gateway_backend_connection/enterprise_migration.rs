use super::{
    RUNTIME_GATEWAY_SCHEMA_VERSION, runtime_gateway_postgres_acquire_migration_lock,
    runtime_gateway_postgres_index_exists, runtime_gateway_postgres_observed_schema_version,
    runtime_gateway_postgres_table_exists, runtime_gateway_postgres_table_has_column,
    runtime_gateway_sqlite_index_exists, runtime_gateway_sqlite_observed_schema_version,
    runtime_gateway_sqlite_open_for_migration, runtime_gateway_sqlite_table_exists,
    runtime_gateway_sqlite_table_has_column,
};
use anyhow::{Context, Result, anyhow, bail};
use postgres::Client as PostgresClient;
use prodex_storage_postgres::{
    PostgresBackendOpenMode, PostgresRuntimeMode, REQUIRED_POSTGRES_SCHEMA_VERSION,
    plan_postgres_backend_open, plan_postgres_migrations,
};
use prodex_storage_sqlite::{
    REQUIRED_SQLITE_SCHEMA_VERSION, SqliteBackendOpenMode, SqliteRuntimeMode,
    plan_sqlite_backend_open, plan_sqlite_migrations,
};
use rusqlite::{Connection, OptionalExtension};
use sha2::{Digest, Sha256};
use std::path::Path;

mod siem_shape;
use siem_shape::{
    sqlite_siem_outbox_has_v13_marker, validate_postgres_siem_outbox_shape,
    validate_sqlite_siem_outbox_shape,
};

const MIGRATIONS_TABLE: &str = "prodex_enterprise_schema_migrations";
const SQLITE_MIGRATIONS_TABLE_SQL: &str = r#"
    CREATE TABLE IF NOT EXISTS prodex_enterprise_schema_migrations (
        version INTEGER PRIMARY KEY,
        name TEXT NOT NULL,
        checksum TEXT NOT NULL,
        applied_at_epoch INTEGER NOT NULL
    );
    "#;
const POSTGRES_MIGRATIONS_TABLE_SQL: &str = r#"
    CREATE TABLE IF NOT EXISTS prodex_enterprise_schema_migrations (
        version BIGINT PRIMARY KEY,
        name TEXT NOT NULL,
        checksum TEXT NOT NULL,
        applied_at_epoch BIGINT NOT NULL
    );
    "#;

pub(super) fn runtime_gateway_sqlite_require_schema(conn: &Connection) -> Result<()> {
    let version = runtime_gateway_sqlite_observed_schema_version(conn)?
        .ok_or_else(|| anyhow!("gateway sqlite schema has not been migrated"))?;
    if version < RUNTIME_GATEWAY_SCHEMA_VERSION {
        bail!("gateway sqlite schema is too old");
    }
    let enterprise_version = sqlite_observed_version(conn)?.ok_or_else(|| {
        anyhow!("gateway sqlite enterprise accounting schema has not been migrated")
    })?;
    let enterprise_version = u32::try_from(enterprise_version)
        .map_err(|_| anyhow!("gateway sqlite enterprise schema version is invalid"))?;
    plan_sqlite_backend_open(
        SqliteBackendOpenMode::GatewayStartup,
        Some(prodex_storage_sqlite::SqliteMigrationVersion(
            enterprise_version,
        )),
    )
    .map_err(|error| anyhow!(error))?;
    require_sqlite_schema(conn)
}

pub(super) fn runtime_gateway_postgres_require_schema(client: &mut PostgresClient) -> Result<()> {
    let version = runtime_gateway_postgres_observed_schema_version(client)?
        .ok_or_else(|| anyhow!("gateway postgres schema has not been migrated"))?;
    if version < RUNTIME_GATEWAY_SCHEMA_VERSION {
        bail!("gateway postgres schema is too old");
    }
    let enterprise_version = postgres_observed_version(client)?.ok_or_else(|| {
        anyhow!("gateway postgres enterprise accounting schema has not been migrated")
    })?;
    let enterprise_version = u32::try_from(enterprise_version)
        .map_err(|_| anyhow!("gateway postgres enterprise schema version is invalid"))?;
    plan_postgres_backend_open(
        PostgresBackendOpenMode::GatewayStartup,
        Some(prodex_storage_postgres::PostgresMigrationVersion(
            enterprise_version,
        )),
    )
    .map_err(|error| anyhow!(error))?;
    require_postgres_schema(client)
}

pub(crate) fn runtime_gateway_sqlite_migrate_enterprise_state(path: &Path) -> Result<usize> {
    let mut conn = runtime_gateway_sqlite_open_for_migration(path)?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    conn.pragma_update(None, "foreign_keys", true)?;
    let _ = conn.pragma_update(None, "journal_mode", "WAL");
    apply_sqlite_migrations(&mut conn)
}

pub(crate) fn runtime_gateway_postgres_migrate_enterprise_state(
    url: &str,
    tls: &prodex_storage_postgres_runtime::PostgresTlsConfig,
) -> Result<usize> {
    let mut client = prodex_storage_postgres_runtime::connect_blocking(url, tls)
        .context("failed to connect to gateway postgres state")?;
    runtime_gateway_postgres_acquire_migration_lock(&mut client)?;
    apply_postgres_migrations(&mut client)
}

fn migration_checksum(name: &str, sql: &str) -> String {
    let mut digest = Sha256::new();
    digest.update(name.as_bytes());
    digest.update([0]);
    digest.update(sql.as_bytes());
    digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn apply_sqlite_migrations(conn: &mut Connection) -> Result<usize> {
    let ledger_exists = runtime_gateway_sqlite_table_exists(conn, MIGRATIONS_TABLE)?;
    let observed_version = if ledger_exists {
        sqlite_observed_version(conn)?
    } else {
        None
    };
    if observed_version.is_some_and(|version| version > i64::from(REQUIRED_SQLITE_SCHEMA_VERSION.0))
    {
        bail!("gateway sqlite enterprise schema is newer than supported");
    }
    let legacy_version = if !ledger_exists
        || (observed_version.is_none()
            && runtime_gateway_sqlite_table_exists(conn, "prodex_tenants")?)
    {
        infer_legacy_sqlite_version(conn)?
    } else {
        0
    };
    conn.execute_batch(SQLITE_MIGRATIONS_TABLE_SQL)
        .context("failed to ensure gateway sqlite enterprise migrations table")?;
    let plan = plan_sqlite_migrations(SqliteRuntimeMode::ExternalMigrator)?;

    if legacy_version > 0 {
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        for migration in plan
            .migrations
            .iter()
            .filter(|migration| i64::from(migration.version.0) <= legacy_version)
        {
            let checksum = migration_checksum(migration.name, migration.sql);
            tx.execute(
                "INSERT INTO prodex_enterprise_schema_migrations
                 (version, name, checksum, applied_at_epoch)
                 VALUES (?1, ?2, ?3, strftime('%s', 'now'))",
                rusqlite::params![i64::from(migration.version.0), migration.name, checksum],
            )?;
        }
        tx.commit()?;
    }

    apply_sqlite_migration_plan(conn, &plan)
}

fn apply_sqlite_migration_plan(
    conn: &mut Connection,
    plan: &prodex_storage_sqlite::SqliteMigrationPlan,
) -> Result<usize> {
    let mut applied = 0;
    for migration in &plan.migrations {
        let version = i64::from(migration.version.0);
        let checksum = migration_checksum(migration.name, migration.sql);
        let recorded = conn
            .query_row(
                "SELECT name, checksum FROM prodex_enterprise_schema_migrations WHERE version = ?1",
                [version],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?;
        if let Some((recorded_name, recorded_checksum)) = recorded {
            if recorded_name != migration.name || recorded_checksum != checksum {
                bail!(
                    "gateway sqlite enterprise migration {} checksum does not match the recorded migration",
                    migration.name
                );
            }
            continue;
        }
        let has_later_version: bool = conn.query_row(
            "SELECT EXISTS(
                SELECT 1 FROM prodex_enterprise_schema_migrations
                WHERE version > ?1 AND version <= ?2
            )",
            rusqlite::params![version, i64::from(REQUIRED_SQLITE_SCHEMA_VERSION.0)],
            |row| row.get(0),
        )?;
        if has_later_version {
            bail!("gateway sqlite enterprise migration ledger contains a version gap");
        }
        let reason_detail_already_present = migration.name == "014_audit_reason_detail"
            && runtime_gateway_sqlite_table_has_column(conn, "prodex_audit_log", "reason_detail")?;
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        if !reason_detail_already_present {
            tx.execute_batch(migration.sql).with_context(|| {
                format!(
                    "failed to apply gateway sqlite enterprise migration {}",
                    migration.name
                )
            })?;
        }
        tx.execute(
            "INSERT INTO prodex_enterprise_schema_migrations
             (version, name, checksum, applied_at_epoch)
             VALUES (?1, ?2, ?3, strftime('%s', 'now'))",
            rusqlite::params![version, migration.name, checksum],
        )?;
        tx.commit()?;
        applied += 1;
    }
    Ok(applied)
}

pub(super) fn apply_postgres_migrations(client: &mut PostgresClient) -> Result<usize> {
    let ledger_exists = runtime_gateway_postgres_table_exists(client, MIGRATIONS_TABLE)?;
    let observed_version = if ledger_exists {
        postgres_observed_version(client)?
    } else {
        None
    };
    if observed_version
        .is_some_and(|version| version > i64::from(REQUIRED_POSTGRES_SCHEMA_VERSION.0))
    {
        bail!("gateway postgres enterprise schema is newer than supported");
    }
    let legacy_version = if !ledger_exists
        || (observed_version.is_none()
            && runtime_gateway_postgres_table_exists(client, "prodex_tenants")?)
    {
        infer_legacy_postgres_version(client)?
    } else {
        0
    };
    client
        .batch_execute(POSTGRES_MIGRATIONS_TABLE_SQL)
        .context("failed to ensure gateway postgres enterprise migrations table")?;
    let plan = plan_postgres_migrations(PostgresRuntimeMode::ExternalMigrator)?;

    if legacy_version > 0 {
        let mut tx = client.transaction()?;
        for migration in plan
            .migrations
            .iter()
            .filter(|migration| i64::from(migration.version.0) <= legacy_version)
        {
            let checksum = migration_checksum(migration.name, migration.sql);
            tx.execute(
                "INSERT INTO prodex_enterprise_schema_migrations
                 (version, name, checksum, applied_at_epoch)
                 VALUES ($1, $2, $3, EXTRACT(EPOCH FROM now())::BIGINT)",
                &[&i64::from(migration.version.0), &migration.name, &checksum],
            )?;
        }
        tx.commit()?;
    }

    apply_postgres_migration_plan(client, &plan)
}

fn apply_postgres_migration_plan(
    client: &mut PostgresClient,
    plan: &prodex_storage_postgres::PostgresMigrationPlan,
) -> Result<usize> {
    let mut applied = 0;
    for migration in &plan.migrations {
        let version = i64::from(migration.version.0);
        let checksum = migration_checksum(migration.name, migration.sql);
        if let Some(row) = client.query_opt(
            "SELECT name, checksum FROM prodex_enterprise_schema_migrations WHERE version = $1",
            &[&version],
        )? {
            let recorded_name: String = row.get(0);
            let recorded_checksum: String = row.get(1);
            if recorded_name != migration.name || recorded_checksum != checksum {
                bail!(
                    "gateway postgres enterprise migration {} checksum does not match the recorded migration",
                    migration.name
                );
            }
            continue;
        }
        let has_later_version: bool = client
            .query_one(
                "SELECT EXISTS(
                    SELECT 1 FROM prodex_enterprise_schema_migrations
                    WHERE version > $1 AND version <= $2
                )",
                &[&version, &i64::from(REQUIRED_POSTGRES_SCHEMA_VERSION.0)],
            )?
            .get(0);
        if has_later_version {
            bail!("gateway postgres enterprise migration ledger contains a version gap");
        }
        let mut tx = client.transaction()?;
        tx.batch_execute(migration.sql).with_context(|| {
            format!(
                "failed to apply gateway postgres enterprise migration {}",
                migration.name
            )
        })?;
        tx.execute(
            "INSERT INTO prodex_enterprise_schema_migrations
             (version, name, checksum, applied_at_epoch)
             VALUES ($1, $2, $3, EXTRACT(EPOCH FROM now())::BIGINT)",
            &[&version, &migration.name, &checksum],
        )?;
        tx.commit()?;
        applied += 1;
    }
    Ok(applied)
}

fn infer_legacy_sqlite_version(conn: &Connection) -> Result<i64> {
    let outbox_exists = runtime_gateway_sqlite_table_exists(conn, "prodex_siem_outbox")?;
    let has_v13_marker = sqlite_siem_outbox_has_v13_marker(conn)?;
    if outbox_exists || has_v13_marker {
        validate_sqlite_siem_outbox_shape(conn, has_v13_marker)?;
        if has_v13_marker {
            if runtime_gateway_sqlite_table_has_column(conn, "prodex_audit_log", "reason_detail")? {
                let bounded_reason_detail = sqlite_reason_detail_byte_limit_triggers_present(conn)?;
                return Ok(if bounded_reason_detail { 15 } else { 14 });
            }
            return Ok(13);
        }
    }
    if runtime_gateway_sqlite_table_has_column(conn, "prodex_audit_log", "reason_detail")? {
        let bounded_reason_detail = sqlite_reason_detail_byte_limit_triggers_present(conn)?;
        return Ok(if bounded_reason_detail { 15 } else { 14 });
    }
    if runtime_gateway_sqlite_table_has_column(
        conn,
        "prodex_governance_mutation_idempotency",
        "resulting_active_revision_id",
    )? {
        return Ok(12);
    }
    if runtime_gateway_sqlite_table_has_column(
        conn,
        "prodex_governance_revision_artifacts",
        "signature_key_id",
    )? {
        return Ok(11);
    }
    if runtime_gateway_sqlite_table_exists(conn, "prodex_audit_legal_holds")? {
        return Ok(10);
    }
    if runtime_gateway_sqlite_table_has_column(conn, "prodex_reservations", "storage_scope")? {
        return Ok(9);
    }
    if runtime_gateway_sqlite_table_has_column(conn, "prodex_tenants", "session_revocation_epoch")?
    {
        return Ok(8);
    }
    if runtime_gateway_sqlite_table_has_column(conn, "prodex_approvals", "termination_reason")? {
        return Ok(7);
    }
    if runtime_gateway_sqlite_table_has_column(
        conn,
        "prodex_governance_sessions",
        "provider_descriptor_revision",
    )? {
        return Ok(6);
    }
    if runtime_gateway_sqlite_index_exists(conn, "prodex_governance_sessions_refresh_idx")? {
        return Ok(5);
    }
    if runtime_gateway_sqlite_table_exists(conn, "prodex_governance_mutation_idempotency")? {
        return Ok(4);
    }
    if runtime_gateway_sqlite_table_exists(conn, "prodex_pricing_revisions")? {
        return Ok(3);
    }
    if runtime_gateway_sqlite_table_exists(conn, "prodex_policy_revisions")? {
        return Ok(2);
    }
    Ok(i64::from(runtime_gateway_sqlite_table_exists(
        conn,
        "prodex_tenants",
    )?))
}

fn sqlite_reason_detail_byte_limit_triggers_present(conn: &Connection) -> Result<bool> {
    conn.query_row(
        "SELECT EXISTS(
            SELECT 1 FROM sqlite_master
            WHERE type = 'trigger'
              AND name = 'prodex_audit_reason_detail_byte_limit_insert'
        ) AND EXISTS(
            SELECT 1 FROM sqlite_master
            WHERE type = 'trigger'
              AND name = 'prodex_audit_reason_detail_byte_limit_update'
        )",
        [],
        |row| row.get(0),
    )
    .map_err(Into::into)
}

pub(super) fn infer_legacy_postgres_version(client: &mut PostgresClient) -> Result<i64> {
    let outbox_shape = validate_postgres_siem_outbox_shape(client)?;
    if runtime_gateway_postgres_table_has_column(
        client,
        "prodex_governance_mutation_idempotency",
        "resulting_active_revision_id",
    )? {
        return Ok(16);
    }
    if runtime_gateway_postgres_table_has_column(
        client,
        "prodex_governance_revision_artifacts",
        "signature_key_id",
    )? {
        return Ok(15);
    }
    if runtime_gateway_postgres_table_exists(client, "prodex_config_publication_events")? {
        return Ok(14);
    }
    if runtime_gateway_postgres_table_exists(client, "prodex_audit_legal_holds")? {
        return Ok(13);
    }
    if runtime_gateway_postgres_table_has_column(client, "prodex_reservations", "storage_scope")? {
        return Ok(12);
    }
    if runtime_gateway_postgres_table_has_column(
        client,
        "prodex_tenants",
        "session_revocation_epoch",
    )? {
        return Ok(11);
    }
    if runtime_gateway_postgres_table_has_column(client, "prodex_approvals", "termination_reason")?
    {
        return Ok(10);
    }
    if runtime_gateway_postgres_table_has_column(
        client,
        "prodex_governance_sessions",
        "provider_descriptor_revision",
    )? {
        return Ok(9);
    }
    let immutable_trigger_exists: bool = client
        .query_one(
            "SELECT EXISTS(
                SELECT 1
                FROM pg_catalog.pg_trigger trigger_row
                JOIN pg_catalog.pg_class table_row ON table_row.oid = trigger_row.tgrelid
                JOIN pg_catalog.pg_namespace namespace_row
                  ON namespace_row.oid = table_row.relnamespace
                WHERE namespace_row.nspname = current_schema()
                  AND table_row.relname = 'prodex_audit_log'
                  AND trigger_row.tgname = 'prodex_audit_log_immutable'
                  AND NOT trigger_row.tgisinternal
            )",
            &[],
        )?
        .get(0);
    if immutable_trigger_exists {
        return Ok(8);
    }
    if runtime_gateway_postgres_index_exists(client, "prodex_governance_sessions_refresh_idx")? {
        return Ok(7);
    }
    if outbox_shape.complete_v6 {
        return Ok(6);
    }
    if runtime_gateway_postgres_table_exists(client, "prodex_governance_mutation_idempotency")? {
        return Ok(5);
    }
    if runtime_gateway_postgres_table_exists(client, "prodex_pricing_revisions")? {
        return Ok(4);
    }
    if runtime_gateway_postgres_table_exists(client, "prodex_policy_revisions")? {
        return Ok(3);
    }
    if runtime_gateway_postgres_table_has_column(client, "prodex_budget_counters", "request_count")?
    {
        return Ok(2);
    }
    Ok(i64::from(runtime_gateway_postgres_table_exists(
        client,
        "prodex_tenants",
    )?))
}

fn sqlite_observed_version(conn: &Connection) -> Result<Option<i64>> {
    if !runtime_gateway_sqlite_table_exists(conn, MIGRATIONS_TABLE)? {
        return Ok(None);
    }
    conn.query_row(
        "SELECT MAX(version) FROM prodex_enterprise_schema_migrations",
        [],
        |row| row.get::<_, Option<i64>>(0),
    )
    .context("failed to read gateway sqlite enterprise schema version")
}

fn postgres_observed_version(client: &mut PostgresClient) -> Result<Option<i64>> {
    if !runtime_gateway_postgres_table_exists(client, MIGRATIONS_TABLE)? {
        return Ok(None);
    }
    Ok(client
        .query_one(
            "SELECT MAX(version)::BIGINT FROM prodex_enterprise_schema_migrations",
            &[],
        )
        .context("failed to read gateway postgres enterprise schema version")?
        .get(0))
}

fn require_sqlite_schema(conn: &Connection) -> Result<()> {
    for table_name in [
        "prodex_tenants",
        "prodex_budget_counters",
        "prodex_reservations",
        "prodex_usage_ledger",
        "prodex_audit_log",
        "prodex_idempotency_records",
    ] {
        if !runtime_gateway_sqlite_table_exists(conn, table_name)? {
            bail!("gateway sqlite enterprise accounting schema has not been migrated");
        }
    }
    Ok(())
}

fn require_postgres_schema(client: &mut PostgresClient) -> Result<()> {
    for table_name in [
        "prodex_tenants",
        "prodex_budget_counters",
        "prodex_reservations",
        "prodex_usage_ledger",
        "prodex_audit_log",
        "prodex_idempotency_records",
    ] {
        if !runtime_gateway_postgres_table_exists(client, table_name)? {
            bail!("gateway postgres enterprise accounting schema has not been migrated");
        }
    }
    Ok(())
}
