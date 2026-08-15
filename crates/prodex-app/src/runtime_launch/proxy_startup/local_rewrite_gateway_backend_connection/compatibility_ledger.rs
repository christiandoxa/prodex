use anyhow::{Context, Result, bail};
use postgres::Client as PostgresClient;
use rusqlite::Connection;

pub(super) fn runtime_gateway_sqlite_compatibility_migration_version(
    conn: &Connection,
) -> Result<Option<i64>> {
    let mut statement = conn
        .prepare(
            "SELECT version
             FROM prodex_gateway_schema_migrations
             ORDER BY version",
        )
        .context("failed to read gateway sqlite compatibility migration ledger")?;
    let versions = statement
        .query_map([], |row| row.get::<_, i64>(0))
        .context("failed to read gateway sqlite compatibility migration ledger")?;
    let mut expected_version = 1_i64;
    let mut observed_version = None;
    for version in versions {
        let version =
            version.context("failed to read gateway sqlite compatibility migration ledger")?;
        if version != expected_version {
            bail!("gateway sqlite compatibility migration ledger contains a version gap");
        }
        observed_version = Some(version);
        expected_version = expected_version
            .checked_add(1)
            .context("gateway sqlite compatibility migration ledger version overflow")?;
    }
    Ok(observed_version)
}

pub(super) fn runtime_gateway_postgres_compatibility_migration_version(
    client: &mut PostgresClient,
) -> Result<Option<i64>> {
    let versions = client
        .query(
            "SELECT version
             FROM prodex_gateway_schema_migrations
             ORDER BY version",
            &[],
        )
        .context("failed to read gateway postgres compatibility migration ledger")?;
    let mut expected_version = 1_i64;
    let mut observed_version = None;
    for row in versions {
        let version: i64 = row.get(0);
        if version != expected_version {
            bail!("gateway postgres compatibility migration ledger contains a version gap");
        }
        observed_version = Some(version);
        expected_version = expected_version
            .checked_add(1)
            .context("gateway postgres compatibility migration ledger version overflow")?;
    }
    Ok(observed_version)
}
