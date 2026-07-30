use anyhow::Result;
use rusqlite::Connection;

use super::runtime_gateway_sqlite_table_has_column;

pub(super) fn runtime_gateway_sqlite_add_ledger_scope_columns(conn: &Connection) -> Result<()> {
    for (column_name, column_definition) in [
        ("typed_request_id", "TEXT"),
        ("tenant_id", "TEXT"),
        ("team_id", "TEXT"),
        ("project_id", "TEXT"),
        ("user_id", "TEXT"),
        ("budget_id", "TEXT"),
    ] {
        if runtime_gateway_sqlite_table_has_column(
            conn,
            "prodex_gateway_billing_ledger",
            column_name,
        )? {
            continue;
        }
        conn.execute_batch(&format!(
            "ALTER TABLE prodex_gateway_billing_ledger ADD COLUMN {column_name} {column_definition};"
        ))?;
    }
    Ok(())
}

pub(super) fn runtime_gateway_postgres_add_ledger_scope_columns(
    tx: &mut postgres::Transaction<'_>,
) -> Result<()> {
    tx.batch_execute(
        r#"
        ALTER TABLE prodex_gateway_billing_ledger
            ADD COLUMN IF NOT EXISTS typed_request_id TEXT;
        ALTER TABLE prodex_gateway_billing_ledger
            ADD COLUMN IF NOT EXISTS tenant_id TEXT;
        ALTER TABLE prodex_gateway_billing_ledger
            ADD COLUMN IF NOT EXISTS team_id TEXT;
        ALTER TABLE prodex_gateway_billing_ledger
            ADD COLUMN IF NOT EXISTS project_id TEXT;
        ALTER TABLE prodex_gateway_billing_ledger
            ADD COLUMN IF NOT EXISTS user_id TEXT;
        ALTER TABLE prodex_gateway_billing_ledger
            ADD COLUMN IF NOT EXISTS budget_id TEXT;
        "#,
    )?;
    Ok(())
}

pub(super) fn runtime_gateway_sqlite_add_virtual_key_id_column(conn: &Connection) -> Result<()> {
    let table_exists: bool = conn.query_row(
        "SELECT EXISTS(
            SELECT 1
            FROM sqlite_master
            WHERE type = 'table'
              AND name = 'prodex_gateway_virtual_keys'
        )",
        [],
        |row| row.get(0),
    )?;
    if !table_exists {
        return Ok(());
    }
    if !runtime_gateway_sqlite_table_has_column(
        conn,
        "prodex_gateway_virtual_keys",
        "virtual_key_id",
    )? {
        conn.execute_batch(
            "ALTER TABLE prodex_gateway_virtual_keys ADD COLUMN virtual_key_id TEXT;",
        )?;
    }
    Ok(())
}

pub(super) fn runtime_gateway_postgres_add_virtual_key_id_column(
    tx: &mut postgres::Transaction<'_>,
) -> Result<()> {
    tx.batch_execute(
        "ALTER TABLE prodex_gateway_virtual_keys ADD COLUMN IF NOT EXISTS virtual_key_id TEXT;",
    )?;
    Ok(())
}

pub(super) fn runtime_gateway_sqlite_add_scim_organization_columns(
    conn: &Connection,
) -> Result<()> {
    for (column_name, column_definition) in [
        ("group_ids_json", "TEXT NOT NULL DEFAULT '[]'"),
        ("department_id", "TEXT"),
    ] {
        if !runtime_gateway_sqlite_table_has_column(conn, "prodex_gateway_scim_users", column_name)?
        {
            conn.execute_batch(&format!(
                "ALTER TABLE prodex_gateway_scim_users ADD COLUMN {column_name} {column_definition};"
            ))?;
        }
    }
    Ok(())
}

pub(super) fn runtime_gateway_postgres_add_scim_organization_columns(
    tx: &mut postgres::Transaction<'_>,
) -> Result<()> {
    tx.batch_execute(
        r#"
        ALTER TABLE prodex_gateway_scim_users
            ADD COLUMN IF NOT EXISTS group_ids_json TEXT NOT NULL DEFAULT '[]';
        ALTER TABLE prodex_gateway_scim_users
            ADD COLUMN IF NOT EXISTS department_id TEXT;
        "#,
    )?;
    Ok(())
}

pub(super) fn runtime_gateway_sqlite_add_ledger_reserved_tokens_column(
    conn: &Connection,
) -> Result<()> {
    if !runtime_gateway_sqlite_table_has_column(
        conn,
        "prodex_gateway_billing_ledger",
        "reserved_tokens",
    )? {
        conn.execute_batch(
            "ALTER TABLE prodex_gateway_billing_ledger ADD COLUMN reserved_tokens INTEGER;",
        )?;
    }
    Ok(())
}

pub(super) fn runtime_gateway_postgres_add_ledger_reserved_tokens_column(
    tx: &mut postgres::Transaction<'_>,
) -> Result<()> {
    tx.batch_execute(
        "ALTER TABLE prodex_gateway_billing_ledger
            ADD COLUMN IF NOT EXISTS reserved_tokens BIGINT;",
    )?;
    Ok(())
}
